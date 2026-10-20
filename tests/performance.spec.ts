import { test, expect } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';

test('record actual browser WASM query measurements at one million events', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-ready', 'true', { timeout: 45000 });
  await page.getByRole('button', { name: 'Text view', exact: true }).click();
  await page.getByRole('button', { name: '1m demo', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).traceglass.snapshot().total)).toBe(1000000);
  await page.getByRole('button', { name: 'Close text view' }).click();
  const samples: any[] = [];
  for (let round = 0; round < 6; round++) {
    for (const query of ['', 'timeout', 'retry', 'gateway', 'error', 'evidence']) {
      const sample = await page.evaluate(query => {
        const app = (window as any).traceglass;
        app.command('search', query);
        const state = app.snapshot();
        return { query, query_ms: state.query_ms, matched: state.matched };
      }, query);
      if (round > 0) samples.push(sample);
    }
  }
  const values = samples.map(s => s.query_ms).sort((a, b) => a - b);
  const runtime = await page.evaluate(() => ({ userAgent:navigator.userAgent, wasm_memory_bytes:(window as any).traceglass.memoryBytes(), startup_ms:(window as any).traceglass.startupMs }));
  const report = {
    tested_at: new Date().toISOString(), url:page.url(), ...runtime,
    dataset: '1,000,000 deterministic compact simulated events',
    metric: 'Rust filtering + aggregation only; excludes input delivery, rendering and first-load costs',
    samples_count:samples.length, median_ms:values[Math.floor(values.length/2)],
    p95_ms:values[Math.ceil(values.length*0.95)-1], samples,
  };
  mkdirSync('validation', { recursive:true });
  writeFileSync(process.env.TRACEGLASS_REPORT || 'validation/browser-local.json', JSON.stringify(report,null,2));
  expect(samples.every(s => Number.isFinite(s.query_ms) && s.matched > 0)).toBeTruthy();
  await page.evaluate(() => (window as any).traceglass.command('reset'));
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.screenshot({path:process.env.TRACEGLASS_SCREENSHOT || 'test-results/traceglass-preview.png'});
});
