import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const readme = readFileSync(join(repoRoot, "README.md"), "utf8");
const packageJson = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
const tauriConfig = JSON.parse(readFileSync(join(repoRoot, "src-tauri/tauri.conf.json"), "utf8"));
const releaseNotes = readFileSync(join(repoRoot, "release/RELEASE_NOTES.md"), "utf8");
const tauriLibrary = readFileSync(join(repoRoot, "src-tauri/src/lib.rs"), "utf8");
const responseExecution = readFileSync(
  join(repoRoot, "src-tauri/src/infrastructure/http/execution/response.rs"),
  "utf8",
);
const secretApplication = readFileSync(join(repoRoot, "src-tauri/src/application/secrets.rs"), "utf8");
const workspaceDomain = readFileSync(join(repoRoot, "src-tauri/src/domain/workspace.rs"), "utf8");

test("README describes the supported release artifacts and commands", () => {
  const releaseVersion = packageJson.version.replaceAll(".", "\\.");
  assert.equal(packageJson.version, "0.3.0");
  assert.doesNotMatch(readme, /実装開始前/);
  assert.match(readme, /Ubuntu 24\.04 LTS x86_64/);
  assert.match(readme, new RegExp(`Postmite_${releaseVersion}_amd64\\.deb`));
  assert.match(readme, new RegExp(`Postmite_${releaseVersion}_amd64\\.AppImage`));
  for (const checksum of ["linux-x86_64-SHA256SUMS", "windows-x86_64-SHA256SUMS"]) {
    assert.match(readme, new RegExp(`sha256sum --check ${checksum}`));
  }
  assert.match(readme, /WindowsとmacOSのProtected valuesはsession-only・memory-only/);
  assert.match(readme, new RegExp(`sudo apt install \\./Postmite_${releaseVersion}_amd64\\.deb`));
  assert.match(readme, new RegExp(`chmod \\+x \\./Postmite_${releaseVersion}_amd64\\.AppImage`));
  assert.match(readme, new RegExp(`Postmite_${releaseVersion}_x64_en-US\\.msi`));
  assert.match(readme, /Postmite_0\.2\.0_aarch64\.dmg/);
  assert.doesNotMatch(readme, /Postmite_0\.3\.0_aarch64\.dmg/);
  assert.match(readme, /v0\.3\.0以降の公開ReleaseではmacOSのDMGを配布しません/);
  assert.match(readme, /git clone https:\/\/github\.com\/sino-s\/postmite\.git/);
  assert.match(readme, /pnpm install --frozen-lockfile/);
  assert.match(readme, /pnpm release:bundle:macos/);
  assert.match(releaseNotes, new RegExp(`Postmite_${releaseVersion}_amd64\\.deb`));
  assert.match(releaseNotes, new RegExp(`Postmite_${releaseVersion}_amd64\\.AppImage`));
  assert.match(readme, /v0\.3\.0では、選択した日本語または英語の表示言語をアプリの再起動後も復元します/);
  assert.match(releaseNotes, /supersedes v0\.2\.0 without moving its tag/);
  assert.match(releaseNotes, /selected English or Japanese display language is restored/);
  assert.doesNotMatch(releaseNotes, /Postmite_0\.3\.0_aarch64\.dmg/);
  assert.doesNotMatch(readme, /Postmite_0\.1\.0_amd64/);
  assert.deepEqual(tauriConfig.bundle.targets, ["deb", "appimage", "msi", "dmg"]);
});

test("README records the local persistence and Secret boundaries", () => {
  const identifier = tauriConfig.identifier.replaceAll(".", "\\.");
  assert.match(readme, new RegExp(`\\$XDG_DATA_HOME/${identifier}`));
  assert.match(readme, new RegExp(`\\$HOME/\\.local/share/${identifier}`));
  assert.match(readme, new RegExp(`\\$XDG_CONFIG_HOME/${identifier}/\\.window-state\\.json`));
  assert.match(readme, new RegExp(`\\$HOME/\\.config/${identifier}/\\.window-state\\.json`));
  assert.match(readme, /XDG_CONFIG_HOME`に絶対パス/);
  assert.match(readme, /postmite\.sqlite3/);
  assert.match(readme, /\$\{TMPDIR:-\/tmp\}\/postmite-response-files/);
  assert.match(readme, /Secret Service/);
  assert.match(readme, /OSのSecret ServiceへSecret値を保存します/);
  assert.match(readme, /セッションストレージ/);
  assert.match(readme, /平文で保存しません/);
  assert.match(readme, /ユーザーが選択した出力先/);
  assert.match(tauriLibrary, /app_data_dir\.join\("postmite\.sqlite3"\)/);
  assert.match(tauriLibrary, /with_state_flags\(StateFlags::SIZE\)/);
  assert.match(responseExecution, /env::temp_dir\(\)\.join\("postmite-response-files"\)/);
  assert.match(secretApplication, /SecretError::Unavailable \| SecretError::Locked/);
  assert.match(secretApplication, /self\.session\.put\(owner, value\)/);
});

test("README keeps update and unsupported-feature claims bounded", () => {
  assert.match(readme, /バックグラウンドで更新を確認しません/);
  assert.match(readme, /Check for updates/);
  assert.match(readme, /自動更新機能はありません/);
  for (const unsupported of ["クラウド同期", "チーム共有", "GraphQL", "WebSocket", "Windows", "macOS"]) {
    assert.match(readme, new RegExp(unsupported));
  }
});

test("README development commands exist and relative links resolve", () => {
  for (const script of ["ci:rust", "ci:frontend"]) {
    assert.equal(typeof packageJson.scripts[script], "string", `missing pnpm script: ${script}`);
  }
  assert.equal(packageJson.scripts.tauri, "tauri dev");
  assert.match(readme, /\npnpm tauri\n/);
  assert.doesNotMatch(readme, /pnpm tauri dev/);

  const relativeLinks = [...readme.matchAll(/\[[^\]]+\]\((\.\/[^)#]+)(?:#[^)]+)?\)/g)].map((match) => match[1]);
  assert.ok(relativeLinks.length > 0, "README should contain relative project links");
  for (const target of relativeLinks) {
    assert.ok(existsSync(resolve(repoRoot, target)), `README link does not resolve: ${target}`);
  }
});

test("README initial-use flow matches the default workspace", () => {
  assert.match(readme, /初回起動時に選択されているPersonalワークスペース/);
  assert.match(workspaceDomain, /DEFAULT_WORKSPACE_NAME: &str = "Personal"/);
});

test("README shell blocks are valid Bash", () => {
  const shellBlocks = [...readme.matchAll(/```bash\n([\s\S]*?)\n```/g)].map((match) => match[1]);
  assert.ok(shellBlocks.length > 0, "README should contain shell examples");
  for (const block of shellBlocks) {
    const syntaxCheck = spawnSync("bash", ["-n"], { input: block, encoding: "utf8" });
    assert.equal(syntaxCheck.status, 0, syntaxCheck.stderr);
  }
});
