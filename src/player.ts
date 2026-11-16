export function audioTiming(videoTime: number, offset: number, duration: number, playing: boolean) {
  if (!Number.isFinite(duration) || duration <= 0) return { time: 0, play: false };
  const target = videoTime - offset;
  return { time: Math.max(0, Math.min(duration, target)), play: playing && target >= 0 && target < duration };
}

/** Desired clock correction. Apply its inverse to video to keep audio gapless. */
export function audioCorrection(target: number, actual: number, force: boolean) {
  const difference = target - actual;
  if (force && Math.abs(difference) > 0.025) return { seek: true, rate: 1 };
  return { seek: false, rate: Math.abs(difference) <= 0.025 ? 1 : Math.max(0.9, Math.min(1.1, 1 + difference)) };
}

/** Native media elements stream the same file; no decoded samples enter JS. */
export class Player {
  readonly video: HTMLVideoElement;
  readonly audio: HTMLAudioElement;
  offset = 0;
  hasAudio = false;
  private tick = 0;
  private audioFailed = false;
  private audioStarting = false;
  private needsWarmAlignment = false;
  private warmAudioStart: number | null = null;
  private aligningVideo = false;
  private audioSeekPending = false;
  private onAudioError: () => void;

  constructor(video: HTMLVideoElement, audio: HTMLAudioElement, onAudioError: () => void) {
    this.video = video; this.audio = audio; this.onAudioError = onAudioError;
    video.muted = false;
    video.addEventListener('play', () => { this.prepareWarmAlignment(); this.sync(true); this.startLoop(); });
    video.addEventListener('pause', () => { this.audio.pause(); window.clearInterval(this.tick); });
    video.addEventListener('ended', () => { this.audio.pause(); window.clearInterval(this.tick); });
    video.addEventListener('seeking', () => {
      if (!this.aligningVideo) { this.audioSeekPending = true; this.prepareWarmAlignment(); this.audio.pause(); }
    });
    video.addEventListener('seeked', () => {
      if (this.aligningVideo) { this.aligningVideo = false; return; }
      this.prepareWarmAlignment(); this.sync(true);
    });
    video.addEventListener('waiting', () => { if (!this.aligningVideo) { this.prepareWarmAlignment(); this.audio.pause(); } });
    video.addEventListener('playing', () => { this.sync(); });
    audio.addEventListener('loadedmetadata', () => { this.sync(true); });
    audio.addEventListener('loadeddata', () => { this.sync(true); });
    audio.addEventListener('playing', () => {
      if (this.needsWarmAlignment && this.warmAudioStart === null) this.warmAudioStart = audio.currentTime;
    });
    audio.addEventListener('waiting', () => { this.prepareWarmAlignment(); });
    audio.addEventListener('timeupdate', () => {
      // Media playback starts before the audio output clock is running on some
      // WebViews. Align once after it advances, rather than repeatedly seeking
      // against startup latency and continually flushing the audio decoder.
      if (this.needsWarmAlignment && !this.audioSeekPending && this.warmAudioStart !== null && !audio.seeking && !video.seeking && !video.paused && audio.currentTime - this.warmAudioStart >= 0.05) {
        const target = audio.currentTime + this.offset;
        if (target >= 0 && target < video.duration) {
          this.needsWarmAlignment = false;
          if (Math.abs(video.currentTime - target) > 0.025) {
            // The audio output clock is the reference: seeking it again would
            // flush its warm decoder and recreate the same startup latency.
            this.aligningVideo = true;
            video.currentTime = target;
          }
        }
      }
    });
    audio.addEventListener('error', () => { if (this.hasAudio && this.offset !== 0 && !this.audioFailed) { this.audioFailed = true; this.onAudioError(); } });
  }

  load(url: string, hasAudio: boolean) {
    this.pause(); this.offset = 0; this.hasAudio = hasAudio; this.audioFailed = false;
    this.audioSeekPending = false;
    this.video.muted = false; this.video.playbackRate = 1;
    this.audio.removeAttribute('src'); this.audio.load();
    this.video.src = url; this.video.load();
  }

  unload() {
    this.pause(); this.offset = 0; this.hasAudio = false; this.audioFailed = false;
    this.audioSeekPending = false;
    this.needsWarmAlignment = false; this.warmAudioStart = null;
    this.video.muted = false; this.video.playbackRate = 1;
    this.video.removeAttribute('src'); this.video.load();
    this.audio.removeAttribute('src'); this.audio.load();
  }

  async toggle() { if (this.video.paused) await this.video.play(); else this.pause(); }
  pause() { this.video.pause(); this.audio.pause(); window.clearInterval(this.tick); this.aligningVideo = false; }
  seek(time: number, duration: number) {
    this.aligningVideo = false; this.audioSeekPending = true; this.prepareWarmAlignment();
    this.video.currentTime = Math.max(0, Math.min(duration, time));
  }
  setOffset(offset: number) {
    if (offset === this.offset) return;
    this.offset = offset;
    this.aligningVideo = false; this.video.playbackRate = 1;
    const separate = this.hasAudio && offset !== 0;
    this.prepareWarmAlignment();
    this.video.muted = separate;
    if (separate && !this.audio.getAttribute('src')) {
      this.audio.src = this.video.currentSrc || this.video.src;
      this.audio.load();
    } else if (!separate) {
      this.audio.pause(); this.audio.removeAttribute('src'); this.audio.load();
    }
    this.sync(true); this.startLoop();
  }

  private sync(force = false) {
    // A seek requested while paused/buffering is still required once playback
    // becomes ready. An ordinary timer tick must not resume the old audio clock.
    if (force) this.audioSeekPending = true;
    if (!this.hasAudio || this.offset === 0 || this.audioFailed || this.audio.readyState < 1) return;
    if (this.aligningVideo) return;
    if (this.video.paused || this.video.seeking || this.video.readyState < 3) { this.audio.pause(); return; }
    const timing = audioTiming(this.video.currentTime, this.offset, this.audio.duration, !this.video.paused && !this.video.seeking && this.video.readyState >= 3);
    if (!timing.play) this.audio.pause();
    if (this.audio.seeking) return;
    const correction = audioCorrection(timing.time, this.audio.currentTime, this.audioSeekPending);
    this.audioSeekPending = false;
    const videoRate = 1 / correction.rate;
    if (timing.play && !this.audio.paused && this.video.playbackRate !== videoRate) this.video.playbackRate = videoRate;
    if (correction.seek) { this.audio.currentTime = timing.time; return; }
    if (timing.play) {
      if (this.audio.paused && !this.audioStarting) {
        this.audioStarting = true;
        void this.audio.play().catch(() => { /* retry on the next user playback action */ }).finally(() => { this.audioStarting = false; });
      }
    }
  }

  private startLoop() {
    window.clearInterval(this.tick);
    // rAF stops when a native WebView is occluded, even while media clocks
    // continue. This low-frequency timer exists only during offset playback.
    if (this.hasAudio && this.offset !== 0 && !this.video.paused) this.tick = window.setInterval(() => this.sync(), 150);
  }

  private prepareWarmAlignment() { this.needsWarmAlignment = this.hasAudio && this.offset !== 0; this.warmAudioStart = null; }

}
