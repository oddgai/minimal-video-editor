---
id: "0005"
status: Accepted
date: "2026-11-14"
---

# ADR 0005: Use corrected audio as the preview clock when offset

## Context

When the audio offset is zero, the native video element can decode and play the media together. When an offset is applied, audio must be played from a separate element. Repeatedly seeking that decoder to follow video can interrupt sound, especially after timeline scrubbing, while allowing the clocks to drift makes lip synchronization unreliable.

## Decision

Keep the native combined media path when the offset is zero. When an offset is active, play the corrected audio separately and treat its advancing playback clock as the reference. Align video after audio starts or recovers, and correct ordinary drift by adjusting video playback rate rather than repeatedly seeking the audio decoder.

This ADR does not set user-facing offset units, shortcut bindings, or the exact drift correction interval; those belong to the UI and player implementation.

## Consequences

- Normal playback avoids a second audio decoder when no correction is requested.
- Offset preview can preserve continuous audio through seeking and synchronization adjustments.
- Video playback rate may vary slightly while correcting drift; the exported media is unaffected by preview synchronization.
- WebView buffering and event ordering still require real-media regression tests.

## Alternatives considered

- **Use video as the clock and repeatedly seek audio:** Simple to reason about initially, but can interrupt audio and lose alignment during buffered seeks.
- **Resample or time-stretch audio to follow video:** Adds processing and changes preview audio behavior even though the requested correction is a constant offset.

## References

- `docs/specs/2026-11-14-playback-fix.md` and `docs/specs/2026-11-15-seek-audio-fix.md` — playback clock decisions and seek regression evidence (checked 2026-11-16).
- `src/player.ts` and `tests/e2e/player.spec.ts` — separate offset audio path, synchronization, and seek tests (checked 2026-11-16).
- `README.md` — user-facing audio correction and preview limitations (checked 2026-11-16).
