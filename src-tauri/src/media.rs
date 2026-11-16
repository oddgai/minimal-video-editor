use serde::Serialize;
use serde_json::Value;
use std::{
    env,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone, Debug)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Tools {
    pub fn discover() -> Result<Self, String> {
        Ok(Self {
            ffmpeg: find_tool("ffmpeg", "MVE_FFMPEG")?,
            ffprobe: find_tool("ffprobe", "MVE_FFPROBE")?,
        })
    }
}

fn find_tool(name: &str, variable: &str) -> Result<PathBuf, String> {
    if let Some(value) = env::var_os(variable) {
        let path = PathBuf::from(value);
        return path
            .is_file()
            .then_some(path)
            .ok_or_else(|| format!("The executable specified by {variable} could not be found."));
    }
    let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
    let mut dirs = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
            dirs.push(parent.join("../Resources"));
        }
    }
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    if let Some(paths) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&paths));
    }
    dirs.into_iter().map(|dir| dir.join(&filename)).find(|path| path.is_file()).ok_or_else(|| {
        format!("{name} was not found. On macOS, run brew install ffmpeg and restart the app. On Windows, add the FFmpeg bin folder to PATH.")
    })
}

pub fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.stdin(Stdio::null());
    command
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub duration: f64,
    pub start_time: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub audio_start: f64,
    pub size: u64,
}

pub fn probe_media(tools: &Tools, path: &Path) -> Result<MediaInfo, String> {
    if !path.is_file() {
        return Err("The video file could not be found.".into());
    }
    let output = command(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Cannot start ffprobe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cannot analyze video: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    parse_metadata(&json, path)
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}

fn parse_metadata(json: &Value, path: &Path) -> Result<MediaInfo, String> {
    let streams = json["streams"]
        .as_array()
        .ok_or("The file does not contain a video stream.")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
        .ok_or("The file does not contain a video stream.")?;
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let duration = number(&json["format"]["duration"])
        .or_else(|| number(&video["duration"]))
        .filter(|d| *d > 0.0)
        .ok_or("Cannot read the video duration.")?;
    let start_time = number(&json["format"]["start_time"])
        .or_else(|| number(&video["start_time"]))
        .unwrap_or(0.0);
    let rate = video["avg_frame_rate"].as_str().unwrap_or("0/1");
    let fps = rate
        .split_once('/')
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(30.0);
    Ok(MediaInfo {
        path: path.to_string_lossy().into(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        duration,
        start_time,
        width: video["width"].as_u64().unwrap_or(0) as u32,
        height: video["height"].as_u64().unwrap_or(0) as u32,
        fps,
        video_codec: video["codec_name"].as_str().unwrap_or("unknown").into(),
        audio_codec: audio
            .and_then(|a| a["codec_name"].as_str())
            .map(String::from),
        audio_start: audio
            .and_then(|a| number(&a["start_time"]))
            .unwrap_or(start_time)
            - start_time,
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    })
}

#[derive(Clone, Default)]
pub struct ScanControl {
    inner: Arc<ScanInner>,
}

#[derive(Default)]
struct ScanInner {
    cancelled: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl ScanControl {
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.inner.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }
    pub(crate) fn same_job(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
    fn check(&self) -> Result<(), String> {
        if self.inner.cancelled.load(Ordering::SeqCst) {
            Err("Keyframe analysis cancelled.".into())
        } else {
            Ok(())
        }
    }
}

impl Drop for ScanInner {
    fn drop(&mut self) {
        if let Ok(slot) = self.child.get_mut() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub fn scan_keyframes(tools: &Tools, path: &Path) -> Result<Vec<f64>, String> {
    scan_keyframes_with_control(tools, path, &ScanControl::default())
}

pub fn scan_keyframes_with_control(
    tools: &Tools,
    path: &Path,
    control: &ScanControl,
) -> Result<Vec<f64>, String> {
    control.check()?;
    let info = probe_media(tools, path)?;
    control.check()?;
    let mut child = command(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "packet=pts_time,flags",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Cannot start keyframe analysis.")?;
    {
        let mut slot = control.inner.child.lock().map_err(|e| e.to_string())?;
        if control.inner.cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return control.check().map(|_| vec![]);
        }
        *slot = Some(child);
    }
    let mut keys = vec![0.0];
    let read_result = (|| -> Result<(), String> {
        for line in BufReader::new(stdout).lines() {
            control.check()?;
            let line = line.map_err(|e| e.to_string())?;
            let mut fields = line.split(',');
            if let (Some(time), Some(flags)) = (fields.next(), fields.next()) {
                if flags.contains('K') {
                    if let Ok(raw) = time.parse::<f64>() {
                        let t = raw - info.start_time;
                        if t.is_finite() && t > 0.0001 && t < info.duration {
                            keys.push(t);
                        }
                    }
                }
            }
        }
        Ok(())
    })();
    if read_result.is_err() {
        control.cancel();
    }
    let status = control
        .inner
        .child
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("The keyframe analysis process has ended.")?
        .wait()
        .map_err(|e| e.to_string())?;
    control.check()?;
    read_result?;
    if !status.success() {
        return Err("Keyframe analysis failed.".into());
    }
    keys.sort_by(f64::total_cmp);
    keys.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_nonzero_timestamps_and_ignores_cover_art() {
        let json = serde_json::json!({"format":{"duration":"5.5","start_time":"10"},"streams":[
            {"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}},
            {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},
            {"codec_type":"audio","codec_name":"aac","start_time":"10.25"}
        ]});
        let m = parse_metadata(&json, Path::new("test.mp4")).unwrap();
        assert_eq!(m.video_codec, "h264");
        assert_eq!(m.audio_start, 0.25);
        assert!((m.fps - 29.97002997).abs() < 0.0001);
    }
}
