use media::Tools;
use minimal_video_editor::{
    detection::{detect_chapters, DetectionCache, DetectionControl},
    media,
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn fixture(dir: &Path, graph: &str, offset: bool) -> PathBuf {
    let path = dir.join("fade 映像's source.mkv");
    let mut cmd = Command::new(&Tools::discover().unwrap().ffmpeg);
    cmd.args([
        "-v", "error", "-f", "lavfi", "-i", graph, "-an", "-c:v", "ffv1", "-threads", "2",
    ]);
    if offset {
        cmd.args(["-output_ts_offset", "10"]);
    }
    let out = cmd.arg(&path).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    path
}

fn both_fades() -> &'static str {
    "testsrc2=s=160x90:r=25:d=3,fade=t=out:st=1.6:d=0.8:color=black[a];smptebars=s=160x90:r=25:d=3,fade=t=in:st=0:d=0.8:color=black,fade=t=out:st=1.6:d=0.8:color=white[b];testsrc2=s=160x90:r=25:d=3,fade=t=in:st=0:d=0.8:color=white[c];[a][b][c]concat=n=3:v=1:a=0"
}

// Returning both ends of a blackout, failing to detect white, or not snapping
// to supplied keyframes must each break this test.
#[test]
fn groups_black_and_white_fades_and_snaps_each_transition_once() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), false);
    let before = std::fs::read(&input).unwrap();
    let last = Cell::new(0.0_f64);
    let result = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[9.0, 5.75, 0.0, 2.75, 2.75, f64::NAN],
        &DetectionControl::default(),
        |p| {
            assert!(p.fraction.is_finite() && p.fraction >= last.get() && p.fraction <= 1.0);
            assert!(!p.stage.is_empty());
            last.set(p.fraction);
        },
    )
    .unwrap();
    assert_eq!(result, vec![2.75, 5.75]);
    assert_eq!(last.get(), 1.0);
    assert_eq!(std::fs::read(&input).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn unchanged_video_reuses_completed_detection_without_starting_tools() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), false);
    let keys = [0.0, 2.75, 5.75, 9.0];
    let mut cache = DetectionCache::default();
    let expected = cache
        .detect(
            &Tools::discover().unwrap(),
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {},
        )
        .unwrap();
    assert_eq!(expected, vec![2.75, 5.75]);
    let unavailable = Tools {
        ffmpeg: dir.path().join("missing-ffmpeg"),
        ffprobe: dir.path().join("missing-ffprobe"),
    };
    let fraction = Cell::new(0.0);
    assert_eq!(
        cache
            .detect(
                &unavailable,
                &input,
                &keys,
                &DetectionControl::default(),
                |p| fraction.set(p.fraction)
            )
            .unwrap(),
        expected
    );
    assert_eq!(fraction.get(), 1.0);
    let cancelled = DetectionControl::default();
    cancelled.cancel();
    assert!(cache
        .detect(&unavailable, &input, &keys, &cancelled, |_| {})
        .unwrap_err()
        .contains("cancel"));
    let cancelled_on_hit = DetectionControl::default();
    assert!(cache
        .detect(&unavailable, &input, &keys, &cancelled_on_hit, |_| {
            cancelled_on_hit.cancel()
        })
        .unwrap_err()
        .contains("cancel"));
    cache.clear();
    assert!(cache
        .detect(
            &unavailable,
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
}

#[test]
fn changed_source_or_keyframes_invalidate_cache_and_failures_do_not_populate_it() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), false);
    let keys = [0.0, 2.75, 5.75, 9.0];
    let unavailable = Tools {
        ffmpeg: dir.path().join("missing-ffmpeg"),
        ffprobe: dir.path().join("missing-ffprobe"),
    };
    let mut cache = DetectionCache::default();
    assert!(cache
        .detect(
            &unavailable,
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
    assert_eq!(
        cache
            .detect(
                &Tools::discover().unwrap(),
                &input,
                &keys,
                &DetectionControl::default(),
                |_| {}
            )
            .unwrap(),
        vec![2.75, 5.75]
    );
    assert!(cache
        .detect(
            &unavailable,
            &input,
            &[0.0, 3.0, 6.0, 9.0],
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
    let modified = std::fs::metadata(&input).unwrap().modified().unwrap();
    let copy = dir.path().join("another-source.mkv");
    std::fs::copy(&input, &copy).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&copy)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(cache
        .detect(
            &unavailable,
            &copy,
            &keys,
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&input)
        .unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(modified + Duration::from_secs(5)))
        .unwrap();
    assert!(cache
        .detect(
            &unavailable,
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
    file.set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&input)
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert!(cache
        .detect(
            &unavailable,
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {}
        )
        .is_err());
}

#[test]
fn excludes_leading_black_and_trailing_white_without_content_on_both_sides() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), "testsrc2=s=160x90:r=25:d=6,fade=t=in:st=0:d=0.8:color=black,fade=t=out:st=4.8:d=0.8:color=white", false);
    let last = Cell::new(0.0);
    let result = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        &DetectionControl::default(),
        |p| last.set(p.fraction),
    )
    .unwrap();
    assert!(result.is_empty(), "edge fades returned {result:?}");
    assert_eq!(last.get(), 1.0);
}

#[test]
fn detects_a_hard_cut_to_black_between_visible_scenes() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), "testsrc2=s=160x90:r=25:d=2[a];color=c=black:s=160x90:r=25:d=1[b];smptebars=s=160x90:r=25:d=3[c];[a][b][c]concat=n=3:v=1:a=0", false);
    let result = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        &DetectionControl::default(),
        |_| {},
    )
    .unwrap();

    assert_eq!(result, vec![2.0]);
}

#[test]
fn ignores_normal_scene_changes_and_steady_nearly_black_or_white_content() {
    for graph in [
        "testsrc2=s=160x90:r=25:d=6",
        "color=c=0x050505:s=160x90:r=25:d=6",
        "color=c=0xfafafa:s=160x90:r=25:d=6",
        "testsrc2=s=160x90:r=25:d=3,fade=t=out:st=1.6:d=0.8:color=0x202020[a];smptebars=s=160x90:r=25:d=3,fade=t=in:st=0:d=0.8:color=0x202020[b];[a][b]concat=n=2:v=1:a=0",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = fixture(dir.path(), graph, false);
        let result = detect_chapters(&Tools::discover().unwrap(), &input,
            &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &DetectionControl::default(), |_| {}).unwrap();
        assert!(result.is_empty(), "non-fade content returned {result:?}: {graph}");
    }
}

#[test]
fn normalizes_nonzero_source_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), true);
    let result = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[0.0, 2.75, 5.75, 9.0],
        &DetectionControl::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(result, vec![2.75, 5.75]);
}

#[test]
fn cancellation_before_scan_returns_no_partial_result() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), false);
    let control = DetectionControl::default();
    assert!(control.same_job(&control.clone()));
    assert!(!control.same_job(&DetectionControl::default()));
    control.cancel();
    let error = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[0.0, 3.0, 6.0],
        &control,
        |_| panic!("cancelled scan reported progress"),
    )
    .unwrap_err();
    assert!(error.contains("cancel"), "{error}");
}

#[test]
fn cancellation_during_decode_reaps_process_and_leaves_only_source() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), "testsrc2=s=160x90:r=25:d=30", false);
    let control = DetectionControl::default();
    let observed_frames = Cell::new(false);
    let start = Instant::now();
    let error = detect_chapters(
        &Tools::discover().unwrap(),
        &input,
        &[0.0, 3.0, 6.0],
        &control,
        |p| {
            if p.fraction > 0.01 {
                observed_frames.set(true);
                control.cancel();
            }
        },
    )
    .unwrap_err();
    assert!(observed_frames.get());
    assert!(error.contains("cancel"), "{error}");
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn sparse_keyframes_deduplicate_snaps_and_never_return_source_endpoints() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), both_fades(), false);
    for (keys, expected) in [
        (vec![0.0, 4.4, 9.0], vec![4.4]),
        (vec![0.0, 9.0], vec![]),
        (vec![], vec![]),
    ] {
        let result = detect_chapters(
            &Tools::discover().unwrap(),
            &input,
            &keys,
            &DetectionControl::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(result, expected);
    }
}
