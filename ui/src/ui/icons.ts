export type IconName = "ok" | "warn" | "alarm" | "neutral" | "no_data" | "stale" | "history" | "info" | "demo" | "sync" | "crew" | "left" | "right" | "close" | "chevron";
const paths: Record<IconName, string> = {
  ok: '<circle cx="12" cy="12" r="9"/><path d="m7.5 12 3 3 6-6"/>',
  warn: '<path d="M12 3 22 21H2Z M12 9v5 M12 17v.2"/>',
  alarm: '<path d="m8 2-6 6v8l6 6h8l6-6V8l-6-6Z M12 7v7 M12 17v.2"/>',
  neutral: '<circle cx="12" cy="12" r="9"/><path d="M8 12h8"/>',
  no_data: '<rect x="5" y="3" width="14" height="18" rx="1"/><path d="M9 12h6"/>',
  stale: '<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>',
  history: '<path d="M5 20v-5 M12 20V9 M19 20V4"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6 M12 7v.2"/>',
  demo: '<rect x="3" y="5" width="18" height="14" rx="1"/><path d="m10 9 5 3-5 3Z"/>',
  sync: '<path d="M19 8a8 8 0 0 0-13-2L3 9 M3 4v5h5 M5 16a8 8 0 0 0 13 2l3-3 M21 20v-5h-5"/>',
  crew: '<circle cx="12" cy="7" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2"/>',
  left: '<path d="m14 5-7 7 7 7"/>',
  right: '<path d="m10 5 7 7-7 7"/>',
  close: '<path d="m6 6 12 12 M18 6 6 18"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
};
export function icon(name: IconName): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", "icon");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  // Fixed, author-owned SVG geometry only. No API content enters markup.
  svg.innerHTML = paths[name];
  return svg;
}
