export type IconName = "ok" | "warn" | "alarm" | "neutral" | "no_data" | "stale" | "history" | "info" | "demo" | "sync" | "crew" | "left" | "right" | "close" | "chevron" | "pulse" | "pressure" | "oxygen" | "mood" | "temperature" | "glucose" | "sleep" | "nutrition" | "radiation" | "earth";
const paths: Record<IconName, string> = {
  pulse: '<path d="M20.5 4.5a5.3 5.3 0 0 0-7.5 0L12 5.6l-1.1-1.1a5.3 5.3 0 0 0-7.5 7.5L12 21l8.5-9a5.3 5.3 0 0 0 0-7.5Z"/>',
  pressure: '<rect x="3" y="3" width="11" height="13" rx="3"/><path d="M8 7v5M14 10h3a4 4 0 0 1 4 4v3a4 4 0 0 1-8 0v-1"/>',
  oxygen: '<path d="M12 3c-2 4-7 8-7 12a7 7 0 0 0 14 0c0-4-5-8-7-12Z"/><path d="M8 15a4 4 0 0 0 4 4"/>',
  mood: '<circle cx="12" cy="12" r="9"/><path d="M8 14s1 3 4 3 4-3 4-3M8 9h.01M16 9h.01"/>',
  temperature: '<path d="M9 14.5V5a3 3 0 0 1 6 0v9.5a5 5 0 1 1-6 0Z M12 8v10"/>',
  glucose: '<path d="M12 2C10 7 5 11 5 15a7 7 0 0 0 14 0c0-4-5-8-7-13Z"/>',
  sleep: '<path d="M21 13a9 9 0 0 1-10-10A9 9 0 1 0 21 13Z"/>',
  nutrition: '<path d="M4 3v6a3 3 0 0 0 6 0V3M7 3v18M16 21V3c4 2 5 7 5 11h-5"/>',
  radiation: '<circle cx="12" cy="12" r="2"/><path d="m9 8-3-5a10 10 0 0 0-4 7h6M15 8l3-5a10 10 0 0 1 4 7h-6M9 16l-3 5a10 10 0 0 0 12 0l-3-5"/>',
  earth: '<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18M5 6h14M5 18h14"/>',
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
