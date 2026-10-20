import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  timeout: 60000,
  fullyParallel: false,
  workers: 1,
  reporter: [['list'], ['json', { outputFile: 'test-results/results.json' }]],
  use: {
    baseURL: process.env.TRACEGLASS_URL || 'http://127.0.0.1:8793',
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
    launchOptions: { args: ['--enable-unsafe-webgpu', '--enable-unsafe-swiftshader'] },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
});
