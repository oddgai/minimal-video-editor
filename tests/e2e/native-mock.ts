import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';

const harness = window as unknown as {
  holdExport?: boolean;
  finishExport?: (error?: string) => void;
  exportProgress?: (jobId: string, fraction: number, stage: string) => Promise<void>;
  dropMedia?: () => Promise<void>;
  editorCommand?: (command: string) => Promise<void>;
  quitRequested?: boolean;
  holdDetection?: boolean;
  detectionResult?: number[];
  finishDetection?: (error?: string) => void;
};
harness.exportProgress = (jobId, fraction, stage) => emit('export-progress', { jobId, fraction, stage });
harness.dropMedia = () => emit('tauri://drag-drop', { paths: ['/tmp/another.mp4'], position: { x: 0, y: 0 } });
harness.editorCommand = command => emit('editor-command', command);

// Exercise the real UI at the IPC boundary. FFmpeg and file correctness are
// covered separately by native integration tests with real media files.
Object.assign(window, { isTauri: true });
mockWindows('main');
mockIPC(async (command, args) => {
  switch (command) {
    case 'open_media': return { path: '/tmp/旅行 sample.mp4', previewUrl: '/test-video.mp4', name: '旅行 sample.mp4', duration: 12, startTime: 0, width: 1920, height: 1080, fps: 24, videoCodec: 'h264', audioCodec: 'aac', audioStart: 0, size: 1024 ** 2 * 42 };
    case 'analyze_keyframes': return [0, 2, 4, 6, 8, 10];
    case 'detect_chapters': {
      await emit('detection-progress', { jobId: (args as { jobId: string }).jobId, fraction: 0.5, stage: 'Scanning black and white fades' });
      if (harness.holdDetection) return new Promise<number[]>((resolve, reject) => {
        harness.finishDetection = error => error ? reject(new Error(error)) : resolve(harness.detectionResult ?? [2, 6, 10]);
      });
      return harness.detectionResult ?? [2, 6, 10];
    }
    case 'cancel_detection': harness.finishDetection?.('Chapter detection cancelled.'); return null;
    case 'choose_output': return '/tmp/edited.mp4';
    case 'start_export': {
      Object.assign(window, { lastExport: args });
      if (harness.holdExport) return new Promise<void>((resolve, reject) => {
        harness.finishExport = error => error ? reject(new Error(error)) : resolve();
      });
      return null;
    }
    case 'cancel_export': harness.finishExport?.('Export cancelled.'); return null;
    case 'close_media': return null;
    case 'quit_app': harness.quitRequested = true; return null;
    default: throw new Error(`Unexpected IPC command: ${command}`);
  }
}, { shouldMockEvents: true });
Object.assign((window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__, { convertFileSrc: () => '/test-video.mp4' });
