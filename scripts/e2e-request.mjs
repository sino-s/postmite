import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binaryPath = join(repoRoot, "src-tauri", "target", "release", "postmite");

export async function main() {
  if (!process.env.POSTMITE_E2E_SKIP_BUILD) {
    run("pnpm", ["build:tauri"]);
  }

  assertDisplayAvailable();
  assertFile(binaryPath, "release binary");

  const fixture = await startFixture();
  const tempDir = mkdtempSync(join(tmpdir(), "postmite-e2e-"));
  const reportFile = join(tempDir, "request-smoke.json");
  const appDataDir = join(tempDir, "app-data");
  const output = [];

  const child = spawn(binaryPath, [], {
    cwd: repoRoot,
    env: {
      ...process.env,
      GDK_BACKEND: process.env.GDK_BACKEND ?? "x11",
      GSETTINGS_BACKEND: "memory",
      NO_AT_BRIDGE: process.env.NO_AT_BRIDGE ?? "1",
      POSTMITE_E2E_REQUEST_REPORT_FILE: reportFile,
      POSTMITE_E2E_REQUEST_URL: fixture.url,
      POSTMITE_PERF_APP_DATA_DIR: appDataDir,
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
    const report = JSON.parse(readFileSync(reportFile, "utf8"));
    assertReport(report);
    console.log(JSON.stringify(report, null, 2));
  } finally {
    await terminate(child);
    await fixture.close();
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

  throw new Error("A display server is required. Run under `xvfb-run -a pnpm e2e:request`.");
}

function assertFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} not found at ${path}`);
  }
}

function startFixture() {
  const server = createServer((request, response) => {
    if (request.method !== "GET" || request.url !== "/smoke") {
      response.writeHead(404);
      response.end("not found");
      return;
    }

    const body = '{"ok":true,"source":"e2e"}';
    response.writeHead(200, {
      "content-length": String(Buffer.byteLength(body)),
      "content-type": "application/json",
      "x-postmite-smoke": "ok",
    });
    response.end(body);
  });

  return new Promise((resolveFixture, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("fixture address unavailable"));
        return;
      }

      resolveFixture({
        url: `http://127.0.0.1:${address.port}/smoke`,
        close: () =>
          new Promise((resolveClose, rejectClose) => {
            server.close((error) => {
              if (error) {
                rejectClose(error);
                return;
              }
              resolveClose();
            });
          }),
      });
    });
  });
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

function assertReport(report) {
  if (report.status !== 200) {
    throw new Error(`expected status 200, received ${report.status}`);
  }
  if (!Array.isArray(report.headers)) {
    throw new Error("expected response headers");
  }
  if (
    !report.headers.some(
      (header) =>
        header.name === "x-postmite-smoke" && header.value === "ok",
    )
  ) {
    throw new Error("expected x-postmite-smoke response header");
  }
  if (!report.bodyPreview.includes('"ok":true')) {
    throw new Error("expected JSON body preview");
  }
  if (typeof report.elapsedMs !== "number" || report.elapsedMs < 0) {
    throw new Error("expected elapsed timing");
  }
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

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
