import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  aggregateSamples,
  bytesToMiB,
  DEFAULT_BUDGETS,
  DEFAULT_SAMPLE_COUNT,
  displayMetadata,
  evaluate,
  failedChecks,
  MIB,
  measurementEnvironment,
  median,
  samplingPlan,
  sumProcessTreeMemoryKiB,
  sumProcessTreePssKiB,
  sumProcessTreeRssKiB,
} from "./perf-release.mjs";

describe("release performance budgets", () => {
  it("documents independently calibrated Linux PSS budgets", () => {
    expect(DEFAULT_BUDGETS).toEqual({
      coldStartMs: 2_000,
      singleTabPssMiB: 300,
      tenTabPssMiB: 300,
      packageSizeMiB: 30,
    });
    expect(DEFAULT_SAMPLE_COUNT).toBe(3);
  });

  it("keeps the release compiler pin explicit", () => {
    const toolchain = readFileSync("rust-toolchain.toml", "utf8");
    expect(toolchain).toContain('channel = "1.88.0"');
  });

  it("passes metrics at or below deterministic budgets", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_000,
        singleTabRssMiB: 350,
        singleTabPssMiB: 300,
        tenTabRssMiB: 360,
        tenTabPssMiB: 299,
        packageSizeBytes: 30 * MIB,
        packageSizeMiB: 30,
      },
      {
        coldStartMs: 2_000,
        singleTabPssMiB: 300,
        tenTabPssMiB: 300,
        packageSizeBytes: 30 * MIB,
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
        name: "singleTabPssMiB",
        actual: 300,
        budget: 300,
        pass: true,
      },
      {
        name: "tenTabPssMiB",
        actual: 299,
        budget: 300,
        pass: true,
      },
      {
        name: "packageSizeBytes",
        actual: 30 * MIB,
        budget: 30 * MIB,
        pass: true,
      },
    ]);
  });

  it("fails metrics above deterministic budgets", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_001,
        singleTabRssMiB: 350,
        singleTabPssMiB: 300.01,
        tenTabRssMiB: 360,
        tenTabPssMiB: 301,
        packageSizeBytes: 30 * MIB + 1,
        packageSizeMiB: 30.01,
      },
      {
        coldStartMs: 2_000,
        singleTabPssMiB: 300,
        tenTabPssMiB: 300,
        packageSizeMiB: 30,
      },
    );

    expect(checks.every((check) => !check.pass)).toBe(true);
  });

  it("returns failed checks for explicit regression reporting", () => {
    const checks = evaluate(
      {
        coldStartMs: 2_001,
        singleTabRssMiB: 349,
        singleTabPssMiB: 299,
        tenTabRssMiB: 350,
        tenTabPssMiB: 298,
        packageSizeMiB: 29,
      },
      {
        coldStartMs: 2_000,
        singleTabPssMiB: 300,
        tenTabPssMiB: 300,
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

  it("fails exactly one byte above the package-size boundary", () => {
    const atBoundary = evaluate({
      coldStartMs: 200,
      singleTabRssMiB: 1,
      singleTabPssMiB: 1,
      tenTabRssMiB: 1,
      tenTabPssMiB: 1,
      packageSizeBytes: 30 * MIB,
      packageSizeMiB: 30,
    });
    const aboveBoundary = evaluate({
      coldStartMs: 200,
      singleTabRssMiB: 1,
      singleTabPssMiB: 1,
      tenTabRssMiB: 1,
      tenTabPssMiB: 1,
      packageSizeBytes: 30 * MIB + 1,
      packageSizeMiB: 30,
    });

    expect(atBoundary.at(-1)?.pass).toBe(true);
    expect(aboveBoundary.at(-1)).toMatchObject({
      actual: 30 * MIB + 1,
      budget: 30 * MIB,
      pass: false,
    });
  });

  it("rounds bytes to MiB for stable output", () => {
    expect(bytesToMiB(31_457_280)).toBe(30);
    expect(bytesToMiB(1_572_864)).toBe(1.5);
  });

  it("falls back to RSS when PSS is unavailable", () => {
    const checks = evaluate(
      {
        coldStartMs: 200,
        singleTabRssMiB: 290,
        singleTabPssMiB: null,
        tenTabRssMiB: 301,
        tenTabPssMiB: null,
        packageSizeMiB: 20,
      },
      {
        coldStartMs: 2_000,
        singleTabPssMiB: 300,
        tenTabPssMiB: 300,
        packageSizeMiB: 30,
      },
    );

    expect(checks[1]).toEqual({
      name: "singleTabRssMiB",
      actual: 290,
      budget: 300,
      pass: true,
    });
    expect(checks[2]).toEqual({
      name: "tenTabRssMiB",
      actual: 301,
      budget: 300,
      pass: false,
    });
  });
});

describe("release performance sampling", () => {
  it("alternates scenarios and selects deterministic medians", () => {
    expect(samplingPlan(3)).toEqual([1, 10, 10, 1, 1, 10]);
    expect(median([12, 8, 10])).toBe(10);
    expect(median([12, 8, 10, 14])).toBe(11);
    expect(aggregateSamples([
      { readyMs: 240, rssMiB: 390, pssMiB: 270 },
      { readyMs: 200, rssMiB: 370, pssMiB: 250 },
      { readyMs: 220, rssMiB: 380, pssMiB: 260 },
    ])).toEqual({ readyMs: 220, rssMiB: 380, pssMiB: 260 });
  });

  it("does not report a partial PSS aggregate", () => {
    expect(aggregateSamples([
      { readyMs: 200, rssMiB: 370, pssMiB: 250 },
      { readyMs: 220, rssMiB: 380, pssMiB: null },
      { readyMs: 240, rssMiB: 390, pssMiB: 270 },
    ]).pssMiB).toBeNull();
  });
});

describe("release performance display metadata", () => {
  it("separates the host Wayland session from the forced X11 child", () => {
    const environment = {
      DISPLAY: ":0",
      WAYLAND_DISPLAY: "wayland-0",
      XDG_SESSION_TYPE: "wayland",
    };
    const appEnvironment = measurementEnvironment(environment);

    expect(appEnvironment).toMatchObject({
      GDK_BACKEND: "x11",
      WEBKIT_DISABLE_DMABUF_RENDERER: "1",
    });
    expect(displayMetadata(environment, appEnvironment)).toEqual({
      hostSession: "wayland",
      hostDisplay: ":0",
      hostWaylandDisplay: "wayland-0",
      appGdkBackend: "x11",
      webkitDisableDmabufRenderer: "1",
    });
  });

  it("preserves explicit application renderer overrides", () => {
    const environment = {
      GDK_BACKEND: "wayland",
      WEBKIT_DISABLE_DMABUF_RENDERER: "0",
      WAYLAND_DISPLAY: "wayland-1",
    };
    const appEnvironment = measurementEnvironment(environment);

    expect(appEnvironment.GDK_BACKEND).toBe("wayland");
    expect(appEnvironment.WEBKIT_DISABLE_DMABUF_RENDERER).toBe("0");
    expect(displayMetadata(environment, appEnvironment).appGdkBackend).toBe("wayland");
  });
});

describe("process tree memory aggregation", () => {
  it("sums root and descendant RSS and PSS from procfs", () => {
    const procRoot = mkdtempSync(join(tmpdir(), "postmite-proc-"));

    try {
      writeProc(procRoot, "100", 1_000, 500, ["101", "102"]);
      writeProc(procRoot, "101", 2_000, 1_000, ["103"]);
      writeProc(procRoot, "102", 3_000, 1_500, []);
      writeProc(procRoot, "103", 4_000, 2_000, []);

      expect(sumProcessTreeRssKiB("100", procRoot)).toBe(10_000);
      expect(sumProcessTreePssKiB("100", procRoot)).toBe(5_000);
      expect(sumProcessTreeMemoryKiB("100", procRoot)).toEqual({
        rssKiB: 10_000,
        pssKiB: 5_000,
      });
    } finally {
      rmSync(procRoot, { force: true, recursive: true });
    }
  });

  it("returns null PSS when any process lacks smaps_rollup", () => {
    const procRoot = mkdtempSync(join(tmpdir(), "postmite-proc-"));

    try {
      writeProc(procRoot, "100", 1_000, 500, ["101"]);
      writeProc(procRoot, "101", 2_000, null, []);

      expect(sumProcessTreeMemoryKiB("100", procRoot)).toEqual({
        rssKiB: 3_000,
        pssKiB: null,
      });
    } finally {
      rmSync(procRoot, { force: true, recursive: true });
    }
  });
});

function writeProc(procRoot, pid, rssKiB, pssKiB, children) {
  const taskDir = join(procRoot, pid, "task", pid);
  mkdirSync(taskDir, { recursive: true });
  writeFileSync(join(procRoot, pid, "status"), `Name:\tpostmite\nVmRSS:\t${rssKiB} kB\n`);
  if (pssKiB !== null) {
    writeFileSync(join(procRoot, pid, "smaps_rollup"), `Rss:\t${rssKiB} kB\nPss:\t${pssKiB} kB\n`);
  }
  writeFileSync(join(taskDir, "children"), children.join(" "));
}
