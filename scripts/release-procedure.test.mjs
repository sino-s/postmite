import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const procedure = readFileSync(
  resolve(process.cwd(), "release/RELEASING.md"),
  "utf8",
);
const packageManifest = JSON.parse(
  readFileSync(resolve(process.cwd(), "package.json"), "utf8"),
);
const tauriConfig = JSON.parse(
  readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
);
const cargoManifest = readFileSync(
  resolve(process.cwd(), "src-tauri/Cargo.toml"),
  "utf8",
);
const releaseNotes = readFileSync(
  resolve(process.cwd(), "release/RELEASE_NOTES.md"),
  "utf8",
);
const ciWorkflow = readFileSync(
  resolve(process.cwd(), ".github/workflows/ci.yml"),
  "utf8",
);

describe("tracked release procedure", () => {
  it("binds publication to the exact reviewed commit and immutable tag", () => {
    expect(packageManifest.version).toBe("0.2.0");
    expect(procedure).toContain("RELEASE_COMMIT=$(git rev-parse origin/main)");
    expect(procedure).toContain('--commit "$RELEASE_COMMIT"');
    expect(procedure).toContain('git tag --annotate "$RELEASE_TAG" "$RELEASE_COMMIT"');
    expect(procedure).toContain('git push origin "refs/tags/$RELEASE_TAG"');
    expect(procedure).toContain("Treat a pushed release tag as immutable.");
    expect(procedure).toContain('PREVIOUS_RELEASE_TAG="v0.1.1"');
    expect(procedure).toContain('test "$RELEASE_TAG" = "v0.2.0"');
    expect(procedure).toContain('gh release view "$PREVIOUS_RELEASE_TAG"');
    expect(procedure).toContain('! gh release view "$RELEASE_TAG"');
    expect(procedure).toContain("this procedure never moves, deletes, or replaces v0.1.1");
    expect(procedure).toContain("--verify-tag");
    expect(tauriConfig.version).toBe(packageManifest.version);
    expect(cargoManifest).toMatch(
      new RegExp(`^version = "${packageManifest.version.replaceAll(".", "\\.")}"$`, "m"),
    );

    for (const job of [
      "Pull request quality gates",
      "Release Tauri build",
      "Release performance",
      "Ubuntu release artifacts",
      "Ubuntu release smoke",
      "Windows x64 release",
      "Apple Silicon macOS release",
      "Download and audit all release artifacts",
    ]) {
      expect(procedure).toContain(`- \`${job}\``);
    }
  });

  it("enumerates the complete workflow artifact evidence", () => {
    for (const name of [
      "*.deb",
      "*.AppImage",
      "*.msi",
      "*.dmg",
      "SHA256SUMS",
      "DEPENDENCY_LICENSES.json",
      "THIRD_PARTY_NOTICES.md",
      "APPIMAGE_BUDGET.json",
      "RELEASE_CANDIDATE.json",
      "RELEASE_TARGET.json",
      "RELEASE_NOTES.md",
    ]) {
      expect(procedure).toContain(`- \`${name}\``);
    }
    expect(procedure).toContain('gh run download "$TAG_RUN_ID"');
    expect(procedure).toContain('gh release download "$RELEASE_TAG"');
    expect(procedure).toContain("sha256sum --check SHA256SUMS");
    expect(procedure).toContain("Audit v0.2.0 cross-platform CI artifacts");
    expect(procedure).toContain("verify_cross_platform_target");
    expect(procedure).toContain("set -euo pipefail");
    expect(procedure).toContain("architecture_tokens");
    expect(procedure).toContain(".artifactTarget.platformLabel == $platform_label");
    expect(procedure).toContain(".artifactTarget.packageExtensions == $package_extensions");
    expect(procedure).toContain('.version == "0.2.0"');
    expect(ciWorkflow).toContain('id: release-version');
    expect(ciWorkflow).toContain('artifacts/release/v${{ steps.release-version.outputs.value }}/linux-x86_64');
    expect(ciWorkflow).toContain("x86_64-pc-windows-msvc");
    expect(ciWorkflow).toContain("aarch64-apple-darwin");
    expect(ciWorkflow).toContain("postmite-windows-x86_64");
    expect(ciWorkflow).toContain("postmite-macos-aarch64");
    expect(ciWorkflow).not.toContain("artifacts/release/v0.1.0/linux-x86_64");
  });

  it("keeps publication explicit and bounded to the Ubuntu preview", () => {
    expect(procedure).toContain("Only `sino-s`");
    expect(procedure).toContain("unsigned Debian package and an unsigned AppImage");
    expect(releaseNotes).toContain("These cross-platform preview packages are unsigned");
    expect(releaseNotes).toContain("package signing is not included in v0.2.0");
    expect(releaseNotes).toContain("supersedes v0.1.1");
    expect(procedure).toContain("Ubuntu 24.04 x86_64");
    expect(procedure).toContain("windows-x86_64");
    expect(procedure).toContain("macos-aarch64");
    expect(procedure).toContain("the v0.2.0 cross-platform artifacts are audited above before publication");
    expect(releaseNotes).toContain("Windows x64");
    expect(releaseNotes).toContain("Apple Silicon macOS");
    expect(releaseNotes).toContain("session-only");
    expect(procedure).toContain("POSTMITE_SESSION_ONLY_SECRETS");
    expect(procedure).toContain("Check for updates");
    expect(procedure).toContain("Milestone v0.2.0");
    expect(procedure).toContain("never silently replace assets");
    expect(existsSync(resolve(process.cwd(), "README.md"))).toBe(true);
    expect(procedure).toContain("[repository README](../README.md)");
  });
});
