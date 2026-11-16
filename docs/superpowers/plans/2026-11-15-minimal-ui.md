# Minimal English UI

> **For agentic workers:** Use superpowers:executing-plans to implement inline. Detailed UI decisions were delegated by the user earlier.

**Goal:** Apply the user's five screenshot annotations: English UI; only Choose file in the empty preview; no branded header/sidebar/footer/chapter editing panel or timeline captions.

**Architecture:** Retain the existing playback, chapter model, native export and shortcuts. Use one full-width video preview, one compact chapter timeline and one bottom control row. Put audio offset in this row with file/open/export/info/shortcut controls. File details live in a dialog. Editing actions remain available through chapter blocks and shortcuts.

**Tech stack:** Existing TypeScript, CSS, Tauri; no dependencies or preview transformations.

## Constraints and review focus

- Initial screen: Choose file button and an unobtrusive keyboard icon; no marketing copy or disabled editing panels.
- Loaded screen: no header/brand/status heading, inspector, chapter list/details/buttons, timeline caption/count legend, or footer. Keep full-width chapter blocks and keyframe markers.
- Preserve play/pause and seek behavior, compact edited timeline, chapter shortcuts, undo/redo, offset seconds, export progress and modal guards, Cmd W/Q, drag-and-drop.
- Accessible English labels/tooltips for icon controls; English custom native menu/dialog/error/progress text. User filenames are unchanged. System-supplied menu labels follow OS locale.
- Verify full layouts at 1280×850 and minimum 900×680, keyboard focus after load/close, empty chapter state, disabled unsupported audio, and details/help modal keyboard handling.

## Tasks

- [x] Adapt the existing UI regressions to the timeline-only chapter UI and English controls; add coverage for on-demand file info/audio controls. Observed the Choose file assertion fail before implementation.
- [x] Replace template/styles, remove DOM bindings for deleted elements, move audio and utility controls, and add file info dialog. Preserve player/model logic.
- [x] Translate remaining frontend/native user-facing copy; keep cancellation recognition working with English errors. Update documentation and native playback probe selectors.
- [x] Run Node, type, browser and Rust checks: 17 Node + 12 Playwright + 16 Rust tests pass; typecheck/fmt/clippy pass. Inspected empty/loaded/file info screenshots at 1280×850 and loaded screen at 900×680.
- [x] Fresh scoped review `minimal_ui_review`: no Critical/Important findings. Reviewed removed bindings, close/reopen, empty chapters, shortcuts, modal/cancel behavior, translations and narrow screenshot.
- [x] Verify native playback with the updated DOM: provided H.264/AAC source decodes at 960×540, nonblack pixels, advancing audio with −0.25 s correction, 0.0005 s final offset error, no media errors; probe exits 0.
- [x] Build macOS release app: bundle succeeds, 4.57 MiB. Frontend assets shrink to 9.13 kB CSS / 48.92 kB JS; no dependencies added.
