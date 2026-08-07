# Publishing Postmite v0.3.0

End-user installation and initial-use instructions live in the [repository README](../README.md).

This procedure publishes the reviewed Postmite v0.3.0 preview.
It exposes Ubuntu 24.04 x86_64 `.deb` and AppImage packages plus a Windows x64
`.msi`. All packages are unsigned. Apple Silicon macOS is source-build only.
Protected values are session-only and memory-only on Windows and macOS.
Ubuntu publishes an unsigned Debian package and an unsigned AppImage. The
`POSTMITE_SESSION_ONLY_SECRETS` override is not used for the final Linux check.

The v0.2.0 tag, Release notes, and assets are historical and immutable. The
v0.3.0 Release publishes Ubuntu and Windows packages only; Apple Silicon macOS
is source-build only.

## Release artifact policy

For v0.3.0 and later releases:

- CI must not build, upload, audit, checksum, or publish a macOS DMG.
- Public release artifacts are limited to Ubuntu x86_64 and Windows x64.
- Apple Silicon users clone the repository, run `pnpm install --frozen-lockfile`,
  and start the app with `pnpm tauri` on their Mac.
- `pnpm release:bundle:macos` remains available for a local unsigned DMG when
  a local bundle is needed; it is not a public release artifact.
- The v0.2.0 tag, Release notes, and already-published assets are never deleted
  or replaced.

The public v0.2.0 tag, Release, notes, and assets are immutable. v0.3.0
supersedes that release and must be published as a new immutable tag; this
procedure never moves, deletes, or replaces v0.2.0.
In all publication stages, this procedure never moves, deletes, or replaces v0.2.0.

## Authority and stop rule

Only `sino-s`, or a release publisher explicitly delegated by `sino-s`, may
push a release tag or publish a GitHub Release. A second person or agent must
review the release-preparation pull request before publication starts.

Stop at any failed check. Do not skip a check, publish a workspace-built
package, move an existing tag, or repair the release outside the Issue and pull
request workflow. This procedure never moves, deletes, or replaces v0.2.0.

## Prepare the release commit

Start from a clean, current `main`:

```bash
git switch main
git fetch origin --prune --tags
git pull --ff-only origin main
```

Set release-specific variables:

```bash
RELEASE_VERSION=$(node -p "require('./package.json').version")
RELEASE_TAG="v$RELEASE_VERSION"
RELEASE_COMMIT=$(git rev-parse origin/main)
PREVIOUS_RELEASE_TAG="v0.2.0"
test "$(git branch --show-current)" = "main"
test "$(git rev-parse HEAD)" = "$RELEASE_COMMIT"
test -z "$(git status --porcelain)"
test "$RELEASE_TAG" = "v0.3.0"
```

Require matching package versions and the approved target bundle set:

```bash
CARGO_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' src-tauri/Cargo.toml | head -n 1)
TAURI_VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
test "$RELEASE_VERSION" = "$CARGO_VERSION"
test "$RELEASE_VERSION" = "$TAURI_VERSION"
test "$(node -p "require('./src-tauri/tauri.conf.json').identifier")" = "io.github.sino-s.postmite"
test "$(node -p "require('./src-tauri/tauri.conf.json').bundle.targets.join(',')")" = "deb,appimage,msi,dmg"
```

Verify the prior public release remains present and immutable, and the new tag
and Release are absent:

```bash
git show-ref --verify "refs/tags/$PREVIOUS_RELEASE_TAG"
PREVIOUS_RELEASE_COMMIT=$(git rev-parse "$PREVIOUS_RELEASE_TAG^{}")
test "$(git ls-remote origin "refs/tags/$PREVIOUS_RELEASE_TAG^{}" | cut -f1)" = "$PREVIOUS_RELEASE_COMMIT"
test "$(gh release view "$PREVIOUS_RELEASE_TAG" --json isDraft --jq .isDraft)" = "false"
test "$(gh release view "$PREVIOUS_RELEASE_TAG" --json isPrerelease --jq .isPrerelease)" = "false"
! git show-ref --verify "refs/tags/$RELEASE_TAG"
! git ls-remote --exit-code --tags origin "refs/tags/$RELEASE_TAG"
! gh release view "$RELEASE_TAG"
```

## Verify the release commit

Run the source-controlled release gates:

```bash
pnpm install --frozen-lockfile
pnpm test:release-procedure
pnpm test:readme
pnpm test -- scripts/release-targets.test.mjs scripts/release-candidate.node.mjs
pnpm typecheck
pnpm lint
pnpm release:verify
pnpm release:inspect-candidate
```

Find the `main` push workflow for the exact release commit and require every
release job to succeed:

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

The exact-commit run must show success for all of these jobs:

- `Pull request quality gates`
- `Release Tauri build`
- `Release performance`
- `Ubuntu release artifacts`
- `Ubuntu release smoke`
- `Windows x64 release`
- `Download and audit all release artifacts`

Confirm the reviewed release notes and project-name gate before tagging:

```bash
sed -n '1,240p' release/RELEASE_NOTES.md
sed -n '1,200p' release/TRADEMARK_GATE.md
```

The manual **Check for updates** behavior remains opt-in and is not changed by
publication.

## Create the immutable tag

Reconfirm the exact commit, then create and push the annotated tag:

```bash
printf '%s\n' "$RELEASE_TAG" "$RELEASE_COMMIT"
git tag --annotate "$RELEASE_TAG" "$RELEASE_COMMIT" --message "Postmite $RELEASE_VERSION"
test "$(git rev-parse "$RELEASE_TAG^{}")" = "$RELEASE_COMMIT"
git push origin "refs/tags/$RELEASE_TAG"
test "$(git ls-remote origin "refs/tags/$RELEASE_TAG^{}" | cut -f1)" = "$RELEASE_COMMIT"
```

Treat a pushed release tag as immutable. Never force-push, delete and recreate,
or move it to another commit.

The tag starts release CI. Wait for the exact tag run and inspect all jobs:

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

Require the same seven successful jobs listed above. Do not use artifacts from
a pull request, another commit, another tag, or a local bundle.

## Audit Linux and Windows CI artifacts

Download the Linux and Windows artifacts from the exact successful tag run and
verify every package, checksum, target, and redacted release-candidate record.

The already-published v0.2.0 macOS asset is historical and is not staged or
verified by future release procedures.

The audited evidence includes:

- `*.deb`
- `*.AppImage`
- `*.msi`
- `SHA256SUMS`
- `DEPENDENCY_LICENSES.json`
- `THIRD_PARTY_NOTICES.md`
- `APPIMAGE_BUDGET.json`
- `RELEASE_CANDIDATE.json`
- `RELEASE_TARGET.json`
- `RELEASE_NOTES.md`

```bash
set -euo pipefail
CROSS_PLATFORM_STAGE=$(mktemp -d)
gh run download "$TAG_RUN_ID" --name postmite-ubuntu-x86_64 --dir "$CROSS_PLATFORM_STAGE/linux-x86_64"
gh run download "$TAG_RUN_ID" --name postmite-windows-x86_64 --dir "$CROSS_PLATFORM_STAGE/windows-x86_64"

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
  if [ "$key" = "linux-x86_64" ]; then
    architecture_tokens="amd64 x86_64"
  else
    architecture_tokens="x64 x86_64"
  fi

  for evidence in SHA256SUMS DEPENDENCY_LICENSES.json THIRD_PARTY_NOTICES.md \
    RELEASE_NOTES.md RELEASE_CANDIDATE.json RELEASE_TARGET.json; do
    test -s "$directory/$evidence"
  done
  for extension in $package_globs; do
    package_path=$(find "$directory" -maxdepth 1 -type f -name "*$extension" -print -quit)
    test -n "$package_path"
    test -s "$package_path"
    package_name="$package_path"
    package_name="${package_name##*/}"
    package_matches=false
    for token in $architecture_tokens; do
      case "$package_name" in *"$token"*) package_matches=true ;; esac
    done
    test "$package_matches" = true
  done
  jq -e \
    '.version == "0.3.0" and .productName == "Postmite" and
     .packageIdentifier == "io.github.sino-s.postmite" and
     .publisher == "sino-s" and
     .githubRelease == "not published automatically" and
     (.nativeCapabilities | type == "array" and length == 3)' \
    "$directory/RELEASE_CANDIDATE.json"
  jq -e \
    --arg key "$key" --arg platform "$platform" \
    --arg platform_label "$platform_label" --arg architecture "$architecture" \
    --arg rust_target "$rust_target" --argjson bundles "$bundles" \
    --argjson package_extensions "$package_extensions" \
    '.key == $key and .platform == $platform and
     .platformLabel == $platform_label and .architecture == $architecture and
     .rustTarget == $rust_target and .bundles == $bundles and
     .packageExtensions == $package_extensions' \
    "$directory/RELEASE_TARGET.json"
  jq -e \
    --arg platform_label "$platform_label" \
    --argjson package_extensions "$package_extensions" \
    '.artifactTarget.platformLabel == $platform_label and
     .artifactTarget.packageExtensions == $package_extensions' \
    "$directory/RELEASE_CANDIDATE.json"
  (cd "$directory" && sha256sum --check SHA256SUMS)
}

verify_cross_platform_target "$CROSS_PLATFORM_STAGE/linux-x86_64" linux-x86_64 linux "Ubuntu 24.04" \
  x86_64 x86_64-unknown-linux-gnu '["deb","appimage"]' '[".deb",".AppImage"]' ".deb .AppImage"
verify_cross_platform_target "$CROSS_PLATFORM_STAGE/windows-x86_64" windows-x86_64 windows Windows \
  x86_64 x86_64-pc-windows-msvc '["msi"]' '[".msi"]' ".msi"
test -s "$CROSS_PLATFORM_STAGE/linux-x86_64/APPIMAGE_BUDGET.json"
```

## Stage and publish the public Release

GitHub Release assets share one flat namespace, so prefix evidence filenames by
target while keeping packages and the single release notes file at the root:

```bash
PUBLIC_STAGE=$(mktemp -d)
for target in linux-x86_64 windows-x86_64; do
  directory="$CROSS_PLATFORM_STAGE/$target"
  find "$directory" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.AppImage' -o -name '*.msi' \) \
    -exec cp {} "$PUBLIC_STAGE/" \;
  cp "$directory/SHA256SUMS" "$PUBLIC_STAGE/$target-SHA256SUMS"
  for evidence in DEPENDENCY_LICENSES.json THIRD_PARTY_NOTICES.md RELEASE_CANDIDATE.json RELEASE_TARGET.json; do
    cp "$directory/$evidence" "$PUBLIC_STAGE/$target-$evidence"
  done
done
cp "$CROSS_PLATFORM_STAGE/linux-x86_64/RELEASE_NOTES.md" "$PUBLIC_STAGE/RELEASE_NOTES.md"
cp "$CROSS_PLATFORM_STAGE/linux-x86_64/APPIMAGE_BUDGET.json" "$PUBLIC_STAGE/linux-x86_64-APPIMAGE_BUDGET.json"
(cd "$PUBLIC_STAGE" && sha256sum Postmite_*) > "$PUBLIC_STAGE/SHA256SUMS"
```

Create a draft first and inspect the assets. Only `sino-s` may publish it:

```bash
gh release create "$RELEASE_TAG" "$PUBLIC_STAGE"/* \
  --verify-tag --draft --title "Postmite $RELEASE_VERSION" \
  --notes-file "$PUBLIC_STAGE/RELEASE_NOTES.md"
gh release view "$RELEASE_TAG" \
  --json tagName,isDraft,isPrerelease,assets,url
```

Require the tag, draft state, notes, packages, and target-prefixed evidence to
match the audited stage. Publish as the Latest release only after inspection:

```bash
gh release edit "$RELEASE_TAG" --verify-tag --draft=false --prerelease=false --latest
gh release view "$RELEASE_TAG" \
  --json tagName,isDraft,isPrerelease,publishedAt,assets,url
```

## Verify public downloads

Download the public assets into a new directory. Do not reuse the staging area:

```bash
PUBLIC_DOWNLOAD=$(mktemp -d)
gh release download "$RELEASE_TAG" --dir "$PUBLIC_DOWNLOAD"
test -s "$PUBLIC_DOWNLOAD/Postmite_0.3.0_amd64.deb"
test -s "$PUBLIC_DOWNLOAD/Postmite_0.3.0_amd64.AppImage"
test -s "$PUBLIC_DOWNLOAD/Postmite_0.3.0_x64_en-US.msi"
(cd "$PUBLIC_DOWNLOAD" && sha256sum --check linux-x86_64-SHA256SUMS)
(cd "$PUBLIC_DOWNLOAD" && sha256sum --check windows-x86_64-SHA256SUMS)
(cd "$PUBLIC_DOWNLOAD" && sha256sum --check SHA256SUMS)
test "$(git ls-remote origin "refs/tags/$RELEASE_TAG^{}" | cut -f1)" = "$RELEASE_COMMIT"
test "$(gh release view "$RELEASE_TAG" --json isDraft --jq .isDraft)" = "false"
test "$(gh release view "$RELEASE_TAG" --json isPrerelease --jq .isPrerelease)" = "false"
```

Record the release commit, tag, main and tag run IDs, Release URL, asset names,
and checksum results on Issue #151. Never record Secret values, request headers,
or local paths containing personal information.

## Boundaries and rollback

Windows packages remain unsigned. The historical v0.2.0 macOS package was also unsigned;
native Credential Manager and Keychain persistence, signing, notarization,
automatic publishing, and automatic updates are not part of v0.3.0. Never silently replace public assets,
rewrite notes to hide a defect, delete and recreate the tag, or move the tag.
A correction requires a new Issue, PR, version, and immutable tag.
A failed release must never silently replace assets.

## Close the release work

Only after the public download and checksum checks pass:

1. Add the complete, redacted evidence comment to Issue #151.
2. Check the child Issue and acceptance criteria in Epic #150.
3. Close Issue #151 and Epic #150 as completed.
4. Close Milestone v0.3.0 only when it has no open Issues.
