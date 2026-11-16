//! Native WKWebView/WebView2 playback regression probe, separate from the app.
//! MVE_PLAYBACK_FILE=/path/to/video.mp4 cargo run --example playback_probe
//! Uses the production UI/player and loopback streaming; only file dialogs and
//! keyframe analysis are replaced so the probe can run without interaction.
use serde_json::{json, Value};
use std::{path::PathBuf, process::Command, sync::Mutex};
use tauri::{webview::PageLoadEvent, Manager};
#[path = "../src/preview.rs"]
mod preview;

struct ProbeState {
    path: PathBuf,
    result: Mutex<Option<bool>>,
    preview: preview::PreviewServer,
}

#[tauri::command]
fn open_media(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProbeState>,
) -> Result<Option<Value>, String> {
    app.asset_protocol_scope()
        .allow_file(&state.path)
        .map_err(|e| e.to_string())?;
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(&state.path)
        .output()
        .map_err(|e| e.to_string())?;
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let streams = metadata["streams"].as_array().ok_or("missing streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("missing video")?;
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let rate = video["avg_frame_rate"]
        .as_str()
        .unwrap_or("30/1")
        .split_once('/')
        .unwrap();
    Ok(Some(json!({
        "path": state.path, "name": "Native playback probe", "duration": metadata["format"]["duration"].as_str().unwrap().parse::<f64>().unwrap(),
        "startTime": 0, "width": video["width"], "height": video["height"], "fps": rate.0.parse::<f64>().unwrap() / rate.1.parse::<f64>().unwrap(),
        "videoCodec": video["codec_name"], "audioCodec": audio.map(|a| &a["codec_name"]), "audioStart": 0, "size": std::fs::metadata(&state.path).unwrap().len()
        ,"previewUrl": state.preview.select(&state.path).map_err(|e| e.to_string())?
    })))
}

#[tauri::command]
fn analyze_keyframes() -> Vec<f64> {
    if std::env::var("MVE_PROBE_MODE").as_deref() == Ok("reorder") {
        (0..6).map(|index| index as f64 * 2.0).collect()
    } else {
        vec![0.0]
    }
}

#[tauri::command]
fn probe_report(app: tauri::AppHandle, state: tauri::State<'_, ProbeState>, report: Value) {
    println!("PLAYBACK_REPORT {}", report);
    let offset = report["offset"].as_f64().unwrap_or(0.0);
    let seek_mode = std::env::var("MVE_PROBE_MODE").as_deref() == Ok("seek");
    let reorder_mode = std::env::var("MVE_PROBE_MODE").as_deref() == Ok("reorder");
    let reorder_ok = !reorder_mode
        || report["reorderSamples"].as_array().is_some_and(|s| {
            s.len() == 3
                && !s[0]["paused"].as_bool().unwrap_or(true)
                && s[0]["v"].as_f64().unwrap_or(100.0) < 1.0
                && s[1]["paused"].as_bool().unwrap_or(false)
                && s[1]["edited"].as_str() == Some("00:00:12.000")
                && s[1]["v"].as_f64().unwrap_or(0.0) > 7.9
                && s[1]["v"].as_f64().unwrap_or(100.0) < 8.0
                && !s[2]["paused"].as_bool().unwrap_or(true)
                && s[2]["v"].as_f64().unwrap_or(0.0) >= 8.0
        });
    let seeks_ok = !seek_mode
        || report["seekSamples"].as_array().is_some_and(|samples| {
            samples.len() == 6
                && samples.iter().all(|s| {
                    !s["audioPaused"].as_bool().unwrap_or(true)
                        && s["audioAdvance"].as_f64().unwrap_or(0.0) > 0.3
                        && s["v"].as_f64().unwrap_or(0.0)
                            >= s["target"].as_f64().unwrap_or(0.0) - 0.5
                        && (s["v"].as_f64().unwrap_or(0.0)
                            - offset
                            - s["a"].as_f64().unwrap_or(0.0))
                        .abs()
                            < 0.3
                })
        });
    let audio_ok = if offset.abs() < 0.0001 {
        report["audioSource"].is_null()
    } else {
        let video = report["videoTime"].as_f64().unwrap_or(0.0);
        let audio = report["audioTime"].as_f64().unwrap_or(0.0);
        audio > 1.0
            && (video - offset - audio).abs() < 0.15
            && (seek_mode
                || reorder_mode
                || report["events"]["audio"]["seeking"].as_u64().unwrap_or(0) <= 3)
    };
    let success = audio_ok
        && seeks_ok
        && reorder_ok
        && report["videoError"].is_null()
        && report["audioError"].is_null()
        && report["pixelRange"].as_u64().unwrap_or(0) > 0
        && report["videoFrames"].as_u64().unwrap_or(0) > 0
        && report["videoWidth"].as_u64().unwrap_or(0) > 0
        && report["videoTime"].as_f64().unwrap_or(0.0) > 2.0;
    *state.result.lock().unwrap() = Some(success);
    app.exit(if success { 0 } else { 1 });
}

fn main() {
    let path = PathBuf::from(std::env::var_os("MVE_PLAYBACK_FILE").expect("Set MVE_PLAYBACK_FILE"))
        .canonicalize()
        .unwrap();
    let script = format!("window.__MVE_PROBE_MODE__={};window.__MVE_PROBE_URL__={};window.__MVE_PROBE_OFFSET__={};{}", serde_json::to_string(&std::env::var("MVE_PROBE_MODE").unwrap_or_default()).unwrap(), serde_json::to_string(&std::env::var("MVE_PROBE_URL").unwrap_or_default()).unwrap(), std::env::var("MVE_PROBE_OFFSET").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(0.0), include_str!("../../tests/native/playback-probe.js"));
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.show()?;
            window.set_focus()?;
            Ok(())
        })
        .manage(ProbeState {
            path,
            result: Mutex::new(None),
            preview: preview::PreviewServer::start().unwrap(),
        })
        .invoke_handler(tauri::generate_handler![
            open_media,
            analyze_keyframes,
            probe_report
        ])
        .on_page_load(move |webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let _ = webview.eval(&script);
            }
        })
        .run(tauri::generate_context!())
        .expect("native playback probe failed");
}
