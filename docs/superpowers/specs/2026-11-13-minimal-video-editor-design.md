# Minimal Video Editor

## Intent
A small, keyboard-first desktop application for cutting local video without re-encoding and correcting a constant audio offset. macOS is the first supported platform; keep the implementation compatible with Windows. The user delegates detailed design and prioritizes responsiveness and low overhead.

## Decisions
- Tauri 2, Rust, and vanilla TypeScript/CSS. Use the system WebView instead of shipping Chromium. No frontend framework, remote services, telemetry, accounts, or full-file JavaScript reads.
- FFmpeg/ffprobe are external executables, discovered next to the application, from explicit environment overrides, common macOS install paths, and PATH. Provide actionable setup errors. Bundling these executables and their licenses is a separate distribution concern.
- One source file per editing session. Keep an ordered list of source-time ranges. Start with the whole source, support keeping or removing a marked range and undo/redo.
- Probe basic metadata first. Scan packet timestamps and keyframe flags in a background task without decoding the video. Store only keyframe timestamps. Editing at random-access boundaries is enabled after analysis; ordinary playback remains available during analysis.
- Markers snap to known keyframes (the source end is also an allowed end). Show the actual marked positions. No silent re-encoding. Some open-GOP codecs can still need a safer random-access point; state this limitation.
- Stream-copy each retained range to temporary files in the destination container, concatenate when necessary with a consistent presentation timestamp origin, and apply a constant audio timestamp offset in a final remux. Positive offset means audio plays later. Export only the primary video and optional primary audio. Preserve video/audio codecs. Output defaults to the source container when supported.
- Cancel and reap background keyframe scans on source replacement and window/application exit. Keep scans sequential per current session.
- Never overwrite source or existing output files. Export to a private temporary directory beside the destination, then publish with a no-clobber operation. Cancel stops the active FFmpeg child and removes temporary outputs. Only one export at a time.
- Preview uses native video/audio elements and loopback HTTP byte-range streaming with 64 KiB transfer buffers, a random per-selection URL, and at most eight connections. This avoids WKWebView large-MP4 failures observed with the capped custom asset protocol. No full-file reads or proxy conversion. At zero offset, play audio directly through the video element. Separate audio is loaded only for offset preview; use its advancing output clock as the reference and adjust video, rather than repeatedly seeking or changing the tempo of audio. Unsupported WebView codecs receive a clear message rather than an automatic expensive transcode.
- A compact Japanese interface: preview, timeline with keyframes and retained ranges, markers, retained-range list, audio offset in seconds, export progress, and shortcut reference.

## Shortcuts
Command/Ctrl+O open; Space play/pause; Left/Right step one nominal frame; Shift+Left/Right previous/next keyframe; Alt+Left/Right seek 5 seconds; Home/End source bounds; I/O set markers; Backspace/Delete remove marked range; Enter keep marked range; Command/Ctrl+Z undo; Shift+Command/Ctrl+Z redo; Command/Ctrl+E export; [/] decrease/increase audio delay 0.01 seconds; ? shortcut reference. Do not intercept editing keys inside inputs.

## Alternatives
Electron is easier to integrate but adds a Chromium/Node runtime. Native Swift/AppKit is attractive for macOS but creates a second frontend for Windows. Tauri fits the requested platform direction and overhead preference. These are implementation choices delegated by the user, not independently approved preferences.

## Limits of the first version
No automatic lip synchronization, drift correction, filters, transitions, multitrack editing, or automatic transcoding. Native preview codec support varies by OS. Nominal frame stepping uses the reported frame rate and is approximate for variable-frame-rate sources. Stream-copy boundaries, particularly audio packet durations and open GOPs, constrain precision. Windows builds are checked in CI; interactive testing requires Windows.

## Acceptance
Open a local H.264/AAC MP4, navigate keyframes, mark and remove/keep ranges with shortcuts, undo/redo, preview an audio offset, and export with unchanged encoded video/audio packets. Exercise silent media, paths with Unicode/spaces/apostrophes, invalid ranges, source overwrite attempts, cancellation, repeated exports, and positive/negative audio offsets. Typecheck/build frontend, test editing behavior, compile/test Rust, run real FFmpeg integration tests, and build a macOS app.
