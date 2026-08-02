import { describe, expect, it } from "vitest";
import {
  getReleaseTarget,
  RELEASE_TARGETS,
  releaseTargetMetadata,
  releaseTargetMetadataMatches,
} from "./release-targets.mjs";

describe("release target contract", () => {
  it("defines only the supported platform and architecture directories", () => {
    expect(Object.keys(RELEASE_TARGETS)).toEqual([
      "linux-x86_64",
      "windows-x86_64",
      "macos-aarch64",
    ]);
    expect(RELEASE_TARGETS["windows-x86_64"].rustTarget).toBe("x86_64-pc-windows-msvc");
    expect(RELEASE_TARGETS["macos-aarch64"].rustTarget).toBe("aarch64-apple-darwin");
  });

  it("keeps package extensions and architecture tokens coupled to each target", () => {
    expect(RELEASE_TARGETS["linux-x86_64"].packageExtensions).toEqual([".deb", ".AppImage"]);
    expect(RELEASE_TARGETS["windows-x86_64"].packageExtensions).toEqual([".msi"]);
    expect(RELEASE_TARGETS["macos-aarch64"].packageExtensions).toEqual([".dmg"]);
    for (const target of Object.values(RELEASE_TARGETS)) {
      expect(target.architectureTokens.length).toBeGreaterThan(0);
      expect(releaseTargetMetadata(target)).toMatchObject({
        key: target.key,
        architecture: target.architecture,
        rustTarget: target.rustTarget,
      });
    }
  });

  it("rejects target metadata that changes any platform or package contract", () => {
    const target = RELEASE_TARGETS["windows-x86_64"];
    const metadata = releaseTargetMetadata(target);

    expect(releaseTargetMetadataMatches(metadata, target)).toBe(true);
    expect(releaseTargetMetadataMatches({ ...metadata, platform: "linux" }, target)).toBe(false);
    expect(releaseTargetMetadataMatches({ ...metadata, bundles: ["dmg"] }, target)).toBe(false);
    expect(releaseTargetMetadataMatches({ ...metadata, packageExtensions: [".zip"] }, target)).toBe(false);
  });

  it("rejects an unrecognized target instead of inferring from the runner", () => {
    expect(() => getReleaseTarget("linux-arm64")).toThrow("Unsupported release target");
  });
});
