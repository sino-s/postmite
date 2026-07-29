# UI Screenshot Evidence

Run this command before opening a pull request that changes visible UI:

```sh
pnpm screenshots:ui
```

The command starts the Vite app in `VITE_POSTMITE_SCREENSHOTS=1` mode and writes PNG files under:

```text
artifacts/screenshots/ui/
```

Attach the generated PNG paths to the pull request `Results` section. The fixture data uses public example hosts and intentionally omits Secret values, token strings, credential fields, and cookie values. Do not add real endpoints, cookies, Authorization values, tokens, or credentials to screenshot fixtures.

If Chromium is not installed for Playwright, run:

```sh
pnpm exec playwright install chromium
```
