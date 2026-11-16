import { test, expect, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { serveMedia } from './media-route.ts';

const command = process.platform === 'darwin' ? 'Meta' : 'Control';

test.beforeAll(() => {
  mkdirSync('test-results', { recursive: true });
  execFileSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=24', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000', '-t', '12', '-c:v', 'libx264', '-preset', 'ultrafast', '-g', '48', '-bf', '0', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-y', 'test-results/chapter-playback.mp4']);
});

async function openEditor(page: Page, realMedia = false) {
  await page.route(/\/src\/main\.ts(?:\?.*)?$/, async route => {
    const response = await route.fetch();
    await route.fulfill({ response, body: `import '/tests/e2e/native-mock.ts';\n${await response.text()}` });
  });
  if (realMedia) await serveMedia(page, '**/test-video.mp4', 'test-results/chapter-playback.mp4');
  else await page.route('**/test-video.mp4', route => route.fulfill({ status: 404, body: '' }));
  await page.goto('/'); await page.locator('#empty-open').click();
  await expect(page.locator('#export')).toBeEnabled();
  await page.locator('#workspace').focus();
}

async function splitAtFourAndEight(page: Page) {
  await page.keyboard.press('ArrowRight'); await page.keyboard.press('ArrowRight'); await page.keyboard.press('s');
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(2);
  await page.keyboard.press('ArrowRight'); await page.keyboard.press('ArrowRight'); await page.keyboard.press('s');
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(3);
}

test('empty screen, shortcuts dialog, and browser-only error are usable', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 850 });
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Choose file', exact: true })).toBeVisible();
  expect(await page.locator('#workspace').innerText()).toBe('Choose file');
  await expect(page.locator('#editor-controls')).not.toBeVisible();
  await page.screenshot({ path: 'test-results/empty.png', fullPage: true });
  await expect(page.locator('#export')).toBeDisabled();
  await page.keyboard.press('?'); await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.locator('#shortcut-list')).toContainText('Split chapter');
  await expect(page.locator('#shortcut-list')).not.toContainText('1 frame');
  await page.keyboard.press('Escape'); await page.locator('#empty-open').click();
  await expect(page.getByRole('status')).toContainText('desktop app');
});

test('file details are available on demand and audio offset stays usable in the compact controls', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 850 });
  await openEditor(page, true);
  await page.getByRole('button', { name: 'File info', exact: true }).click();
  const details = page.getByRole('dialog', { name: 'File info', exact: true });
  await expect(details).toBeVisible();
  await expect(details.locator('#filename')).toHaveText('旅行 sample.mp4');
  await expect(details.locator('#meta-resolution')).toHaveText('1920 × 1080');
  await expect(details.locator('#meta-video')).toHaveText('H264');
  await page.screenshot({ path: 'test-results/minimal-file-info.png', fullPage: true });
  await page.keyboard.press('Escape');
  await expect(details).not.toBeVisible();
  await page.getByRole('button', { name: 'Delay audio by 1 second' }).click();
  await expect(page.locator('#offset')).toHaveValue('1');
  await page.locator('#offset').fill('-0.25'); await page.locator('#workspace').focus();
  await expect(page.locator('#offset')).toHaveValue('-0.25');
  await page.getByRole('button', { name: 'Reset audio offset' }).click();
  await expect(page.locator('#offset')).toHaveValue('0');
  await page.screenshot({ path: 'test-results/minimal-loaded.png', fullPage: true });
  await page.setViewportSize({ width: 900, height: 680 });
  await page.screenshot({ path: 'test-results/minimal-compact.png', fullPage: true });
});

test('chapter and seekbar clicks snap to the nearest retained keyframe and preserve playback', async ({ page }) => {
  await openEditor(page, true);
  await splitAtFourAndEight(page);
  await page.keyboard.press(`${command}+ArrowLeft`); await page.keyboard.press('d');
  const box = (await page.locator('#timeline').boundingBox())!;
  const click = (time: number, chapter: boolean) => page.mouse.click(box.x + box.width * time / 8, box.y + (chapter ? 65 : 35));
  await click(6.3, true);
  await expect(page.locator('#current-time')).toHaveText('00:00:06.000');
  await expect(page.locator('#jump-button')).toHaveAttribute('title', /Source 00:00:10\.000/);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '1');
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '0'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await click(6.3, false);
  await expect(page.locator('#current-time')).toHaveText('00:00:06.000');
  await click(3.8, true);
  await expect(page.locator('#jump-button')).toHaveAttribute('title', /Source 00:00:08\.000/);
  const paused = () => page.locator('#video').evaluate(v => (v as HTMLVideoElement).paused);
  await expect.poll(paused).toBe(true);
  await page.locator('#play').click();
  await click(1.8, true);
  await expect.poll(paused).toBe(false);
  await expect.poll(() => page.locator('#video').evaluate(v => { const time = (v as HTMLVideoElement).currentTime; return time >= 2 && time < 3.5; })).toBe(true);
  await page.mouse.click(box.x + box.width - 2, box.y + 35);
  await expect.poll(paused).toBe(false);
  await expect.poll(() => page.locator('#video').evaluate(v => { const time = (v as HTMLVideoElement).currentTime; return time >= 10 && time < 11.5; })).toBe(true);
  await page.locator('#play').click();
});

test('chapters reorder by keyboard and drag, export in that order, and play backwards in source time', async ({ page }) => {
  await openEditor(page, true); await splitAtFourAndEight(page);
  await page.keyboard.press(`${command}+Shift+ArrowLeft`); await page.keyboard.press(`${command}+Shift+ArrowLeft`);
  await expect(page.locator('.chapter-block').first()).toHaveAttribute('title', /Source 00:00:08\.000/);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '0');
  await page.keyboard.press('ArrowRight'); await expect(page.locator('#jump-button')).toHaveAttribute('title', /Source 00:00:10\.000/);
  await page.keyboard.press('ArrowRight'); await expect(page.locator('#current-time')).toHaveText('00:00:04.000');
  await expect(page.locator('#jump-button')).toHaveAttribute('title', /Source 00:00:00\.000/);
  await page.keyboard.press(`${command}+s`);
  await page.getByRole('button', { name: 'OK', exact: true }).click();
  expect(await page.evaluate(() => (window as unknown as { lastExport: unknown }).lastExport)).toMatchObject({ request: { ranges: [{ start: 8, end: 12 }, { start: 0, end: 8 }] } });
  await page.locator('#dismiss-notice').click();
  const first = (await page.locator('.chapter-block').first().boundingBox())!;
  const lane = (await page.locator('#chapter-lane').boundingBox())!;
  await page.mouse.move(first.x + first.width / 2, first.y + first.height / 2); await page.mouse.down();
  await page.mouse.move(lane.x + lane.width - 8, first.y + first.height / 2, { steps: 8 }); await page.mouse.up();
  await expect(page.locator('.chapter-block').first()).toHaveAttribute('title', /Source 00:00:00\.000/);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '2');
  await page.locator('#workspace').focus(); await page.keyboard.press(`${command}+z`);
  await expect(page.locator('.chapter-block').first()).toHaveAttribute('title', /Source 00:00:08\.000/);
  await page.keyboard.press(`${command}+Shift+z`); await expect(page.locator('.chapter-block').first()).toHaveAttribute('title', /Source 00:00:00\.000/);
  await page.keyboard.press(`${command}+z`);
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '3.7'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await page.locator('#play').click();
  await expect.poll(() => page.locator('#video').evaluate(v => !((v as HTMLVideoElement).paused) && (v as HTMLVideoElement).currentTime < 1)).toBe(true);
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '11.7'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await expect.poll(() => page.locator('#video').evaluate(v => { const video = v as HTMLVideoElement; return video.paused && video.currentTime > 7.9 && video.currentTime < 8; })).toBe(true);
  await expect(page.locator('#current-time')).toHaveText('00:00:12.000');
  await page.locator('#jump-button').click(); await expect(page.locator('#jump-time')).toHaveValue('00:00:12.000'); await page.keyboard.press('Escape');
  await page.locator('#workspace').focus(); await page.keyboard.press('ArrowRight');
  await expect(page.locator('#current-time')).toHaveText('00:00:12.000');
  await page.keyboard.press('ArrowLeft');
  await expect(page.locator('#jump-button')).toHaveAttribute('title', /Source 00:00:06\.000/);
  await expect(page.locator('#current-time')).toHaveText('00:00:10.000');
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '12'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await page.locator('#play').click();
  await expect.poll(() => page.locator('#video').evaluate(v => !((v as HTMLVideoElement).paused) && (v as HTMLVideoElement).currentTime >= 8 && (v as HTMLVideoElement).currentTime < 9)).toBe(true);
});

test('fade detection applies one undoable split without restoring deleted or reordered footage and can cancel', async ({ page }) => {
  await openEditor(page);
  await splitAtFourAndEight(page); await page.keyboard.press(`${command}+ArrowLeft`); await page.keyboard.press('d');
  await page.keyboard.press(`${command}+Shift+ArrowLeft`);
  await page.evaluate(() => Object.assign(window, { holdDetection: true }));
  await page.locator('#offset').fill('-0.25');
  await page.evaluate(() => (window as unknown as { editorCommand: (command: string) => Promise<void> }).editorCommand('detect'));
  const dialog = page.getByRole('dialog', { name: /Detect|Detection/ });
  await expect(dialog).toBeVisible(); await expect(dialog.locator('progress')).toHaveJSProperty('value', 0.5);
  await page.keyboard.press('Escape'); await page.keyboard.press(`${command}+w`); await page.keyboard.press(`${command}+q`);
  await expect(dialog).toBeVisible(); await expect(page.locator('.chapter-block')).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Cancel' }).click(); await expect(dialog).not.toBeVisible();
  await expect(page.locator('#offset')).toHaveValue('-0.25');
  await expect(page.locator('.chapter-block')).toHaveCount(2);
  await page.evaluate(() => Object.assign(window, { holdDetection: false }));
  await page.getByRole('button', { name: 'Detect chapters', exact: true }).click();
  await expect(dialog).toContainText('Added 2 boundaries');
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.locator('.chapter-block')).toHaveCount(4);
  await expect(page.locator('.chapter-block').first()).toHaveAttribute('title', /Source 00:00:08\.000/);
  await expect(page.locator('#timeline-duration')).toHaveText('00:00:08.000');
  await page.locator('#workspace').focus(); await page.keyboard.press(`${command}+z`);
  await expect(page.locator('.chapter-block')).toHaveCount(2);
  await page.evaluate(() => Object.assign(window, { detectionResult: [] }));
  await page.getByRole('button', { name: 'Detect chapters', exact: true }).click();
  await expect(dialog).toContainText('No new boundaries found');
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.locator('.chapter-block')).toHaveCount(2);
});

test('split/select/delete shortcuts compact chapters, preserve history, and export with command S', async ({ page }) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await openEditor(page, true);
  for (const id of ['in-point', 'out-point', 'mark-in', 'mark-out', 'prev-frame', 'next-frame']) await expect(page.locator(`#${id}`)).toHaveCount(0);
  await splitAtFourAndEight(page);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '2');
  await page.keyboard.press(`${command}+ArrowLeft`);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '1');
  await page.keyboard.press('d');
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(2);
  await expect(page.locator('#timeline-duration')).toHaveText('00:00:08.000');
  await expect(page.locator('#current-time')).toHaveText('00:00:04.000');
  await expect(page.locator('#jump-button')).toHaveAttribute('title', /00:00:08\.000/);
  const layout = await page.locator('#chapter-lane .chapter-block').evaluateAll(nodes => nodes.map(node => ({ left: (node as HTMLElement).style.left, width: (node as HTMLElement).style.width })));
  expect(layout).toEqual([{ left: '0%', width: '50%' }, { left: '50%', width: '50%' }]);
  await page.keyboard.press('ArrowLeft'); await expect(page.locator('#jump-button')).toHaveAttribute('title', /00:00:02\.000/);
  await page.keyboard.press('ArrowRight'); await expect(page.locator('#jump-button')).toHaveAttribute('title', /00:00:08\.000/);
  await page.keyboard.press(`${command}+z`); await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(3);
  await page.keyboard.press(`${command}+Shift+z`); await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(2);
  await page.locator('#chapter-lane .chapter-block').first().click();
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '0');
  await page.keyboard.press(`${command}+ArrowRight`);
  await expect(page.locator('.chapter-block[aria-pressed="true"]')).toHaveAttribute('data-index', '1');
  await page.keyboard.press('s'); await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(2);
  await page.locator('#offset').fill('0.25'); await page.keyboard.press('d');
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(2);
  await page.keyboard.press(`${command}+e`);
  expect(await page.evaluate(() => (window as unknown as { lastExport?: unknown }).lastExport)).toBeUndefined();
  await page.keyboard.press(`${command}+s`);
  await page.getByRole('dialog', { name: /Export/ }).getByRole('button', { name: 'OK', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Exported');
  const payload = await page.evaluate(() => (window as unknown as { lastExport: unknown }).lastExport);
  expect(payload).toMatchObject({ request: { input: '/tmp/旅行 sample.mp4', ranges: [{ start: 0, end: 4 }, { start: 8, end: 12 }], audioOffset: 0.25 } });
  await page.screenshot({ path: 'test-results/chapters.png', fullPage: true });
  expect(errors).toEqual([]);
});

test('deleting every chapter disables editing/export and undo restores it', async ({ page }) => {
  await openEditor(page); await page.keyboard.press('d');
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(0);
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(0);
  for (const id of ['play', 'scrubber', 'export']) await expect(page.locator(`#${id}`)).toBeDisabled();
  await page.keyboard.press(`${command}+z`); await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(1);
  await expect(page.locator('#export')).toBeEnabled();
  await page.locator('#shortcuts').focus(); await page.keyboard.press('Space');
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape'); await page.locator('#shortcuts').focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog')).toBeVisible();
});

test('export dialog tracks its job, stays visible until completion, and recovers from cancellation and errors', async ({ page }) => {
  await openEditor(page);
  await splitAtFourAndEight(page);
  await page.evaluate(() => Object.assign(window, { holdExport: true }));
  const dialog = page.getByRole('dialog', { name: /Export/ });
  await page.keyboard.press(`${command}+s`);
  await expect(dialog).toBeVisible();
  await expect(page.locator('#export-output')).toHaveText('/tmp/edited.mp4');
  await page.evaluate(async () => {
    const w = window as unknown as { lastExport: { jobId: string }; exportProgress: (id: string, fraction: number, stage: string) => Promise<void> };
    await w.exportProgress(w.lastExport.jobId, 0.42, 'Copying chapters');
    await w.exportProgress('old-job', 0.99, 'Stale event');
  });
  await expect(dialog.locator('#export-percent')).toHaveText('42%');
  await expect(dialog.locator('progress')).toHaveJSProperty('value', 0.42);
  await expect(dialog.locator('#export-stage')).toHaveText('Copying chapters');
  await page.keyboard.press('Escape');
  await page.mouse.click(5, 5);
  await expect(dialog).toBeVisible();
  await page.keyboard.press('d');
  await page.keyboard.press(`${command}+w`); await page.keyboard.press(`${command}+q`);
  await page.evaluate(async () => {
    const w = window as unknown as { dropMedia: () => Promise<void>; editorCommand: (command: string) => Promise<void> };
    await w.dropMedia(); await w.editorCommand('close'); await w.editorCommand('quit');
  });
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(3);
  expect(await page.evaluate(() => (window as unknown as { quitRequested?: boolean }).quitRequested)).not.toBe(true);
  await page.evaluate(() => (window as unknown as { finishExport: () => void }).finishExport());
  await expect(dialog).toBeVisible();
  await expect(dialog.locator('progress')).toHaveJSProperty('value', 1);
  await expect(dialog.getByRole('button', { name: 'Cancel' })).not.toBeVisible();
  await page.keyboard.press(`${command}+w`); await page.keyboard.press(`${command}+q`);
  await page.evaluate(async () => {
    const w = window as unknown as { dropMedia: () => Promise<void>; editorCommand: (command: string) => Promise<void> };
    await w.dropMedia(); await w.editorCommand('close'); await w.editorCommand('quit');
  });
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(3);
  expect(await page.evaluate(() => (window as unknown as { quitRequested?: boolean }).quitRequested)).not.toBe(true);
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator('#export')).toBeEnabled();
  await expect(page.getByRole('status')).toContainText('Exported');
  await page.keyboard.press(`${command}+s`);
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('status')).toContainText('cancelled');
  await page.keyboard.press(`${command}+s`);
  await expect(dialog).toBeVisible();
  await page.evaluate(() => (window as unknown as { finishExport: (error: string) => void }).finishExport('Cannot save file'));
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('status')).toContainText('Cannot save file');
  await expect(page.locator('#export')).toBeEnabled();
});

test('command W unloads media and edits, permits reopening, and command Q requests app exit', async ({ page }) => {
  await openEditor(page, true);
  await splitAtFourAndEight(page);
  await page.locator('#offset').fill('1.25'); await page.locator('#workspace').focus();
  await page.keyboard.press(`${command}+w`);
  await expect(page.locator('#empty-preview')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Choose file', exact: true })).toBeVisible();
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(0);
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(0);
  await expect(page.locator('#video')).not.toHaveAttribute('src');
  await expect(page.locator('#audio')).not.toHaveAttribute('src');
  await expect(page.locator('#offset')).toHaveValue('0');
  for (const id of ['export', 'play']) await expect(page.locator(`#${id}`)).toBeDisabled();
  await page.locator('#empty-open').click();
  await expect(page.locator('#chapter-lane .chapter-block')).toHaveCount(1);
  await expect(page.locator('#export')).toBeEnabled();
  await page.keyboard.press(`${command}+q`);
  await expect.poll(() => page.evaluate(() => (window as unknown as { quitRequested: boolean }).quitRequested)).toBe(true);
});

test('real preview skips deleted frames, stops at the retained end, and restarts from the edited beginning', async ({ page }) => {
  await openEditor(page, true);
  await expect.poll(() => page.locator('#video').evaluate(v => (v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  await splitAtFourAndEight(page);
  await page.keyboard.press(`${command}+ArrowLeft`); await page.keyboard.press('d');
  await page.evaluate(() => {
    const video = document.getElementById('video') as HTMLVideoElement;
    const frames: number[] = [];
    const frameLog: { frame: number; paused: boolean; seeking: boolean; clock: number }[] = [];
    const collect: VideoFrameRequestCallback = (_now, frame) => { frames.push(frame.mediaTime); frameLog.push({ frame: frame.mediaTime, paused: video.paused, seeking: video.seeking, clock: video.currentTime }); video.requestVideoFrameCallback(collect); };
    video.requestVideoFrameCallback(collect); Object.assign(window, { chapterFrames: frames, chapterFrameLog: frameLog });
    const scrub = document.getElementById('scrubber') as HTMLInputElement; scrub.value = '3.7'; scrub.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.locator('#play').click();
  await expect.poll(() => page.locator('#video').evaluate(v => (v as HTMLVideoElement).currentTime)).toBeGreaterThan(8.2);
  await page.locator('#play').click();
  const frames = await page.evaluate(() => (window as unknown as { chapterFrames: number[] }).chapterFrames);
  expect(frames.some(t => t >= 8)).toBe(true); expect(frames.filter(t => t >= 4 && t < 8), JSON.stringify(await page.evaluate(() => (window as unknown as { chapterFrameLog: unknown }).chapterFrameLog))).toEqual([]);
  await page.locator('#chapter-lane .chapter-block').last().click(); await page.keyboard.press('d');
  await expect(page.locator('#timeline-duration')).toHaveText('00:00:04.000');
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '3.7'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await page.locator('#play').click();
  await expect.poll(() => page.locator('#video').evaluate(v => { const video = v as HTMLVideoElement; return video.paused && video.currentTime > 3.9 && video.currentTime < 4; })).toBe(true);
  await expect(page.locator('#current-time')).toHaveText('00:00:04.000');
  await page.locator('#play').click();
  await expect.poll(() => page.locator('#video').evaluate(v => { const video = v as HTMLVideoElement; return !video.paused && video.currentTime < 1; })).toBe(true);
});

test('navigation preserves playing or paused state and seeking to the edited end stops playback', async ({ page }) => {
  await openEditor(page, true);
  await expect.poll(() => page.locator('#video').evaluate(v => (v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  await splitAtFourAndEight(page);
  const paused = () => page.locator('#video').evaluate(v => (v as HTMLVideoElement).paused);
  const time = () => page.locator('#video').evaluate(v => (v as HTMLVideoElement).currentTime);
  const scrub = (value: string) => page.locator('#scrubber').evaluate((input, value) => { (input as HTMLInputElement).value = value; input.dispatchEvent(new Event('input', { bubbles: true })); }, value);
  await scrub('0.5'); await expect.poll(paused).toBe(true);
  await page.locator('#play').click(); await page.locator('#workspace').focus();
  await page.keyboard.press('ArrowRight'); await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThanOrEqual(2);
  await page.locator('#next-key').click(); await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThanOrEqual(4);
  await page.locator('#prev-key').click(); await expect.poll(paused).toBe(false);
  await page.locator('#workspace').focus(); await page.keyboard.press('ArrowLeft'); await expect.poll(paused).toBe(false);
  await scrub('2.5'); await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThanOrEqual(2.5);
  await page.locator('.chapter-block').nth(1).click(); await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThanOrEqual(4);
  await page.keyboard.press(`${command}+ArrowRight`); await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThanOrEqual(8);
  await page.locator('#chapter-lane .chapter-block').first().click(); await expect.poll(paused).toBe(false); await expect.poll(time).toBeLessThan(4);
  await page.locator('#jump-button').click(); await page.locator('#jump-time').fill('1'); await page.locator('#jump-time').press('Enter');
  await expect.poll(paused).toBe(false); await expect.poll(time).toBeGreaterThan(1.1);
  await page.locator('#play').click(); await expect.poll(paused).toBe(true);
  await page.locator('#workspace').focus(); await page.keyboard.press('ArrowRight'); await expect.poll(paused).toBe(true);
  await scrub('3'); await expect.poll(paused).toBe(true);
  await page.locator('.chapter-block').nth(1).click(); await expect.poll(paused).toBe(true);
  await page.locator('#jump-button').click(); await page.locator('#jump-time').fill('0.5'); await page.locator('#jump-time').press('Enter'); await expect.poll(paused).toBe(true);
  await page.locator('#play').click(); await scrub('12');
  await expect.poll(paused).toBe(true); await expect(page.locator('#current-time')).toHaveText('00:00:12.000');
});

test('repeated backward keyframe navigation progresses backward during playback', async ({ page }) => {
  await openEditor(page, true);
  await expect.poll(() => page.locator('#video').evaluate(v => (v as HTMLVideoElement).readyState)).toBeGreaterThanOrEqual(2);
  const time = () => page.locator('#video').evaluate(v => (v as HTMLVideoElement).currentTime);
  const paused = () => page.locator('#video').evaluate(v => (v as HTMLVideoElement).paused);
  const displayedTime = async () => {
    const [h, m, s] = (await page.locator('#current-time').textContent())!.split(':').map(Number);
    return h * 3600 + m * 60 + s;
  };
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '6'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await page.locator('#play').click(); await page.locator('#workspace').focus();
  await expect.poll(displayedTime).toBeGreaterThan(6.2);
  await page.keyboard.press('ArrowRight'); await expect.poll(displayedTime).toBeGreaterThan(8.2);
  await page.keyboard.press('ArrowLeft'); await expect.poll(time).toBeLessThan(7); await expect.poll(paused).toBe(false);
  await expect.poll(displayedTime).toBeGreaterThan(6.2);
  await page.locator('#prev-key').click(); await expect.poll(time).toBeLessThan(5); await expect.poll(paused).toBe(false);
  await expect.poll(displayedTime).toBeGreaterThan(4.2);
  await page.locator('#workspace').focus(); await page.keyboard.press('ArrowLeft'); await expect.poll(time).toBeLessThan(3);
  await expect.poll(displayedTime).toBeGreaterThan(2.2);
  await page.keyboard.press('ArrowLeft'); await expect.poll(time).toBeLessThan(1); await expect.poll(paused).toBe(false);
  // Once normal playback has crossed the following keyframe, use its current
  // position again rather than keeping an old navigation target forever.
  await expect.poll(displayedTime).toBeGreaterThan(2.2);
  await page.keyboard.press('ArrowLeft'); await expect.poll(time).toBeLessThan(3); await expect.poll(time).toBeGreaterThanOrEqual(2);
  await page.locator('#play').click();
  await page.locator('#scrubber').evaluate(input => { (input as HTMLInputElement).value = '6'; input.dispatchEvent(new Event('input', { bubbles: true })); });
  await page.locator('#workspace').focus(); await page.keyboard.press('ArrowLeft'); await expect.poll(time).toBe(4);
  await page.locator('#prev-key').click(); await expect.poll(time).toBe(2); await expect.poll(paused).toBe(true);
});
