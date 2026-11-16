import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Editor, formatTime, parseTime } from '../src/editor.ts';

const editor = () => new Editor(12, [0, 2, 4, 6, 8, 10]);

test('a source starts as one selected chapter and splits at a keyframe', () => {
  const e = editor();
  assert.equal(e.selectedChapterIndex, 0);
  assert.equal(e.split(4), true);
  assert.deepEqual(e.chapters, [{ start: 0, end: 4 }, { start: 4, end: 12 }]);
  assert.equal(e.selectedChapterIndex, 1); assert.equal(e.outputDuration, 12);
});

test('source ends, chapter boundaries, non-keyframes and invalid times never split', () => {
  const e = editor();
  for (const t of [0, 12, 3.1, -2, NaN, Infinity]) assert.equal(e.split(t), false);
  assert.equal(e.canUndo, false); e.split(4); assert.equal(e.split(4), false);
  assert.equal(e.chapters.length, 2);
});

test('deleting a middle chapter compacts the timeline but preserves source export coordinates', () => {
  const e = editor(); e.split(4); e.split(8); e.selectChapter(1); e.deleteChapter();
  assert.deepEqual(e.ranges, [{ start: 0, end: 4 }, { start: 8, end: 12 }]);
  assert.equal(e.outputDuration, 8); assert.equal(e.selectedChapterIndex, 1);
  assert.deepEqual(e.timeline.map(c => [c.timelineStart, c.timelineEnd]), [[0, 4], [4, 8]]);
  assert.equal(e.split(6), false);
});

test('deleting first and last chapters selects a surviving neighbor', () => {
  const e = editor(); e.split(4); e.split(8);
  e.selectChapter(0); e.deleteChapter(); assert.equal(e.selectedChapterIndex, 0); assert.equal(e.chapters[0].start, 4);
  e.selectChapter(1); e.deleteChapter(); assert.equal(e.selectedChapterIndex, 0); assert.deepEqual(e.chapters, [{ start: 4, end: 8 }]);
});

test('all chapters can be deleted and restored with selection by undo', () => {
  const e = editor(); e.deleteChapter();
  assert.deepEqual(e.chapters, []); assert.equal(e.selectedChapterIndex, -1); assert.equal(e.outputDuration, 0);
  assert.equal(e.timelineToSource(0), null); assert.equal(e.playableSourceTime(0), null); assert.equal(e.split(4), false);
  e.deleteChapter(); e.undo(); assert.deepEqual(e.chapters, [{ start: 0, end: 12 }]); assert.equal(e.selectedChapterIndex, 0);
});

test('undo/redo restores splits, deletion, offset and selection without merging adjacent chapters', () => {
  const e = editor(); e.split(4); e.setOffset(0.25); e.deleteChapter();
  e.undo(); assert.equal(e.chapters.length, 2); assert.equal(e.selectedChapterIndex, 1);
  e.undo(); assert.equal(e.audioOffset, 0); e.undo(); assert.equal(e.chapters.length, 1); assert.equal(e.selectedChapterIndex, 0);
  e.redo(); assert.equal(e.chapters.length, 2); assert.equal(e.selectedChapterIndex, 1);
  e.setOffset(-0.1); assert.equal(e.canRedo, false);
});

test('chapter navigation is bounded and selection is not an undoable edit', () => {
  const e = editor(); e.selectChapter(3); assert.equal(e.selectedChapterIndex, 0); assert.equal(e.canUndo, false);
  e.split(4); e.split(8); e.selectChapter(0); e.moveChapter(-1); assert.equal(e.selectedChapterIndex, 0);
  e.moveChapter(1); assert.equal(e.selectedChapterIndex, 1); e.moveChapter(1); e.moveChapter(1); assert.equal(e.selectedChapterIndex, 2);
});

test('compact splice boundaries map to the next source chapter, including removed prefix and suffix', () => {
  const e = editor(); e.split(2); e.split(4); e.split(8); e.split(10);
  e.selectChapter(4); e.deleteChapter(); e.selectChapter(2); e.deleteChapter(); e.selectChapter(0); e.deleteChapter();
  assert.deepEqual(e.ranges, [{ start: 2, end: 4 }, { start: 8, end: 10 }]);
  assert.equal(e.timelineToSource(0), 2); assert.equal(e.timelineToSource(1), 3); assert.equal(e.timelineToSource(2), 8);
  assert.equal(e.timelineToSource(4), 10); assert.equal(e.timelineToSource(100), 10);
  assert.equal(e.sourceToTimeline(3), 1); assert.equal(e.sourceToTimeline(6), 2); assert.equal(e.sourceToTimeline(9), 3);
  assert.equal(e.playableSourceTime(4), 8); assert.equal(e.playableSourceTime(0), 2); assert.equal(e.playableSourceTime(10), null);
});

test('keyframe navigation and ticks omit deleted chapters and stay within retained keyframes', () => {
  const e = editor(); e.split(4); e.split(8); e.selectChapter(1); e.deleteChapter();
  assert.deepEqual(e.timelineKeyframes, [{ source: 0, time: 0 }, { source: 2, time: 2 }, { source: 8, time: 4 }, { source: 10, time: 6 }]);
  assert.equal(e.keyframe(2, 1), 8); assert.equal(e.keyframe(8, -1), 2);
  assert.equal(e.keyframe(10, 1), 10); assert.equal(e.keyframe(0, -1), 0); assert.equal(e.keyframe(8.000001, -1), 2);
});

test('nearest keyframe uses compact timeline distance and never selects deleted footage', () => {
  const e = editor();
  assert.equal(e.nearestKeyframe(3.6), 4);
  assert.equal(e.nearestKeyframe(1), 0);
  assert.equal(e.nearestKeyframe(-1), 0);
  assert.equal(e.nearestKeyframe(12), 10);
  e.split(4); e.split(8); e.selectChapter(1); e.deleteChapter();
  assert.equal(e.nearestKeyframe(3.6), 8);
  assert.equal(e.nearestKeyframe(5.6), 10);
  assert.equal(e.nearestKeyframe(NaN), null);
  e.selectChapter(0); e.deleteChapter(); e.deleteChapter();
  assert.equal(e.nearestKeyframe(0), null);
});

test('reordering chapters moves the selected edit with undo and uses edited keyframe order', () => {
  const e = editor(); e.split(4); e.split(8);
  assert.equal(e.reorderChapter(2, 0), true);
  assert.deepEqual(e.ranges, [{ start: 8, end: 12 }, { start: 0, end: 4 }, { start: 4, end: 8 }]);
  assert.equal(e.selectedChapterIndex, 0);
  assert.deepEqual(e.exportRanges, [{ start: 8, end: 12 }, { start: 0, end: 8 }]);
  assert.deepEqual(e.timelineKeyframes, [{ source: 8, time: 0 }, { source: 10, time: 2 }, { source: 0, time: 4 }, { source: 2, time: 6 }, { source: 4, time: 8 }, { source: 6, time: 10 }]);
  assert.equal(e.timelineToSource(4), 0); assert.equal(e.sourceToTimeline(1), 5); assert.equal(e.sourceToTimeline(5), 9);
  assert.equal(e.playableSourceTime(1), 1); assert.equal(e.playableSourceTime(9), 9);
  assert.equal(e.keyframe(10, 1), 0); assert.equal(e.keyframe(0, -1), 10); assert.equal(e.nearestKeyframe(5.6), 2);
  e.undo(); assert.deepEqual(e.ranges, [{ start: 0, end: 4 }, { start: 4, end: 8 }, { start: 8, end: 12 }]);
  e.redo(); assert.equal(e.ranges[0].start, 8);
  for (const [from, to] of [[0, 0], [-1, 0], [0, 3], [NaN, 0]]) assert.equal(e.reorderChapter(from, to), false);
  e.undo(); assert.equal(e.ranges[0].start, 0);
});

test('automatic boundaries split retained reordered chapters as one undoable edit', () => {
  const e = editor(); e.split(4); e.split(8); e.selectChapter(1); e.deleteChapter(); e.reorderChapter(1, 0); e.setOffset(-1);
  assert.equal(e.splitAtKeyframes([0, 2, 3, 6, 10, 10, 12, NaN]), 2);
  assert.deepEqual(e.ranges, [{ start: 8, end: 10 }, { start: 10, end: 12 }, { start: 0, end: 2 }, { start: 2, end: 4 }]);
  assert.equal(e.audioOffset, -1); e.undo();
  assert.deepEqual(e.ranges, [{ start: 8, end: 12 }, { start: 0, end: 4 }]);
  assert.equal(e.audioOffset, -1);
});

test('keyframes are sanitized and nonfinite audio offsets cannot change history', () => {
  const e = new Editor(12, [4, NaN, 2, 4, -2, 30, 0]);
  assert.deepEqual(e.keyframes, [0, 2, 4]); e.setOffset(NaN); assert.equal(e.canUndo, false);
  e.setOffset(0.25); e.undo(); assert.equal(e.audioOffset, 0);
});

test('export coalesces adjacent chapters without changing chapter boundaries or undo history', () => {
  const e = editor(); e.split(4); e.split(8);
  assert.deepEqual(e.exportRanges, [{ start: 0, end: 12 }]);
  assert.equal(e.chapters.length, 3);
  e.selectChapter(1); e.deleteChapter();
  assert.deepEqual(e.exportRanges, [{ start: 0, end: 4 }, { start: 8, end: 12 }]);
  e.undo(); assert.equal(e.chapters.length, 3);
});

test('time fields round-trip hours and reject malformed/nonfinite values', () => {
  assert.equal(formatTime(3661.125), '01:01:01.125'); assert.equal(parseTime('01:01:01.125'), 3661.125);
  assert.equal(parseTime('1.25'), 1.25); assert.equal(parseTime('01:99:00'), null); assert.equal(parseTime('Infinity'), null);
  assert.equal(parseTime(''), null); assert.equal(formatTime(59.9999), '00:01:00.000');
});
