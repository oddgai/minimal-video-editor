//! Cheap candidate-window planning for ordinary, non-fragmented H.264 MP4s.
//! Only the MP4 sample index is read; video payload remains untouched here.
use crate::media::MediaInfo;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const MAX_MOOV: u64 = 64 * 1024 * 1024;
const MAX_SAMPLES: usize = 10_000_000;
const LOW_SAMPLE_KEYFRAME_GAP: f64 = 3.0;
const EXTENDED_CANDIDATE_RADIUS: f64 = 14.0;
const MODERATE_SAMPLE_RATIO: f64 = 0.35;
const MAX_CANDIDATE_COVERAGE: f64 = 0.12;

fn candidate_coverage_accepted(covered: f64, duration: f64) -> bool {
    covered / duration.max(0.001) <= MAX_CANDIDATE_COVERAGE
}

fn low_sample_keyframe(
    time: f64,
    size: u32,
    size_limit: u32,
    duration: f64,
    keyframes: &[f64],
) -> Option<f64> {
    if size > size_limit || time <= 0.5 || time >= duration - 0.5 {
        return None;
    }
    let next = keyframes.partition_point(|key| *key < time);
    let previous = next.checked_sub(1).and_then(|index| keyframes.get(index));
    let following = keyframes.get(next);
    let nearest = match (previous, following) {
        (Some(previous), Some(following)) => {
            if time - previous <= following - time {
                *previous
            } else {
                *following
            }
        }
        (Some(previous), None) => *previous,
        (None, Some(following)) => *following,
        (None, None) => return None,
    };
    ((nearest - time).abs() <= LOW_SAMPLE_KEYFRAME_GAP).then_some(nearest)
}

fn moderate_sample_keyframe(
    time: f64,
    size: u32,
    macroblocks: u64,
    duration: f64,
    keyframes: &[f64],
) -> Option<f64> {
    let limit = (macroblocks as f64 * MODERATE_SAMPLE_RATIO) as u32;
    low_sample_keyframe(time, size, limit, duration, keyframes)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ScanWindow {
    pub start: f64,
    pub end: f64,
}

fn candidate_window(time: f64, duration: f64, radius: f64) -> ScanWindow {
    ScanWindow {
        start: ((time - radius).max(0.0) * 5.0).floor() / 5.0,
        end: ((time + radius).min(duration) * 5.0).ceil() / 5.0,
    }
}

#[derive(Clone, Copy)]
struct BoxRef {
    kind: [u8; 4],
    start: u64,
    end: u64,
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn read_i32(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_be_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn read_u64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        data.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn boxes(data: &[u8], base: u64) -> Option<Vec<BoxRef>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if data.len() - at < 8 {
            return None;
        }
        let size32 = read_u32(data, at)? as u64;
        let kind: [u8; 4] = data.get(at + 4..at + 8)?.try_into().ok()?;
        let (size, header) = if size32 == 1 {
            (read_u64(data, at + 8)?, 16usize)
        } else if size32 == 0 {
            ((data.len() - at) as u64, 8usize)
        } else {
            (size32, 8usize)
        };
        if size < header as u64 || size > (data.len() - at) as u64 {
            return None;
        }
        let end = base.checked_add(at as u64)?.checked_add(size)?;
        out.push(BoxRef {
            kind,
            start: base + at as u64 + header as u64,
            end,
        });
        at += usize::try_from(size).ok()?;
    }
    Some(out)
}

fn child<'a>(data: &'a [u8], base: u64, kind: &[u8; 4]) -> Option<(BoxRef, &'a [u8])> {
    let item = boxes(data, base)?.into_iter().find(|b| &b.kind == kind)?;
    let start = usize::try_from(item.start - base).ok()?;
    let end = usize::try_from(item.end - base).ok()?;
    Some((item, data.get(start..end)?))
}

fn version_time(data: &[u8], version0: usize, version1: usize) -> Option<u32> {
    match *data.first()? {
        0 => read_u32(data, version0),
        1 => read_u32(data, version1),
        _ => None,
    }
}

fn edit_origin(
    trak: &[u8],
    trak_base: u64,
    media_scale: u32,
    movie_scale: u32,
    info: &MediaInfo,
) -> Option<f64> {
    let (_, edts) = match child(trak, trak_base, b"edts") {
        Some(value) => value,
        None => return Some(info.start_time),
    };
    let (elst_box, elst) = child(edts, 0, b"elst")?;
    let _ = elst_box;
    if media_scale == 0 || movie_scale == 0 || elst.len() < 8 {
        return None;
    }
    let version = *elst.first()?;
    if version > 1 {
        return None;
    }
    let count = usize::try_from(read_u32(elst, 4)?).ok()?;
    if !(1..=2).contains(&count) {
        return None;
    }
    let entry_size = if version == 0 { 12 } else { 20 };
    if elst.len() < 8usize.checked_add(count.checked_mul(entry_size)?)? {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let at = 8 + index * entry_size;
        let (segment_duration, media_time, rate_at) = if version == 0 {
            (
                read_u32(elst, at)? as u64,
                read_i32(elst, at + 4)? as i64,
                at + 8,
            )
        } else {
            (
                read_u64(elst, at)?,
                i64::from_be_bytes(elst.get(at + 8..at + 16)?.try_into().ok()?),
                at + 16,
            )
        };
        let integer_rate = i16::from_be_bytes(elst.get(rate_at..rate_at + 2)?.try_into().ok()?);
        let fraction_rate =
            i16::from_be_bytes(elst.get(rate_at + 2..rate_at + 4)?.try_into().ok()?);
        if integer_rate != 1 || fraction_rate != 0 {
            return None;
        }
        entries.push((segment_duration, media_time));
    }
    let (media_time, empty_lead) = if count == 1 {
        if entries[0].1 < 0 {
            return None;
        }
        (entries[0].1, 0.0)
    } else {
        if entries[0].1 != -1 || entries[1].1 < 0 {
            return None;
        }
        (entries[1].1, entries[0].0 as f64 / movie_scale as f64)
    };
    Some(media_time as f64 / media_scale as f64 - empty_lead + info.start_time)
}

fn runs(data: &[u8], signed: bool, expected: usize) -> Option<Vec<i64>> {
    if data.len() < 8 {
        return None;
    }
    let count = usize::try_from(read_u32(data, 4)?).ok()?;
    if count > expected || 8usize.checked_add(count.checked_mul(8)?)? > data.len() {
        return None;
    }
    let mut values = Vec::with_capacity(expected);
    for i in 0..count {
        let n = usize::try_from(read_u32(data, 8 + i * 8)?).ok()?;
        let value = if signed {
            read_i32(data, 12 + i * 8)? as i64
        } else {
            read_u32(data, 12 + i * 8)? as i64
        };
        if n == 0 || values.len().checked_add(n)? > expected {
            return None;
        }
        values.resize(values.len() + n, value);
    }
    (values.len() == expected).then_some(values)
}

fn read_moov(file: &mut File, file_len: u64) -> Option<Vec<u8>> {
    let mut pos = 0u64;
    while pos.checked_add(8)? <= file_len {
        file.seek(SeekFrom::Start(pos)).ok()?;
        let mut header = [0; 16];
        file.read_exact(&mut header[..8]).ok()?;
        let size32 = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = header[4..8].try_into().ok()?;
        let (size, header_len) = if size32 == 1 {
            file.read_exact(&mut header[8..16]).ok()?;
            (u64::from_be_bytes(header[8..16].try_into().ok()?), 16u64)
        } else if size32 == 0 {
            (file_len - pos, 8)
        } else {
            (size32, 8)
        };
        if size < header_len || pos.checked_add(size)? > file_len {
            return None;
        }
        if &kind == b"moov" {
            if size - header_len > MAX_MOOV {
                return None;
            }
            let mut bytes = vec![0; usize::try_from(size - header_len).ok()?];
            file.read_exact(&mut bytes).ok()?;
            return Some(bytes);
        }
        // Fragmented MP4 and zero-sized boxes are left to the precise scanner.
        if &kind == b"moof" || size32 == 0 {
            return None;
        }
        pos += size;
    }
    None
}

fn video_sample_index(
    moov: &[u8],
    movie_scale: u32,
    width: u32,
    height: u32,
    info: &MediaInfo,
    keyframes: &[f64],
) -> Option<Vec<ScanWindow>> {
    let root = boxes(moov, 0)?;
    for trak in root.iter().filter(|b| &b.kind == b"trak") {
        let trak_bytes = moov.get(trak.start as usize..trak.end as usize)?;
        let (mdia_box, mdia) = child(trak_bytes, trak.start, b"mdia")?;
        let (_, hdlr) = child(mdia, mdia_box.start, b"hdlr")?;
        if hdlr.get(8..12)? != b"vide" {
            continue;
        }
        let (_, mdhd) = child(mdia, mdia_box.start, b"mdhd")?;
        let timescale = version_time(mdhd, 12, 20)?;
        if timescale == 0 {
            return None;
        }
        let origin = edit_origin(trak_bytes, trak.start, timescale, movie_scale, info)?;
        let (minf_box, minf) = child(mdia, mdia_box.start, b"minf")?;
        let (stbl_box, stbl) = child(minf, minf_box.start, b"stbl")?;
        // Restrict acceleration to the supported H.264 sample entries.
        let (_, stsd) = child(stbl, stbl_box.start, b"stsd")?;
        if stsd.get(12..16)? != b"avc1" && stsd.get(12..16)? != b"avc3" {
            return None;
        }
        let table = boxes(stbl, stbl_box.start)?;
        let get = |kind: &[u8; 4]| -> Option<&[u8]> {
            let b = table.iter().find(|b| &b.kind == kind)?;
            stbl.get((b.start - stbl_box.start) as usize..(b.end - stbl_box.start) as usize)
        };
        let stsz = get(b"stsz")?;
        let fixed = read_u32(stsz, 4)?;
        let count = usize::try_from(read_u32(stsz, 8)?).ok()?;
        if count == 0 || count > MAX_SAMPLES {
            return None;
        }
        let mut sizes = Vec::with_capacity(count);
        if fixed != 0 {
            sizes.resize(count, fixed);
        } else {
            if stsz.len() < 12usize.checked_add(count.checked_mul(4)?)? {
                return None;
            }
            for i in 0..count {
                sizes.push(read_u32(stsz, 12 + i * 4)?);
            }
            // Uniform packet sizes carry no useful blank-frame signal.
            if sizes.iter().all(|size| *size == sizes[0]) {
                return None;
            }
        }
        let stts = get(b"stts")?;
        let durations = runs(stts, false, count)?;
        let ctts = table.iter().find(|b| &b.kind == b"ctts").and_then(|b| {
            stbl.get((b.start - stbl_box.start) as usize..(b.end - stbl_box.start) as usize)
        });
        let offsets = if let Some(ctts) = ctts {
            runs(ctts, ctts.first() == Some(&1), count)?
        } else {
            vec![0; count]
        };
        let mut dts = 0i64;
        let mut pts = Vec::with_capacity(count);
        for (duration, offset) in durations.into_iter().zip(offsets) {
            pts.push((dts + offset) as f64 / timescale as f64);
            dts = dts.checked_add(duration)?;
        }
        if !origin.is_finite() {
            return None;
        }
        let mb = u64::from(width.div_ceil(16)) * u64::from(height.div_ceil(16));
        let low_limit = (mb as f64 * 0.3) as u32;
        let very_low_limit = (mb as f64 * 0.1) as u32;
        let high_limit = (mb as f64 * 0.7) as u32;
        let mut samples = Vec::new();
        for (time, size) in pts.into_iter().zip(sizes) {
            let time = time - origin;
            if !time.is_finite() || time < 0.0 || time > info.duration {
                continue;
            }
            if size <= high_limit {
                samples.push((time, size));
            }
        }
        samples.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut sorted_keys: Vec<f64> = keyframes
            .iter()
            .copied()
            .filter(|time| time.is_finite())
            .collect();
        sorted_keys.sort_by(f64::total_cmp);
        sorted_keys.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
        if sorted_keys.is_empty() {
            return None;
        }

        let mut events = Vec::new();
        let mut extended_events = Vec::new();
        for (threshold, high_band) in [(low_limit, false), (high_limit, true)] {
            let mut group: Vec<(f64, u32)> = Vec::new();
            let mut consider_group = |group: &mut Vec<(f64, u32)>| {
                if group.is_empty() {
                    return;
                }
                let (time, size) = *group.iter().min_by_key(|sample| sample.1).unwrap();
                let ratio = size as f64 / mb as f64;
                let next_key = sorted_keys.partition_point(|key| *key < time);
                let delta = sorted_keys.get(next_key).map(|key| key - time);
                let in_range = time > 0.5 && time < info.duration - 0.5;
                let near_key = match (high_band, delta) {
                    (false, Some(delta)) => (0.0..=0.15).contains(&delta),
                    (true, Some(delta)) => (0.10..=0.15).contains(&delta),
                    (_, None) => false,
                };
                let in_size_band = if high_band {
                    ratio > 0.3 && size <= threshold
                } else {
                    size <= threshold
                };
                if in_range && near_key && in_size_band {
                    events.push(time);
                }
                if !high_band {
                    if let Some(key) =
                        low_sample_keyframe(time, size, very_low_limit, info.duration, &sorted_keys)
                    {
                        extended_events.push(key);
                    }
                } else if let Some(key) =
                    moderate_sample_keyframe(time, size, mb, info.duration, &sorted_keys)
                {
                    events.push(key);
                }
                group.clear();
            };
            for &(time, size) in &samples {
                if size > threshold {
                    continue;
                }
                if group
                    .last()
                    .is_some_and(|(previous, _)| time - *previous > 4.0)
                {
                    consider_group(&mut group);
                }
                group.push((time, size));
            }
            consider_group(&mut group);
        }
        events.sort_by(f64::total_cmp);
        events.dedup_by(|a, b| (*a - *b).abs() < 0.4);
        let mut intervals: Vec<(f64, f64)> = Vec::new();
        for time in events {
            let window = candidate_window(time, info.duration, 2.0);
            intervals.push((window.start, window.end));
        }
        for time in extended_events {
            let window = candidate_window(time, info.duration, EXTENDED_CANDIDATE_RADIUS);
            intervals.push((window.start, window.end));
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::with_capacity(intervals.len());
        for (start, end) in intervals {
            if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        let covered: f64 = merged.iter().map(|(a, b)| b - a).sum();
        // An unselective sample index must not silently become an incomplete scan.
        if !candidate_coverage_accepted(covered, info.duration) {
            return None;
        }
        let windows: Vec<ScanWindow> = merged
            .into_iter()
            .map(|(start, end)| ScanWindow { start, end })
            .collect();
        return Some(windows);
    }
    None
}

pub(super) fn candidate_windows(
    path: &Path,
    info: &MediaInfo,
    keyframes: &[f64],
) -> Option<Vec<ScanWindow>> {
    if !info.video_codec.eq_ignore_ascii_case("h264") || info.width == 0 || info.height == 0 {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let moov = read_moov(&mut file, len)?;
    let (_, mvhd) = child(&moov, 0, b"mvhd")?;
    let movie_scale = version_time(mvhd, 12, 20)?;
    video_sample_index(&moov, movie_scale, info.width, info.height, info, keyframes)
}

#[cfg(test)]
mod tests {
    use super::{
        candidate_coverage_accepted, candidate_window, low_sample_keyframe,
        moderate_sample_keyframe, EXTENDED_CANDIDATE_RADIUS,
    };

    #[test]
    fn associates_extreme_low_samples_with_the_nearest_keyframe_on_either_side() {
        let keyframes = [485.485, 490.49, 495.495];

        assert_eq!(
            low_sample_keyframe(490.824, 60, 800, 1000.0, &keyframes),
            Some(490.49)
        );
        assert_eq!(
            low_sample_keyframe(493.4, 60, 800, 1000.0, &keyframes),
            Some(495.495)
        );
        assert_eq!(
            low_sample_keyframe(490.824, 900, 800, 1000.0, &keyframes),
            None
        );
        assert_eq!(
            low_sample_keyframe(480.0, 60, 800, 1000.0, &keyframes),
            None
        );
    }

    #[test]
    fn extended_candidate_window_keeps_context_on_both_sides_of_a_fade() {
        let window = candidate_window(490.49, 1000.0, EXTENDED_CANDIDATE_RADIUS);

        assert_eq!(window.start, 476.4);
        assert_eq!(window.end, 504.6);
    }

    #[test]
    fn moderate_low_sample_adds_a_local_window_at_the_nearest_keyframe() {
        let keyframes = [123.84, 125.08, 127.04, 129.04];

        assert_eq!(
            moderate_sample_keyframe(127.12, 493, 1620, 7000.0, &keyframes),
            Some(127.04)
        );
        assert_eq!(
            moderate_sample_keyframe(127.12, 700, 1620, 7000.0, &keyframes),
            None
        );
    }

    #[test]
    fn falls_back_for_dense_candidate_windows() {
        assert!(!candidate_coverage_accepted(0.357, 1.0));
        assert!(!candidate_coverage_accepted(0.60, 1.0));
    }
}
