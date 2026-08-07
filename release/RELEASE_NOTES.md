# Postmite 0.3.0

v0.3.0 supersedes v0.2.0 without moving its tag, replacing its Release notes, or changing any published asset. It keeps the local-first workspace and Environment model while making the selected display language durable across application launches and changing future macOS distribution to source-build only.

## Changes since v0.2.0

- The selected English or Japanese display language is restored after Postmite exits and starts again.
- Public Releases now provide Ubuntu 24.04 x86_64 and Windows x64 packages only.
- Apple Silicon macOS remains supported as a local source build. Clone the repository, install the pinned toolchain and dependencies, and run `pnpm tauri`; `pnpm release:bundle:macos` remains available for an unsigned local DMG.
- The already-published v0.2.0 macOS DMG remains immutable historical release evidence and is not included in v0.3.0.

## Downloads

- `Postmite_0.3.0_amd64.deb` and `Postmite_0.3.0_amd64.AppImage` target Ubuntu 24.04 x86_64.
- `Postmite_0.3.0_x64_en-US.msi` targets Windows x64.
- Verify Ubuntu with `sha256sum --check linux-x86_64-SHA256SUMS` or Windows with `sha256sum --check windows-x86_64-SHA256SUMS` before installation or execution.
- `SHA256SUMS` is included as a complete checksum for all v0.3.0 packages.

These preview packages are unsigned; package signing is not included in v0.3.0. There is no public v0.3.0 macOS package.

## Platform and Secret boundaries

Linux uses the Secret Service when it is available. Protected values on Windows and Apple Silicon macOS are session-only and remain memory-only until separate native Credential Manager and Keychain security Issues are completed.

Secret values are not written to SQLite, release metadata, logs, diagnostics, snapshots, fixtures, screenshots, IPC errors, or uploaded artifacts.

## Update checks

Postmite does not poll for updates. A release lookup is sent only after the user selects **Check for updates**. Automatic updates are not included in this preview.

## Package identity and publisher

Postmite uses the `io.github.sino-s.postmite` package identifier. The publisher is `sino-s`, the owner of the `sino-s/postmite` source repository. `release/TRADEMARK_GATE.md` records the project-name decision; it does not assert a registered trademark.
