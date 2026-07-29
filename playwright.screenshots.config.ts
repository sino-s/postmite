import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/ui-screenshots",
  testMatch: /.*\.playwright\.ts/,
  outputDir: "artifacts/screenshots/playwright-results",
  reporter: [["list"]],
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:1420",
    colorScheme: "light",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "pnpm dev",
    env: {
      VITE_POSTMITE_SCREENSHOTS: "1",
    },
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:1420",
  },
  projects: [
    {
      name: "desktop-light",
      use: {
        viewport: { width: 1440, height: 1100 },
      },
    },
    {
      name: "desktop-dark-compact",
      use: {
        colorScheme: "dark",
        viewport: { width: 1440, height: 1100 },
      },
    },
    {
      name: "narrow-light",
      use: {
        ...devices["Pixel 7"],
        colorScheme: "light",
        viewport: { width: 390, height: 1200 },
      },
    },
  ],
});
