# Playback fix and pnpm migration

## Reproduction

The reported 834.8 MiB MP4 contains 960×540 H.264 High / yuv420p at 30000/1001 fps and AAC-LC stereo. The original native WKWebView probe reproduced audio playback with zero video dimensions/decoded frames and 29 audio seeks over eight seconds. This was a preview failure, not an unsupported codec or export problem.

## Changes

- Replace custom `asset://` media URLs with loopback HTTP streaming. The custom asset implementation caps each byte-range response at roughly 1 MiB; the same video decoded normally through an HTTP range server. The production server responds to exact ranges, including HEAD and suffix requests, with bounded 64 KiB buffers and at most eight active connections. It serves only the selected file through a fresh random route, validates the loopback Host, revokes old routes on selection changes, and stops on application exit. No full-file loading, transcoding, or preview copy is needed.
- At zero audio offset, use the video's own audio stream and decoder clock. Load a separate audio element only when offset preview is active.
- Treat advancing audio as the reference clock. User play/seek/offset changes may position audio, but ordinary clock drift changes video playback rate instead of continuously flushing the audio decoder. Align video once after audio startup or recovery. Drive synchronization through a low-frequency timer while offset playback is active, so obscured WebViews do not depend on rendering callbacks. Do not require a second `playing` event after a buffered seek; WKWebView can omit it.
- Add real-media browser tests and a native playback probe. The native probe checks actual decoded frames/nonblack canvas pixels, audio clock advancement, bounded offset error and seek count. Browser tests also withhold rendering callbacks and repeated `playing` events to cover the native event sequence.
- Switch package management to pnpm 12.16.1: `packageManager`, `mise.toml`, `pnpm-lock.yaml`, startup/build commands, CI and documentation. Remove the npm lockfile. Frozen offline pnpm install succeeds.

## Verification, 2026-11-14

- Node model/timing tests: 13 passing.
- Rust: 2 unit + 9 FFmpeg export integration + 3 real HTTP streaming tests passing.
- Browser UI and real decoding: 4 passing, including input/shortcut behavior and offset reset.
- TypeScript check, frontend build, Rust formatting and clippy with denied warnings pass.
- Native WKWebView playback using the reported file: all three probes exit 0. No source file mutation.

| Offset | Video dimensions | Audio seek events in 8 s | Measured offset error |
| --- | --- | --- | --- |
| 0 | 960×540 | 0 | native combined playback |
| +0.250 s | 960×540 | 0 | 0.0021 s |
| −0.250 s | 960×540 | 1 | 0.0107 s |

Export still uses FFmpeg stream copy with unchanged codec payloads. Windows interactive playback remains unverified; the architecture and build settings remain compatible with Windows.
