import { defineConfig, devices } from "@playwright/test";

// Runs the frontend against a real browser engine (Chromium), rather than
// jsdom (used by the Vitest unit tests), specifically so layout/sizing bugs
// - collapsed elements, mismatched overlay dimensions, horizontal overflow -
// are caught: jsdom never computes an actual box model, so it can't see
// those regressions at all, only DOM structure/attribute ones.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [["line"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
  },
});
