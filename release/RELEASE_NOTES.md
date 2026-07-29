# Postmite 0.1.0 preview

## Ubuntu 24.04 x86_64

- `postmite_0.1.0_amd64.deb` installs the preview package.
- `Postmite_0.1.0_amd64.AppImage` runs without installation after it is made executable.
- Verify the SHA-256 checksum before installing or running either artifact.
- The AppImage contains Ubuntu's WebKit runtime closure under `usr/lib`; `APPIMAGE_BUDGET.json` records that excluded runtime size and the compressed Postmite payload used for the 30-MiB budget.

## Update checks

Postmite does not poll for updates. A release lookup is sent only after the user selects **Check for updates**. Automatic updates are not included in this preview.

## Package identity

This preview uses the temporary `dev.postmite.preview` package identifier. It is not the final public release identifier.
