//! Read-only end-to-end benchmark. Set MVE_DETECTION_BENCH_FILE to a local video.
use minimal_video_editor::{
    detection::{DetectionCache, DetectionControl},
    media::{scan_keyframes, Tools},
};
use std::{path::PathBuf, time::Instant};

fn main() {
    let input = PathBuf::from(
        std::env::var_os("MVE_DETECTION_BENCH_FILE").expect("Set MVE_DETECTION_BENCH_FILE"),
    );
    let tools = Tools::discover().expect("FFmpeg is required");
    let keys = scan_keyframes(&tools, &input).expect("Read keyframes");
    let precise = std::env::var("MVE_DETECTION_BENCH_PRECISE").is_ok_and(|value| value == "1");
    let start = Instant::now();
    let mut cache = DetectionCache::default();
    let boundaries = cache
        .detect_with_mode(
            &tools,
            &input,
            &keys,
            precise,
            &DetectionControl::default(),
            |_| {},
        )
        .expect("Detect chapters");
    let seconds = start.elapsed().as_secs_f64();
    let repeat = Instant::now();
    let cached = cache
        .detect_with_mode(
            &tools,
            &input,
            &keys,
            precise,
            &DetectionControl::default(),
            |_| {},
        )
        .expect("Repeat detection");
    assert_eq!(boundaries, cached);
    println!(
        "{}",
        serde_json::json!({"seconds": seconds, "cachedSeconds": repeat.elapsed().as_secs_f64(), "precise": precise, "boundaries": boundaries})
    );
}
