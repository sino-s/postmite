# Postmite 0.1.1

v0.1.1 is a corrective release that supersedes v0.1.0 without moving its tag
or replacing its published assets. It adds shipped UI flows to create, select,
rename, and delete local workspaces and to create, select, edit, order, and
delete Environment variables.

Plain Environment values persist in the local workspace database. Secret
values remain references to the Linux Secret Service, with an explicitly
reported session-only fallback when protected storage is unavailable. An
unavailable Secret reference is rejected before network execution and is never
silently sent as a placeholder.

## Ubuntu 24.04 x86_64

- `Postmite_0.1.1_amd64.deb` installs the package.
- `Postmite_0.1.1_amd64.AppImage` runs without installation after it is made executable.
- These Ubuntu preview packages are unsigned; package signing is not included in v0.1.1.
- Verify the SHA-256 checksum before installing or running either artifact.
- The AppImage contains Ubuntu's WebKit runtime closure under `usr/lib`; `APPIMAGE_BUDGET.json` records that excluded runtime size and the compressed Postmite payload used for the 30-MiB budget.

## Update checks

Postmite does not poll for updates. A release lookup is sent only after the user selects **Check for updates**. Automatic updates are not included in this preview.

## Package identity and publisher

Postmite uses the `io.github.sino-s.postmite` package identifier. The publisher is `sino-s`, the owner of the `sino-s/postmite` source repository. `release/TRADEMARK_GATE.md` records the project-name decision; it does not assert a registered trademark.

## Cross-platform v0.2.0 release pipeline

The v0.2.0 release pipeline covers Ubuntu x86_64 `.deb` and AppImage packages,
Windows x64 (`x86_64-pc-windows-msvc`) `.msi` packages, and Apple Silicon macOS
(`aarch64-apple-darwin`) `.dmg` packages. Each platform and architecture has a
separate artifact directory, target metadata record, SHA-256 checksum, dependency
license record, third-party notice, and release-candidate evidence.

Windows and macOS packages are unsigned in this release slice. Protected values
are session-only and remain memory-only on Windows and macOS until separate native
Credential Manager and Keychain security Issues are completed. No protected value
is written to SQLite, release metadata, logs, diagnostics, snapshots, fixtures,
screenshots, IPC errors, or uploaded artifacts.
