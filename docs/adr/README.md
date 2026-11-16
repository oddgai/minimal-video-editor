# Architecture Decision Records

This directory records decisions that shape the architecture and constrain future changes. ADRs explain why a decision was made; they are not implementation plans, progress reports, or substitutes for code and user documentation.

## Index

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-use-tauri-for-the-desktop-app.md) | Use Tauri 2 with a Rust backend and TypeScript frontend | Accepted |
| [0002](0002-use-keyframe-aligned-stream-copy-for-editing.md) | Use keyframe-aligned stream copy for edits and exports | Accepted |
| [0003](0003-stream-preview-over-loopback-http.md) | Preview local media through bounded loopback HTTP streaming | Accepted |
| [0004](0004-preserve-detection-correctness-when-accelerating.md) | Keep chapter detection on demand and preserve full-scan results | Accepted |
| [0005](0005-use-audio-as-the-preview-clock-when-offset.md) | Use corrected audio as the preview clock when offset | Accepted |

## Authoring

- Number ADRs sequentially and name each file `NNNN-short-title.md`.
- Use the sections in [template.md](template.md). Keep each ADR focused on one durable decision.
- Write Context as verifiable project facts. Cite the relevant source code, tests, design documents, or primary external documentation in References, with the date checked.
- State the decision at the policy level. Put changing implementation details, task status, benchmarks, and migration steps in the code, tests, plans, or pull request instead.
- Include meaningful tradeoffs and alternatives. Say what this ADR leaves undecided and where that decision belongs.
- Use `Accepted`, `Proposed`, `Deprecated`, or `Superseded by ADR NNNN` for status. Do not rewrite history when a decision changes; add a replacement ADR and update this index.

These conventions use the concise Context / Decision / Consequences structure and emphasize verifiable evidence, a clear scope, and durable decisions. The guide was informed by [TOKIUM's ADR authoring guideline article](https://zenn.dev/tokium_dev/articles/adr-authoring-guideline) (checked 2026-11-16); this repository uses a small subset suited to a single-app project.
