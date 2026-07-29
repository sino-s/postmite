import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binaryPath = join(repoRoot, "src-tauri", "target", "release", "postmite");

if (!process.env.POSTMITE_E2E_SKIP_BUILD) {
  run("pnpm", ["build:tauri"]);
}

assertDisplayAvailable();
assertFile(binaryPath, "release binary");

const tempDir = join(repoRoot, "target", "postmite-security-e2e");
const appDataDir = join(tempDir, "app-data");
const initialReportFile = join(tempDir, "initial-security.json");
const restartReportFile = join(tempDir, "restart-security.json");

rmSync(tempDir, { force: true, recursive: true });
mkdirSync(tempDir, { recursive: true });

const initial = await runSecurityPhase("initial", initialReportFile, appDataDir);
assertInitialReport(initial);

const restart = await runSecurityPhase("restart", restartReportFile, appDataDir);
assertRestartReport(restart);

run("pnpm", ["security:scan-fixtures"], {
  POSTMITE_SECURITY_SCAN_ROOT: tempDir,
});

console.log(
  JSON.stringify(
    {
      initial: summarize(initial),
      restart: summarize(restart),
      scanRoot: tempDir,
    },
    null,
    2,
  ),
);

async function runSecurityPhase(phase, reportFile, appDataDir) {
  const output = [];
  const child = spawn(binaryPath, [], {
    cwd: repoRoot,
    env: {
      ...process.env,
      GDK_BACKEND: process.env.GDK_BACKEND ?? "x11",
      GSETTINGS_BACKEND: "memory",
      NO_AT_BRIDGE: process.env.NO_AT_BRIDGE ?? "1",
      POSTMITE_E2E_SECURITY_PHASE: phase,
      POSTMITE_E2E_SECURITY_REPORT_FILE: reportFile,
      POSTMITE_PERF_APP_DATA_DIR: appDataDir,
      POSTMITE_SESSION_ONLY_SECRETS: "1",
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
    await waitForFileOrExit(reportFile, child, output, 30_000);
    return JSON.parse(readFileSync(reportFile, "utf8"));
  } finally {
    await terminate(child);
  }
}

function assertInitialReport(report) {
  assert(report.phase === "initial", "expected initial report");
  assert(report.protectedClasses.length >= 5, "expected protected classes");
  assert(
    report.protectedClasses.every((entry) => entry.referenceSafe),
    "expected safe Secret references",
  );
  assert(report.cookieDefaultMasked, "expected masked cookies by default");
  assert(
    report.cookieRevealRequiresExplicitAction,
    "expected explicit reveal action",
  );
  assert(report.sessionCookiePresent, "expected session cookie before restart");
  assert(report.persistentCookiePresent, "expected persistent cookie before restart");
  assert(
    report.persistentCookieValueAvailable,
    "expected persistent cookie available before restart",
  );
  assert(report.historyRequestRedacted, "expected redacted history request");
  assert(report.historyResponseRedacted, "expected redacted history response");
  assert(report.ipcErrorRedacted, "expected redacted IPC errors");
  assert(
    report.oauthTemporaryArtifactsCleaned,
    "expected OAuth temporary cleanup",
  );
}

function assertRestartReport(report) {
  assert(report.phase === "restart", "expected restart report");
  assert(!report.sessionCookiePresent, "expected session cookie cleanup");
  assert(report.persistentCookiePresent, "expected persistent metadata");
  assert(
    !report.persistentCookieValueAvailable,
    "expected no persistent value after session fallback restart",
  );
  assert(report.cookieDefaultMasked, "expected masked cookies after restart");
}

function summarize(report) {
  return {
    phase: report.phase,
    protectedClasses: report.protectedClasses.length,
    cookieDefaultMasked: report.cookieDefaultMasked,
    sessionCookiePresent: report.sessionCookiePresent,
    persistentCookiePresent: report.persistentCookiePresent,
    persistentCookieValueAvailable: report.persistentCookieValueAvailable,
    historyRequestRedacted: report.historyRequestRedacted,
    historyResponseRedacted: report.historyResponseRedacted,
  };
}

function run(command, args, env = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}`);
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertDisplayAvailable() {
  if (process.env.DISPLAY || process.env.WAYLAND_DISPLAY) {
    return;
  }

  throw new Error("A display server is required. Run under `xvfb-run -a pnpm e2e:security`.");
}

function assertFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} not found at ${path}`);
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
