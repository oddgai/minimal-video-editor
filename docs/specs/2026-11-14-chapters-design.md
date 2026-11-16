# Chapter editing

The user replaces marker-based cutting with chapters and specifies new shortcuts. Detailed design remains delegated; keep pnpm, Tauri, the corrected player, and FFmpeg stream copy.

- Start with one selected chapter spanning the source. Split only at an interior retained keyframe; select the new right-hand chapter. Source ends, existing boundaries, and deleted gaps cannot split.
- Delete the selected chapter from the list and timeline, close its gap, and select the next survivor or preceding survivor when deleting the last. Splits, deletion and audio offset support undo/redo including selection. Chapter selection alone does not create history.
- Keep chapter boundaries in source seconds for lossless export. Derive compact timeline spans. Display edited time, map scrubbing back to source time, skip deleted intervals in preview, and stop at the last retained end. Clicking/selecting a chapter seeks to its start. Navigation preserves playing/paused state (updated at the user's request); seeking to the edited end stops playback. Empty edits remain undoable and cannot play/split/export.
- Only retained keyframes appear and participate in navigation. Left/right clamp at retained keyframe bounds. Split requires being on an actual keyframe, without silently moving the cut.
- Replace marker/frame-step controls with split, previous/next chapter, delete, and selected chapter details. Highlight selection in timeline and sidebar.
- Command/Ctrl+S export, Left/Right keyframes, S split, Command/Ctrl+Left/Right chapters, D delete. Remove old export, frame-step, 5-second, source-bound and marker shortcuts. Keep Open, Space, undo/redo, audio offset keys, and help. Preserve input editing and focused-button activation; export commits an active input.
- Chapters mean editing segments, not embedded container chapter metadata. Export surviving source ranges in order.

Approach: store source ranges and derive the compact timeline. Rewriting ranges into edited times would lose FFmpeg source coordinates; retaining deleted gaps would conflict with the requested timeline behavior.
