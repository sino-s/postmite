import assert from "node:assert/strict";
import test from "node:test";
import { inspectReleaseCandidate } from "./release-candidate.mjs";

test("release candidate remains an unpublished Ubuntu preview with no native capability grants", () => {
  assert.deepEqual(inspectReleaseCandidate(), {
    version: "0.1.0",
    productName: "Postmite",
    packageIdentifier: "io.github.sino-s.postmite",
    publisher: "sino-s",
    trademark: "project name approved by publisher; no registered-mark claim",
    githubRelease: "not published automatically",
    nativeCapabilities: [],
    updateChecks: "manual command only; regression-tested before the request",
    platforms: ["Ubuntu 24.04 x86_64"],
  });
});
