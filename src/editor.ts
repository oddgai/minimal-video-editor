export interface Range { start: number; end: number }
export interface TimelineChapter extends Range { index: number; timelineStart: number; timelineEnd: number }
interface Snapshot { chapters: Range[]; selectedChapterIndex: number; audioOffset: number }
const EPSILON = 0.0001;

export class Editor {
  readonly duration: number;
  keyframes: number[] = [];
  chapters: Range[];
  selectedChapterIndex = 0;
  audioOffset = 0;
  private past: Snapshot[] = [];
  private future: Snapshot[] = [];

  constructor(duration: number, keyframes: number[]) {
    if (!Number.isFinite(duration) || duration <= 0) throw new Error('Cannot read the video duration.');
    this.duration = duration;
    this.chapters = [{ start: 0, end: duration }];
    this.setKeyframes(keyframes);
  }

  setKeyframes(values: number[]) {
    this.keyframes = [...new Set(values.filter(t => Number.isFinite(t) && t >= 0 && t < this.duration))].sort((a, b) => a - b);
  }

  get ranges(): Range[] { return this.chapters.map(c => ({ ...c })); }
  get exportRanges(): Range[] {
    const result: Range[] = [];
    for (const chapter of this.chapters) {
      const last = result[result.length - 1];
      if (last && Math.abs(last.end - chapter.start) < EPSILON) last.end = chapter.end;
      else result.push({ ...chapter });
    }
    return result;
  }
  get outputDuration() { return this.chapters.reduce((sum, c) => sum + c.end - c.start, 0); }
  get selectedChapter(): Range | null { return this.chapters[this.selectedChapterIndex] ?? null; }
  get canUndo() { return this.past.length > 0; }
  get canRedo() { return this.future.length > 0; }

  get timeline(): TimelineChapter[] {
    let time = 0;
    return this.chapters.map((c, index) => {
      const result = { ...c, index, timelineStart: time, timelineEnd: time + c.end - c.start };
      time = result.timelineEnd; return result;
    });
  }

  get timelineKeyframes(): { source: number; time: number }[] {
    const points: { source: number; time: number }[] = [];
    let time = 0;
    for (const c of this.chapters) {
      let cursor = lowerBound(this.keyframes, c.start - EPSILON);
      while (cursor < this.keyframes.length && this.keyframes[cursor] < c.end - EPSILON) {
        const source = this.keyframes[cursor++]; points.push({ source, time: time + source - c.start });
      }
      time += c.end - c.start;
    }
    return points;
  }

  canSplit(time: number): boolean { return this.splitPoint(time) !== null; }

  split(time: number): boolean {
    const point = this.splitPoint(time);
    if (!point) return false;
    this.record();
    const chapter = this.chapters[point.index];
    this.chapters.splice(point.index, 1, { start: chapter.start, end: point.time }, { start: point.time, end: chapter.end });
    this.selectedChapterIndex = point.index + 1;
    return true;
  }

  deleteChapter(): boolean {
    if (!this.selectedChapter) return false;
    this.record(); this.chapters.splice(this.selectedChapterIndex, 1);
    this.selectedChapterIndex = this.chapters.length ? Math.min(this.selectedChapterIndex, this.chapters.length - 1) : -1;
    return true;
  }

  selectChapter(index: number) {
    if (Number.isInteger(index) && index >= 0 && index < this.chapters.length) this.selectedChapterIndex = index;
  }

  moveChapter(direction: -1 | 1) {
    if (this.chapters.length) this.selectChapter(Math.max(0, Math.min(this.chapters.length - 1, this.selectedChapterIndex + direction)));
  }

  reorderChapter(from: number, to: number): boolean {
    if (!Number.isInteger(from) || !Number.isInteger(to) || from === to || from < 0 || to < 0 || from >= this.chapters.length || to >= this.chapters.length) return false;
    this.record();
    const [chapter] = this.chapters.splice(from, 1);
    this.chapters.splice(to, 0, chapter); this.selectedChapterIndex = to;
    return true;
  }

  splitAtKeyframes(values: number[]): number {
    const keys = [...new Set(values.flatMap(value => {
      if (!Number.isFinite(value)) return [];
      const key = this.keyframes[lowerBound(this.keyframes, value - EPSILON)];
      return key !== undefined && Math.abs(key - value) < EPSILON ? [key] : [];
    }))].sort((a, b) => a - b);
    const next: Range[] = []; let count = 0, selected = this.selectedChapterIndex;
    this.chapters.forEach((chapter, index) => {
      if (index === this.selectedChapterIndex) selected = next.length;
      let start = chapter.start;
      for (let i = lowerBound(keys, start + EPSILON); i < keys.length && keys[i] < chapter.end - EPSILON; i++) {
        next.push({ start, end: keys[i] }); start = keys[i]; count++;
      }
      next.push({ start, end: chapter.end });
    });
    if (count) { this.record(); this.chapters = next; this.selectedChapterIndex = selected; }
    return count;
  }

  timelineToSource(time: number): number | null {
    if (!Number.isFinite(time) || !this.chapters.length) return null;
    let remaining = Math.max(0, time);
    for (const c of this.chapters) {
      const length = c.end - c.start;
      if (remaining < length) return c.start + remaining;
      remaining -= length;
    }
    return this.chapters[this.chapters.length - 1].end;
  }

  sourceToTimeline(source: number): number {
    if (!Number.isFinite(source)) return 0;
    const timeline = this.timeline;
    const current = timeline.find(c => source >= c.start && source < c.end);
    if (current) return current.timelineStart + source - current.start;
    const next = timeline.filter(c => c.start > source).sort((a, b) => a.start - b.start)[0];
    return next?.timelineStart ?? this.outputDuration;
  }

  /** Source position that may play next; null means the edited video has ended. */
  playableSourceTime(source: number): number | null {
    if (!Number.isFinite(source)) return null;
    if (this.chapters.some(c => source >= c.start && source < c.end - EPSILON)) return source;
    return this.chapters.filter(c => c.start > source).sort((a, b) => a.start - b.start)[0]?.start ?? null;
  }

  keyframe(time: number, direction: -1 | 1): number {
    const points = this.timelineKeyframes;
    if (!points.length) return this.chapters[0]?.start ?? 0;
    const i = lowerBound(points.map(p => p.time), this.sourceToTimeline(time) + (direction === 1 ? EPSILON : -EPSILON));
    return (direction === 1 ? (points[i] ?? points[points.length - 1]) : (points[i - 1] ?? points[0])).source;
  }

  /** Closest retained keyframe to an edited timeline position, in source seconds. */
  nearestKeyframe(time: number): number | null {
    if (!Number.isFinite(time) || !this.chapters.length) return null;
    const points = this.timelineKeyframes;
    if (!points.length) return this.timelineToSource(time);
    const i = lowerBound(points.map(p => p.time), time);
    const before = points[Math.max(0, i - 1)], after = points[i];
    return !after || time - before.time <= after.time - time ? before.source : after.source;
  }

  setOffset(value: number) {
    if (!Number.isFinite(value) || Math.abs(value) > 3600) return;
    const rounded = Math.round(value * 1000) / 1000;
    if (rounded === this.audioOffset) return;
    this.record(); this.audioOffset = rounded;
  }

  undo() {
    const previous = this.past.pop(); if (!previous) return;
    this.future.push(this.snapshot()); this.restore(previous);
  }

  redo() {
    const next = this.future.pop(); if (!next) return;
    this.past.push(this.snapshot()); this.restore(next);
  }

  private splitPoint(time: number): { index: number; time: number } | null {
    if (!Number.isFinite(time)) return null;
    const key = this.keyframes[lowerBound(this.keyframes, time - EPSILON)];
    if (key === undefined || Math.abs(key - time) > EPSILON) return null;
    const index = this.chapters.findIndex(c => key > c.start + EPSILON && key < c.end - EPSILON);
    return index < 0 ? null : { index, time: key };
  }

  private snapshot(): Snapshot { return { chapters: this.ranges, selectedChapterIndex: this.selectedChapterIndex, audioOffset: this.audioOffset }; }
  private restore(s: Snapshot) { this.chapters = s.chapters; this.selectedChapterIndex = s.selectedChapterIndex; this.audioOffset = s.audioOffset; }
  private record() { this.past.push(this.snapshot()); if (this.past.length > 100) this.past.shift(); this.future = []; }
}

function lowerBound(values: number[], target: number) {
  let lo = 0, hi = values.length;
  while (lo < hi) { const mid = (lo + hi) >>> 1; if (values[mid] < target) lo = mid + 1; else hi = mid; }
  return lo;
}

export function formatTime(seconds: number): string {
  const ms = Math.max(0, Math.round((Number.isFinite(seconds) ? seconds : 0) * 1000));
  const hours = Math.floor(ms / 3600000), minutes = Math.floor(ms / 60000) % 60, secs = Math.floor(ms / 1000) % 60;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${String(ms % 1000).padStart(3, '0')}`;
}

export function parseTime(text: string): number | null {
  const value = text.trim();
  if (/^\d+(?:\.\d+)?$/.test(value)) { const n = Number(value); return Number.isFinite(n) ? n : null; }
  const match = /^(\d+):([0-5]\d):([0-5]\d(?:\.\d+)?)$/.exec(value);
  if (!match) return null;
  const n = Number(match[1]) * 3600 + Number(match[2]) * 60 + Number(match[3]);
  return Number.isFinite(n) ? n : null;
}
