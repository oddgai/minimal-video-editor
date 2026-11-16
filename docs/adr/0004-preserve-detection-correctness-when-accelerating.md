---
id: "0004"
status: Accepted
date: "2026-11-16"
---

# ADR 0004: Preserve full-scan results when accelerating chapter detection

## Context

Automatic detection decodes low-resolution grayscale samples to find black and white intervals. Scanning every frame sample in a long video can take minutes, while skipping too much media can miss chapter boundaries or report false ones. Users expect the faster mode to retain the results of the precise full scan.

## Decision

Run detection only when the user requests it. Accelerate ordinary H.264 MP4 scans by using the file's sample index to choose local regions, then run the normal detector within those regions. If the sample index is unsupported, the candidate regions are too extensive, or a format cannot be analyzed safely, use a full scan. A faster path must preserve the full-scan boundary results for the supported and verified media set.

This decision establishes correctness as a constraint on optimization. It does not freeze the sample-index heuristic, brightness thresholds, or benchmark target; those are implementation details that may change while preserving this constraint.

## Consequences

- Typical supported files can be detected much faster without reducing the detector's precision inside selected regions.
- Dense or unsupported files can remain as slow as a full scan; this is the intentional fallback when a safe acceleration is unavailable.
- Changes to candidate selection require comparison against full-scan results, including coverage of varied real media.
- Hard black or white intervals with visible content on both sides count as boundaries, whether they fade gradually or cut abruptly.

## Alternatives considered

- **Always scan the full video:** Provides the reference result but can take minutes on long files.
- **Use sparse approximate scans without fallback:** Improves speed on more files, but can miss real boundaries or introduce false positives.

## References

- `docs/plans/2026-11-16-detection-another-10x.md` — observed real-media comparisons and fallback rationale (checked 2026-11-16).
- `src-tauri/src/detection.rs`, `src-tauri/src/detection_candidates.rs`, and `src-tauri/tests/detection_integration.rs` — scanner behavior and regressions (checked 2026-11-16).
- `README.md` — detection controls, supported behavior, and limitations (checked 2026-11-16).
