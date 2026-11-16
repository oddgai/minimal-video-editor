import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Range } from './editor.ts';

export interface MediaInfo {
  path: string; previewUrl: string; name: string; duration: number; startTime: number; width: number; height: number;
  fps: number; videoCodec: string; audioCodec: string | null; audioStart: number; size: number;
}
export interface ExportProgress { jobId: string; fraction: number; stage: string }
export const desktop = isTauri();

export async function openMedia(path?: string) {
  if (!desktop) throw new Error('Open the desktop app to edit local videos.');
  return invoke<MediaInfo | null>('open_media', { path: path ?? null });
}
export const analyzeKeyframes = () => invoke<number[]>('analyze_keyframes');
export const chooseOutput = () => invoke<string | null>('choose_output');
export const cancelExport = () => invoke('cancel_export');
export const detectChapters = (jobId: string, precise: boolean) => invoke<number[]>('detect_chapters', { jobId, precise });
export const cancelDetection = () => invoke('cancel_detection');
export const onDetectionProgress = (handler: (p: ExportProgress) => void) => listen<ExportProgress>('detection-progress', e => handler(e.payload));
export const closeMedia = () => invoke('close_media');
export const quitApp = () => invoke('quit_app');
export const onEditorCommand = (handler: (command: string) => void) => listen<string>('editor-command', e => handler(e.payload));
export const exportMedia = (input: string, output: string, ranges: Range[], audioOffset: number, jobId: string) =>
  invoke('start_export', { request: { input, output, ranges, audioOffset }, jobId });
export const onProgress = (handler: (p: ExportProgress) => void) => listen<ExportProgress>('export-progress', e => handler(e.payload));
export const onDrop = (handler: (paths: string[]) => void, hover: (active: boolean) => void) => getCurrentWebview().onDragDropEvent(e => {
  hover(e.payload.type === 'over' || e.payload.type === 'enter');
  if (e.payload.type === 'drop') handler(e.payload.paths);
});
