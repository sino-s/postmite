# Publishing Postmite

This procedure publishes the Ubuntu 24.04 x86_64 Postmite v0.1.1 corrective release.
End-user installation and initial-use instructions live in the
[repository README](../README.md).

Postmite v0.1.1 ships an unsigned Debian package and an unsigned AppImage.
The v0.2.0 release pipeline additionally verifies an unsigned Windows x64 MSI and
an unsigned Apple Silicon macOS DMG. Native protected-value persistence on those
platforms is not part of this procedure: values are session-only and memory-only.
Package signing, automatic publication, and automatic updates are not part of
this procedure.

The public v0.1.0 tag, Release, notes, and assets are immutable. v0.1.1
supersedes that release and must be published as a new tag with new artifacts;
this procedure never moves, deletes, or replaces v0.1.0.

## Authority and stop rule

Only `sino-s`, or a release publisher explicitly delegated by `sino-s`, may
push a release tag, create or publish a GitHub Release, replace the Latest
release, or close the release Milestone. A second person or agent must review
the release-preparation pull requests before the publisher starts.

Sections through [Verify the release commit](#verify-the-release-commit) are
preflight. The commands under [Create the immutable tag](#create-the-immutable-tag)
and later headings change public repository state. Stop before those commands
unless the publisher has explicitly authorized this release.

At any failed check, stop. Do not skip the check, weaken a budget, publish a
workspace-built package, or repair the release outside the Issue and pull
request workflow.

## Required tools and environment

Use a clean clone on Ubuntu 24.04 x86_64 with the repository's documented
Node, pnpm, Rust, Tauri, and system build dependencies. The publisher also
needs `git`, `gh`, `jq`, `sha256sum`, and `dpkg-deb`. Authenticate `gh` as the
authorized publisher:

```bash
gh auth status
gh repo view sino-s/postmite --json nameWithOwner,defaultBranchRef
```

Do not set `POSTMITE_SESSION_ONLY_SECRETS` during the final desktop acceptance
test. That test must exercise the Linux Secret Service integration.

Cross-platform CI jobs use these explicit target directories and artifacts:

- `linux-x86_64`: one `.deb` and one `.AppImage`, plus `APPIMAGE_BUDGET.json`.
- `windows-x86_64`: one `.msi` from `x86_64-pc-windows-msvc`.
- `macos-aarch64`: one `.dmg` from `aarch64-apple-darwin`.
- `*.msi` is the Windows x64 package evidence in `windows-x86_64`.
- `*.dmg` is the Apple Silicon package evidence in `macos-aarch64`.

Every directory also contains `SHA256SUMS`, `DEPENDENCY_LICENSES.json`,
`THIRD_PARTY_NOTICES.md`, `RELEASE_NOTES.md`, `RELEASE_CANDIDATE.json`, and
`RELEASE_TARGET.json`. Download all three CI artifacts and run
`sha256sum --check SHA256SUMS` in each directory before any publication step.

## Prepare the release commit

Start from a clean, current `main`. These synchronization commands change only
the local clone:

```bash
git switch main
git fetch origin --prune --tags
git pull --ff-only origin main
```

Set release-specific variables. Do not reuse broad environment variables such
as `HOME` for release paths.

```bash
RELEASE_VERSION=$(node -p "require('./package.json').version")
RELEASE_TAG="v${RELEASE_VERSION}"
RELEASE_COMMIT=$(git rev-parse origin/main)
PREVIOUS_RELEASE_TAG="v0.1.0"
```

Run the read-only repository and version checks:

```bash
test "$(git branch --show-current)" = "main"
test "$(git rev-parse HEAD)" = "$RELEASE_COMMIT"
test -z "$(git status --porcelain)"

CARGO_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' src-tauri/Cargo.toml | head -n 1)
TAURI_VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
test "$RELEASE_VERSION" = "$CARGO_VERSION"
test "$RELEASE_VERSION" = "$TAURI_VERSION"

test "$RELEASE_TAG" = "v0.1.1"
test "$(node -p "require('./src-tauri/tauri.conf.json').identifier")" = "io.github.sino-s.postmite"
test "$(node -p "require('./src-tauri/tauri.conf.json').bundle.targets.join(',')")" = "deb,appimage,msi,dmg"
```

First verify that the prior public release remains present and immutable. The
local and remote peeled tag targets must agree, and the GitHub Release must be
public rather than a draft or prerelease.

```bash
git show-ref --verify "refs/tags/$PREVIOUS_RELEASE_TAG"
PREVIOUS_RELEASE_COMMIT=$(git rev-parse "$PREVIOUS_RELEASE_TAG^{}")
test "$(git ls-remote origin "refs/tags/$PREVIOUS_RELEASE_TAG^{}" | cut -f1)" = "$PREVIOUS_RELEASE_COMMIT"
test "$(gh release view "$PREVIOUS_RELEASE_TAG" --json isDraft --jq .isDraft)" = "false"
test "$(gh release view "$PREVIOUS_RELEASE_TAG" --json isPrerelease --jq .isPrerelease)" = "false"
```

Then require the corrective tag and Release to be absent. A non-zero result is
expected for each command. Stop if any command finds an existing v0.1.1 tag or
Release; do not overwrite it.

```bash
! git show-ref --verify "refs/tags/$RELEASE_TAG"
! git ls-remote --exit-code --tags origin "refs/tags/$RELEASE_TAG"
! gh release view "$RELEASE_TAG"
```

Verify that all release-preparation Issues are closed and that publication
Issue #121 is open:

```bash
for ISSUE_NUMBER in 118 119 120 125 127; do
  test "$(gh issue view "$ISSUE_NUMBER" --json state --jq .state)" = "CLOSED"
done
test "$(gh issue view 121 --json state --jq .state)" = "OPEN"
```

## Verify the release commit

Run the source-controlled release gates from the clean checkout:

```bash
pnpm install --frozen-lockfile
pnpm release:verify
pnpm release:inspect-candidate
```

Find the `main` push workflow for the exact release commit:

```bash
MAIN_RUN_ID=$(gh run list \
  --workflow CI \
  --branch main \
  --event push \
  --commit "$RELEASE_COMMIT" \
  --limit 1 \
  --json databaseId \
  --jq '.[0].databaseId')
test -n "$MAIN_RUN_ID"
test "$(gh run view "$MAIN_RUN_ID" --json headSha --jq .headSha)" = "$RELEASE_COMMIT"
test "$(gh run view "$MAIN_RUN_ID" --json conclusion --jq .conclusion)" = "success"
gh run view "$MAIN_RUN_ID" --json jobs \
  --jq '.jobs[] | [.name, .status, .conclusion] | @tsv'
```

The exact-commit run must show `success` for all of these jobs:

- `Pull request quality gates`
- `Release Tauri build`
- `Release performance`
- `Ubuntu release artifacts`
- `Ubuntu release smoke`
- `Windows x64 release`
- `Apple Silicon macOS release`
- `Download and audit all release artifacts`

Stop if the run is absent, uses another commit, is incomplete, skips one of
these release jobs, or has any non-success conclusion.

Confirm the reviewed release notes state the supported target, checksum step,
manual-only update lookup, package identity, and publisher:

```bash
sed -n '1,240p' release/RELEASE_NOTES.md
sed -n '1,200p' release/TRADEMARK_GATE.md
```

## Create the immutable tag

The following commands create public state. Reconfirm publisher authority and
the exact commit before running them:

```bash
printf '%s\n' "$RELEASE_TAG" "$RELEASE_COMMIT"
git tag --annotate "$RELEASE_TAG" "$RELEASE_COMMIT" \
  --message "Postmite $RELEASE_VERSION"
test "$(git rev-parse "$RELEASE_TAG^{}")" = "$RELEASE_COMMIT"
git push origin "refs/tags/$RELEASE_TAG"
test "$(git ls-remote origin "refs/tags/$RELEASE_TAG^{}" | cut -f1)" = "$RELEASE_COMMIT"
```

Treat a pushed release tag as immutable. Never force-push, delete and recreate,
or move it to a different commit.

The tag push starts the release-oriented CI workflow. Find that tag run, wait
for it, and inspect every job:

```bash
TAG_RUN_ID=$(gh run list \
  --workflow CI \
  --event push \
  --commit "$RELEASE_COMMIT" \
  --limit 20 \
  --json databaseId,headBranch \
  --jq ".[] | select(.headBranch == \"$RELEASE_TAG\") | .databaseId" \
  | head -n 1)
test -n "$TAG_RUN_ID"
gh run watch "$TAG_RUN_ID" --exit-status
gh run view "$TAG_RUN_ID" --json jobs \
  --jq '.jobs[] | [.name, .status, .conclusion] | @tsv'
```

Require the same eight successful jobs listed for the `main` run. Do not use
artifacts from a pull request, another commit, another tag, or a local bundle.

## Audit v0.2.0 cross-platform CI artifacts

The v0.2.0 release pipeline is not published automatically. Before any future
publisher-specific publication procedure, download and verify all three
platform-specific CI artifacts from the exact successful run:

```bash
set -euo pipefail

CROSS_PLATFORM_STAGE=$(mktemp -d)
gh run download "$TAG_RUN_ID" --name postmite-ubuntu-x86_64 \
  --dir "$CROSS_PLATFORM_STAGE/linux-x86_64"
gh run download "$TAG_RUN_ID" --name postmite-windows-x86_64 \
  --dir "$CROSS_PLATFORM_STAGE/windows-x86_64"
gh run download "$TAG_RUN_ID" --name postmite-macos-aarch64 \
  --dir "$CROSS_PLATFORM_STAGE/macos-aarch64"

verify_cross_platform_target() {
  directory="$1"
  key="$2"
  platform="$3"
  platform_label="$4"
  architecture="$5"
  rust_target="$6"
  bundles="$7"
  package_extensions="$8"
  package_globs="$9"
  architecture_tokens="${10}"

  for evidence in SHA256SUMS DEPENDENCY_LICENSES.json THIRD_PARTY_NOTICES.md \
    RELEASE_NOTES.md RELEASE_CANDIDATE.json RELEASE_TARGET.json; do
    test -s "$directory/$evidence"
  done
  jq -e \
    '(.version | type == "string" and length > 0) and
     .productName == "Postmite" and
     .packageIdentifier == "io.github.sino-s.postmite" and
     .publisher == "sino-s" and
     .githubRelease == "not published automatically" and
     .nativeCapabilityBoundary == "main window: event listen/unlisten and clipboard text write only" and
     (.nativeCapabilities | type == "array" and length == 3)' \
    "$directory/RELEASE_CANDIDATE.json"
  for extension in $package_globs; do
    package_path=$(find "$directory" -maxdepth 1 -type f -name "*$extension" -print -quit)
    test -n "$package_path"
    test -s "$package_path"
    package_name="${package_path##*/}"
    package_matches=false
    for token in $architecture_tokens; do
      case "$package_name" in
        *"$token"*) package_matches=true ;;
      esac
    done
    test "$package_matches" = true
  done
  test "$(wc -l < "$directory/SHA256SUMS")" -eq "$(echo "$package_globs" | wc -w)"
  jq -e \
    --arg key "$key" \
    --arg platform "$platform" \
    --arg platform_label "$platform_label" \
    --arg architecture "$architecture" \
    --arg rust_target "$rust_target" \
    --argjson bundles "$bundles" \
    --argjson package_extensions "$package_extensions" \
    '.key == $key and .platform == $platform and
     .platformLabel == $platform_label and .architecture == $architecture and
     .rustTarget == $rust_target and .bundles == $bundles and
     .packageExtensions == $package_extensions' \
    "$directory/RELEASE_TARGET.json"
  jq -e \
    --arg key "$key" \
    --arg platform "$platform" \
    --arg platform_label "$platform_label" \
    --arg architecture "$architecture" \
    --arg rust_target "$rust_target" \
    --argjson bundles "$bundles" \
    --argjson package_extensions "$package_extensions" \
    '.artifactTarget.key == $key and .artifactTarget.platform == $platform and
     .artifactTarget.platformLabel == $platform_label and
     .artifactTarget.architecture == $architecture and
     .artifactTarget.rustTarget == $rust_target and
     .artifactTarget.bundles == $bundles and
     .artifactTarget.packageExtensions == $package_extensions' \
    "$directory/RELEASE_CANDIDATE.json"
  (cd "$directory" && sha256sum --check SHA256SUMS)
}

verify_cross_platform_target \
  "$CROSS_PLATFORM_STAGE/linux-x86_64" linux-x86_64 linux "Ubuntu 24.04" \
  x86_64 x86_64-unknown-linux-gnu '["deb","appimage"]' '[".deb",".AppImage"]' ".deb .AppImage" "amd64 x86_64"
verify_cross_platform_target \
  "$CROSS_PLATFORM_STAGE/windows-x86_64" windows-x86_64 windows Windows \
  x86_64 x86_64-pc-windows-msvc '["msi"]' '[".msi"]' ".msi" "x64 x86_64"
verify_cross_platform_target \
  "$CROSS_PLATFORM_STAGE/macos-aarch64" macos-aarch64 macos "Apple Silicon macOS" \
  aarch64 aarch64-apple-darwin '["dmg"]' '[".dmg"]' ".dmg" "arm64 aarch64"
test -s "$CROSS_PLATFORM_STAGE/linux-x86_64/APPIMAGE_BUDGET.json"
```

## Stage and verify the workflow artifacts

The v0.1.1 corrective publication below is intentionally Ubuntu-only. Download
its single platform artifact from the successful tag run into a new temporary
directory; the v0.2.0 cross-platform artifacts are audited above and are not
published by this procedure.

```bash
RELEASE_STAGE=$(mktemp -d)
gh run download "$TAG_RUN_ID" \
  --name postmite-ubuntu-x86_64 \
  --dir "$RELEASE_STAGE"
find "$RELEASE_STAGE" -maxdepth 1 -type f -printf '%f\n' | sort
```

Exactly these nine files must be present. The package filenames may contain
the version and architecture, but there must be exactly one `.deb` and one
`.AppImage`:

- `*.deb`
- `*.AppImage`
- `SHA256SUMS`
- `DEPENDENCY_LICENSES.json`
- `THIRD_PARTY_NOTICES.md`
- `APPIMAGE_BUDGET.json`
- `RELEASE_CANDIDATE.json`
- `RELEASE_TARGET.json`
- `RELEASE_NOTES.md`

Verify the downloaded evidence before creating a GitHub Release:

```bash
test "$(find "$RELEASE_STAGE" -maxdepth 1 -type f -name '*.deb' | wc -l)" -eq 1
test "$(find "$RELEASE_STAGE" -maxdepth 1 -type f -name '*.AppImage' | wc -l)" -eq 1
test "$(find "$RELEASE_STAGE" -maxdepth 1 -type f | wc -l)" -eq 9
for EVIDENCE_FILE in \
  SHA256SUMS \
  DEPENDENCY_LICENSES.json \
  THIRD_PARTY_NOTICES.md \
  APPIMAGE_BUDGET.json \
  RELEASE_CANDIDATE.json \
  RELEASE_TARGET.json \
  RELEASE_NOTES.md; do
  test -s "$RELEASE_STAGE/$EVIDENCE_FILE"
done
(cd "$RELEASE_STAGE" && sha256sum --check SHA256SUMS)
dpkg-deb --info "$RELEASE_STAGE"/*.deb
jq -e \
  '.version == "0.1.1" and
   .packageIdentifier == "io.github.sino-s.postmite" and
   .publisher == "sino-s" and
   .platforms == ["Ubuntu 24.04 x86_64", "Windows x86_64", "Apple Silicon macOS aarch64"] and
   .artifactTarget.key == "linux-x86_64"' \
  "$RELEASE_STAGE/RELEASE_CANDIDATE.json"
```

## Create and inspect the draft Release

Create a draft first. `--verify-tag` prevents `gh` from silently creating a tag
at another commit:

```bash
gh release create "$RELEASE_TAG" "$RELEASE_STAGE"/* \
  --verify-tag \
  --draft \
  --title "Postmite $RELEASE_VERSION" \
  --notes-file "$RELEASE_STAGE/RELEASE_NOTES.md"
gh release view "$RELEASE_TAG" \
  --json tagName,targetCommitish,isDraft,isPrerelease,assets,url
```

Require the tag name, `isDraft: true`, `isPrerelease: false`, and the same nine
non-empty assets. The peeled remote tag check above, rather than
`targetCommitish`, is the authority for the exact release commit. Review the
rendered notes and the unsigned Ubuntu-only boundary of the v0.1.1 corrective
publication in the GitHub web UI. The v0.2.0 Windows/macOS artifacts remain
unsigned CI artifacts here; their publication requires a separate authorized
procedure. A draft may be discarded by the publisher if it has never been
public, but its pushed tag remains immutable.

## Publish and verify public assets

Publishing is the final externally visible action. After the publisher approves
the draft, publish it as Latest:

```bash
gh release edit "$RELEASE_TAG" \
  --verify-tag \
  --draft=false \
  --prerelease=false \
  --latest
gh release view "$RELEASE_TAG" \
  --json tagName,targetCommitish,isDraft,isPrerelease,publishedAt,assets,url
```

Download the public assets again. Do not reuse `RELEASE_STAGE` for this check:

```bash
PUBLIC_STAGE=$(mktemp -d)
gh release download "$RELEASE_TAG" --dir "$PUBLIC_STAGE"
test "$(find "$PUBLIC_STAGE" -maxdepth 1 -type f | wc -l)" -eq 9
(cd "$PUBLIC_STAGE" && sha256sum --check SHA256SUMS)
test "$(git ls-remote origin "refs/tags/$RELEASE_TAG^{}" | cut -f1)" = "$RELEASE_COMMIT"
test "$(git ls-remote origin "refs/tags/$PREVIOUS_RELEASE_TAG^{}" | cut -f1)" = "$PREVIOUS_RELEASE_COMMIT"
test "$(gh release view "$PREVIOUS_RELEASE_TAG" --json isDraft --jq .isDraft)" = "false"
```

Record the release commit, tag, `MAIN_RUN_ID`, `TAG_RUN_ID`, Release URL,
package filenames, and SHA-256 results on Issue #121. Do not record Secret
values, local data paths containing personal information, or request headers.

## Clean Ubuntu acceptance

Use the public assets on a clean Ubuntu 24.04 x86_64 desktop VM or machine,
with a new desktop user and an unlocked Secret Service. Download the assets
with `gh release download`; do not copy files from the build machine.

1. Run `sha256sum --check SHA256SUMS` in the download directory.
2. Install the Debian package with `sudo apt-get install ./<package>.deb`, launch
   `/usr/bin/postmite`, and confirm the main window opens.
3. In a separate clean-user pass, make the AppImage executable with
   `chmod +x ./<package>.AppImage`, launch it, and confirm the main window opens.
4. Start an operator-controlled local HTTP fixture on `127.0.0.1`. From
   Postmite, send a GET request to it and confirm a `200` response and visible
   response body.
5. Create a workspace, collection, saved request, and non-Secret environment
   value. Exit normally, relaunch the same package, and confirm the selected
   workspace, collection, request, and environment value are restored.
6. With `POSTMITE_SESSION_ONLY_SECRETS` unset, create a throwaway Secret
   environment value. Exit normally and relaunch. Execute a request against a
   local fixture that reports only pass or fail and confirm the Secret resolves
   after restart. Never print, screenshot, log, or paste the value into Issue
   evidence.
7. Select **Check for updates**. Confirm the request is manual-only and that the
   latest GitHub Release resolves to the current `0.1.1` version without an
   update-check error.

Run the persistence and Secret checks for the Debian package. Run at least the
launch and request checks independently for the AppImage. Record pass/fail and
the clean-machine description on Issue #121.

## Failure, correction, and rollback

- Before pushing the tag: stop, open a focused implementation Issue, merge its
  reviewed fix, and restart from the new exact `main` commit.
- After pushing the tag but before publication: do not move the tag. Leave the
  failed tag unpublished, fix through a new Issue and pull request, increment
  the version, and publish a superseding tag such as `v0.1.2`.
- For a draft error: the authorized publisher may delete the never-public draft
  and recreate it for the same immutable tag only when the artifacts are
  byte-for-byte the outputs of the successful tag workflow. Otherwise use a
  new version.
- After publication: never silently replace assets, rewrite notes to hide a
  defect, delete and recreate the tag, or move the tag. Record the problem,
  publish a visible warning when users are at risk, fix through the Issue/PR
  workflow, and create a higher patch release.
- Postmite v0.1.1 has no automatic updater. A rollback means withdrawing the
  recommendation to install the affected release and publishing a corrected
  higher version; it does not mean mutating an installed client remotely.

## Close the release work

Only after every public-download and clean-Ubuntu check passes:

1. Add the complete, redacted evidence comment to Issue #121.
2. Check all child Issues and acceptance criteria in Epic #9.
3. Close Issue #121 as completed.
4. Close Epic #9 as completed.
5. Close Milestone v0.1.0.

The corresponding state-changing commands are:

```bash
gh issue close 121 --reason completed
gh issue close 9 --reason completed
gh api repos/sino-s/postmite/milestones/1 \
  --method PATCH \
  --raw-field state=closed
```

Finally, verify both GitHub Releases remain public, both Issues are closed, the
Milestone is closed with no open Issues, v0.1.0 still resolves to
`PREVIOUS_RELEASE_COMMIT`, v0.1.1 resolves to `RELEASE_COMMIT`, and local
`main` is clean and equal to `origin/main`.
