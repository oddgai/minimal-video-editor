import './styles.css';
import { Editor, formatTime, parseTime } from './editor.ts';
import { Player } from './player.ts';
import * as bridge from './bridge.ts';
import type { MediaInfo } from './bridge.ts';
import { icon } from './icons.ts';

const isMac = /Mac/.test(navigator.platform);
const mod = isMac ? '⌘' : 'Ctrl';
const app = document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML = `
  <main id="workspace" class="workspace" tabindex="-1" aria-label="Video editor">
    <div class="preview" id="dropzone">
      <video id="video" preload="metadata" playsinline muted aria-label="Video preview" hidden></video>
      <audio id="audio" preload="metadata"></audio>
      <div id="empty-preview" class="empty-preview"><button id="empty-open" class="button secondary">Choose file</button></div>
      <div id="preview-error" class="preview-message" hidden><strong>Preview unavailable</strong><p>This format is not supported by your system. You can still edit and export it.</p></div>
      <div id="empty-chapters-preview" class="preview-message" hidden><strong>No chapters</strong><p>Undo to restore your edit.</p></div>
      <div id="drop-overlay" class="drop-overlay" hidden>Drop to open</div>
    </div>
    <div id="editor-controls" class="editor-controls" hidden>
      <div class="timeline" id="timeline" role="group" aria-label="Chapter timeline">
        <div class="timeline-labels" id="timeline-labels"></div>
        <canvas id="keyframe-canvas" aria-hidden="true"></canvas>
        <div class="chapter-lane" id="chapter-lane"></div>
        <div id="chapter-drop-marker" class="chapter-drop-marker" hidden></div>
        <div id="playhead" class="playhead" hidden></div>
        <input id="scrubber" type="range" min="0" max="1" step="0.001" value="0" aria-label="Playback position in seconds" disabled />
      </div>
      <div class="transport">
        <div class="transport-position">
          <div class="playback-buttons">
            <button id="prev-key" class="icon-button" title="Previous keyframe (←)" aria-label="Previous keyframe" disabled>${icon('prev', 16)}</button>
            <button id="play" class="icon-button play-button" title="Play / pause (Space)" aria-label="Play" disabled>${icon('play', 18)}</button>
            <button id="next-key" class="icon-button" title="Next keyframe (→)" aria-label="Next keyframe" disabled>${icon('next', 16)}</button>
          </div>
          <button id="jump-button" class="timecode" aria-label="Go to time" disabled><span id="current-time">00:00:00.000</span><span class="separator">/</span><span id="timeline-duration">00:00:00.000</span></button>
          <span id="analysis-status" class="spinner" aria-label="Analyzing keyframes" hidden></span>
        </div>
        <div class="transport-actions">
          <div class="audio-control" role="group" aria-label="Audio offset">
            <label for="offset">Audio offset</label>
            <div class="offset-control">
              <button id="offset-minus" aria-label="Advance audio by 1 second" title="Advance audio by 1 second ([)" disabled>−</button>
              <input id="offset" type="number" step="any" min="-3600" max="3600" value="0" aria-label="Audio offset in seconds" aria-describedby="audio-note" disabled /><span>s</span>
              <button id="offset-plus" aria-label="Delay audio by 1 second" title="Delay audio by 1 second (])" disabled>+</button>
            </div>
            <button id="reset-offset" class="icon-button" aria-label="Reset audio offset" title="Reset audio offset" disabled>${icon('undo', 15)}</button>
            <span id="audio-note" class="sr-only">Positive values delay audio; negative values advance it.</span>
          </div>
          <div class="utility-buttons">
            <button id="detect-chapters" class="icon-button" aria-label="Detect chapters" title="Fast fade scan (${mod} Shift D); Shift-click for full scan" disabled>${icon('detect', 18)}</button>
            <button id="open" class="icon-button" aria-label="Open file" title="Open file (${mod} O)">${icon('folder', 17)}</button>
            <button id="file-info" class="icon-button" aria-label="File info" title="File info" disabled>${icon('info', 17)}</button>
            <button id="shortcuts" class="icon-button" aria-label="Keyboard shortcuts" title="Keyboard shortcuts (?)">${icon('keyboard', 18)}</button>
            <button id="export" class="button primary" title="Export (${mod} S)" disabled>${icon('export', 16)}Export</button>
          </div>
        </div>
      </div>
    </div>
    <button id="empty-shortcuts" class="icon-button empty-help" aria-label="Keyboard shortcuts" title="Keyboard shortcuts (?)">${icon('keyboard', 18)}</button>
  </main>
  <dialog id="file-info-dialog" aria-labelledby="file-info-title"><div class="dialog-heading"><h2 id="file-info-title">File info</h2><button id="close-file-info" class="icon-button" aria-label="Close file info">${icon('close')}</button></div><h3 id="filename"></h3><p id="filepath" class="file-path"></p><dl class="file-metadata"><div><dt>Video</dt><dd id="meta-video">—</dd></div><div><dt>Resolution</dt><dd id="meta-resolution">—</dd></div><div><dt>Frame rate</dt><dd id="meta-fps">—</dd></div><div><dt>Audio</dt><dd id="meta-audio">—</dd></div><div><dt>Duration</dt><dd id="meta-duration">—</dd></div><div><dt>Size</dt><dd id="meta-size">—</dd></div></dl></dialog>
  <dialog id="export-progress" class="export-progress" aria-labelledby="export-title" aria-describedby="export-stage export-output"><h2 id="export-title">Exporting</h2><p id="export-output" class="export-output"></p><div aria-live="polite"><strong id="export-stage">Preparing export</strong><span id="export-percent">0%</span></div><progress id="progress-bar" aria-label="Export progress" max="1" value="0"></progress><div class="dialog-actions"><button id="cancel-export" class="button secondary" autofocus>Cancel</button><button id="export-ok" class="button primary" hidden>OK</button></div></dialog>
  <dialog id="detection-progress" class="export-progress" aria-labelledby="detection-title" aria-describedby="detection-stage"><h2 id="detection-title">Detect chapters</h2><p id="detection-description" class="export-output">Find fades to black or white and split at the nearest keyframes.</p><div aria-live="polite"><strong id="detection-stage">Preparing detection</strong><span id="detection-percent">0%</span></div><progress id="detection-bar" aria-label="Chapter detection progress" max="1" value="0"></progress><div class="dialog-actions"><button id="cancel-detection" class="button secondary" autofocus>Cancel</button><button id="detection-ok" class="button primary" hidden>OK</button></div></dialog>
  <div id="notice" class="notice" role="status" aria-live="polite" hidden><span id="notice-text"></span><button id="dismiss-notice" class="icon-button" aria-label="Dismiss notification">${icon('close', 15)}</button></div>
  <dialog id="shortcuts-dialog" aria-labelledby="shortcuts-title"><div class="dialog-heading"><h2 id="shortcuts-title">Keyboard shortcuts</h2><button id="close-shortcuts" class="icon-button" aria-label="Close keyboard shortcuts">${icon('close')}</button></div><div id="shortcut-list" class="shortcut-list"></div><p class="hint">Editing shortcuts are disabled while typing in a field.</p></dialog>
  <dialog id="jump-dialog" aria-labelledby="jump-title"><form id="jump-form"><div class="dialog-heading"><h2 id="jump-title">Go to time</h2></div><label for="jump-time" class="muted">Edited time (hh:mm:ss.mmm or seconds)</label><input id="jump-time" class="dialog-input" placeholder="00:00:00.000" required /><div class="dialog-actions"><button type="button" id="cancel-jump" class="button secondary">Cancel</button><button class="button primary" type="submit">Go</button></div></form></dialog>
`;

function el<T extends HTMLElement = HTMLElement>(id: string) { return document.getElementById(id) as T; }
function disable(id: string, value: boolean) { (el(id) as HTMLButtonElement | HTMLInputElement).disabled = value; }
function text(id: string, value: string) { el(id).textContent = value; }
const video = el<HTMLVideoElement>('video');
const player = new Player(video, el<HTMLAudioElement>('audio'), () => notify('Audio preview is unavailable for this format. The offset will still be applied on export.', true));
let info: MediaInfo | null = null;
let editor: Editor | null = null;
let position = 0;
let keyframeTarget: number | null = null;
let atTimelineEnd = false;
let loading = false;
let analyzed = false;
let exporting = false;
let choosingOutput = false;
let previewFailed = false;
let generation = 0;
let activeJob = '';
let detecting = false;
let activeDetectionJob = '';
let detectionCancelRequested = false;
let boundaryTimer = 0;
let scrubberWasPlaying = false;
let playbackSpanIndex = 0;
let chapterDragging = false;
let cancelChapterDrag: (() => void) | null = null;

function operationDialogOpen() { return el<HTMLDialogElement>('export-progress').open || el<HTMLDialogElement>('detection-progress').open; }
function busy() { return loading || exporting || detecting || choosingOutput || chapterDragging || operationDialogOpen(); }
function notify(message: string, error = false) { text('notice-text', message); el('notice').hidden = false; el('notice').classList.toggle('error', error); }
function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }
function stateStatus(message: string, working = false) {
  el('analysis-status').hidden = !working;
  el('analysis-status').title = message;
  el('analysis-status').setAttribute('aria-label', message);
}

async function open(path?: string) {
  if (busy()) return;
  loading = true; player.pause(); stateStatus('Opening file', true); updateControls();
  try {
    const next = await bridge.openMedia(path);
    if (!next) { stateStatus(info ? 'Ready' : 'Ready'); return; }
    const token = ++generation;
    info = next; editor = new Editor(next.duration, []); position = 0; playbackSpanIndex = 0; keyframeTarget = null; atTimelineEnd = false; analyzed = false; previewFailed = false;
    el('notice').hidden = true; el('preview-error').hidden = true;
    text('filename', next.name); el('filename').title = next.path;
    text('filepath', next.path); text('meta-duration', formatTime(next.duration));
    text('meta-video', next.videoCodec.toUpperCase()); text('meta-resolution', `${next.width} × ${next.height}`);
    text('meta-fps', `${next.fps.toFixed(3).replace(/\.?0+$/, '')} fps`); text('meta-audio', next.audioCodec?.toUpperCase() ?? 'None');
    text('meta-size', next.size >= 1024 ** 3 ? `${(next.size / 1024 ** 3).toFixed(2)} GB` : `${(next.size / 1024 ** 2).toFixed(1)} MB`);
    el('empty-preview').hidden = true;
    player.load(next.previewUrl, next.audioCodec !== null);
    stateStatus('Analyzing keyframes', true); render();
    el('workspace').focus();
    void bridge.analyzeKeyframes().then(keys => {
      if (token !== generation || !editor) return;
      editor.setKeyframes(keys); analyzed = true; stateStatus('Ready'); render();
    }).catch(error => { if (token === generation) { stateStatus('Analysis failed'); notify(errorMessage(error), true); } });
  } catch (error) { notify(errorMessage(error), true); stateStatus(info ? 'Ready' : 'Ready'); }
  finally { loading = false; updateControls(); }
}

function updateControls() {
  const session = !!editor && !busy();
  const source = session && !!editor?.chapters.length;
  const editing = source && analyzed;
  el('editor-controls').hidden = !info; el('empty-shortcuts').hidden = !!info;
  for (const id of ['play', 'jump-button', 'scrubber']) disable(id, !source || (id === 'play' && previewFailed));
  for (const id of ['prev-key', 'next-key']) disable(id, !editing);
  for (const id of ['offset', 'offset-minus', 'offset-plus', 'reset-offset']) disable(id, !source || !info?.audioCodec);
  disable('file-info', !info || busy()); disable('export', !editing); disable('open', busy()); disable('empty-open', busy());
  disable('detect-chapters', !editing);
  text('empty-open', loading ? 'Opening…' : 'Choose file');
}

function render() {
  if (!editor || !info) { updateControls(); return; }
  const total = editor.outputDuration;
  el<HTMLInputElement>('offset').value = String(editor.audioOffset); player.setOffset(editor.audioOffset);
  text('timeline-duration', formatTime(total));
  el<HTMLInputElement>('scrubber').max = String(total || 1);
  text('audio-note', !info.audioCodec ? 'This file has no audio track.' : editor.audioOffset === 0 ? 'Positive values delay audio; negative values advance it.' : `Audio ${editor.audioOffset > 0 ? 'delayed' : 'advanced'} by ${Math.abs(editor.audioOffset)} seconds.`);
  el('chapter-lane').replaceChildren();
  editor.timeline.forEach(c => {
    const block = document.createElement('button');
    block.className = `chapter-block${c.index === editor!.selectedChapterIndex ? ' selected' : ''}`; block.dataset.index = String(c.index);
    block.style.left = `${c.timelineStart / total * 100}%`; block.style.width = `${(c.end - c.start) / total * 100}%`;
    block.textContent = String(c.index + 1).padStart(2, '0'); block.setAttribute('aria-label', `Select chapter ${c.index + 1}`);
    block.setAttribute('aria-pressed', String(c.index === editor!.selectedChapterIndex));
    block.title = `Source ${formatTime(c.start)} → ${formatTime(c.end)} · Drag to reorder`; block.disabled = busy();
    const dragged = attachChapterDrag(block, c.index);
    block.addEventListener('click', event => {
      if (dragged()) return;
      if (event.detail === 0) { selectChapter(c.index); return; }
      if (!editor || busy()) return;
      editor.selectChapter(c.index); seekTimelineClick(event.clientX); render();
    }); el('chapter-lane').append(block);
  });
  el('empty-chapters-preview').hidden = editor.chapters.length > 0;
  video.hidden = previewFailed || !editor.chapters.length;
  el('preview-error').hidden = !previewFailed || !editor.chapters.length;
  el('playhead').hidden = !editor.chapters.length;
  el('timeline-labels').replaceChildren();
  for (let i = 0; i <= 4; i++) { const label = document.createElement('span'); label.textContent = formatTime(total * i / 4).slice(total >= 3600 ? 0 : 3, -4); el('timeline-labels').append(label); }
  updateControls(); drawKeyframes(); updatePosition(position);
}

function drawKeyframes() {
  const canvas = el<HTMLCanvasElement>('keyframe-canvas'), width = canvas.clientWidth, height = canvas.clientHeight, ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * ratio); canvas.height = Math.round(height * ratio);
  const ctx = canvas.getContext('2d'); if (!ctx) return;
  ctx.scale(ratio, ratio); ctx.clearRect(0, 0, width, height); ctx.strokeStyle = '#687c6d'; ctx.lineWidth = 1;
  let lastX = -2;
  if (editor && editor.outputDuration > 0) for (const point of editor.timelineKeyframes) {
    const x = Math.round(point.time / editor.outputDuration * width) + 0.5;
    if (x - lastX < 3) continue;
    ctx.beginPath(); ctx.moveTo(x, 29); ctx.lineTo(x, 42); ctx.stroke(); lastX = x;
  }
}

function updatePosition(source: number) {
  position = source;
  const total = editor?.outputDuration ?? 0, time = atTimelineEnd ? total : editor?.sourceToTimeline(source) ?? 0;
  text('current-time', formatTime(time)); el('jump-button').title = `Source ${formatTime(source)} · Go to time`;
  el<HTMLInputElement>('scrubber').value = String(time); el('playhead').style.left = `${total ? time / total * 100 : 0}%`; updateControls();
}

function seekSource(source: number, end = false) {
  if (!info || !editor?.chapters.length) return;
  keyframeTarget = null;
  window.clearTimeout(boundaryTimer);
  const playable = editor.playableSourceTime(source), last = editor.chapters[editor.chapters.length - 1];
  atTimelineEnd = end || playable === null;
  if (atTimelineEnd) player.pause();
  const logical = atTimelineEnd ? last.end : playable!;
  const spans = editor.exportRanges;
  playbackSpanIndex = atTimelineEnd ? spans.length - 1 : Math.max(0, spans.findIndex(c => logical >= c.start && logical < c.end));
  updatePosition(logical);
  // At the edited end, hold a frame inside the last surviving chapter.
  const actual = atTimelineEnd ? Math.max(last.start, last.end - 0.001) : logical;
  if (!previewFailed) try { player.seek(actual, info.duration); } catch { /* metadata may still be loading */ }
}

function scheduleBoundary() {
  window.clearTimeout(boundaryTimer);
  if (!editor || video.paused || video.seeking || video.readyState < 3 || atTimelineEnd) return;
  const spans = editor.exportRanges;
  const index = playbackSpanIndex;
  if (index < 0) return;
  const chapter = spans[index], next = spans[index + 1];
  // Decoders can present a frame ahead of currentTime. Reserve one frame plus
  // scheduling margin, rather than letting that first deleted frame appear.
  const lead = Math.min(1 / (info?.fps ?? 30) + 0.015, (chapter.end - chapter.start) / 2);
  const delay = Math.max(1, (chapter.end - video.currentTime - lead) / Math.max(0.1, video.playbackRate) * 1000);
  boundaryTimer = window.setTimeout(() => {
    if (!editor || video.paused || video.seeking) return;
    if (video.currentTime < chapter.end - lead) { scheduleBoundary(); return; }
    if (next) seekSource(next.start);
    else { player.pause(); seekSource(chapter.end, true); }
  }, Math.min(delay, 2_147_483_647));
}

function seekTimeline(time: number) {
  const source = editor?.timelineToSource(time);
  if (source !== null && source !== undefined) seekSource(source, time >= editor!.outputDuration);
}

function seekTimelineClick(clientX: number) {
  if (!editor || busy()) return;
  const bounds = el('timeline').getBoundingClientRect();
  if (!bounds.width) return;
  const source = editor.nearestKeyframe((clientX - bounds.left) / bounds.width * editor.outputDuration);
  if (source !== null) seekSource(source);
}

function selectChapter(index: number) {
  if (!editor || busy()) return;
  editor.selectChapter(index);
  if (editor.selectedChapter) seekSource(editor.selectedChapter.start);
  render();
}

function reorderChapter(from: number, to: number) {
  if (!editor || busy() || !analyzed || !editor.reorderChapter(from, to)) return;
  player.pause(); seekSource(editor.selectedChapter!.start); render(); el('workspace').focus();
}

function attachChapterDrag(block: HTMLButtonElement, from: number) {
  let press: { id: number; x: number; y: number } | null = null;
  let moved = false, suppressClick = false, to = from;
  const clear = () => {
    if (press && block.hasPointerCapture(press.id)) block.releasePointerCapture(press.id);
    press = null; chapterDragging = false; block.classList.remove('dragging');
    el('chapter-drop-marker').hidden = true; document.body.classList.remove('chapter-dragging');
    cancelChapterDrag = null; updateControls();
  };
  block.addEventListener('pointerdown', event => {
    if (event.button !== 0 || busy() || !analyzed || cancelChapterDrag) return;
    press = { id: event.pointerId, x: event.clientX, y: event.clientY }; moved = false; suppressClick = false; to = from;
    block.setPointerCapture(event.pointerId);
    cancelChapterDrag = () => { suppressClick = moved; clear(); };
  });
  block.addEventListener('pointermove', event => {
    if (!press || event.pointerId !== press.id) return;
    if (!moved && Math.hypot(event.clientX - press.x, event.clientY - press.y) < 6) return;
    if (!moved) { moved = true; chapterDragging = true; player.pause(); block.classList.add('dragging'); document.body.classList.add('chapter-dragging'); }
    const others = [...el('chapter-lane').querySelectorAll<HTMLButtonElement>('.chapter-block')].filter(other => other !== block);
    to = others.filter(other => { const r = other.getBoundingClientRect(); return event.clientX > r.left + r.width / 2; }).length;
    const bounds = el('timeline').getBoundingClientRect();
    const marker = el('chapter-drop-marker'); marker.hidden = false;
    marker.style.left = `${others[to] ? others[to].getBoundingClientRect().left - bounds.left : bounds.width - 2}px`;
  });
  block.addEventListener('pointerup', event => {
    if (!press || event.pointerId !== press.id) return;
    const reorder = moved; suppressClick = moved; clear();
    if (reorder) { event.preventDefault(); reorderChapter(from, to); }
  });
  block.addEventListener('pointercancel', () => { suppressClick = moved; clear(); });
  block.addEventListener('lostpointercapture', () => { if (press) { suppressClick = moved; clear(); } });
  return () => { const value = suppressClick; suppressClick = false; return value; };
}

function edit(action: 'split' | 'deleteChapter' | 'undo' | 'redo') {
  if (!editor || busy() || ((action === 'split' || action === 'deleteChapter') && !analyzed)) return;
  player.pause();
  if (action === 'split') {
    if (!editor.split(position)) { notify('Use ← / → to move to a keyframe inside a chapter, then press S to split.'); return; }
  } else editor[action]();
  if (editor.selectedChapter) seekSource(editor.selectedChapter.start);
  else { atTimelineEnd = false; updatePosition(0); }
  render();
}

function offset(delta: number) { if (editor && info?.audioCodec && !busy()) { editor.setOffset(editor.audioOffset + delta); render(); } }
async function closeFile() {
  if (busy() || el<HTMLDialogElement>('export-progress').open) return;
  loading = true; player.pause(); updateControls();
  try {
    await bridge.closeMedia();
    ++generation; window.clearTimeout(boundaryTimer); player.unload();
    info = null; editor = null; analyzed = false; previewFailed = false;
    position = 0; playbackSpanIndex = 0; keyframeTarget = null; atTimelineEnd = false;
    for (const dialog of document.querySelectorAll<HTMLDialogElement>('dialog[open]')) dialog.close();
    for (const id of ['preview-error', 'empty-chapters-preview', 'notice', 'drop-overlay', 'playhead']) el(id).hidden = true;
    video.hidden = true; el('empty-preview').hidden = false;
    text('filename', ''); text('filepath', '');
    for (const id of ['meta-video', 'meta-resolution', 'meta-fps', 'meta-audio', 'meta-duration', 'meta-size']) text(id, '—');
    text('timeline-duration', formatTime(0)); text('audio-note', 'Positive values delay audio; negative values advance it.');
    el('chapter-lane').replaceChildren(); el('timeline-labels').replaceChildren();
    el<HTMLInputElement>('offset').value = '0'; el<HTMLInputElement>('scrubber').max = '1';
    drawKeyframes(); updatePosition(0); stateStatus('Ready');
  } catch (error) { notify(errorMessage(error), true); }
  finally { loading = false; updateControls(); if (!info) el('empty-open').focus(); }
}

function editorCommand(command: string) {
  if (el<HTMLDialogElement>('export-progress').open || busy()) return;
  if (command === 'open') void open();
  else if (command === 'export') void exportFile();
  else if (command === 'detect') void detectChapters();
  else if (command === 'close') void closeFile();
  else if (command === 'quit') void bridge.quitApp().catch(error => notify(errorMessage(error), true));
}
function keyframe(direction: -1 | 1) {
  if (!editor?.chapters.length || !analyzed || busy()) return;
  // A source timestamp at edited EOF may start an earlier reordered chapter.
  if (atTimelineEnd) {
    if (direction === -1) {
      const target = editor.timelineKeyframes.at(-1)?.source ?? editor.chapters.at(-1)!.start;
      seekSource(target); keyframeTarget = target;
    }
    return;
  }
  let origin = position;
  if (!video.paused && !previewFailed && video.readyState >= 1) {
    origin = video.seeking ? position : video.currentTime;
    if (keyframeTarget !== null) {
      const next = editor.sourceToTimeline(editor.keyframe(keyframeTarget, 1));
      const targetTime = editor.sourceToTimeline(keyframeTarget);
      // Playback advances immediately after a seek. Until it crosses the next
      // keyframe, repeated navigation must advance from the requested frame,
      // rather than repeatedly seeking back to the start of the same GOP.
      if (video.seeking || next <= targetTime || editor.sourceToTimeline(origin) < next - 0.0001) origin = keyframeTarget;
    }
  }
  const target = editor.keyframe(origin, direction);
  const targetTime = editor.sourceToTimeline(target), originTime = editor.sourceToTimeline(origin);
  if (direction === 1 ? targetTime <= originTime + 0.0001 : targetTime >= originTime - 0.0001) return;
  seekSource(target); keyframeTarget = target;
}
function moveChapter(direction: -1 | 1) { if (editor && !busy()) { editor.moveChapter(direction); selectChapter(editor.selectedChapterIndex); } }
async function toggle() {
  if (!info || !editor?.chapters.length || busy() || previewFailed) return;
  if (atTimelineEnd || editor.playableSourceTime(position) === null) seekSource(editor.chapters[0].start);
  try { await player.toggle(); } catch (error) { notify(`Cannot play: ${errorMessage(error)}`, true); }
}

async function exportFile() {
  if (!editor?.chapters.length || !info || !analyzed || busy()) return;
  player.pause(); choosingOutput = true; updateControls();
  try {
    const output = await bridge.chooseOutput(); if (!output) return;
    const job = crypto.randomUUID(); activeJob = job; exporting = true; stateStatus('Exporting', true);
    text('export-title', 'Exporting'); text('export-output', output);
    el<HTMLProgressElement>('progress-bar').value = 0; text('export-percent', '0%'); text('export-stage', 'Preparing export');
    el('cancel-export').hidden = false; el('export-ok').hidden = true; disable('cancel-export', false);
    render(); el<HTMLDialogElement>('export-progress').showModal();
    await bridge.exportMedia(info.path, output, editor.exportRanges, editor.audioOffset, job);
    text('export-title', 'Export complete'); text('export-stage', 'Saved'); text('export-percent', '100%'); el<HTMLProgressElement>('progress-bar').value = 1;
    el('cancel-export').hidden = true; el('export-ok').hidden = false; el('export-ok').focus(); notify(`Exported: ${output}`);
  } catch (error) { el<HTMLDialogElement>('export-progress').close(); notify(errorMessage(error), !errorMessage(error).toLowerCase().includes('cancel')); }
  finally { exporting = false; choosingOutput = false; activeJob = ''; stateStatus('Ready'); render(); }
}

async function detectChapters(precise = false) {
  if (!editor?.chapters.length || !info || !analyzed || busy()) return;
  el('offset').blur();
  player.pause(); detecting = true; detectionCancelRequested = false; const job = crypto.randomUUID(); activeDetectionJob = job;
  text('detection-title', precise ? 'Full chapter scan' : 'Detect chapters'); text('detection-stage', 'Preparing detection'); text('detection-percent', '0%');
  text('detection-description', precise ? 'Scan every frame to find fades to black or white.' : 'Quickly scan likely fade sections, then split at the nearest keyframes.');
  el<HTMLProgressElement>('detection-bar').value = 0;
  el('cancel-detection').hidden = false; disable('cancel-detection', false); el('detection-ok').hidden = true;
  render(); el<HTMLDialogElement>('detection-progress').showModal();
  try {
    const points = await bridge.detectChapters(job, precise);
    if (detectionCancelRequested) throw new Error('Chapter detection cancelled.');
    const count = editor.splitAtKeyframes(points);
    seekSource(position, atTimelineEnd);
    text('detection-title', 'Detection complete'); text('detection-stage', count ? `Added ${count} boundaries.` : 'No new boundaries found.');
    text('detection-percent', '100%'); el<HTMLProgressElement>('detection-bar').value = 1;
    el('cancel-detection').hidden = true; el('detection-ok').hidden = false; el('detection-ok').focus();
  } catch (error) {
    el<HTMLDialogElement>('detection-progress').close(); notify(errorMessage(error), !errorMessage(error).toLowerCase().includes('cancel'));
  } finally { detecting = false; activeDetectionJob = ''; render(); }
}

el('open').addEventListener('click', () => void open()); el('empty-open').addEventListener('click', () => void open());
el('export').addEventListener('click', () => void exportFile()); el('play').addEventListener('click', () => void toggle());
el('detect-chapters').addEventListener('click', event => void detectChapters((event as MouseEvent).shiftKey));
el('prev-key').addEventListener('click', () => keyframe(-1)); el('next-key').addEventListener('click', () => keyframe(1));
el<HTMLInputElement>('scrubber').addEventListener('input', e => { seekTimeline(Number((e.target as HTMLInputElement).value)); });
el('scrubber').addEventListener('pointerdown', () => { scrubberWasPlaying = !video.paused; });
el('scrubber').addEventListener('pointercancel', () => { scrubberWasPlaying = false; });
el('scrubber').addEventListener('click', event => {
  if (event.detail === 0 || busy()) return;
  seekTimelineClick(event.clientX);
  // The native slider can briefly seek to EOF before this click snaps back to
  // the last keyframe. Keep the playback state from the start of the gesture.
  if (scrubberWasPlaying && video.paused) void toggle();
  scrubberWasPlaying = false;
});
el('offset-minus').addEventListener('click', () => offset(-1)); el('offset-plus').addEventListener('click', () => offset(1));
el('reset-offset').addEventListener('click', () => { if (editor && !busy()) { editor.setOffset(0); render(); } });
el<HTMLInputElement>('offset').addEventListener('change', e => {
  const field = e.target as HTMLInputElement;
  if (editor && !busy() && field.value !== '' && field.validity.valid) editor.setOffset(Number(field.value));
  else notify('Enter an audio offset between -3600 and 3600 seconds.', true);
  render();
});
el('cancel-export').addEventListener('click', () => { disable('cancel-export', true); text('export-stage', 'Cancelling…'); void bridge.cancelExport().catch(error => notify(errorMessage(error), true)); });
el('export-ok').addEventListener('click', () => { el<HTMLDialogElement>('export-progress').close(); render(); });
el('export-progress').addEventListener('cancel', e => e.preventDefault());
el('cancel-detection').addEventListener('click', () => {
  detectionCancelRequested = true;
  disable('cancel-detection', true); text('detection-stage', 'Cancelling…');
  void bridge.cancelDetection().catch(error => notify(errorMessage(error), true));
});
el('detection-ok').addEventListener('click', () => { el<HTMLDialogElement>('detection-progress').close(); render(); });
el('detection-progress').addEventListener('cancel', e => e.preventDefault());
el('dismiss-notice').addEventListener('click', () => { el('notice').hidden = true; });
video.addEventListener('timeupdate', () => {
  if (!editor?.chapters.length || previewFailed) return;
  const last = editor.chapters[editor.chapters.length - 1];
  if (atTimelineEnd && video.paused) { updatePosition(last.end); return; }
  if (video.seeking) return;
  const spans = editor.exportRanges, current = spans[playbackSpanIndex];
  if (!current) return;
  if (video.currentTime >= current.end - 0.0001) {
    const next = spans[playbackSpanIndex + 1];
    if (next) { const resume = video.ended; seekSource(next.start); if (resume && !busy()) void toggle(); }
    else { player.pause(); seekSource(last.end, true); }
  } else if (video.currentTime < current.start - 0.0001) seekSource(current.start);
  else updatePosition(video.currentTime);
});
video.addEventListener('ended', () => {
  const next = editor?.exportRanges[playbackSpanIndex + 1];
  if (video.ended && next && !busy()) { seekSource(next.start); void toggle(); }
});
video.addEventListener('loadedmetadata', () => { if (position > 0 || editor?.chapters[0]?.start) seekSource(position); });
video.addEventListener('play', () => { el('play').innerHTML = icon('pause', 20); el('play').setAttribute('aria-label', 'Pause'); scheduleBoundary(); });
video.addEventListener('pause', () => { keyframeTarget = null; window.clearTimeout(boundaryTimer); el('play').innerHTML = icon('play', 20); el('play').setAttribute('aria-label', 'Play'); });
for (const event of ['playing', 'seeked', 'ratechange']) video.addEventListener(event, scheduleBoundary);
for (const event of ['seeking', 'waiting', 'ended']) video.addEventListener(event, () => window.clearTimeout(boundaryTimer));
video.addEventListener('error', () => { if (info) { previewFailed = true; player.pause(); video.hidden = true; el('preview-error').hidden = false; updateControls(); } });
new ResizeObserver(drawKeyframes).observe(el('timeline'));

const shortcuts = [
  ['Open file', `${mod} O`], ['Export', `${mod} S`], ['Play / pause', 'Space'], ['Previous / next keyframe', '← / →'],
  ['Close file', `${mod} W`], ['Quit', `${mod} Q`],
  ['Split chapter', 'S'], ['Select chapter', `${mod} ← / →`], ['Delete chapter', 'D'],
  ['Move chapter', `${mod} Shift ← / →`],
  ['Detect chapters (fast)', `${mod} Shift D`], ['Detect chapters (full scan)', `${mod} ${isMac ? 'Option' : 'Alt'} Shift D`],
  ['Undo', `${mod} Z`], ['Redo', `${mod} Shift Z`], ['Advance / delay audio 1 s', '[ / ]'], ['Keyboard shortcuts', '?'],
];
for (const [label, key] of shortcuts) { const row = document.createElement('div'); const name = document.createElement('span'); name.textContent = label; const kbd = document.createElement('kbd'); kbd.textContent = key; row.append(name, kbd); el('shortcut-list').append(row); }
for (const id of ['shortcuts', 'empty-shortcuts']) el(id).addEventListener('click', () => el<HTMLDialogElement>('shortcuts-dialog').showModal());
el('file-info').addEventListener('click', () => el<HTMLDialogElement>('file-info-dialog').showModal());
el('close-file-info').addEventListener('click', () => el<HTMLDialogElement>('file-info-dialog').close());
el('close-shortcuts').addEventListener('click', () => el<HTMLDialogElement>('shortcuts-dialog').close());
el('jump-button').addEventListener('click', () => { el<HTMLInputElement>('jump-time').value = formatTime(atTimelineEnd ? editor?.outputDuration ?? 0 : editor?.sourceToTimeline(position) ?? 0); el<HTMLDialogElement>('jump-dialog').showModal(); el<HTMLInputElement>('jump-time').select(); });
el('cancel-jump').addEventListener('click', () => el<HTMLDialogElement>('jump-dialog').close());
el('jump-form').addEventListener('submit', e => {
  e.preventDefault(); const time = parseTime(el<HTMLInputElement>('jump-time').value);
  if (time !== null && editor && time <= editor.outputDuration) { seekTimeline(time); el<HTMLDialogElement>('jump-dialog').close(); }
  else notify('Enter a time within the edited video.', true);
});
for (const id of ['shortcuts-dialog', 'jump-dialog', 'file-info-dialog']) el<HTMLDialogElement>(id).addEventListener('click', e => {
  if (e.target === e.currentTarget) { const box = el(id).getBoundingClientRect(); if (e.clientX < box.left || e.clientX > box.right || e.clientY < box.top || e.clientY > box.bottom) el<HTMLDialogElement>(id).close(); }
});

document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && cancelChapterDrag) { e.preventDefault(); cancelChapterDrag(); return; }
  const target = e.target as HTMLElement, command = isMac ? e.metaKey : e.ctrlKey, key = e.key.toLowerCase();
  if (operationDialogOpen()) { if (command) e.preventDefault(); return; }
  if (command && !e.altKey && (key === 'w' || key === 'q')) { e.preventDefault(); if (!e.repeat) editorCommand(key === 'w' ? 'close' : 'quit'); return; }
  if (document.querySelector('dialog[open]')) return;
  if (command && !e.altKey && (key === 'o' || key === 's')) {
    e.preventDefault(); if (e.repeat) return;
    if (target.closest('input, textarea, [contenteditable="true"]')) target.blur();
    if (key === 'o') void open(); else void exportFile(); return;
  }
  if (target.closest('input, textarea, [contenteditable="true"]')) return;
  if (command && e.shiftKey && key === 'd') { e.preventDefault(); if (!e.repeat) void detectChapters(e.altKey); return; }
  if (command && key === 'z') { e.preventDefault(); edit(e.shiftKey ? 'redo' : 'undo'); return; }
  if (command && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
    e.preventDefault(); const direction = e.key === 'ArrowLeft' ? -1 : 1;
    if (!busy()) { if (e.shiftKey && editor) reorderChapter(editor.selectedChapterIndex, editor.selectedChapterIndex + direction); else moveChapter(direction); }
    return;
  }
  if (command || e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key === '?') { e.preventDefault(); el<HTMLDialogElement>('shortcuts-dialog').showModal(); return; }
  if (!editor || busy()) return;
  switch (key) {
    case ' ': if (target.closest('button')) return; e.preventDefault(); if (!e.repeat) void toggle(); break;
    case 'arrowleft': case 'arrowright': if (!e.shiftKey) { e.preventDefault(); keyframe(key === 'arrowleft' ? -1 : 1); } break;
    case 's': e.preventDefault(); if (!e.repeat) edit('split'); break;
    case 'd': e.preventDefault(); if (!e.repeat) edit('deleteChapter'); break;
    case '[': e.preventDefault(); offset(-1); break;
    case ']': e.preventDefault(); offset(1); break;
  }
});

if (bridge.desktop) {
  void bridge.onEditorCommand(editorCommand).catch(error => notify(errorMessage(error), true));
  void bridge.onProgress(p => {
    if (!exporting || p.jobId !== activeJob) return;
    text('export-stage', p.stage); text('export-percent', `${Math.round(p.fraction * 100)}%`); el<HTMLProgressElement>('progress-bar').value = p.fraction;
  }).catch(error => notify(`Cannot display export progress: ${errorMessage(error)}`, true));
  void bridge.onDetectionProgress(p => {
    if (!detecting || detectionCancelRequested || p.jobId !== activeDetectionJob) return;
    text('detection-stage', p.stage); text('detection-percent', `${Math.round(p.fraction * 100)}%`);
    el<HTMLProgressElement>('detection-bar').value = p.fraction;
  }).catch(error => notify(`Cannot display detection progress: ${errorMessage(error)}`, true));
  void bridge.onDrop(paths => { if (paths[0]) void open(paths[0]); }, active => { el('drop-overlay').hidden = !active || exporting; }).catch(error => notify(errorMessage(error), true));
} else stateStatus('Browser preview');
updateControls();
