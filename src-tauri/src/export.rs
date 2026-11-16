use crate::media::{command, probe_media, scan_keyframes, Tools};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Range {
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub input: String,
    pub output: String,
    pub ranges: Vec<Range>,
    pub audio_offset: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub fraction: f64,
    pub stage: String,
}

#[derive(Clone, Default)]
pub struct ExportControl {
    inner: Arc<ControlInner>,
}

#[derive(Default)]
struct ControlInner {
    cancelled: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl ExportControl {
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.inner.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }
    fn check(&self) -> Result<(), String> {
        if self.inner.cancelled.load(Ordering::SeqCst) {
            Err("Export cancelled.".into())
        } else {
            Ok(())
        }
    }
}

fn validate(request: &ExportRequest, duration: f64, keys: &[f64]) -> Result<(), String> {
    if request.ranges.is_empty() || request.ranges.len() > 1000 {
        return Err("Choose between 1 and 1000 ranges to export.".into());
    }
    if !request.audio_offset.is_finite() || request.audio_offset.abs() > 3600.0 {
        return Err("Audio offset must be between -3600 and 3600 seconds.".into());
    }
    for range in &request.ranges {
        if !range.start.is_finite()
            || !range.end.is_finite()
            || range.start < 0.0
            || range.end > duration + 0.001
            || range.end <= range.start
        {
            return Err("The export ranges contain invalid times.".into());
        }
        if !keys.iter().any(|k| (k - range.start).abs() < 0.002)
            || ((range.end - duration).abs() > 0.002
                && !keys.iter().any(|k| (k - range.end).abs() < 0.002))
        {
            return Err("Range boundaries must be keyframes or the end of the source.".into());
        }
    }
    let mut source_order: Vec<_> = request.ranges.iter().collect();
    source_order.sort_by(|a, b| a.start.total_cmp(&b.start));
    if source_order
        .windows(2)
        .any(|pair| pair[1].start < pair[0].end - 0.0001)
    {
        return Err("The export ranges overlap in the source video.".into());
    }
    Ok(())
}

pub fn export_media(
    tools: &Tools,
    request: &ExportRequest,
    control: &ExportControl,
    progress: impl Fn(Progress),
) -> Result<(), String> {
    export_with_keyframes(tools, request, control, None, progress)
}

pub fn export_with_keyframes(
    tools: &Tools,
    request: &ExportRequest,
    control: &ExportControl,
    cached_keys: Option<&[f64]>,
    progress: impl Fn(Progress),
) -> Result<(), String> {
    control.check()?;
    let input = Path::new(&request.input);
    let output = Path::new(&request.output);
    if output.exists() {
        return Err("A file already exists at the destination. Choose a different name.".into());
    }
    let parent = output
        .parent()
        .filter(|p| p.is_dir())
        .ok_or("The destination folder could not be found.")?;
    let extension = output
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["mp4", "mov", "mkv", "webm", "m4v"].contains(&extension.as_str()) {
        return Err("Choose MP4, MOV, MKV or WebM as the output format.".into());
    }
    let info = probe_media(tools, input)?;
    let correct_audio = info.audio_codec.is_some() && request.audio_offset.abs() > 0.00001;
    let scanned;
    let keys = if let Some(keys) = cached_keys {
        keys
    } else {
        scanned = scan_keyframes(tools, input)?;
        &scanned
    };
    validate(request, info.duration, keys)?;
    control.check()?;
    let work = tempfile::Builder::new()
        .prefix(".minimal-video-editor-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let mut entries = String::new();
    let count = request.ranges.len();
    let total: f64 = request.ranges.iter().map(|r| r.end - r.start).sum();
    let mut completed = 0.0;
    for (index, range) in request.ranges.iter().enumerate() {
        let filename = format!("part-{index:04}.{extension}");
        let part = work.path().join(&filename);
        let length = range.end - range.start;
        let mut args = vec![
            "-ss".into(),
            decimal(range.start),
            "-i".into(),
            request.input.clone(),
        ];
        if correct_audio {
            // Select audio in source coordinates BEFORE discarding video ranges.
            // At source video time t, the corrected audio is t - offset.
            // When that is before zero, keep only the genuine leading gap.
            args.extend([
                "-ss".into(),
                decimal((range.start - request.audio_offset).max(0.0)),
                "-itsoffset".into(),
                decimal((request.audio_offset - range.start).max(0.0)),
                "-i".into(),
                request.input.clone(),
            ]);
        }
        args.extend([
            "-t".into(),
            decimal(length),
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            if correct_audio {
                "1:a:0?".into()
            } else {
                "0:a:0?".into()
            },
            "-c".into(),
            "copy".into(),
            // -t is evaluated against decode timestamps during stream-copy. B-frames
            // can otherwise leak packets from the following GOP past the end marker.
            // Only drop out-of-range packets; amount=0 leaves their payload untouched.
            "-bsf:v".into(),
            format!("noise=amount=0:drop='gte(pts*tb,{})'", decimal(length)),
            "-avoid_negative_ts".into(),
            // Preserve the same presentation origin in every segment. Shifting
            // each segment by its earliest DTS gives B-frame GOPs different
            // offsets depending on the accompanying audio packet boundary.
            "disabled".into(),
        ]);
        if correct_audio {
            // Input seeking can return audio packets before its requested time.
            // Drop that preroll instead of carrying it across chapter joins.
            args.extend([
                "-bsf:a".into(),
                format!(
                    "noise=amount=0:drop='lt(pts*tb,0)+gte(pts*tb,{})'",
                    decimal(length)
                ),
            ]);
            if ["mp4", "mov", "m4v"].contains(&extension.as_str()) {
                // Declare the audio stream even if this entire chapter has no
                // source audio. Otherwise concat loses audio in later chapters.
                args.extend([
                    "-movflags".into(),
                    "empty_moov+delay_moov+frag_keyframe".into(),
                ]);
            }
        }
        args.push(part.to_string_lossy().into());
        run_ffmpeg(
            tools,
            &args,
            control,
            work.path(),
            length,
            completed / total * 0.7,
            length / total * 0.7,
            &format!("Copying range {}/{}", index + 1, count),
            &progress,
        )?;
        entries.push_str(&format!(
            "file '{filename}'\nduration {}\n",
            decimal(length)
        ));
        completed += length;
    }
    let joined = if count == 1 {
        work.path().join(format!("part-0000.{extension}"))
    } else {
        let list = work.path().join("segments.txt");
        fs::write(&list, entries).map_err(|e| e.to_string())?;
        let joined = work.path().join(format!("joined.{extension}"));
        run_ffmpeg(
            tools,
            &[
                "-f".into(),
                "concat".into(),
                "-safe".into(),
                "1".into(),
                "-auto_convert".into(),
                "0".into(),
                "-i".into(),
                list.to_string_lossy().into(),
                "-map".into(),
                "0".into(),
                "-c".into(),
                "copy".into(),
                joined.to_string_lossy().into(),
            ],
            control,
            work.path(),
            total,
            0.7,
            0.1,
            "Joining ranges",
            &progress,
        )?;
        joined
    };
    let staged = tempfile::Builder::new()
        .prefix("result-")
        .suffix(&format!(".{extension}"))
        .tempfile_in(work.path())
        .map_err(|e| e.to_string())?
        .into_temp_path();
    let mut args = vec!["-i".into(), joined.to_string_lossy().into()];
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a:0?".into(),
    ]);
    args.extend([
        "-c".into(),
        "copy".into(),
        "-avoid_negative_ts".into(),
        "make_zero".into(),
    ]);
    if ["mp4", "mov", "m4v"].contains(&extension.as_str()) {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args.push(staged.to_string_lossy().into());
    run_ffmpeg(
        tools,
        &args,
        control,
        work.path(),
        total,
        0.8,
        0.19,
        "Saving video",
        &progress,
    )?;
    control.check()?;
    staged.persist_noclobber(output).map_err(|e| {
        format!("Cannot save. Check for an existing file and available disk space: {e}")
    })?;
    progress(Progress {
        fraction: 1.0,
        stage: "Export complete".into(),
    });
    Ok(())
}

fn decimal(value: f64) -> String {
    format!("{value:.6}")
}

#[allow(clippy::too_many_arguments)]
fn run_ffmpeg(
    tools: &Tools,
    args: &[String],
    control: &ExportControl,
    work: &Path,
    duration: f64,
    base: f64,
    weight: f64,
    stage: &str,
    progress: &impl Fn(Progress),
) -> Result<(), String> {
    control.check()?;
    progress(Progress {
        fraction: base.max(0.001),
        stage: stage.into(),
    });
    control.check()?;
    let error_path = work.join("ffmpeg-error.txt");
    let errors = fs::File::create(&error_path).map_err(|e| e.to_string())?;
    let mut child = command(&tools.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::from(errors))
        .spawn()
        .map_err(|e| format!("Cannot start FFmpeg: {e}"))?;
    let stdout = child.stdout.take().ok_or("Cannot read FFmpeg progress.")?;
    {
        let mut slot = control.inner.child.lock().map_err(|e| e.to_string())?;
        if control.inner.cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return control.check();
        }
        *slot = Some(child);
    }
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(raw) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = raw.parse::<f64>() {
                progress(Progress {
                    fraction: base + (us / 1_000_000.0 / duration).clamp(0.0, 1.0) * weight,
                    stage: stage.into(),
                });
            }
        }
    }
    let status = control
        .inner
        .child
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Cannot read FFmpeg process status.")?
        .wait()
        .map_err(|e| e.to_string())?;
    control.check()?;
    if !status.success() {
        let error = fs::read_to_string(error_path).unwrap_or_default();
        return Err(format!("Lossless export failed. Try a container compatible with the source codecs, such as MKV.\n{}", error.chars().take(2000).collect::<String>()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overlapping_and_out_of_bounds_ranges() {
        let mut r = ExportRequest {
            input: "in.mp4".into(),
            output: "out.mp4".into(),
            ranges: vec![
                Range {
                    start: 0.0,
                    end: 4.0,
                },
                Range {
                    start: 2.0,
                    end: 6.0,
                },
            ],
            audio_offset: 0.0,
        };
        assert!(validate(&r, 6.0, &[0.0, 2.0, 4.0]).is_err());
        r.ranges = vec![Range {
            start: 0.0,
            end: 7.0,
        }];
        assert!(validate(&r, 6.0, &[0.0]).is_err());
    }
}
