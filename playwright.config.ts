import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/layout",
  testMatch: "**/*.pw.ts",
  workers: 2,
  use: {
    baseURL: "http://127.0.0.1:1420",
    // Windows desktop uses WebView2; Edge exercises the same browser engine.
    channel: process.env.PLAYWRIGHT_CHANNEL || "msedge",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
  },
});
