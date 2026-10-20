import { test, expect } from '@playwright/test';
const snapshot = (page: any) => page.evaluate(() => (window as any).traceglass.snapshot());
async function open(page: any) {
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-ready', 'true', { timeout: 45000 });
  await expect(page.locator('body > canvas')).toBeVisible();
}

test('real WASM boot, million-event search, filters and row inspection', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await open(page);
  expect((await snapshot(page)).total).toBeGreaterThanOrEqual(100000);
  await page.getByRole('button', { name: 'Text view', exact: true }).click();
  await page.getByRole('button', { name: '1m demo', exact: true }).click();
  await expect.poll(async () => (await snapshot(page)).total).toBe(1000000);
  await page.getByLabel('Search messages, services and levels').fill('timeout');
  await expect.poll(async () => (await snapshot(page)).query_text).toBe('timeout');
  const searched = await snapshot(page);
  expect(searched.matched).toBeGreaterThan(0);
  expect(searched.matched).toBeLessThan(1000000);
  expect(searched.first_rows.every((row: any) => row.message.toLowerCase().includes('timeout'))).toBeTruthy();
  await page.getByRole('combobox', { name: 'Level', exact: true }).selectOption('error');
  await expect.poll(async () => (await snapshot(page)).level.toLowerCase()).toBe('error');
  expect((await snapshot(page)).first_rows.every((row: any) => row.level.toLowerCase() === 'error')).toBeTruthy();
  await page.locator('.event-row').first().click();
  await expect(page.locator('#text-detail')).toContainText('timeout');
  await page.getByLabel('Search messages, services and levels').fill('no-such-event-xyz');
  await expect(page.locator('#text-rows')).toContainText('No matching events');
  await page.getByRole('button', { name: 'Reset filters' }).click();
  await expect.poll(async () => (await snapshot(page)).matched).toBe(1000000);
  expect(errors).toEqual([]);
});

test('local Japanese JSONL import is searchable and invalid import preserves data', async ({ page }) => {
  await open(page);
  const requests: string[] = [];
  page.on('request', request => { if (request.method() !== 'GET') requests.push(request.url()); });
  await page.getByRole('button', { name: 'Text view', exact: true }).click();
  await page.locator('#jsonl-file').setInputFiles('web/sample.jsonl');
  await expect.poll(async () => (await snapshot(page)).total).toBe(5);
  await page.getByLabel('Search messages, services and levels').fill('根拠');
  await expect.poll(async () => (await snapshot(page)).matched).toBe(1);
  await expect(page.locator('.event-row')).toContainText('根拠の取得に失敗しました');
  await page.locator('.event-row').click();
  await expect(page.locator('#text-detail')).toContainText('SOURCE_TIMEOUT');
  await page.locator('#jsonl-file').setInputFiles({ name:'invalid.jsonl', mimeType:'application/x-ndjson', buffer:Buffer.from('not json') });
  await expect(page.locator('#toast')).toContainText(/line|JSON|invalid/i);
  expect((await snapshot(page)).total).toBe(5);
  expect(requests).toEqual([]);
});

test('compact viewport keeps accessible search and local controls usable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page);
  await page.getByRole('button', { name: 'Text view', exact: true }).click();
  await page.getByLabel('Search messages, services and levels').fill('retry');
  await expect.poll(async () => (await snapshot(page)).query_text).toBe('retry');
  await expect(page.locator('.event-row').first()).toBeVisible();
  await page.screenshot({ path:'test-results/mobile-text.png' });
  await page.getByRole('button', { name:'Close text view' }).click();
  await expect(page.locator('#text-view')).toBeHidden();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.screenshot({ path:'test-results/mobile-canvas.png' });
});
