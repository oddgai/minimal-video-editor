import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { serveMedia } from './media-route.ts';

test.beforeAll(() => {
  mkdirSync('test-results', { recursive: true });
  execFileSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=30', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000', '-t', '12', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-y', 'test-results/playback.mp4']);
});

test('zero offset plays sound through the video element without a second decoder', async ({ page }) => {
  await serveMedia(page, '**/playback-fixture.mp4', 'test-results/playback.mp4');
  await page.goto('/');
  const state = await page.evaluate(async () => {
    const modulePath = '/src/player.ts'; const { Player } = await import(modulePath);
    const video = document.createElement('video'); const audio = document.createElement('audio'); document.body.append(video, audio);
    const player = new Player(video, audio, () => {}); player.load('/playback-fixture.mp4', true);
    await new Promise(resolve => video.addEventListener('loadeddata', resolve, { once: true }));
    await player.toggle(); await new Promise(resolve => setTimeout(resolve, 2000));
    const result = { muted: video.muted, audioSource: audio.getAttribute('src'), videoTime: video.currentTime, frames: video.getVideoPlaybackQuality().totalVideoFrames };
    player.pause(); return result;
  });
  expect(state.muted).toBe(false); expect(state.audioSource).toBeNull(); expect(state.videoTime).toBeGreaterThan(0.5); expect(state.frames).toBeGreaterThan(5);
});

test('offset preview advances audio smoothly and reset returns to native sound', async ({ page }) => {
  await serveMedia(page, '**/playback-fixture.mp4', 'test-results/playback.mp4');
  await page.goto('/');
  const state = await page.evaluate(async () => {
    // An obscured native WebView can stop rAF while media continues playing.
    // Keep real decoders and clocks, but withhold rendering callbacks.
    window.requestAnimationFrame = () => 0;
    const modulePath = '/src/player.ts'; const { Player } = await import(modulePath);
    const video = document.createElement('video'); const audio = document.createElement('audio'); document.body.append(video, audio);
    let playingEvents = 0;
    video.addEventListener('playing', event => {
      // WKWebView does not emit another playing event after an in-buffer seek.
      if (playingEvents++ > 0) event.stopImmediatePropagation();
    });
    const player = new Player(video, audio, () => {}); player.load('/playback-fixture.mp4', true);
    await new Promise(resolve => video.addEventListener('loadeddata', resolve, { once: true }));
    let seeks = 0; audio.addEventListener('seeking', () => seeks++);
    player.setOffset(-0.25);
    await new Promise(resolve => audio.addEventListener('loadeddata', resolve, { once: true }));
    await player.toggle(); await new Promise(resolve => setTimeout(resolve, 3000));
    const videoTime = video.currentTime, audioTime = audio.currentTime;
    const drift = Math.abs(videoTime + 0.25 - audioTime);
    player.setOffset(0);
    const result = { drift, seeks, mutedAfterReset: video.muted, audioSourceAfterReset: audio.getAttribute('src'), videoTime, audioTime };
    player.pause(); return result;
  });
  expect(state.drift, JSON.stringify(state)).toBeLessThan(0.15); expect(state.seeks).toBeLessThanOrEqual(3);
  expect(state.mutedAfterReset).toBe(false); expect(state.audioSourceAfterReset).toBeNull();
});

test('offset replay aligns to the selected position even if play arrives while the decoder is buffering', async ({ page }) => {
  await serveMedia(page, '**/playback-fixture.mp4', 'test-results/playback.mp4');
  await page.goto('/');
  const states = await page.evaluate(async () => {
    const modulePath = '/src/player.ts'; const { Player } = await import(modulePath);
    const states = [];
    for (const offset of [-1, 1]) {
      const video = document.createElement('video'), audio = document.createElement('audio'); document.body.append(video, audio);
      let buffering = false;
      const readyState = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, 'readyState')!.get!;
      // WKWebView can deliver play/seeked at HAVE_CURRENT_DATA, before the
      // decoder is ready to advance. Keep actual media, seeks and play promises.
      Object.defineProperty(video, 'readyState', { get: () => buffering ? 2 : readyState.call(video) });
      const player = new Player(video, audio, () => {}); player.load('/playback-fixture.mp4', true);
      await new Promise(resolve => video.addEventListener('loadeddata', resolve, { once: true }));
      player.setOffset(offset);
      await new Promise(resolve => audio.addEventListener('loadeddata', resolve, { once: true }));
      player.seek(3, 12);
      await new Promise(resolve => video.addEventListener('seeked', resolve, { once: true }));
      await player.toggle(); await new Promise(resolve => setTimeout(resolve, 500)); player.pause();
      buffering = true; player.seek(8, 12);
      await new Promise(resolve => video.addEventListener('seeked', resolve, { once: true }));
      await player.toggle(); buffering = false;
      await new Promise(resolve => setTimeout(resolve, 900));
      states.push({ offset, videoTime: video.currentTime, audioTime: audio.currentTime, audioPaused: audio.paused, drift: Math.abs(video.currentTime - offset - audio.currentTime) });
      player.pause(); video.remove(); audio.remove();
    }
    return states;
  });
  for (const state of states) {
    expect(state.videoTime, JSON.stringify(state)).toBeGreaterThanOrEqual(8);
    expect(state.audioTime, JSON.stringify(state)).toBeGreaterThanOrEqual(8 - state.offset);
    expect(state.audioPaused, JSON.stringify(state)).toBe(false);
    expect(state.drift, JSON.stringify(state)).toBeLessThan(0.15);
  }
});
