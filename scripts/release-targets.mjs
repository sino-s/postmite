import { isDeepStrictEqual } from "node:util";

const targets = {
  "linux-x86_64": {
    key: "linux-x86_64",
    platform: "linux",
    platformLabel: "Ubuntu 24.04",
    architecture: "x86_64",
    rustTarget: "x86_64-unknown-linux-gnu",
    bundles: ["deb", "appimage"],
    packageExtensions: [".deb", ".AppImage"],
    architectureTokens: ["amd64", "x86_64"],
    appImageBudget: true,
  },
  "windows-x86_64": {
    key: "windows-x86_64",
    platform: "windows",
    platformLabel: "Windows",
    architecture: "x86_64",
    rustTarget: "x86_64-pc-windows-msvc",
    bundles: ["msi"],
    packageExtensions: [".msi"],
    architectureTokens: ["x64", "x86_64"],
    appImageBudget: false,
  },
  "macos-aarch64": {
    key: "macos-aarch64",
    platform: "macos",
    platformLabel: "Apple Silicon macOS",
    architecture: "aarch64",
    rustTarget: "aarch64-apple-darwin",
    bundles: ["dmg"],
    packageExtensions: [".dmg"],
    architectureTokens: ["aarch64", "arm64"],
    appImageBudget: false,
  },
};

export const RELEASE_TARGETS = Object.freeze(
  Object.fromEntries(
    Object.entries(targets).map(([key, target]) => [
      key,
      Object.freeze({
        ...target,
        bundles: Object.freeze([...target.bundles]),
        packageExtensions: Object.freeze([...target.packageExtensions]),
        architectureTokens: Object.freeze([...target.architectureTokens]),
      }),
    ]),
  ),
);

export function getReleaseTarget(key = process.env.POSTMITE_RELEASE_TARGET ?? "linux-x86_64") {
  const target = RELEASE_TARGETS[key];
  if (!target) {
    throw new Error(`Unsupported release target: ${key}. Expected one of ${Object.keys(RELEASE_TARGETS).join(", ")}.`);
  }
  return target;
}

export function releaseTargetMetadata(target) {
  return {
    key: target.key,
    platform: target.platform,
    platformLabel: target.platformLabel,
    architecture: target.architecture,
    rustTarget: target.rustTarget,
    bundles: [...target.bundles],
    packageExtensions: [...target.packageExtensions],
  };
}

export function releaseTargetMetadataMatches(actual, target) {
  return isDeepStrictEqual(actual, releaseTargetMetadata(target));
}
