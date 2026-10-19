// Inline SVG icons. No emoji anywhere in the UI.
const PATHS = {
  close: '<path d="M6 6l12 12M18 6L6 18"/>',
  play: '<path d="M8 5v14l11-7z"/>',
  bookmark: '<path d="M7 4h10v17l-5-4-5 4z"/>',
  check: '<path d="M5 12l5 5 9-10"/>',
};

export function icon(name, extra = '') {
  return `<svg class="ic ${extra}" viewBox="0 0 24 24" aria-hidden="true">${PATHS[name] ?? ''}</svg>`;
}
