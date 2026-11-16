---
id: "0001"
status: Accepted
date: "2026-11-13"
---

# ADR 0001: Use Tauri 2 for the desktop app

## Context

The editor needs to run on macOS and preferably Windows, handle large local video files with low overhead, and invoke FFmpeg without reading entire media files into the UI process. A macOS-only native application would not meet the cross-platform goal; shipping a browser runtime would increase the application footprint.

## Decision

Build the desktop shell with Tauri 2, use Rust for native file, media, and process operations, and use TypeScript with the operating system WebView for the interface. Keep media processing local and invoke the installed FFmpeg tools.

This decision sets the application boundary and technology stack. It does not decide FFmpeg packaging, signing, or notarization; those belong to distribution planning.

## Consequences

- The application can share its UI and Rust media operations across macOS and Windows while using the system WebView.
- The app avoids bundling Chromium and Node.js.
- Native platform behavior and WebView codec support can differ, so playback and release behavior require platform-specific verification.
- Users need compatible FFmpeg and ffprobe executables available to the app.

## Alternatives considered

- **Electron:** Offers a bundled, consistent Chromium runtime and Node.js integration, but increases the runtime footprint.
- **Native Swift/AppKit:** Fits macOS well but would require a separate Windows implementation to meet the platform goal.

## References

- `docs/specs/2026-11-13-minimal-video-editor-design.md` — platform goals and selected stack (checked 2026-11-16).
- `src-tauri/tauri.conf.json`, `src-tauri/src/` and `src/` — current desktop shell, Rust commands, and TypeScript UI (checked 2026-11-16).
- [Tauri 2 documentation](https://v2.tauri.app/) — desktop architecture and supported platforms (checked 2026-11-16).
