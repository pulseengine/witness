import { defineConfig } from "@playwright/test";

const REPORTS_DIR = process.env.WITNESS_VIZ_FIXTURE
  ?? "/tmp/v081-suite";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  timeout: 60_000,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  use: {
    baseURL: "http://localhost:3037",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: `../../crates/witness-viz/target/release/witness-viz --reports-dir ${REPORTS_DIR} --port 3037`,
    port: 3037,
    timeout: 60_000,
    reuseExistingServer: !process.env.CI,
    cwd: ".",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
