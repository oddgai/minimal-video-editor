# Faster chapter detection

**Goal:** Reduce first-scan time without lowering fade sensitivity, and avoid re-decoding an unchanged video on repeated scans.

**Evidence:** Read-only 120-second 960×540 H.264 benchmark: CPU 2 threads 1.103 s, CPU 4 threads 0.619 s, VideoToolbox 2.753–2.818 s. Every path produced the same 8,640,000 bytes and SHA-256 sample hash. The original user file is no longer at its supplied path, so these are synthetic fixture measurements, not promised timings for that file.

**Approach:** Cap CPU decode parallelism at four available cores; retain 5 fps / 160×90 samples and the exact fade algorithm. Cache one successful result, keyed by canonical source path, file size/modified timestamp and keyframes. Clear on closing media; never cache a failed or partial scan. Cancel must work on cache hits too.

- [x] Benchmark the entire Rust detector in debug and release before/after, with reproducible CLI example `detection_bench`.
- [x] Add regression tests for repeated scan cache hits, source/path/mtime/keyframe changes, clear, cancellation (including cancellation in cache-hit callback) and failed scans; observed missing-cache API failure before implementation.
- [x] Increase bounded decode parallelism, add cache, and integrate with native detection/session lifecycle.
- [x] Confirm unchanged samples/results. Three-run release medians: 1.027791 s → 0.574855 s (1.7879×); debug 1.019045 s → 0.576673 s. Repeated cached lookup median 0.0000165 s inside the detector, excluding app UI/IPC overhead. Same synthetic 120-second 960×540 H.264 input, unchanged boundaries; exact sample SHA-256 already matched across thread counts.
- [x] Independent read-only review `detection_speed_review`: no Critical/Important findings. Size/mtime fingerprint cannot identify an external rewrite preserving both metadata values; normal file changes and closes invalidate reuse.
- [x] Full Rust suite: 26 tests passing, including 9 detection/cache tests; 20 Node tests, typecheck, fmt and clippy pass. Frontend UI code was not changed.
- [x] macOS release build succeeds; app bundle 4.62 MiB.
