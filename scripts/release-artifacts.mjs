import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { inspectReleaseCandidate } from "./release-candidate.mjs";
import { getReleaseTarget, releaseTargetMetadata, releaseTargetMetadataMatches } from "./release-targets.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
const version = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8")).version;
const cargoTargetDir = process.env.CARGO_TARGET_DIR
  ? resolve(repoRoot, process.env.CARGO_TARGET_DIR)
  : join(repoRoot, "src-tauri", "target");
const target = getReleaseTarget();
const sourceBundleDirectories = [
  join(cargoTargetDir, target.rustTarget, "release", "bundle"),
  join(cargoTargetDir, "release", "bundle"),
];
const outputDir = join(repoRoot, "artifacts", "release", `v${version}`, target.key);
const packageBudgetBytes = 30 * 1024 * 1024;

function fail(message) {
  throw new Error(message);
}

function command(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 20 * 1024 * 1024,
    ...options,
  });
  if (result.status !== 0) fail(`${command} ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  return result.stdout;
}

function filesRecursively(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesRecursively(path) : [path];
  });
}

function digest(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function byteSize(directory) {
  return filesRecursively(directory).reduce((total, path) => total + statSync(path).size, 0);
}

function appImageBudget(appImagePath) {
  const extractionDirectory = mkdtempSync(join(tmpdir(), "postmite-appimage-"));
  try {
    command(appImagePath, ["--appimage-extract"], { cwd: extractionDirectory });
    const root = join(extractionDirectory, "squashfs-root");
    const webViewRuntime = join(root, "usr", "lib");
    if (!existsSync(webViewRuntime)) fail("AppImage must contain the bundled OS WebView runtime under usr/lib.");
    const productSquashfs = join(extractionDirectory, "postmite-product.squashfs");
    command("mksquashfs", [root, productSquashfs, "-noappend", "-no-progress", "-e", "usr/lib"]);
    return {
      appImageBytes: statSync(appImagePath).size,
      osWebViewRuntimeBytes: byteSize(webViewRuntime),
      productPayloadBytes: statSync(productSquashfs).size,
      excludedPath: "usr/lib",
    };
  } finally {
    rmSync(extractionDirectory, { recursive: true, force: true });
  }
}

function releasePackages() {
  const sourceBundleDir = sourceBundleDirectories.find((directory) => existsSync(directory));
  if (!sourceBundleDir) fail(`No Tauri bundle directory found for ${target.rustTarget}.`);
  const files = filesRecursively(sourceBundleDir);
  return target.packageExtensions.map((extension) => {
    const matches = files.filter((path) => path.endsWith(extension));
    if (matches.length !== 1) fail(`Expected one ${extension} Tauri release bundle for ${target.key}.`);
    return matches[0];
  });
}

function dependencyLicenses() {
  const npm = filesRecursively(join(repoRoot, "node_modules")).flatMap((path) => {
    if (basename(path) !== "package.json") return [];
    const metadata = JSON.parse(readFileSync(path, "utf8"));
    return metadata.name && metadata.version ? [{ ecosystem: "npm", name: metadata.name, version: metadata.version, license: metadata.license ?? "UNKNOWN" }] : [];
  });
  const cargo = JSON.parse(command("cargo", ["metadata", "--manifest-path", "src-tauri/Cargo.toml", "--locked", "--format-version", "1"])).packages
    .map((pkg) => ({ ecosystem: "cargo", name: pkg.name, version: pkg.version, license: pkg.license ?? "UNKNOWN" }));
  return [...npm, ...cargo].sort((left, right) => `${left.ecosystem}:${left.name}`.localeCompare(`${right.ecosystem}:${right.name}`));
}

function collect() {
  const packages = releasePackages();
  rmSync(outputDir, { recursive: true, force: true });
  mkdirSync(outputDir, { recursive: true });
  for (const source of packages) {
    const destination = join(outputDir, basename(source));
    writeFileSync(destination, readFileSync(source));
    chmodSync(destination, statSync(source).mode);
  }
  const artifacts = packages.map((source) => basename(source)).sort();
  const checksums = artifacts.map((name) => `${digest(join(outputDir, name))}  ${name}`).join("\n") + "\n";
  writeFileSync(join(outputDir, "SHA256SUMS"), checksums);
  writeFileSync(join(outputDir, "DEPENDENCY_LICENSES.json"), `${JSON.stringify(dependencyLicenses(), null, 2)}\n`);
  if (target.appImageBudget) {
    const appImage = join(outputDir, artifacts.find((name) => name.endsWith(".AppImage")));
    writeFileSync(join(outputDir, "APPIMAGE_BUDGET.json"), `${JSON.stringify(appImageBudget(appImage), null, 2)}\n`);
  }
  writeFileSync(join(outputDir, "THIRD_PARTY_NOTICES.md"), "# Third-party notices\n\nSee `DEPENDENCY_LICENSES.json` for the complete npm and Cargo dependency license record included with this release.\n");
  writeFileSync(join(outputDir, "RELEASE_NOTES.md"), readFileSync(join(repoRoot, "release", "RELEASE_NOTES.md")));
  writeFileSync(join(outputDir, "RELEASE_TARGET.json"), `${JSON.stringify(releaseTargetMetadata(target), null, 2)}\n`);
  writeFileSync(join(outputDir, "RELEASE_CANDIDATE.json"), `${JSON.stringify(inspectReleaseCandidate({ targetKey: target.key }), null, 2)}\n`);
  console.log(outputDir);
}

function verifySource() {
  const targetConfig = getReleaseTarget();
  const config = JSON.parse(readFileSync(join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8"));
  if (config.identifier !== "io.github.sino-s.postmite") fail("Release package identifier must use the approved public namespace.");
  if (targetConfig.bundles.some((bundle) => !config.bundle.targets.includes(bundle))) fail(`Release configuration must include ${targetConfig.bundles.join(", ")} bundle targets.`);
  if (!readFileSync(join(repoRoot, "release", "RELEASE_NOTES.md"), "utf8").includes("does not poll")) fail("Release notes must document opt-in update checks.");
  console.log("release source configuration verified");
}

function verify() {
  const artifacts = target.packageExtensions.map((extension) => readdirSync(outputDir).find((name) => name.endsWith(extension)));
  if (artifacts.some((name) => !name)) fail(`Release directory ${target.key} must contain ${target.packageExtensions.join(" and ")} artifacts.`);
  if (artifacts.some((name) => !target.architectureTokens.some((token) => name.toLowerCase().includes(token.toLowerCase())))) fail(`Release package names must identify ${target.architecture}.`);
  for (const name of artifacts.filter((name) => name.endsWith(".deb"))) {
    const path = join(outputDir, name);
    if (statSync(path).size > packageBudgetBytes) fail(`${name} exceeds the 30 MiB package budget.`);
  }
  if (target.appImageBudget) {
    const appImageBudgetRecord = JSON.parse(readFileSync(join(outputDir, "APPIMAGE_BUDGET.json"), "utf8"));
    if (appImageBudgetRecord.excludedPath !== "usr/lib" || appImageBudgetRecord.osWebViewRuntimeBytes <= 0) fail("AppImage budget must identify its bundled OS WebView runtime.");
    if (appImageBudgetRecord.productPayloadBytes > packageBudgetBytes) fail("AppImage product payload exceeds the 30 MiB package budget excluding its OS WebView runtime.");
  }
  const checksums = readFileSync(join(outputDir, "SHA256SUMS"), "utf8").trim().split("\n");
  if (checksums.length !== artifacts.length) fail("SHA256SUMS must contain exactly one entry for each release package.");
  for (const name of artifacts) {
    if (!checksums.includes(`${digest(join(outputDir, name))}  ${name}`)) fail(`Checksum mismatch for ${name}.`);
  }
  const licenses = JSON.parse(readFileSync(join(outputDir, "DEPENDENCY_LICENSES.json"), "utf8"));
  if (!licenses.some((entry) => entry.ecosystem === "npm") || !licenses.some((entry) => entry.ecosystem === "cargo")) fail("Dependency license record must include npm and Cargo packages.");
  for (const name of ["THIRD_PARTY_NOTICES.md", "RELEASE_NOTES.md", "RELEASE_CANDIDATE.json", "RELEASE_TARGET.json"]) if (!existsSync(join(outputDir, name))) fail(`Missing ${name}.`);
  if (target.appImageBudget && !existsSync(join(outputDir, "APPIMAGE_BUDGET.json"))) fail("Missing APPIMAGE_BUDGET.json.");
  const targetEvidence = JSON.parse(readFileSync(join(outputDir, "RELEASE_TARGET.json"), "utf8"));
  if (!releaseTargetMetadataMatches(targetEvidence, target)) fail("Release target metadata does not match the selected target.");
  const candidateEvidence = JSON.parse(readFileSync(join(outputDir, "RELEASE_CANDIDATE.json"), "utf8"));
  if (!releaseTargetMetadataMatches(candidateEvidence.artifactTarget, target)) fail("Release candidate target metadata does not match the selected target.");
  if (target.platform === "linux") command("dpkg-deb", ["--info", join(outputDir, artifacts.find((name) => name.endsWith(".deb")))]);
  console.log("release artifacts verified");
}

const action = process.argv[2];
if (action === "collect") collect();
else if (action === "verify") verify();
else if (action === "verify-source") verifySource();
else fail("Usage: node scripts/release-artifacts.mjs <collect|verify|verify-source>");
