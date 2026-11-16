import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';

/** Keep byte-range semantics when replacing the native transport in browser tests. */
export async function serveMedia(page: Page, url: string, file: string) {
  const bytes = readFileSync(file);
  await page.route(url, route => {
    const range = route.request().headers()['range'];
    let start = 0, end = bytes.length - 1;
    if (range) {
      const match = /^bytes=(\d*)-(\d*)$/.exec(range);
      if (!match || (!match[1] && !match[2])) return route.fulfill({ status: 416, headers: { 'Content-Range': `bytes */${bytes.length}` }, body: '' });
      start = match[1] ? Number(match[1]) : Math.max(0, bytes.length - Number(match[2]));
      end = match[1] && match[2] ? Math.min(Number(match[2]), bytes.length - 1) : bytes.length - 1;
      if (start > end || start >= bytes.length) return route.fulfill({ status: 416, headers: { 'Content-Range': `bytes */${bytes.length}` }, body: '' });
    }
    return route.fulfill({
      status: range ? 206 : 200,
      contentType: 'video/mp4',
      headers: { 'Accept-Ranges': 'bytes', 'Content-Length': String(end - start + 1), ...(range ? { 'Content-Range': `bytes ${start}-${end}/${bytes.length}` } : {}) },
      body: bytes.subarray(start, end + 1),
    });
  });
}
