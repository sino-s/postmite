import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const version = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8")).version;
const artifactDir = join(repoRoot, "artifacts", "release", `v${version}`, "linux-x86_64");
const canonicalNativeCapabilities = [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
  "clipboard-manager:allow-write-text",
];
const nativeCapabilityBoundary = "main window: event listen/unlisten and clipboard text write only";

function fail(message) {
  throw new Error(message);
}

function read(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function hasCanonicalNativeCapabilities(permissions) {
  return Array.isArray(permissions)
    && permissions.length === canonicalNativeCapabilities.length
    && permissions.every((permission, index) => permission === canonicalNativeCapabilities[index]);
}

function assertCanonicalNativeCapabilities(permissions) {
  if (!hasCanonicalNativeCapabilities(permissions)) {
    fail("Release candidate native capability allowlist mismatch.");
  }
}

export function inspectReleaseCandidate({ capabilityPath = join(repoRoot, "src-tauri/capabilities/main.json") } = {}) {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  const capability = JSON.parse(readFileSync(capabilityPath, "utf8"));
  const manifest = read("src-tauri/Cargo.toml");
  const trademarkGate = read("release/TRADEMARK_GATE.md");
  const workflow = read(".github/workflows/ci.yml");
  const updateTest = read("src-tauri/src/application/update.rs");
  const releaseNotes = read("release/RELEASE_NOTES.md");

  if (config.productName !== "Postmite") fail("Release candidate product name must be Postmite.");
  if (config.identifier !== "io.github.sino-s.postmite") fail("Release candidate must use the public io.github.sino-s.postmite identifier.");
  if (!manifest.includes('authors = ["sino-s"]') || !manifest.includes('repository = "https://github.com/sino-s/postmite"')) fail("Release candidate publisher must be traceable to the source package metadata.");
  assertCanonicalNativeCapabilities(capability.permissions);
  if (/tauri-plugin-updater|tauri-plugin-upload/i.test(manifest)) fail("Release candidate must not include an automatic publishing or update plugin.");
  if (/gh\s+release\s+create|actions\/create-release|softprops\/action-gh-release/i.test(workflow)) fail("CI must not publish a GitHub release.");
  if (!/sends_no_network_request_before_the_manual_check_then_requests_once/.test(updateTest)) fail("Manual update network behavior must have a regression test.");
  if (!releaseNotes.includes("does not poll") || !releaseNotes.includes("`io.github.sino-s.postmite`") || !releaseNotes.includes("publisher is `sino-s`")) fail("Release notes must describe opt-in updates, package identity, and publisher.");
  if (!trademarkGate.includes("approved for distribution by `sino-s`") || !trademarkGate.includes("does not claim that `Postmite` is a registered trademark")) fail("Release candidate must record a bounded project-name gate.");

  return {
    version,
    productName: config.productName,
    packageIdentifier: config.identifier,
    publisher: "sino-s",
    trademark: "project name approved by publisher; no registered-mark claim",
    githubRelease: "not published automatically",
    nativeCapabilities: [...canonicalNativeCapabilities],
    nativeCapabilityBoundary,
    updateChecks: "manual command only; regression-tested before the request",
    platforms: ["Ubuntu 24.04 x86_64"],
  };
}

export function verifyArtifactEvidence() {
  const required = ["SHA256SUMS", "DEPENDENCY_LICENSES.json", "APPIMAGE_BUDGET.json", "THIRD_PARTY_NOTICES.md", "RELEASE_NOTES.md", "RELEASE_CANDIDATE.json"];
  for (const name of required) {
    if (!existsSync(join(artifactDir, name))) fail(`Missing release-candidate evidence: ${name}.`);
  }
  const evidence = JSON.parse(readFileSync(join(artifactDir, "RELEASE_CANDIDATE.json"), "utf8"));
  if (evidence.packageIdentifier !== "io.github.sino-s.postmite" || evidence.publisher !== "sino-s" || evidence.githubRelease !== "not published automatically") fail("Artifact release-candidate evidence does not preserve the identity and publication gate.");
  assertCanonicalNativeCapabilities(evidence.nativeCapabilities);
  if (evidence.nativeCapabilityBoundary !== nativeCapabilityBoundary) fail("Artifact release-candidate evidence does not preserve the native capability boundary.");
  const packages = readdirSync(artifactDir).filter((name) => name.endsWith(".deb") || name.endsWith(".AppImage"));
  if (packages.length !== 2 || packages.some((name) => statSync(join(artifactDir, name)).size === 0)) fail("Release candidate must include non-empty Debian and AppImage packages.");
}

const action = process.argv[2];
if (action === "inspect") console.log(JSON.stringify(inspectReleaseCandidate(), null, 2));
else if (action === "verify-artifacts") {
  verifyArtifactEvidence();
  console.log("release candidate artifact evidence verified");
} else if (import.meta.url === `file://${process.argv[1]}`) {
  fail("Usage: node scripts/release-candidate.mjs <inspect|verify-artifacts>");
}
