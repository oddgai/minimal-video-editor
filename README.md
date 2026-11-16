# Minimal Video Editor

A lightweight, keyboard-first desktop video editor focused on macOS, with a Windows-compatible architecture. Keep the ranges you need, correct a constant audio offset, and export without re-encoding.

Built with Tauri 2, Rust, and TypeScript. The app uses the operating system's WebView instead of bundling Chromium or Node.js. It does not load whole videos into JavaScript memory or continuously generate waveforms, thumbnails, or proxies. Editing and export run locally.

## Features

- Open local videos using the file picker or drag and drop.
- Play and navigate to keyframes or a specified time.
- Split at keyframes, select chapters, and delete them.
- Keep the timeline compact after deletion and preview the remaining chapters continuously.
- Export the remaining chapters as one video without re-encoding.
- Correct audio timing by a number of seconds in both preview and export.
- Undo and redo edits; monitor or cancel exports.
- Prevent overwriting the source file or an existing destination.

## Development

Requirements: Node.js 24 or later, the pinned pnpm version from `package.json`, stable Rust, FFmpeg, and ffprobe. macOS also requires Xcode Command Line Tools. Windows requires the MSVC C++ build tools and WebView2.

On macOS:

```sh
xcode-select --install  # Only if not already installed
brew install ffmpeg
pnpm install
pnpm desktop
```

The project pins tool versions in `mise.toml`. Review the file before trusting it, then run `mise trust` and `mise install` if desired.

On Windows, add the FFmpeg `bin` directory (containing both `ffmpeg.exe` and `ffprobe.exe`) to `PATH`, then use the same pnpm commands. Interactive Windows behavior has not been verified.

`pnpm dev` starts the UI for browser-based layout checks. Opening, analyzing, and exporting files require the desktop app started by `pnpm desktop`.

## Build the app

```sh
pnpm desktop:build
```

The macOS bundle is written to:

```text
src-tauri/target/release/bundle/macos/Minimal Video Editor.app
```

On Windows, the build configuration in `src-tauri/tauri.windows.conf.json` produces an NSIS installer. macOS builds target the architecture used to build them. The first build may take a while while Rust dependencies compile.

FFmpeg is not currently bundled; installing it is still required. The app searches for executables in this order:

1. Paths set by `MVE_FFMPEG` and `MVE_FFPROBE`.
2. The app executable's directory and, on macOS, `Contents/Resources`.
3. `/opt/homebrew/bin` and `/usr/local/bin`.
4. `PATH`.

Code signing and notarization are not configured. The project plans to bundle per-platform LGPL builds of FFmpeg and ffprobe; see [ADR 0006](docs/adr/0006-bundle-lgpl-ffmpeg.md). The current app still requires a separate FFmpeg installation.

## Usage

1. Open a video with **Choose file** or `⌘ O` / `Ctrl O`. The UI is in English. Keyframes are analyzed in the background, so playback can begin while analysis continues.
2. Use `← / →` to move between keyframes and `S` to split. Each resulting segment is a chapter. The new chapter on the right is selected after a split. Splits are unavailable at the source start/end or an existing boundary.
3. Select a chapter with `⌘ ← / →` (Windows: `Ctrl ← / →`) or click it on the timeline, then press `D` to delete it. Clicking a chapter or the seek bar moves to the nearest retained keyframe. Deleting a chapter removes it from the timeline and closes the gap; playback skips deleted sections.
4. The playhead and timeline show edited time. Click the time display to open **Go to time**; hover over it to see the corresponding source time. Undo splits and deletions with `⌘ Z` / `Ctrl Z`.
5. If needed, enter an audio correction in **Audio offset**. **Positive values delay audio; negative values advance it.** For example, `0.25` delays audio by a quarter second. The plus/minus buttons and shortcuts change the value by one second.
6. Use **Export** or `⌘ S` / `Ctrl S` to save a copy. Supported destination extensions include MP4, MOV, MKV, WebM, and M4V. If the codecs are incompatible with the selected container, the app reports an error; try a container such as MKV.

Open file details with the information icon (**File info**) and shortcuts with the keyboard icon. The shortcut list is also available from the empty screen using the keyboard icon or `?`.

Reorder chapters by dragging them on the timeline or by pressing `⌘ Shift ← / →` (Windows: `Ctrl Shift ← / →`) for the selected chapter. The order affects playback and export. Reordering can be undone with `⌘ Z` / `Ctrl Z`.

Use the detection icon (**Detect chapters**) or `⌘ Shift D` / `Ctrl Shift D` to find transitions to black or white. Each detected location is snapped to a nearby keyframe. Existing deletions, order, and audio correction are preserved; all new splits can be undone together. Detection can be cancelled in its progress dialog, and cancellation leaves the edit unchanged.

Detection runs only when requested. It analyzes small grayscale frames at 5 fps and uses up to four decoder threads, depending on available CPU cores. One successful result is cached in memory and reused if the source path, file size, modification time, and keyframes are unchanged. Closing the video clears the cache. The first scan decodes the video and can take time on long files. Black or white intervals with visible content on both sides are split, whether they fade gradually or cut abruptly. Short changes, incomplete fades, or bright transitions obscured by titles may be missed; leading/trailing dark intervals and ordinary scene cuts are ignored.

Navigation to keyframes, chapters, timeline positions, or a specified time preserves playback when already playing. A paused video stays paused; moving to the edited end stops playback. Repeated arrow presses during playback use the previous requested destination as their origin until normal playback passes the next keyframe, then return to using the live playhead.

## Keyboard shortcuts

| Action | macOS | Windows |
| --- | --- | --- |
| Open video | `⌘ O` | `Ctrl O` |
| Export | `⌘ S` | `Ctrl S` |
| Close video and return to the start screen | `⌘ W` | `Ctrl W` |
| Quit the app | `⌘ Q` | `Ctrl Q` |
| Play / pause | `Space` | `Space` |
| Move between keyframes | `← / →` | Same |
| Split chapter | `S` | Same |
| Select chapter | `⌘ ← / →` | `Ctrl ← / →` |
| Reorder chapter | `⌘ Shift ← / →` | `Ctrl Shift ← / →` |
| Detect transitions | `⌘ Shift D` | `Ctrl Shift D` |
| Delete chapter | `D` | Same |
| Undo | `⌘ Z` | `Ctrl Z` |
| Redo | `⌘ Shift Z` | `Ctrl Shift Z` |
| Advance / delay audio by one second | `[ / ]` | Same |
| Show shortcuts | `?` | Same |

Editing shortcuts are disabled while typing in an input. `⌘ S` / `Ctrl S` commits the active input before export. Focused buttons can also be activated with Enter or Space.

During export, a progress dialog stays in front. It shows progress, the current stage, and the destination, and provides a cancel button. On success it shows 100% and an OK button. The editor, drag and drop, and app shortcuts (including quit) remain disabled until OK is pressed.

## Precision and limitations

- **Lossless cuts are keyframe-aligned.** Arbitrary-frame cuts and partial re-encoding are not supported. Formats that reference frames outside a keyframe, such as some open-GOP streams, may show artifacts near boundaries.
- Audio is selected from the source after applying the correction, then cut with the video. Audio outside the source duration is silent so the video duration stays unchanged. Stream-copy packet boundaries can introduce timing error based on the codec's packet duration. For example, correcting the whole video by −12 seconds leaves the last 12 seconds without source audio.
- Preview depends on codecs supported by the operating system's WebView. H.264/AAC MP4 is recommended. Even if formats such as MKV cannot be previewed, analysis, time navigation, editing, and export remain available. The app does not automatically run an expensive conversion.
- Preview uses local HTTP streaming and a temporary URL scoped to the selected file. It transfers data in 64 KiB chunks, so it does not load the whole video into memory. The video is not uploaded or pre-converted.
- With no audio correction, video and audio play together. When correction is active, a separate audio stream provides the playback clock and video playback rate is adjusted for small drift. This does not change export speed or quality.
- Keyframe navigation and splits use packet timestamps. Adjacent chapters are joined during export to avoid unnecessary cuts and joins.
- Export includes the first video stream and, if present, the first audio stream. Multiple audio streams, subtitles, and embedded container chapter metadata are not preserved. “Chapters” in this app are editing segments.
- Preview seeks the source file and plays retained chapters in order; rendering at joins and audio correction may differ from the exported file.
- The app corrects a constant audio offset. Automatic synchronization, drift that changes over time, filters, and multitrack editing are not supported.
- Export temporarily requires additional free disk space. Intermediate ranges, joins, and the final output are written beside the destination. The final file is published only on success; temporary files are removed after ordinary completion or cancellation.

## Verification

```sh
pnpm test
pnpm check
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm exec playwright install chromium
pnpm test:ui
```

Rust integration tests generate media with FFmpeg and check SHA-256 hashes of copied video/audio packets, B-frame decoding, positive and negative offsets, silent video, paths containing Unicode/spaces/quotes, overwrite protection, and cleanup after cancellation. UI tests replace the IPC boundary to check chapter operations and shortcuts, and use generated videos to verify displayed frames after deletion, end-of-playback behavior, and audio correction. Interactive tests of native playback and dialogs on each OS are still needed.

Architecture decisions are recorded in [docs/adr](docs/adr), design documents in [docs/specs](docs/specs), and implementation notes in [docs/plans](docs/plans). The implementation uses the [Tauri command API](https://v2.tauri.app/develop/calling-rust/), [FFmpeg stream copy](https://ffmpeg.org/ffmpeg.html#Streamcopy), [audio timestamp offsets](https://ffmpeg.org/ffmpeg.html#Main-options), and [packet bitstream filters](https://ffmpeg.org/ffmpeg-bitstream-filters.html#noise).

## Native macOS playback probe

To test playback in the macOS WebView, build the frontend and run the standalone probe. It reads the video without converting or exporting it.

```sh
pnpm build
MVE_PLAYBACK_FILE="/path/to/video.mp4" MVE_PROBE_OFFSET="0.25" \
  cargo run --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --example playback_probe
```

To test audio recovery after seeking, use a video at least 200 seconds long and set `MVE_PROBE_MODE=seek`. The probe alternates six paused and playing seeks, then checks the corrected position, audio clock, and synchronization.

The probe checks decoded frames/rendered pixels, playback clocks, audio offset, and seek counts, and returns exit code 1 if a threshold is missed. It skips only the file picker and full keyframe scan; it uses the production UI, player, and streaming path.
