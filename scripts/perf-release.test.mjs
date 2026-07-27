import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  bytesToMiB,
  evaluate,
  failedChecks,
  sumProcessTreeRssKiB,
} from "./perf-release.mjs";

describe("release performance budgets", () => {
  it("passes metrics at or below deterministic budgets", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_000,
        aggregateRssMiB: 150,
        packageSizeMiB: 30,
      },
      {
        coldStartMs: 2_000,
        aggregateRssMiB: 150,
        packageSizeMiB: 30,
      },
    );

    expect(checks).toEqual([
      {
        name: "coldStartMs",
        actual: 2_000,
        budget: 2_000,
        pass: true,
      },
      {
        name: "aggregateRssMiB",
        actual: 150,
        budget: 150,
        pass: true,
      },
      {
        name: "packageSizeMiB",
        actual: 30,
        budget: 30,
        pass: true,
      },
    ]);
  });

  it("fails metrics above deterministic budgets", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_001,
        aggregateRssMiB: 150.01,
        packageSizeMiB: 30.01,
      },
      {
        coldStartMs: 2_000,
        aggregateRssMiB: 150,
        packageSizeMiB: 30,
      },
    );

    expect(checks.every((check) => !check.pass)).toBe(true);
  });

  it("returns failed checks for explicit regression reporting", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_001,
        aggregateRssMiB: 149,
        packageSizeMiB: 29,
      },
      {
        coldStartMs: 2_000,
        aggregateRssMiB: 150,
        packageSizeMiB: 30,
      },
    );

    expect(failedChecks({ checks })).toEqual([
      {
        name: "coldStartMs",
        actual: 2_001,
        budget: 2_000,
        pass: false,
      },
    ]);
  });

  it("rounds bytes to MiB for stable output", () => {
    expect(bytesToMiB(31_457_280)).toBe(30);
    expect(bytesToMiB(1_572_864)).toBe(1.5);
  });
});

describe("process tree RSS aggregation", () => {
  it("sums root and descendant RSS from procfs", () => {
    const procRoot = mkdtempSync(join(tmpdir(), "postmite-proc-"));

    try {
      writeProc(procRoot, "100", 1_000, ["101", "102"]);
      writeProc(procRoot, "101", 2_000, ["103"]);
      writeProc(procRoot, "102", 3_000, []);
      writeProc(procRoot, "103", 4_000, []);

      expect(sumProcessTreeRssKiB("100", procRoot)).toBe(10_000);
    } finally {
      rmSync(procRoot, { force: true, recursive: true });
    }
  });
});

function writeProc(procRoot, pid, rssKiB, children) {
  const taskDir = join(procRoot, pid, "task", pid);
  mkdirSync(taskDir, { recursive: true });
  writeFileSync(join(procRoot, pid, "status"), `Name:\tpostmite\nVmRSS:\t${rssKiB} kB\n`);
  writeFileSync(join(taskDir, "children"), children.join(" "));
}
