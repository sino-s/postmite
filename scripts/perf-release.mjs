import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { arch, cpus, freemem, platform, release, tmpdir, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_BUDGETS = Object.freeze({
  coldStartMs: 2_000,
  singleTabPssMiB: 300,
  tenTabPssMiB: 300,
  packageSizeMiB: 30,
});

export const DEFAULT_SAMPLE_COUNT = 3;

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binaryPath = join(repoRoot, "src-tauri", "target", "release", "postmite");

export async function main() {
  if (!process.env.POSTMITE_PERF_SKIP_BUILD) {
    run("pnpm", ["build:tauri"]);
  }

  assertDisplayAvailable();
  assertFile(binaryPath, "release binary");

  const appEnvironment = measurementEnvironment();
  const samples = { singleTab: [], tenTab: [] };
  for (const tabCount of samplingPlan(DEFAULT_SAMPLE_COUNT)) {
    const sample = await measureApp({ appEnvironment, tabCount });
    samples[tabCount === 1 ? "singleTab" : "tenTab"].push(sample);
  }
  const singleTab = aggregateSamples(samples.singleTab);
  const tenTab = aggregateSamples(samples.tenTab);
  const packageSizeMiB = bytesToMiB(statSync(binaryPath).size);

  const metrics = {
    coldStartMs: singleTab.readyMs,
    singleTabRssMiB: singleTab.rssMiB,
    singleTabPssMiB: singleTab.pssMiB,
    tenTabRssMiB: tenTab.rssMiB,
    tenTabPssMiB: tenTab.pssMiB,
    packageSizeMiB,
  };
  const result = {
    measuredAt: new Date().toISOString(),
    machine: machineMetadata(process.env, appEnvironment),
    sampling: {
      aggregation: "median",
      sampleCountPerScenario: DEFAULT_SAMPLE_COUNT,
      order: samplingPlan(DEFAULT_SAMPLE_COUNT).map((tabCount) =>
        tabCount === 1 ? "singleTab" : "tenTab"
      ),
      scenarios: {
        singleTab: { samples: samples.singleTab, selected: singleTab },
        tenTab: { samples: samples.tenTab, selected: tenTab },
      },
    },
    budgets: {
      coldStartMs: DEFAULT_BUDGETS.coldStartMs,
      singleTabPssMiB: DEFAULT_BUDGETS.singleTabPssMiB,
      singleTabRssMiB: null,
      tenTabPssMiB: DEFAULT_BUDGETS.tenTabPssMiB,
      tenTabRssMiB: null,
      packageSizeMiB: DEFAULT_BUDGETS.packageSizeMiB,
    },
    metrics,
    checks: evaluate(metrics, DEFAULT_BUDGETS),
  };

  printResult(result);

  const failed = failedChecks(result);
  if (process.env.POSTMITE_PERF_STRICT && failed.length > 0) {
    throw new Error(
      `performance budget failed: ${failed.map((check) => check.name).join(", ")}`,
    );
  }
}

export function evaluate(metrics, budgets = DEFAULT_BUDGETS) {
  return [
    {
      name: "coldStartMs",
      actual: metrics.coldStartMs,
      budget: budgets.coldStartMs,
      pass: metrics.coldStartMs <= budgets.coldStartMs,
    },
    memoryCheck(metrics, budgets, "singleTab"),
    memoryCheck(metrics, budgets, "tenTab"),
    {
      name: "packageSizeMiB",
      actual: metrics.packageSizeMiB,
      budget: budgets.packageSizeMiB,
      pass: metrics.packageSizeMiB <= budgets.packageSizeMiB,
    },
  ];
}

function memoryCheck(metrics, budgets, scenario) {
  const pssName = `${scenario}PssMiB`;
  const rssName = `${scenario}RssMiB`;
  const metricName = metrics[pssName] === null || metrics[pssName] === undefined
    ? rssName
    : pssName;
  const budget = budgets[pssName];
  return {
    name: metricName,
    actual: metrics[metricName],
    budget,
    pass: metrics[metricName] <= budget,
  };
}

export function samplingPlan(sampleCount) {
  const order = [];
  for (let index = 0; index < sampleCount; index += 1) {
    order.push(...(index % 2 === 0 ? [1, 10] : [10, 1]));
  }
  return order;
}

export function aggregateSamples(samples) {
  if (samples.length === 0) throw new Error("at least one performance sample is required");
  return {
    readyMs: median(samples.map((sample) => sample.readyMs)),
    rssMiB: median(samples.map((sample) => sample.rssMiB)),
    pssMiB: samples.some((sample) => sample.pssMiB === null)
      ? null
      : median(samples.map((sample) => sample.pssMiB)),
  };
}

export function median(values) {
  if (values.length === 0) throw new Error("at least one value is required");
  const ordered = [...values].sort((left, right) => left - right);
  const middle = Math.floor(ordered.length / 2);
  return ordered.length % 2 === 0
    ? (ordered[middle - 1] + ordered[middle]) / 2
    : ordered[middle];
}

export function failedChecks(result) {
  return result.checks.filter((check) => !check.pass);
}

export function bytesToMiB(bytes) {
  return Number((bytes / 1024 / 1024).toFixed(2));
}

export function sumProcessTreeRssKiB(rootPid, procRoot = "/proc") {
  return sumProcessTreeMemoryKiB(rootPid, procRoot).rssKiB;
}

export function sumProcessTreePssKiB(rootPid, procRoot = "/proc") {
  return sumProcessTreeMemoryKiB(rootPid, procRoot).pssKiB;
}

export function sumProcessTreeMemoryKiB(rootPid, procRoot = "/proc") {
  const seen = new Set();
  const stack = [String(rootPid)];
  let rssKiB = 0;
  let pssKiB = 0;
  let pssComplete = true;

  while (stack.length > 0) {
    const pid = stack.pop();
    if (!pid || seen.has(pid)) {
      continue;
    }
    seen.add(pid);

    rssKiB += readRssKiB(pid, procRoot);
    const processPssKiB = readPssKiB(pid, procRoot);
    if (processPssKiB === null) {
      pssComplete = false;
    } else {
      pssKiB += processPssKiB;
    }
    stack.push(...readChildren(pid, procRoot));
  }

  return {
    rssKiB,
    pssKiB: pssComplete ? pssKiB : null,
  };
}

async function measureApp({ appEnvironment, tabCount }) {
  const tempDir = mkdtempSync(join(tmpdir(), "postmite-perf-"));
  const readyFile = join(tempDir, "ready.json");
  const appDataDir = join(tempDir, "app-data");
  const startedAt = performance.now();
  const output = [];
  const child = spawn(binaryPath, [], {
    cwd: repoRoot,
    env: {
      ...process.env,
      ...appEnvironment,
      POSTMITE_PERF_APP_DATA_DIR: appDataDir,
      POSTMITE_PERF_READY_FILE: readyFile,
      POSTMITE_PERF_TAB_COUNT: String(tabCount),
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => output.push(chunk));
  child.stderr.on("data", (chunk) => output.push(chunk));

  try {
    await waitForFileOrExit(readyFile, child, output, 30_000);
    const readyMs = Math.round(performance.now() - startedAt);
    await sleep(500);
    const memory = sumProcessTreeMemoryKiB(child.pid);
    return {
      readyMs,
      rssMiB: bytesToMiB(memory.rssKiB * 1024),
      pssMiB: memory.pssKiB === null ? null : bytesToMiB(memory.pssKiB * 1024),
    };
  } finally {
    await terminate(child);
    rmSync(tempDir, { force: true, recursive: true });
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}`);
  }
}

function assertDisplayAvailable() {
  if (process.env.DISPLAY || process.env.WAYLAND_DISPLAY) {
    return;
  }

  throw new Error("A display server is required. Run under `xvfb-run -a pnpm perf:release` in CI.");
}

function assertFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} not found at ${path}`);
  }
}

export function measurementEnvironment(environment = process.env) {
  return {
    GDK_BACKEND: environment.GDK_BACKEND ?? "x11",
    GSETTINGS_BACKEND: "memory",
    NO_AT_BRIDGE: environment.NO_AT_BRIDGE ?? "1",
    WEBKIT_DISABLE_DMABUF_RENDERER:
      environment.WEBKIT_DISABLE_DMABUF_RENDERER ?? "1",
  };
}

export function displayMetadata(environment, appEnvironment) {
  const hostSession = environment.XDG_SESSION_TYPE
    ?? (environment.WAYLAND_DISPLAY ? "wayland" : environment.DISPLAY ? "x11" : "unknown");
  return {
    hostSession,
    hostDisplay: environment.DISPLAY ?? null,
    hostWaylandDisplay: environment.WAYLAND_DISPLAY ?? null,
    appGdkBackend: appEnvironment.GDK_BACKEND,
    webkitDisableDmabufRenderer: appEnvironment.WEBKIT_DISABLE_DMABUF_RENDERER,
  };
}

export function machineMetadata(
  environment = process.env,
  appEnvironment = measurementEnvironment(environment),
) {
  const cpuList = cpus();
  return {
    platform: platform(),
    release: release(),
    arch: arch(),
    cpuModel: cpuList[0]?.model ?? "unknown",
    cpuCount: cpuList.length,
    totalMemoryMiB: bytesToMiB(totalmem()),
    freeMemoryMiB: bytesToMiB(freemem()),
    node: process.version,
    rustc: commandVersion("rustc", ["--version"]),
    pnpm: commandVersion("pnpm", ["--version"]),
    display: displayMetadata(environment, appEnvironment),
  };
}

function commandVersion(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    return "unavailable";
  }

  return result.stdout.trim();
}

function printResult(result) {
  console.log(JSON.stringify(result, null, 2));
  console.log("");
  for (const check of result.checks) {
    const status = check.pass ? "PASS" : "FAIL";
    console.log(`${status} ${check.name}: ${check.actual} <= ${check.budget}`);
  }
}

function readRssKiB(pid, procRoot) {
  try {
    const status = readFileSync(join(procRoot, String(pid), "status"), "utf8");
    const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status);
    return match ? Number(match[1]) : 0;
  } catch {
    return 0;
  }
}

function readPssKiB(pid, procRoot) {
  try {
    const status = readFileSync(join(procRoot, String(pid), "smaps_rollup"), "utf8");
    const match = /^Pss:\s+(\d+)\s+kB$/m.exec(status);
    return match ? Number(match[1]) : null;
  } catch {
    return null;
  }
}

function readChildren(pid, procRoot) {
  try {
    const content = readFileSync(
      join(procRoot, String(pid), "task", String(pid), "children"),
      "utf8",
    );
    return content.trim() ? content.trim().split(/\s+/) : [];
  } catch {
    return [];
  }
}

function waitForFileOrExit(path, child, output, timeoutMs) {
  const deadline = performance.now() + timeoutMs;
  return new Promise((resolveWait, reject) => {
    const poll = () => {
      if (existsSync(path)) {
        resolveWait();
        return;
      }

      if (child.exitCode !== null || child.signalCode !== null) {
        reject(
          new Error(
            `postmite exited before writing ${path}: ${formatChildOutput(output)}`,
          ),
        );
        return;
      }

      if (performance.now() >= deadline) {
        reject(
          new Error(`timed out waiting for ${path}: ${formatChildOutput(output)}`),
        );
        return;
      }

      setTimeout(poll, 25);
    };

    poll();
  });
}

function formatChildOutput(output) {
  const text = output.join("").trim();
  return text.length > 0 ? text.slice(-2_000) : "no child output";
}

function sleep(ms) {
  return new Promise((resolveSleep) => {
    setTimeout(resolveSleep, ms);
  });
}

function terminate(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve();
  }

  return new Promise((resolveTerminate) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolveTerminate();
    }, 2_000);
    child.once("exit", () => {
      clearTimeout(timer);
      resolveTerminate();
    });
    child.kill("SIGTERM");
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
