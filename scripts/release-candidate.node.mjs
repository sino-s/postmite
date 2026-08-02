import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { inspectReleaseCandidate, verifyArtifactEvidence } from "./release-candidate.mjs";
import { RELEASE_TARGETS, releaseTargetMetadata } from "./release-targets.mjs";

const canonicalPermissions = [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
  "clipboard-manager:allow-write-text",
];

function createArtifactFixture(t, targetKey = "windows-x86_64") {
  const root = mkdtempSync(join(tmpdir(), "postmite-release-artifact-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const target = RELEASE_TARGETS[targetKey];
  const directory = join(root, target.key);
  mkdirSync(directory, { recursive: true });
  const packageName = `postmite-${target.architecture === "x86_64" ? "x64" : "arm64"}${target.packageExtensions[0]}`;
  const packagePath = join(directory, packageName);
  writeFileSync(packagePath, "package fixture\n");
  const checksum = createHash("sha256").update(readFileSync(packagePath)).digest("hex");
  writeFileSync(join(directory, "SHA256SUMS"), `${checksum}  ${packageName}\n`);
  writeFileSync(join(directory, "DEPENDENCY_LICENSES.json"), "[]\n");
  writeFileSync(join(directory, "THIRD_PARTY_NOTICES.md"), "# notices\n");
  writeFileSync(join(directory, "RELEASE_NOTES.md"), "# notes\n");
  writeFileSync(join(directory, "RELEASE_TARGET.json"), `${JSON.stringify(releaseTargetMetadata(target))}\n`);
  writeFileSync(join(directory, "RELEASE_CANDIDATE.json"), `${JSON.stringify(inspectReleaseCandidate({ targetKey }))}\n`);
  return { root, directory, packageName, target };
}

function inspectWithPermissions(t, permissions) {
  const directory = mkdtempSync(join(tmpdir(), "postmite-capability-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const capabilityPath = join(directory, "main.json");
  writeFileSync(capabilityPath, `${JSON.stringify({ permissions }, null, 2)}\n`);
  return inspectReleaseCandidate({ capabilityPath });
}

test("release candidate remains unpublished with bounded native capabilities across explicit targets", (t) => {
  const evidence = inspectWithPermissions(t, canonicalPermissions);
  assert.deepEqual(inspectReleaseCandidate({ targetKey: "linux-x86_64" }), {
    version: "0.1.1",
    productName: "Postmite",
    packageIdentifier: "io.github.sino-s.postmite",
    publisher: "sino-s",
    trademark: "project name approved by publisher; no registered-mark claim",
    githubRelease: "not published automatically",
    nativeCapabilities: canonicalPermissions,
    nativeCapabilityBoundary: "main window: event listen/unlisten and clipboard text write only",
    updateChecks: "manual command only; regression-tested before the request",
    platforms: Object.values(RELEASE_TARGETS).map(({ platformLabel, architecture }) => `${platformLabel} ${architecture}`),
    artifactTarget: {
      key: "linux-x86_64",
      platform: "linux",
      platformLabel: "Ubuntu 24.04",
      architecture: "x86_64",
      rustTarget: "x86_64-unknown-linux-gnu",
      bundles: ["deb", "appimage"],
      packageExtensions: [".deb", ".AppImage"],
    },
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

test("artifact verification rejects target metadata changes", (t) => {
  const fixture = createArtifactFixture(t);
  const metadataPath = join(fixture.directory, "RELEASE_TARGET.json");
  const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
  metadata.platform = "linux";
  writeFileSync(metadataPath, `${JSON.stringify(metadata)}\n`);

  assert.throws(
    () => verifyArtifactEvidence({ targetKey: fixture.target.key, artifactRoot: fixture.root }),
    { message: "Release target metadata does not match the selected target." },
  );
});

test("artifact verification rejects wrong package names and checksums", (t) => {
  const fixture = createArtifactFixture(t);
  const packagePath = join(fixture.directory, fixture.packageName);
  const checksumPath = join(fixture.directory, "SHA256SUMS");
  writeFileSync(checksumPath, `${"0".repeat(64)}  ${fixture.packageName}\n`);

  assert.throws(
    () => verifyArtifactEvidence({ targetKey: fixture.target.key, artifactRoot: fixture.root }),
    { message: "Release candidate package checksums do not match SHA256SUMS." },
  );

  rmSync(packagePath);
  const wrongPackageName = "postmite-arm64.msi";
  const wrongPackagePath = join(fixture.directory, wrongPackageName);
  writeFileSync(wrongPackagePath, "package fixture\n");
  const wrongChecksum = createHash("sha256").update(readFileSync(wrongPackagePath)).digest("hex");
  writeFileSync(checksumPath, `${wrongChecksum}  ${wrongPackageName}\n`);

  assert.throws(
    () => verifyArtifactEvidence({ targetKey: fixture.target.key, artifactRoot: fixture.root }),
    { message: `Package ${wrongPackageName} does not identify ${fixture.target.architecture}.` },
  );
});
