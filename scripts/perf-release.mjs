import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { arch, cpus, freemem, platform, release, tmpdir, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_BUDGETS = Object.freeze({
  coldStartMs: 2_000,
  aggregateRssMiB: 150,
  packageSizeMiB: 30,
});

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binaryPath = join(repoRoot, "src-tauri", "target", "release", "postmite");

export async function main() {
  if (!process.env.POSTMITE_PERF_SKIP_BUILD) {
    run("pnpm", ["build:tauri"]);
  }

  assertDisplayAvailable();
  assertFile(binaryPath, "release binary");

  const coldStart = await measureApp({ tabCount: 1 });
  const tenTab = await measureApp({ tabCount: 10 });
  const packageSizeMiB = bytesToMiB(statSync(binaryPath).size);

  const metrics = {
    coldStartMs: coldStart.readyMs,
    aggregateRssMiB: coldStart.rssMiB,
    tenTabRssMiB: tenTab.rssMiB,
    packageSizeMiB,
  };
  const result = {
    measuredAt: new Date().toISOString(),
    machine: machineMetadata(),
    budgets: {
      coldStartMs: DEFAULT_BUDGETS.coldStartMs,
      aggregateRssMiB: DEFAULT_BUDGETS.aggregateRssMiB,
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
    {
      name: "aggregateRssMiB",
      actual: metrics.aggregateRssMiB,
      budget: budgets.aggregateRssMiB,
      pass: metrics.aggregateRssMiB <= budgets.aggregateRssMiB,
    },
    {
      name: "packageSizeMiB",
      actual: metrics.packageSizeMiB,
      budget: budgets.packageSizeMiB,
      pass: metrics.packageSizeMiB <= budgets.packageSizeMiB,
    },
  ];
}

export function failedChecks(result) {
  return result.checks.filter((check) => !check.pass);
}

export function bytesToMiB(bytes) {
  return Number((bytes / 1024 / 1024).toFixed(2));
}

export function sumProcessTreeRssKiB(rootPid, procRoot = "/proc") {
  const seen = new Set();
  const stack = [String(rootPid)];
  let rssKiB = 0;

  while (stack.length > 0) {
    const pid = stack.pop();
    if (!pid || seen.has(pid)) {
      continue;
    }
    seen.add(pid);

    rssKiB += readRssKiB(pid, procRoot);
    stack.push(...readChildren(pid, procRoot));
  }

  return rssKiB;
}

async function measureApp({ tabCount }) {
  const tempDir = mkdtempSync(join(tmpdir(), "postmite-perf-"));
  const readyFile = join(tempDir, "ready.json");
  const appDataDir = join(tempDir, "app-data");
  const startedAt = performance.now();
  const output = [];
  const child = spawn(binaryPath, [], {
    cwd: repoRoot,
    env: {
      ...process.env,
      GDK_BACKEND: process.env.GDK_BACKEND ?? "x11",
      GSETTINGS_BACKEND: "memory",
      NO_AT_BRIDGE: process.env.NO_AT_BRIDGE ?? "1",
      POSTMITE_PERF_APP_DATA_DIR: appDataDir,
      POSTMITE_PERF_READY_FILE: readyFile,
      POSTMITE_PERF_TAB_COUNT: String(tabCount),
      WEBKIT_DISABLE_DMABUF_RENDERER:
        process.env.WEBKIT_DISABLE_DMABUF_RENDERER ?? "1",
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
    const rssMiB = bytesToMiB(sumProcessTreeRssKiB(child.pid) * 1024);
    return { readyMs, rssMiB };
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

function machineMetadata() {
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
    display: process.env.WAYLAND_DISPLAY ? "wayland" : "x11",
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
