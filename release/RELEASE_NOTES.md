# Postmite 0.2.0

v0.2.0 is a cross-platform preview release that supersedes v0.1.1 without moving its tag or replacing its published assets. It keeps the local-first workspace and Environment model while adding downloadable Windows x64 and Apple Silicon macOS packages to the verified Ubuntu release path.

## Downloads

- `Postmite_0.2.0_amd64.deb` and `Postmite_0.2.0_amd64.AppImage` target Ubuntu 24.04 x86_64.
- `Postmite_0.2.0_x64_en-US.msi` targets Windows x64.
- `Postmite_0.2.0_aarch64.dmg` targets Apple Silicon macOS.
- Verify Ubuntu with `sha256sum --check linux-x86_64-SHA256SUMS`, Windows with `sha256sum --check windows-x86_64-SHA256SUMS`, or macOS with `sha256sum --check macos-aarch64-SHA256SUMS` before installation or execution.
- `SHA256SUMS` is also included as a complete all-package checksum for release auditing.

These cross-platform preview packages are unsigned; package signing is not included in v0.2.0.

## Platform and Secret boundaries

Linux uses the Secret Service when it is available. Protected values on Windows and macOS are session-only and remain memory-only until separate native Credential Manager and Keychain security Issues are completed.

Secret values are not written to SQLite, release metadata, logs, diagnostics, snapshots, fixtures, screenshots, IPC errors, or uploaded artifacts.

## Update checks

Postmite does not poll for updates. A release lookup is sent only after the user selects **Check for updates**. Automatic updates are not included in this preview.

## Package identity and publisher

Postmite uses the `io.github.sino-s.postmite` package identifier. The publisher is `sino-s`, the owner of the `sino-s/postmite` source repository. `release/TRADEMARK_GATE.md` records the project-name decision; it does not assert a registered trademark.
