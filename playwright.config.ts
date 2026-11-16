import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests/e2e',
  testMatch: '*.spec.ts',
  use: {
    baseURL: 'http://127.0.0.1:1420', viewport: { width: 1280, height: 900 }, trace: 'retain-on-failure',
    // Tauri's native WebView enables media autoplay; match that policy here.
    launchOptions: { args: ['--autoplay-policy=no-user-gesture-required'] },
  },
  webServer: { command: 'pnpm dev', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI },
});
