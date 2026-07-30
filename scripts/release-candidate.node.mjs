import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { inspectReleaseCandidate } from "./release-candidate.mjs";

const canonicalPermissions = [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
  "clipboard-manager:allow-write-text",
];

function inspectWithPermissions(t, permissions) {
  const directory = mkdtempSync(join(tmpdir(), "postmite-capability-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const capabilityPath = join(directory, "main.json");
  writeFileSync(capabilityPath, `${JSON.stringify({ permissions }, null, 2)}\n`);
  return inspectReleaseCandidate({ capabilityPath });
}

test("release candidate remains an unpublished Ubuntu preview with bounded native capabilities", (t) => {
  const evidence = inspectWithPermissions(t, canonicalPermissions);
  assert.deepEqual(inspectReleaseCandidate(), {
    version: "0.1.0",
    productName: "Postmite",
    packageIdentifier: "io.github.sino-s.postmite",
    publisher: "sino-s",
    trademark: "project name approved by publisher; no registered-mark claim",
    githubRelease: "not published automatically",
    nativeCapabilities: canonicalPermissions,
    nativeCapabilityBoundary: "main window: event listen/unlisten and clipboard text write only",
    updateChecks: "manual command only; regression-tested before the request",
    platforms: ["Ubuntu 24.04 x86_64"],
  });
  assert.deepEqual(evidence.nativeCapabilities, canonicalPermissions);
});

const rejectedPermissionLists = [
  ["a missing permission", ["core:event:allow-listen", "core:event:allow-unlisten"]],
  ["an unexpected permission", [...canonicalPermissions, "core:window:allow-close"]],
  ["a duplicate permission", [...canonicalPermissions, "clipboard-manager:allow-write-text"]],
  ["reordered permissions", [canonicalPermissions[1], canonicalPermissions[0], canonicalPermissions[2]]],
  ["clipboard read access", ["core:event:allow-listen", "core:event:allow-unlisten", "clipboard-manager:allow-read-text"]],
  ["a broad core permission", [...canonicalPermissions, "core:default"]],
  ["shell access", [...canonicalPermissions, "shell:allow-execute"]],
  ["filesystem access", [...canonicalPermissions, "fs:allow-read-text-file"]],
];

for (const [name, permissions] of rejectedPermissionLists) {
  test(`release candidate rejects ${name}`, (t) => {
    assert.throws(
      () => inspectWithPermissions(t, permissions),
      { message: "Release candidate native capability allowlist mismatch." },
    );
  });
}
