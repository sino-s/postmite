import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const checks = [
  {
    name: "coordinator queue fairness and cancellation",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--locked",
      "application::execution::tests",
      "--",
      "--nocapture",
    ],
  },
  {
    name: "local HTTP transport matrix",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--locked",
      "infrastructure::http::tests",
      "--",
      "--nocapture",
    ],
  },
];

for (const check of checks) {
  const result = await runAndMeasure(check);
  console.log(
    `${check.name}: max aggregate RSS ${formatBytes(result.maxRssBytes)}`,
  );
}

async function runAndMeasure({ command, args }) {
  const child = spawn(command, args, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
  });

  let maxRssBytes = 0;
  const interval = setInterval(() => {
    maxRssBytes = Math.max(maxRssBytes, aggregateRssBytes(child.pid));
  }, 25);

  const status = await new Promise((resolveStatus) => {
    child.once("exit", (code, signal) => resolveStatus({ code, signal }));
  });
  clearInterval(interval);
  maxRssBytes = Math.max(maxRssBytes, aggregateRssBytes(child.pid));

  if (status.code !== 0) {
    const suffix = status.signal ? ` signal ${status.signal}` : "";
    throw new Error(`${command} ${args.join(" ")} failed with ${status.code}${suffix}`);
  }

  return { maxRssBytes };
}

function aggregateRssBytes(rootPid) {
  if (!rootPid || !existsSync(`/proc/${rootPid}`)) {
    return 0;
  }

  const pids = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const pid of listPids()) {
      if (pids.has(pid)) {
        continue;
      }
      const parent = parentPid(pid);
      if (parent !== null && pids.has(parent)) {
        pids.add(pid);
        changed = true;
      }
    }
  }

  let total = 0;
  for (const pid of pids) {
    total += rssBytes(pid);
  }
  return total;
}

function listPids() {
  try {
    return readdirSync("/proc")
      .filter((entry) => /^\d+$/.test(entry))
      .map((entry) => Number(entry));
  } catch {
    return [];
  }
}

function parentPid(pid) {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    const end = stat.lastIndexOf(")");
    const fields = stat.slice(end + 2).split(" ");
    return Number(fields[1]);
  } catch {
    return null;
  }
}

function rssBytes(pid) {
  try {
    const status = readFileSync(`/proc/${pid}/status`, "utf8");
    const match = status.match(/^VmRSS:\s+(\d+)\s+kB$/m);
    return match ? Number(match[1]) * 1024 : 0;
  } catch {
    return 0;
  }
}

function formatBytes(bytes) {
  if (bytes <= 0) {
    return "0 B";
  }
  const mib = bytes / 1024 / 1024;
  return `${mib.toFixed(1)} MiB`;
}
