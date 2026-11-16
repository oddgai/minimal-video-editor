const paths = {
  cut: '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="m8.1 8.1 12.4 12.4M8.1 15.9 20.5 3.5M14 14l-3-3"/>',
  folder: '<path d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v1M3 7h18l-2 13H5L3 7Z"/>',
  export: '<path d="M12 15V3m-4 4 4-4 4 4M4 14v5a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5"/>',
  play: '<path d="m8 4 13 8-13 8V4Z"/>',
  pause: '<path d="M8 4v16M16 4v16"/>',
  prev: '<path d="M5 4v16m14-16L8 12l11 8V4Z"/>',
  next: '<path d="M19 4v16M5 4l11 8-11 8V4Z"/>',
  left: '<path d="m14 6-6 6 6 6"/>',
  right: '<path d="m10 6 6 6-6 6"/>',
  undo: '<path d="M3 10h10a7 7 0 0 1 0 14M3 10l6-6m-6 6 6 6" transform="translate(1 -3)"/>',
  redo: '<path d="M21 10H11a7 7 0 0 0 0 14m10-14-6-6m6 6-6 6" transform="translate(-1 -3)"/>',
  trash: '<path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7"/>',
  volume: '<path d="M11 4 5 9H2v6h3l6 5V4Zm4 4a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  film: '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M7 3v18M17 3v18M3 8h4m-4 8h4m10-8h4m-4 8h4"/>',
  close: '<path d="m6 6 12 12M6 18 18 6"/>',
  keyboard: '<rect x="2" y="5" width="20" height="14" rx="3"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 16h10"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
  detect: '<path d="m4 20 12-12 4 4L8 24M14 6l4-4m-8 0v4M3 8h4m13 11v-4" transform="translate(0 -2)"/>',
} as const;

export const icon = (name: keyof typeof paths, size = 18) => `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name]}</svg>`;
