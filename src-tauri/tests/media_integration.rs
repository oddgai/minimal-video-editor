use minimal_video_editor::media::{scan_keyframes_with_control, ScanControl};
use minimal_video_editor::{
    export::{export_media, ExportControl, ExportRequest, Range},
    media::{probe_media, scan_keyframes, Tools},
};
use std::{
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn tools() -> Tools {
    Tools::discover().expect("Install FFmpeg and ffprobe to run media integration tests")
}

fn fixture(dir: &Path, silent: bool) -> std::path::PathBuf {
    let path = dir.join("映像 sample's source.mp4");
    let mut command = Command::new(&tools().ffmpeg);
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x90:rate=24",
    ]);
    if !silent {
        command.args(["-f", "lavfi", "-i", "sine=frequency=880:sample_rate=48000"]);
    }
    command.args([
        "-t",
        "6",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-g",
        "48",
        "-bf",
        "0",
        "-pix_fmt",
        "yuv420p",
    ]);
    if !silent {
        command.args(["-c:a", "aac"]);
    }
    assert!(command.arg(&path).status().unwrap().success());
    path
}

fn request(input: &Path, output: &Path, offset: f64) -> ExportRequest {
    ExportRequest {
        input: input.to_string_lossy().into(),
        output: output.to_string_lossy().into(),
        ranges: vec![
            Range {
                start: 0.0,
                end: 2.0,
            },
            Range {
                start: 4.0,
                end: 6.0,
            },
        ],
        audio_offset: offset,
    }
}

fn packet_hashes(path: &Path, stream: &str) -> Vec<String> {
    let output = Command::new(&tools().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            stream,
            "-show_packets",
            "-show_data_hash",
            "sha256",
            "-show_entries",
            "packet=data_hash",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["data_hash"].as_str().unwrap().into())
        .collect()
}

fn stream_start(path: &Path, stream: &str) -> f64 {
    let output = Command::new(&tools().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            stream,
            "-show_entries",
            "stream=start_time",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn probes_video_and_scans_keyframes_without_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let info = probe_media(&tools(), &input).unwrap();
    assert!((info.duration - 6.0).abs() < 0.1);
    assert_eq!(info.width, 160);
    assert!(info.audio_codec.is_some());
    let keys = scan_keyframes(&tools(), &input).unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys
        .iter()
        .zip([0.0, 2.0, 4.0])
        .all(|(actual, expected)| (actual - expected).abs() < 0.01));
}

#[test]
fn removes_middle_range_with_identical_encoded_video_packets() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let output = dir.path().join("編集's result.mp4");
    export_media(
        &tools(),
        &request(&input, &output, 0.0),
        &ExportControl::default(),
        |_| {},
    )
    .unwrap();
    let info = probe_media(&tools(), &output).unwrap();
    assert!(
        (info.duration - 4.0).abs() < 0.15,
        "duration {}",
        info.duration
    );
    assert_eq!(info.video_codec, "h264");
    assert_eq!(info.audio_codec.as_deref(), Some("aac"));
    let source = packet_hashes(&input, "v:0");
    let result = packet_hashes(&output, "v:0");
    assert_eq!(result.len(), 96);
    assert_eq!(&result[..48], &source[..48]);
    assert_eq!(&result[48..], &source[96..]);
    let audio = packet_hashes(&input, "a:0");
    assert!(packet_hashes(&output, "a:0")
        .iter()
        .all(|hash| audio.contains(hash)));
}

#[test]
fn exports_ranges_in_edited_order_without_reencoding() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let output = dir.path().join("reordered.mp4");
    let mut r = request(&input, &output, 0.0);
    r.ranges = vec![
        Range {
            start: 4.0,
            end: 6.0,
        },
        Range {
            start: 0.0,
            end: 2.0,
        },
    ];
    export_media(&tools(), &r, &ExportControl::default(), |_| {}).unwrap();
    let source = packet_hashes(&input, "v:0");
    let expected: Vec<_> = source[96..].iter().chain(&source[..48]).cloned().collect();
    assert_eq!(packet_hashes(&output, "v:0"), expected);
    assert_eq!(
        decoded_hashes(&output),
        decoded_hashes(&input)[96..]
            .iter()
            .chain(&decoded_hashes(&input)[..48])
            .cloned()
            .collect::<Vec<_>>()
    );
}

#[test]
fn offsets_audio_both_directions_without_reencoding() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    for offset in [0.25, -0.25] {
        let output = dir.path().join(format!("offset-{offset}.mp4"));
        let mut r = request(&input, &output, offset);
        r.ranges = vec![Range {
            start: 0.0,
            end: 6.0,
        }];
        export_media(&tools(), &r, &ExportControl::default(), |_| {}).unwrap();
        let difference = stream_start(&output, "a:0") - stream_start(&output, "v:0");
        assert!(
            (difference - offset.max(0.0)).abs() < 0.05,
            "offset {offset}, difference {difference}"
        );
        assert_eq!(packet_hashes(&input, "v:0"), packet_hashes(&output, "v:0"));
        let audio = packet_hashes(&input, "a:0");
        assert!(packet_hashes(&output, "a:0")
            .iter()
            .all(|hash| audio.contains(hash)));
    }
}

fn audio_packets(path: &Path) -> Vec<(f64, f64, String)> {
    let output = Command::new(&tools().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_packets",
            "-show_data_hash",
            "sha256",
            "-show_entries",
            "packet=pts_time,duration_time,data_hash",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    json["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["pts_time"].as_str().unwrap().parse().unwrap(),
                p["duration_time"].as_str().unwrap().parse().unwrap(),
                p["data_hash"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn corrected_audio_comes_from_original_source_before_single_and_multiple_cuts() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("changing-audio.mp4");
    assert!(Command::new(&tools().ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=24",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.2*sin(2*PI*(220*t+40*t*t)):s=48000",
            "-t",
            "32",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "48",
            "-bf",
            "2",
            "-c:a",
            "aac",
            "-output_ts_offset",
            "10",
        ])
        .arg(&input)
        .status()
        .unwrap()
        .success());
    let info = probe_media(&tools(), &input).unwrap();
    let first_key = stream_start(&input, "v:0") - info.start_time;
    let original_audio: Vec<_> = audio_packets(&input)
        .into_iter()
        .map(|(pts, duration, hash)| (pts - info.start_time, duration, hash))
        .collect();
    let original_video = packet_hashes(&input, "v:0");
    for offset in [-12.0, 12.0] {
        for split in [false, true] {
            let output = dir.path().join(format!("offset-{offset}-{split}.mp4"));
            let ranges = if split {
                vec![
                    Range {
                        start: first_key + 12.0,
                        end: first_key + 14.0,
                    },
                    Range {
                        start: first_key + 16.0,
                        end: first_key + 18.0,
                    },
                ]
            } else {
                vec![Range {
                    start: first_key + 12.0,
                    end: first_key + 16.0,
                }]
            };
            let request = ExportRequest {
                input: input.to_string_lossy().into(),
                output: output.to_string_lossy().into(),
                ranges: ranges.clone(),
                audio_offset: offset,
            };
            export_media(&tools(), &request, &ExportControl::default(), |_| {}).unwrap();
            let audio = audio_packets(&output);
            let label = format!("offset {offset}, split {split}");
            assert!(!audio.is_empty(), "missing audio: {label}");
            let origin = stream_start(&output, "v:0");
            let end = audio.iter().map(|p| p.0 + p.1).fold(0.0_f64, f64::max);
            assert!((audio[0].0 - origin).abs() < 0.05, "silent start: {label}");
            assert!(
                (end - origin - 4.0).abs() < 0.05,
                "silent end {end}, video origin {origin}: {label}"
            );
            let mut expected_video = Vec::new();
            let mut timeline_start = 0.0;
            for range in &ranges {
                expected_video.extend_from_slice(
                    &original_video[((range.start - first_key) * 24.0).round() as usize
                        ..((range.end - first_key) * 24.0).round() as usize],
                );
                for packet in original_audio.iter().filter(|p| {
                    p.0 > range.start - offset + 0.05 && p.0 < range.end - offset - 0.05
                }) {
                    let mapped = audio
                        .iter()
                        .find(|p| p.2 == packet.2)
                        .unwrap_or_else(|| panic!("wrong source audio at {}: {label}", packet.0));
                    let expected = timeline_start + packet.0 - (range.start - offset);
                    assert!(
                        (mapped.0 - origin - expected).abs() < 0.05,
                        "wrong audio placement: {label}, expected {expected}, actual {}",
                        mapped.0 - origin
                    );
                }
                timeline_start += range.end - range.start;
            }
            assert_eq!(
                packet_hashes(&output, "v:0"),
                expected_video,
                "changed video: {label}"
            );
        }
    }
}

#[test]
fn correction_keeps_available_audio_when_a_whole_edge_chapter_has_no_source_audio() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    for offset in [3.0, -3.0] {
        for extension in ["mp4", "mov", "mkv"] {
            let output = dir.path().join(format!("edge-{offset}.{extension}"));
            export_media(
                &tools(),
                &request(&input, &output, offset),
                &ExportControl::default(),
                |_| {},
            )
            .unwrap();
            assert_eq!(packet_hashes(&output, "v:0").len(), 96);
            let audio = audio_packets(&output);
            assert!(
                !audio.is_empty(),
                "lost available audio for offset {offset}"
            );
            let origin = stream_start(&output, "v:0");
            let start = if offset > 0.0 { 2.0 } else { 0.0 };
            let end = if offset > 0.0 { 4.0 } else { 2.0 };
            assert!(
                (audio[0].0 - origin - start).abs() < 0.05,
                "wrong audio start for offset {offset}: {}",
                audio[0].0 - origin
            );
            let actual_end = audio.iter().map(|p| p.0 + p.1).fold(0.0_f64, f64::max) - origin;
            assert!(
                (actual_end - end).abs() < 0.05,
                "wrong audio end for offset {offset}: {actual_end}"
            );
        }
    }
}

#[test]
fn supports_silent_video_and_refuses_overwrites_and_non_keyframe_starts() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), true);
    let output = dir.path().join("silent.mp4");
    let r = request(&input, &output, 0.0);
    let control = ExportControl::default();
    export_media(&tools(), &r, &control, |_| {}).unwrap();
    assert!(probe_media(&tools(), &output)
        .unwrap()
        .audio_codec
        .is_none());
    let before = std::fs::read(&output).unwrap();
    assert!(export_media(&tools(), &r, &control, |_| {}).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), before);
    let mut bad = request(&input, &input, 0.0);
    assert!(export_media(&tools(), &bad, &control, |_| {}).is_err());
    bad.output = dir.path().join("bad.mp4").to_string_lossy().into();
    bad.ranges[0].start = 0.5;
    assert!(export_media(&tools(), &bad, &control, |_| {}).is_err());
    bad.ranges[0].start = f64::NAN;
    assert!(export_media(&tools(), &bad, &control, |_| {}).is_err());
}

#[test]
fn cancellation_never_publishes_a_partial_destination() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let output = dir.path().join("cancelled.mp4");
    let control = ExportControl::default();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let result = export_media(&tools(), &request(&input, &output, 0.0), &control, |p| {
        if p.fraction > 0.0 {
            control.cancel();
            flag.store(true, Ordering::Relaxed);
        }
    });
    assert!(cancelled.load(Ordering::Relaxed));
    assert!(result.is_err());
    assert!(!output.exists());
}

#[test]
fn b_frames_and_nonzero_source_timestamps_keep_the_intended_frames() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("b-frames.mp4");
    assert!(Command::new(&tools().ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=24",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-g",
            "48",
            "-keyint_min",
            "48",
            "-sc_threshold",
            "0",
            "-bf",
            "2",
            "-output_ts_offset",
            "10"
        ])
        .arg(&input)
        .status()
        .unwrap()
        .success());
    let info = probe_media(&tools(), &input).unwrap();
    assert!((info.start_time - 10.0).abs() < 0.01);
    let keys = scan_keyframes(&tools(), &input).unwrap();
    assert!(keys
        .iter()
        .zip([0.0, 2.0, 4.0])
        .all(|(a, b)| (a - b).abs() < 0.01));
    let output = dir.path().join("b-result.mp4");
    export_media(
        &tools(),
        &request(&input, &output, 0.0),
        &ExportControl::default(),
        |_| {},
    )
    .unwrap();
    let source = decoded_hashes(&input);
    let result = decoded_hashes(&output);
    let expected: Vec<_> = source[..48].iter().chain(&source[96..]).cloned().collect();
    assert_eq!(result.len(), 96, "wrong number of visible frames");
    assert_eq!(result, expected);
}

fn decoded_hashes(path: &Path) -> Vec<String> {
    let output = Command::new(&tools().ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:v:0",
            "-fps_mode",
            "passthrough",
            "-f",
            "framemd5",
            "-",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.rsplit(',').next().unwrap().trim().into())
        .collect()
}

#[test]
fn cancellation_kills_an_active_copy_and_cleans_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let output = dir.path().join("active-cancel.mp4");
    let control = ExportControl::default();
    let result = export_media(&tools(), &request(&input, &output, 0.0), &control, |p| {
        if p.fraction > 0.01 {
            control.cancel();
        }
    });
    assert!(result.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn cancelled_keyframe_scan_exits_without_returning_partial_results() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(dir.path(), false);
    let control = ScanControl::default();
    control.cancel();
    let result = scan_keyframes_with_control(&tools(), &input, &control);
    assert!(result.unwrap_err().contains("cancel"));
}

#[test]
fn b_frame_segments_have_continuous_presentation_timestamps_at_the_join() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("b-audio.mp4");
    assert!(Command::new(&tools().ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-g",
            "48",
            "-keyint_min",
            "48",
            "-sc_threshold",
            "0",
            "-bf",
            "2",
            "-c:a",
            "aac"
        ])
        .arg(&input)
        .status()
        .unwrap()
        .success());
    let output = dir.path().join("b-audio-result.mp4");
    export_media(
        &tools(),
        &request(&input, &output, 0.0),
        &ExportControl::default(),
        |_| {},
    )
    .unwrap();
    let packets = Command::new(&tools().ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "packet=pts_time",
            "-of",
            "json",
        ])
        .arg(&output)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&packets.stdout).unwrap();
    let times: Vec<f64> = json["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["pts_time"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(
        (times[48] - times[0] - 2.0).abs() < 0.001,
        "join is {} seconds instead of 2",
        times[48] - times[0]
    );
    assert_eq!(decoded_hashes(&output).len(), 96);
}
