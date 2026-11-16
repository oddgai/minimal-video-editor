---
id: "0003"
status: Accepted
date: "2026-11-14"
---

# ADR 0003: Stream local media to the WebView over loopback HTTP

## Context

Large MP4 files did not play reliably in the macOS WebView through the custom asset URL protocol, whose range responses were capped. Loading the whole video into application memory or creating a converted preview would add memory, startup time, and disk usage.

## Decision

Serve only the selected local media file through a loopback HTTP endpoint that supports byte ranges. Bound each transfer buffer and connection count, scope the route to the current media selection, and revoke it when the selection changes or the app exits.

This ADR does not define preview UI behavior or media codecs supported by each operating system's WebView; those are covered by the player implementation and user documentation.

## Consequences

- Large files can be read on demand without copying the full file into memory or generating a proxy.
- The local server must strictly limit access to the selected file and reject requests that do not match its loopback-only route.
- Playback remains dependent on native WebView codec support.
- Server lifecycle, request ranges, cancellation, and stale media tokens require regression coverage.

## Alternatives considered

- **Custom asset protocol:** Simpler integration, but its capped range responses caused unreliable playback for large MP4s.
- **Load the entire file or generate a proxy:** Avoids range-protocol constraints but uses more memory, time, or disk space.

## References

- `docs/specs/2026-11-14-playback-fix.md` — playback failure investigation and loopback design (checked 2026-11-16).
- `src-tauri/src/preview.rs` and `src-tauri/tests/preview_stream.rs` — range handling, bounded reads, route validation, and tests (checked 2026-11-16).
- `README.md` — user-facing statement of local streaming and supported preview behavior (checked 2026-11-16).
