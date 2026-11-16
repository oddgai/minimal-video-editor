# Audio recovery after seeking with an offset

## Report and reproduction

The user reports intermittent missing audio when moving the seekbar and starting playback with audio correction enabled.

A real-decoder browser regression reproduces the event-ordering fault: deliver `play`/`seeked` while `readyState` is `HAVE_CURRENT_DATA`, then allow the decoder to become ready. The old `sync(true)` returns before seeking audio, and later timer ticks call `sync(false)`, losing the requested alignment. Warm audio alignment can subsequently move video back to the stale audio position. In the failing run, a seek to 8 seconds resumed around video 3.7589 / audio 4.8206 with a −1 second offset.

The pre-fix native stress run did not reproduce complete silence naturally; all six seeks advanced audio, with transient startup drift around 0.21 seconds. Keep this distinct from the deterministic buffering regression.

## Fix

- Retain the explicit audio-seek request across paused, seeking, and buffering states. Clear it when audio timing can actually be corrected, rather than when the request first arrives.
- Mark a pending correction immediately on user seeks, invalidate the previous warm-audio reference, and prevent warm video alignment while a newer seek is pending or video is still seeking.
- Preserve the existing one-time audio seek and low-frequency video rate correction during ordinary playback. Do not repeatedly seek a warmed audio decoder.
- Clear the pending state when loading/unloading media. Keep source files and export processing unchanged.

## Verification

- Browser regression checks both offset signs and confirms the requested video position, advancing audio, and bounded drift after buffering.
- Existing zero-offset native sound, smooth offset playback, chapter navigation, export dialog, and shortcut tests pass: 17 Node tests and 11 Playwright tests.
- Native probe adds six alternating paused/running seekbar sequences against the provided 960×540 H.264/AAC MP4. It checks advancing audio, selected position, startup drift within 0.3 seconds at 1.3 seconds after each seek, and final steady drift within 0.15 seconds after eight seconds. Both −12 and +12 second runs exit 0: every seek resumes audio, with final absolute drift 0.0497 and 0.0098 seconds respectively. No media errors.
- TypeScript check, frontend build, Rust formatting and clippy with denied warnings pass.
- Fresh read-only review `seek_audio_review`: no confirmed Critical or Important findings.
