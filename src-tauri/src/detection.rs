//! Explicit, read-only fade analysis. Only small grayscale samples cross the pipe;
//! no decoded video or temporary media is retained.
use crate::media::{command, probe_media, Tools};
use std::{
    collections::{HashMap, VecDeque},
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime},
};

#[path = "detection_candidates.rs"]
mod candidates;

const FPS: f64 = 5.0;
const PIXELS: usize = 160 * 90;
const CONTEXT: usize = 8; // 1.6 seconds on each side of a black/white interval.

#[derive(Clone, Default)]
pub struct DetectionControl {
    inner: Arc<DetectionInner>,
}

#[derive(Default)]
struct DetectionInner {
    cancelled: AtomicBool,
    children: Mutex<HashMap<u64, Child>>,
    next_child: AtomicU64,
}

impl DetectionControl {
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut children) = self.inner.children.lock() {
            for child in children.values_mut() {
                let _ = child.kill();
            }
        }
    }

    pub fn same_job(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    fn check(&self) -> Result<(), String> {
        if self.inner.cancelled.load(Ordering::SeqCst) {
            Err("Chapter detection cancelled.".into())
        } else {
            Ok(())
        }
    }
}

// Also covers early errors and a panicking progress callback.
struct Running<'a> {
    control: &'a DetectionControl,
    child_id: u64,
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Ok(mut children) = self.control.inner.children.lock() {
            if let Some(mut child) = children.remove(&self.child_id) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub struct DetectionProgress {
    pub fraction: f64,
    pub stage: String,
}

#[derive(Clone, PartialEq)]
struct SourceFingerprint {
    path: PathBuf,
    length: u64,
    modified: SystemTime,
}

impl SourceFingerprint {
    fn read(input: &Path) -> Option<Self> {
        let path = input.canonicalize().ok()?;
        let metadata = std::fs::metadata(&path).ok()?;
        Some(Self {
            path,
            length: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

struct CachedDetection {
    source: SourceFingerprint,
    keyframes: Vec<f64>,
    precise: bool,
    boundaries: Vec<f64>,
}

/// Keep one completed scan, never decoded frames or an incomplete result.
#[derive(Default)]
pub struct DetectionCache {
    last: Option<CachedDetection>,
}

impl DetectionCache {
    pub fn clear(&mut self) {
        self.last = None;
    }

    pub fn detect(
        &mut self,
        tools: &Tools,
        input: &Path,
        keyframes: &[f64],
        control: &DetectionControl,
        progress: impl Fn(DetectionProgress),
    ) -> Result<Vec<f64>, String> {
        self.detect_with_mode(tools, input, keyframes, false, control, progress)
    }

    pub fn detect_with_mode(
        &mut self,
        tools: &Tools,
        input: &Path,
        keyframes: &[f64],
        precise: bool,
        control: &DetectionControl,
        progress: impl Fn(DetectionProgress),
    ) -> Result<Vec<f64>, String> {
        control.check()?;
        let source = SourceFingerprint::read(input);
        if let Some(cached) = self.last.as_ref().filter(|cached| {
            source.as_ref() == Some(&cached.source)
                && cached.keyframes == keyframes
                && cached.precise == precise
        }) {
            progress(DetectionProgress {
                fraction: 1.0,
                stage: "Using previous detection result".into(),
            });
            control.check()?;
            return Ok(cached.boundaries.clone());
        }
        let boundaries =
            detect_chapters_with_mode(tools, input, keyframes, precise, control, progress)?;
        control.check()?;
        // External modifications during a scan must not become a valid cache hit.
        if let Some(source) =
            source.filter(|before| SourceFingerprint::read(input).as_ref() == Some(before))
        {
            self.last = Some(CachedDetection {
                source,
                keyframes: keyframes.to_vec(),
                precise,
                boundaries: boundaries.clone(),
            });
        }
        Ok(boundaries)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shade {
    Black,
    White,
}
#[derive(Clone, Copy)]
struct Sample {
    mean: f64,
    shade: Option<Shade>,
}
impl Sample {
    fn from_frame(frame: &[u8]) -> Self {
        let mut dark = 0;
        let mut light = 0;
        let mut sum = 0_u64;
        for &value in frame {
            dark += usize::from(value <= 12);
            light += usize::from(value >= 243);
            sum += u64::from(value);
        }
        Self {
            mean: sum as f64 / frame.len() as f64,
            shade: if dark as f64 / frame.len() as f64 >= 0.985 {
                Some(Shade::Black)
            } else if light as f64 / frame.len() as f64 >= 0.985 {
                Some(Shade::White)
            } else {
                None
            },
        }
    }
    fn distance(self, shade: Shade) -> f64 {
        match shade {
            Shade::Black => self.mean,
            Shade::White => 255.0 - self.mean,
        }
    }
}

struct Interval {
    shade: Shade,
    start: f64,
    end: f64,
    before: Vec<Sample>,
    after: Vec<Sample>,
}

impl Interval {
    fn boundary(&self) -> Option<f64> {
        let visible_before = self.before.iter().any(|s| s.distance(self.shade) >= 40.0);
        let visible_after = self.after.iter().any(|s| s.distance(self.shade) >= 40.0);
        if visible_before && visible_after {
            Some((self.start + self.end) / 2.0)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct Detector {
    history: VecDeque<Sample>,
    active: Option<Interval>,
    pending: VecDeque<Interval>,
    boundaries: Vec<f64>,
}
impl Detector {
    fn frame(&mut self, sample: Sample, time: f64) {
        for pending in &mut self.pending {
            pending.after.push(sample);
        }
        if self
            .pending
            .front()
            .is_some_and(|p| p.after.len() >= CONTEXT)
        {
            if let Some(boundary) = self.pending.pop_front().and_then(|p| p.boundary()) {
                self.boundaries.push(boundary);
            }
        }
        if self
            .active
            .as_ref()
            .is_some_and(|a| Some(a.shade) != sample.shade)
        {
            let mut ended = self.active.take().unwrap();
            ended.after.push(sample);
            self.pending.push_back(ended);
        }
        if let Some(shade) = sample.shade {
            let active = self.active.get_or_insert_with(|| Interval {
                shade,
                start: time,
                end: time,
                before: self.history.iter().copied().collect(),
                after: Vec::new(),
            });
            active.end = time;
        }
        self.history.push_back(sample);
        if self.history.len() > CONTEXT {
            self.history.pop_front();
        }
    }

    fn finish(mut self) -> Vec<f64> {
        // An interval still active at EOF has no following content, so it is
        // intentionally excluded. Leading intervals likewise lack prior content.
        for pending in self.pending {
            if let Some(boundary) = pending.boundary() {
                self.boundaries.push(boundary);
            }
        }
        self.boundaries
    }
}

enum ScanEvent {
    Frame(usize),
    Finished(usize, WindowResult),
}

type WindowResult = Result<(Vec<f64>, u64), String>;

#[derive(Clone, Copy)]
struct ScanConfig<'a> {
    tools: &'a Tools,
    input: &'a Path,
    info: &'a crate::media::MediaInfo,
    keyframes: &'a [f64],
    fast: bool,
    skip_non_reference: bool,
    decode_threads: usize,
}

fn candidate_scan_config(config: ScanConfig<'_>) -> ScanConfig<'_> {
    ScanConfig {
        fast: true,
        skip_non_reference: false,
        ..config
    }
}

fn snap_candidates(candidates: &[f64], keyframes: &[f64], duration: f64) -> Vec<f64> {
    let mut keys: Vec<f64> = keyframes
        .iter()
        .copied()
        .filter(|time| time.is_finite() && *time >= 0.0 && *time <= duration + 0.001)
        .collect();
    keys.sort_by(f64::total_cmp);
    keys.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
    let mut boundaries = Vec::new();
    for candidate in candidates.iter().copied().filter(|time| time.is_finite()) {
        let index = keys.partition_point(|key| *key < candidate);
        let nearest = match (
            index.checked_sub(1).and_then(|i| keys.get(i)),
            keys.get(index),
        ) {
            (Some(left), Some(right)) => Some(if candidate - left <= right - candidate {
                *left
            } else {
                *right
            }),
            (Some(key), None) | (None, Some(key)) => Some(*key),
            (None, None) => None,
        };
        if let Some(time) = nearest.filter(|time| *time > 0.001 && *time < duration - 0.001) {
            boundaries.push(time);
        }
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
    boundaries
}

fn scan_window(
    config: &ScanConfig<'_>,
    window: candidates::ScanWindow,
    index: usize,
    sender: &mpsc::Sender<ScanEvent>,
    control: &DetectionControl,
) -> Result<(Vec<f64>, u64), String> {
    control.check()?;
    let mut seek = window.start;
    if config.fast {
        if let Some(key) = config
            .keyframes
            .iter()
            .copied()
            .filter(|key| key.is_finite() && *key <= window.start)
            .max_by(f64::total_cmp)
        {
            seek = key.max(0.0);
        }
    }
    let filter = if config.fast {
        format!(
            "setpts=PTS-({:.9})/TB,fps=5:start_time={:.9},trim=start={:.9}:end={:.9},scale=160:90,format=gray",
            config.info.start_time, window.start, window.start, window.end
        )
    } else {
        format!(
            "setpts=PTS-({:.9})/TB,fps=5:start_time=0,scale=160:90,format=gray",
            config.info.start_time
        )
    };
    let mut cmd = command(&config.tools.ffmpeg);
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-threads",
        &config.decode_threads.to_string(),
    ]);
    if config.skip_non_reference {
        cmd.args(["-skip_frame", "noref"]);
    }
    cmd.args(["-filter_threads", "1", "-copyts"]);
    if config.fast {
        cmd.args([
            "-ss",
            &format!("{:.9}", seek + config.info.start_time),
            "-t",
            &format!("{:.9}", window.end - seek + 0.5),
        ]);
    }
    let mut child = cmd
        .arg("-i")
        .arg(config.input)
        .args(["-map", "0:v:0", "-an", "-sn", "-dn", "-vf"])
        .arg(filter)
        .args([
            "-threads",
            "2",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start chapter detection: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let child_id = control.inner.next_child.fetch_add(1, Ordering::Relaxed);
    {
        let mut children = match control.inner.children.lock() {
            Ok(children) => children,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        };
        if control.inner.cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Chapter detection cancelled.".into());
        }
        children.insert(child_id, child);
    }
    let running = Running { control, child_id };
    let errors = thread::spawn(move || {
        let mut tail = VecDeque::with_capacity(4096);
        let mut chunk = [0_u8; 1024];
        while let Ok(count) = stderr.read(&mut chunk) {
            if count == 0 {
                break;
            }
            for &byte in &chunk[..count] {
                if tail.len() == 4096 {
                    tail.pop_front();
                }
                tail.push_back(byte);
            }
        }
        String::from_utf8_lossy(&tail.into_iter().collect::<Vec<_>>()).into_owned()
    });
    let mut detector = Detector::default();
    let mut frame = [0_u8; PIXELS];
    let mut count = 0_u64;
    let read_result = (|| -> Result<(), String> {
        loop {
            control.check()?;
            match stdout.read_exact(&mut frame) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(format!("Cannot read chapter detection frames: {e}")),
            }
            control.check()?;
            detector.frame(
                Sample::from_frame(&frame),
                window.start + count as f64 / FPS,
            );
            count += 1;
            let _ = sender.send(ScanEvent::Frame(index));
        }
        Ok(())
    })();
    if read_result.is_err() {
        control.cancel();
    }
    let status = loop {
        let mut children = control.inner.children.lock().map_err(|e| e.to_string())?;
        let child = children
            .get_mut(&child_id)
            .ok_or("Chapter detection process is unavailable.")?;
        match child
            .try_wait()
            .map_err(|e| format!("Cannot wait for chapter detection: {e}"))?
        {
            Some(status) => {
                children.remove(&child_id);
                break status;
            }
            None => {
                drop(children);
                thread::sleep(Duration::from_millis(10));
            }
        }
    };
    drop(running);
    let error_tail = errors.join().unwrap_or_default();
    control.check()?;
    read_result?;
    if !status.success() || count == 0 {
        return Err(format!("Chapter detection failed. {error_tail}"));
    }
    Ok((detector.finish(), count))
}

fn scan_windows(
    config: ScanConfig<'_>,
    windows: &[candidates::ScanWindow],
    worker_count: usize,
    work_denominator: f64,
    progress_base: f64,
    control: &DetectionControl,
    progress: &impl Fn(DetectionProgress),
) -> Result<Vec<Vec<f64>>, String> {
    if windows.is_empty() {
        return Ok(Vec::new());
    }
    let workers = worker_count.max(1).min(windows.len());
    let cores = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2);
    let decode_threads = (cores / workers).clamp(1, 4);
    let queue = Mutex::new(windows.iter().copied().enumerate().collect::<VecDeque<_>>());
    let (sender, receiver) = mpsc::channel();
    let mut results: Vec<Option<WindowResult>> = std::iter::repeat_with(|| None)
        .take(windows.len())
        .collect();
    let mut decoded = vec![0_u64; windows.len()];
    thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let control = control.clone();
            let queue = &queue;
            let config = ScanConfig {
                decode_threads,
                ..config
            };
            scope.spawn(move || loop {
                let task = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                let Some((index, window)) = task else { break };
                let result = scan_window(&config, window, index, &sender, &control);
                if sender.send(ScanEvent::Finished(index, result)).is_err() {
                    break;
                }
            });
        }
        drop(sender);
        for event in receiver {
            match event {
                ScanEvent::Frame(index) => {
                    decoded[index] += 1;
                    let processed: f64 = decoded
                        .iter()
                        .zip(windows)
                        .map(|(count, window)| (*count as f64 / FPS).min(window.end - window.start))
                        .sum();
                    progress(DetectionProgress {
                        fraction: (progress_base + processed / work_denominator.max(0.001))
                            .clamp(0.0, 0.99),
                        stage: "Scanning black and white fades".into(),
                    });
                }
                ScanEvent::Finished(index, result) => results[index] = Some(result),
            }
        }
    });
    control.check()?;
    results
        .into_iter()
        .map(|result| {
            let (boundaries, _) = result.ok_or("Chapter detection worker did not finish.")??;
            Ok(boundaries)
        })
        .collect()
}

pub fn detect_chapters(
    tools: &Tools,
    input: &Path,
    keyframes: &[f64],
    control: &DetectionControl,
    progress: impl Fn(DetectionProgress),
) -> Result<Vec<f64>, String> {
    detect_chapters_with_mode(tools, input, keyframes, false, control, progress)
}

pub fn detect_chapters_with_mode(
    tools: &Tools,
    input: &Path,
    keyframes: &[f64],
    precise: bool,
    control: &DetectionControl,
    progress: impl Fn(DetectionProgress),
) -> Result<Vec<f64>, String> {
    control.check()?;
    let info = probe_media(tools, input)?;
    control.check()?;
    progress(DetectionProgress {
        fraction: 0.0,
        stage: "Scanning black and white fades".into(),
    });
    control.check()?;
    let planned = if precise {
        None
    } else {
        candidates::candidate_windows(input, &info, keyframes)
    };
    let fast = planned.is_some();
    let windows = planned.unwrap_or_else(|| {
        vec![candidates::ScanWindow {
            start: 0.0,
            end: info.duration,
        }]
    });
    let total_work: f64 = windows.iter().map(|window| window.end - window.start).sum();
    let worker_count = thread::available_parallelism()
        .map(|cores| cores.get().min(4))
        .unwrap_or(2);
    let progress_work = total_work;
    let candidates_found: Vec<f64> = if fast {
        scan_windows(
            candidate_scan_config(ScanConfig {
                tools,
                input,
                info: &info,
                keyframes,
                fast: false,
                skip_non_reference: true,
                decode_threads: 1,
            }),
            &windows,
            worker_count,
            progress_work,
            0.0,
            control,
            &progress,
        )?
        .into_iter()
        .flatten()
        .collect()
    } else {
        let all = [candidates::ScanWindow {
            start: 0.0,
            end: info.duration,
        }];
        scan_windows(
            ScanConfig {
                tools,
                input,
                info: &info,
                keyframes,
                fast: false,
                skip_non_reference: false,
                decode_threads: 1,
            },
            &all,
            1,
            progress_work,
            0.0,
            control,
            &progress,
        )?
        .into_iter()
        .flatten()
        .collect()
    };
    let duration = info.duration;
    let boundaries = snap_candidates(&candidates_found, keyframes, duration);
    control.check()?;
    progress(DetectionProgress {
        fraction: 1.0,
        stage: "Chapter detection complete".into(),
    });
    control.check()?;
    Ok(boundaries)
}

#[cfg(test)]
mod candidate_scan_tests {
    use super::{candidate_scan_config, snap_candidates, ScanConfig};
    use crate::media::{MediaInfo, Tools};
    use std::path::{Path, PathBuf};

    #[test]
    fn candidate_window_scan_keeps_non_reference_frames() {
        let tools = Tools {
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
        };
        let info = MediaInfo {
            path: String::new(),
            name: String::new(),
            duration: 10.0,
            start_time: 0.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            video_codec: "h264".into(),
            audio_codec: None,
            audio_start: 0.0,
            size: 0,
        };
        let keys = [0.0, 5.0, 10.0];
        let coarse = ScanConfig {
            tools: &tools,
            input: Path::new("input.mp4"),
            info: &info,
            keyframes: &keys,
            fast: true,
            skip_non_reference: true,
            decode_threads: 1,
        };

        let exact = candidate_scan_config(coarse);

        assert!(exact.fast);
        assert!(!exact.skip_non_reference);
    }

    #[test]
    fn snapped_scout_and_exact_results_share_keyframe_boundaries() {
        assert_eq!(
            snap_candidates(&[2.76, 5.74], &[0.0, 2.75, 5.75, 9.0], 9.0),
            vec![2.75, 5.75]
        );
    }
}

#[cfg(test)]
mod interval_tests {
    use super::{Interval, Sample, Shade, CONTEXT};

    #[test]
    fn accepts_a_hard_cut_to_black_when_content_is_visible_on_both_sides() {
        let visible = Sample {
            mean: 70.0,
            shade: None,
        };
        let interval = Interval {
            shade: Shade::Black,
            start: 1709.2,
            end: 1711.0,
            before: vec![visible; CONTEXT],
            after: vec![visible; CONTEXT],
        };

        assert_eq!(interval.boundary(), Some(1710.1));
    }
}
