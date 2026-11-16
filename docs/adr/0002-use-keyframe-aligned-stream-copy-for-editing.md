---
id: "0002"
status: Accepted
date: "2026-11-13"
---

# ADR 0002: Use keyframe-aligned stream copy for editing

## Context

The primary editing goal is to remove and reorder portions of a local video while keeping the encoded video and audio unchanged. Re-encoding would add processing time and could reduce quality. Compressed video cannot in general be cut cleanly at an arbitrary frame without decoding and encoding around the boundary.

## Decision

Represent edits as ordered source-time ranges, constrain cut boundaries to keyframes, and use FFmpeg stream copy to assemble the retained ranges. Apply a constant audio timestamp offset during export without re-encoding the media streams.

This decision does not define frame-accurate re-encoding, codec compatibility for every output container, or embedded container chapter metadata. Those remain outside the current editing model and are documented in the user guide.

## Consequences

- For supported inputs and boundaries, export preserves the encoded media payload and avoids video re-encoding.
- Cuts are limited to keyframes; open GOPs and packet timing can affect boundary behavior and audio offset precision.
- Some source codec and destination container combinations cannot be copied directly and may fail rather than silently transcode.
- Future frame-accurate editing would require a separate mode with local re-encoding and explicit quality/performance tradeoffs.

## Alternatives considered

- **Re-encode every export:** Supports arbitrary boundaries and broad output compatibility, but adds time and changes encoded media.
- **Re-encode only around cuts:** Improves boundary precision but adds substantially more pipeline complexity and can create codec or quality differences at joins.

## References

- `docs/specs/2026-11-13-minimal-video-editor-design.md` — source-range and stream-copy design (checked 2026-11-16).
- `src-tauri/src/export.rs` and `src-tauri/tests/media_integration.rs` — range validation, remuxing, and packet-preservation checks (checked 2026-11-16).
- [FFmpeg stream copy documentation](https://ffmpeg.org/ffmpeg.html#Streamcopy) — stream-copy behavior and limitations (checked 2026-11-16).
