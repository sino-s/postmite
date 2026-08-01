import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/resize-performance",
  testMatch: /.*\.playwright\.ts/,
  outputDir: "artifacts/resize-performance/playwright-results",
  reporter: [["list"]],
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:1421",
    trace: "retain-on-failure",
    viewport: { width: 1600, height: 1080 },
  },
  webServer: {
    command: "pnpm build:web && pnpm exec vite preview --host 127.0.0.1 --port 1421 --strictPort",
    env: {
      VITE_POSTMITE_SCREENSHOTS: "1",
    },
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:1421",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
