import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  use: { baseURL: "http://127.0.0.1:5191", browserName: "chromium", viewport: { width: 1440, height: 1000 } },
  webServer: {
    command: "npx vite --host 127.0.0.1 --port 5191",
    url: "http://127.0.0.1:5191",
    reuseExistingServer: false,
  },
});
