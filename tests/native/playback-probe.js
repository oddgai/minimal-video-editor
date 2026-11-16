(() => {
  const launch = async () => {
    const open = document.getElementById('empty-open');
    if (!open) { setTimeout(launch, 100); return; }
    let video = document.getElementById('video');
    const audio = document.getElementById('audio');
    video.crossOrigin = 'anonymous';
    const events = { video: {}, audio: {} };
    for (const [name, media] of [['video', video], ['audio', audio]]) {
      for (const event of ['loadedmetadata', 'loadeddata', 'play', 'playing', 'pause', 'waiting', 'stalled', 'seeking', 'seeked', 'error']) {
        media.addEventListener(event, () => { events[name][event] = (events[name][event] || 0) + 1; });
      }
    }
    open.click();
    await new Promise(resolve => setTimeout(resolve, 2000));
    if (window.__MVE_PROBE_MODE__ === 'single') {
      const src = window.__MVE_PROBE_URL__ || video.src;
      video.pause(); audio.pause(); audio.removeAttribute('src'); audio.load();
      const replacement = document.createElement('video'); replacement.playsInline = true;
      replacement.style.cssText = 'width:100%;height:100%;object-fit:contain';
      replacement.src = src; video.replaceWith(replacement); video = replacement;
      try { await video.play(); } catch (error) { events.video.playError = String(error); }
    } else {
      if (window.__MVE_PROBE_OFFSET__) {
        const field = document.getElementById('offset'); field.value = String(window.__MVE_PROBE_OFFSET__);
        field.dispatchEvent(new Event('change', { bubbles: true }));
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      if (window.__MVE_PROBE_URL__) {
        video.src = window.__MVE_PROBE_URL__; audio.src = window.__MVE_PROBE_URL__;
        video.load(); audio.load();
        await new Promise(resolve => setTimeout(resolve, 2000));
      }
      if (window.__MVE_PROBE_MODE__ === 'volume') { video.muted = false; video.volume = 0; }
      if (window.__MVE_PROBE_MODE__ === 'reorder') {
        const key = (key, shiftKey = false, metaKey = false) => document.getElementById('workspace').dispatchEvent(new KeyboardEvent('keydown', { key, shiftKey, metaKey, bubbles: true }));
        key('ArrowRight'); key('ArrowRight'); key('s'); key('ArrowRight'); key('ArrowRight'); key('s');
        key('ArrowLeft', true, true); key('ArrowLeft', true, true);
        const scrub = document.getElementById('scrubber'); scrub.value = '3.7'; scrub.dispatchEvent(new Event('input', { bubbles: true }));
      }
      events.audio = {}; events.video = {};
      document.getElementById('play').click();
    }
    const seekSamples = [], reorderSamples = [];
    if (window.__MVE_PROBE_MODE__ === 'reorder') {
      const capture = () => ({ v: video.currentTime, paused: video.paused, edited: document.getElementById('current-time').textContent });
      const scrub = value => { const field = document.getElementById('scrubber'); field.value = value; field.dispatchEvent(new Event('input', { bubbles: true })); };
      await new Promise(resolve => setTimeout(resolve, 900)); reorderSamples.push(capture());
      scrub('11.7'); await new Promise(resolve => setTimeout(resolve, 900)); reorderSamples.push(capture());
      document.getElementById('play').click(); await new Promise(resolve => setTimeout(resolve, 300)); reorderSamples.push(capture());
    }
    if (window.__MVE_PROBE_MODE__ === 'seek') {
      await new Promise(resolve => setTimeout(resolve, 1000));
      for (const [index, target] of [60, 180, 40, 120, 90, 30].entries()) {
        const pausedSeek = index % 2 === 0;
        if (pausedSeek && !video.paused) document.getElementById('play').click();
        const scrubber = document.getElementById('scrubber');
        for (const time of [target - 1, target - 0.25, target]) {
          scrubber.value = String(time); scrubber.dispatchEvent(new Event('input', { bubbles: true }));
        }
        if (pausedSeek) document.getElementById('play').click();
        await new Promise(resolve => setTimeout(resolve, 600));
        const audioStart = audio.currentTime;
        await new Promise(resolve => setTimeout(resolve, 700));
        seekSamples.push({ target, pausedSeek, v: video.currentTime, a: audio.currentTime, audioAdvance: audio.currentTime - audioStart, audioPaused: audio.paused, videoSeeking: video.seeking, audioSeeking: audio.seeking, videoReadyState: video.readyState, audioReadyState: audio.readyState });
      }
    }
    const clocks = [];
    const sampleStart = performance.now();
    const sampling = setInterval(() => clocks.push({ ms: Math.round(performance.now() - sampleStart), v: video.currentTime, a: audio.currentTime, r: video.playbackRate }), 250);
    await new Promise(resolve => setTimeout(resolve, 8000));
    clearInterval(sampling);
    let pixelRange = null;
    try {
      const canvas = document.createElement('canvas'); canvas.width = 48; canvas.height = 27;
      const ctx = canvas.getContext('2d'); ctx.drawImage(video, 0, 0, 48, 27);
      const data = ctx.getImageData(0, 0, 48, 27).data;
      const rgb = [...data].filter((_, i) => i % 4 !== 3); pixelRange = Math.max(...rgb) - Math.min(...rgb);
    } catch (error) { pixelRange = String(error); }
    const report = {
      videoTime: video.currentTime, audioTime: audio.currentTime,
      videoFrames: video.getVideoPlaybackQuality?.().totalVideoFrames ?? video.webkitDecodedFrameCount ?? 0,
      videoWidth: video.videoWidth, videoHeight: video.videoHeight,
      videoReadyState: video.readyState, audioReadyState: audio.readyState,
      videoError: video.error && { code: video.error.code, message: video.error.message },
      audioError: audio.error && { code: audio.error.code, message: audio.error.message },
      videoHidden: video.hidden, events,
      videoRate: video.playbackRate,
      audioRate: audio.playbackRate, offset: window.__MVE_PROBE_OFFSET__ || 0,
      audioSource: audio.getAttribute('src'),
      pixelRange,
      clocks, seekSamples, reorderSamples,
      notice: document.getElementById('notice-text').textContent,
    };
    await window.__TAURI_INTERNALS__.invoke('probe_report', { report });
  };
  launch();
})();
