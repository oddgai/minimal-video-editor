import { test } from 'node:test';
import assert from 'node:assert/strict';
import { audioTiming, audioCorrection } from '../src/player.ts';

test('positive offset postpones audio and waits silently before its start', () => {
  assert.deepEqual(audioTiming(0.1, 0.25, 6, true), { time: 0, play: false });
  assert.deepEqual(audioTiming(1, 0.25, 6, true), { time: 0.75, play: true });
});

test('negative offset advances audio and stops beyond the audio duration', () => {
  assert.deepEqual(audioTiming(1, -0.25, 6, true), { time: 1.25, play: true });
  assert.deepEqual(audioTiming(5.9, -0.25, 6, true), { time: 6, play: false });
});

test('paused video never starts audio and nonfinite metadata is safe', () => {
  assert.deepEqual(audioTiming(1, 0.1, 6, false), { time: 0.9, play: false });
  assert.deepEqual(audioTiming(0, 0, NaN, true), { time: 0, play: false });
});

test('small audio drift is corrected by playback rate without repeated seeks', () => {
  const result = audioCorrection(1, 0.9, false);
  assert.equal(result.seek, false); assert.ok(result.rate > 1 && result.rate <= 1.1);
  assert.equal(audioCorrection(1, 1.1, false).seek, false);
  assert.ok(audioCorrection(1, 1.1, false).rate < 1);
  assert.deepEqual(audioCorrection(1, 1.01, false), { seek: false, rate: 1 });
});

test('explicit seeks and large discontinuities reposition audio once', () => {
  assert.deepEqual(audioCorrection(10, 0, true), { seek: true, rate: 1 });
  assert.deepEqual(audioCorrection(10, 0, false), { seek: false, rate: 1.1 });
  assert.deepEqual(audioCorrection(1, 0.9, true), { seek: true, rate: 1 });
  assert.deepEqual(audioCorrection(1, 1, true), { seek: false, rate: 1 });
});
