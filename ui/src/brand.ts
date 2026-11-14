/// <reference types="vite/client" />

import "@fontsource/barlow-semi-condensed/latin-600.css";
import "@fontsource/public-sans/latin-400.css";
import "@fontsource/public-sans/latin-600.css";
import "@fontsource/roboto-mono/latin-400.css";
import "@fontsource/archivo/latin-600.css";
import "@fontsource/source-sans-3/latin-400.css";
import "@fontsource/source-sans-3/latin-600.css";
import "@fontsource/source-code-pro/latin-400.css";
import "@fontsource/libre-franklin/latin-400.css";
import "@fontsource/libre-franklin/latin-600.css";
import "../../docs/brand/tokens.css";
import "./brand.css";

// Standalone specimens only: no core, Tauri, network or persistence calls.
type StateKey = "ok" | "warn" | "alarm" | "not_evaluated" | "no_data" | "stale";
type IconKey = StateKey | "history" | "info" | "demo" | "sync";
interface SpecimenState {
  key: StateKey;
  label: string;
  contract: string;
  title: string;
  description: string;
  value: string;
  valueLabel: string;
}
interface Palette {
  id: string;
  name: string;
  summary: string;
  bg: string;
  surface: string;
  raised: string;
  text: string;
  muted: string;
  border: string;
  divider: string;
  accent: string;
  hover: string;
  pressed: string;
  on: string;
}
interface Pair {
  label: string;
  foreground: string;
  background: string;
  minimum: number;
}

function element<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (!found) throw new Error(`Brand specimen element missing: ${selector}`);
  return found;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => {
    const replacements: Record<string, string> = {
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
    };
    return replacements[character] ?? character;
  });
}

const iconPaths: Record<IconKey, string> = {
  ok: '<circle cx="12" cy="12" r="9"/><path d="m7.5 12 3 3 6-6"/>',
  warn: '<path d="M12 3 22 21H2Z M12 9v5 M12 17v.2"/>',
  alarm: '<path d="m8 2-6 6v8l6 6h8l6-6V8l-6-6Z M12 7v7 M12 17v.2"/>',
  not_evaluated: '<circle cx="12" cy="12" r="9"/><path d="M8 12h8"/>',
  no_data: '<rect x="5" y="3" width="14" height="18" rx="1"/><path d="M9 12h6"/>',
  stale: '<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>',
  history: '<path d="M5 20v-5 M12 20V9 M19 20V4"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6 M12 7v.2"/>',
  demo: '<rect x="3" y="5" width="18" height="14" rx="1"/><path d="m10 9 5 3-5 3Z"/>',
  sync: '<path d="M19 8a8 8 0 0 0-13-2L3 9 M3 4v5h5 M5 16a8 8 0 0 0 13 2l3-3 M21 20v-5h-5"/>',
};

function icon(key: IconKey): string {
  return `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${iconPaths[key]}</svg>`;
}

const states: SpecimenState[] = [
  { key: "ok", label: "Stable", contract: "stable / visual alias: ok", title: "No change detected", description: "No change detected in the evaluated history.", value: "72", valueLabel: "Illustrative reading" },
  { key: "warn", label: "Change", contract: "change / visual alias: warn", title: "Change detected", description: "A change was detected in the recent readings. Review the core evaluation.", value: "—", valueLabel: "Numeric reading omitted in this specimen" },
  { key: "alarm", label: "Alarm", contract: "alarm", title: "Alarm reported", description: "The core reported an alarm. Review the event details and escalation.", value: "—", valueLabel: "Numeric reading omitted in this specimen" },
  { key: "not_evaluated", label: "Not evaluated", contract: "not_evaluated", title: "Evaluation unavailable", description: "An evaluation is not available for this indicator. A recorded value may still be present.", value: "72", valueLabel: "Recorded value / no current evaluation" },
  { key: "no_data", label: "No data", contract: "not_evaluated / no_data", title: "No reading available", description: "No reading is available for this indicator.", value: "—", valueLabel: "No data" },
  { key: "stale", label: "Stale reading", contract: "not_evaluated / stale", title: "Last reading too old", description: "The last reading is too old for a current evaluation. Preserve its original measurement time.", value: "72", valueLabel: "Last recorded / not a current reading" },
];

function chip(key: StateKey): string {
  const state = states.find((entry) => entry.key === key);
  if (!state) return "";
  return `<span class="status-chip" data-state="${key}">${icon(key)}<span>${state.label}</span></span>`;
}

function neutralChip(label: string, symbol: IconKey): string {
  return `<span class="status-chip" data-state="not_evaluated">${icon(symbol)}<span>${escapeHtml(label)}</span></span>`;
}

const palettes: Palette[] = [
  { id: "basalt", name: "Basalt Standard", summary: "Approved. Warm grayscale. A quiet technical-document character with less colored surface area.", bg: "#171717", surface: "#222222", raised: "#2D2D2D", text: "#EEECE8", muted: "#C0BBB3", border: "#95918B", divider: "#47443F", accent: "#C2CDE0", hover: "#D4DCEC", pressed: "#ABBAD2", on: "#171717" },
  { id: "flight", name: "Flight Graphite", summary: "Alternative. Neutral operational surfaces and a restrained steel-blue action ink.", bg: "#10151B", surface: "#192129", raised: "#222D37", text: "#E8EDF2", muted: "#B2BDC8", border: "#81909F", divider: "#36424F", accent: "#A6C4E0", hover: "#BDD3E8", pressed: "#90B3D2", on: "#10151B" },
  { id: "navy", name: "Archive Navy", summary: "Ink-blue structure. The strongest publication-like presence of the three proposals.", bg: "#101827", surface: "#1A2638", raised: "#26354A", text: "#E8EDF5", muted: "#B6C2D3", border: "#8A9AB2", divider: "#3A4C65", accent: "#B8C7E5", hover: "#CFD9ED", pressed: "#A5B8DC", on: "#101827" },
];

const semantics = [
  { key: "ok" as const, label: "Stable", color: "#A5CCB3", background: "#1C3028" },
  { key: "warn" as const, label: "Change", color: "#E8C27D", background: "#352C1D" },
  { key: "alarm" as const, label: "Alarm", color: "#F0A2A2", background: "#382226" },
  { key: "not_evaluated" as const, label: "Neutral", color: "#B2BDC8", background: "#252E38" },
];

function luminance(hex: string): number {
  const channels = [1, 3, 5].map((offset) => {
    const channel = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

function contrast(foreground: string, background: string): number {
  const a = luminance(foreground);
  const b = luminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

function pairsFor(palette: Palette): Pair[] {
  const surfaces = [["background", palette.bg], ["surface", palette.surface], ["raised", palette.raised]];
  const pairs: Pair[] = [];
  for (const [name, background] of surfaces) {
    pairs.push(
      { label: `Text / ${name}`, foreground: palette.text, background, minimum: 4.5 },
      { label: `Muted / ${name}`, foreground: palette.muted, background, minimum: 4.5 },
      { label: `Control border / ${name}`, foreground: palette.border, background, minimum: 3 },
      { label: `Focus / ${name}`, foreground: palette.accent, background, minimum: 3 },
    );
  }
  for (const [name, background] of [["default", palette.accent], ["hover", palette.hover], ["pressed", palette.pressed]]) {
    pairs.push({ label: `Primary text / ${name}`, foreground: palette.on, background, minimum: 4.5 });
  }
  for (const semantic of semantics) {
    pairs.push(
      { label: `${semantic.label} / raised`, foreground: semantic.color, background: palette.raised, minimum: 4.5 },
      { label: `${semantic.label} / tint`, foreground: semantic.color, background: semantic.background, minimum: 4.5 },
    );
  }
  for (const [name, background] of [["default", "#F0A2A2"], ["hover", "#F5B7B7"], ["pressed", "#DF9090"]]) {
    pairs.push({ label: `Emergency text / ${name}`, foreground: "#10151B", background, minimum: 4.5 });
  }
  return pairs;
}

function renderPalettes(): void {
  element("#palette-grid").innerHTML = palettes.map((palette, index) => {
    const swatches = [
      ["Background", palette.bg], ["Surface", palette.surface], ["Raised", palette.raised],
      ["Text", palette.text], ["Muted", palette.muted], ["Border", palette.border],
      ["Divider", palette.divider], ["Action / focus", palette.accent],
      ["Hover", palette.hover], ["Pressed", palette.pressed], ["On action", palette.on],
    ];
    return `<article class="palette-card panel" style="--sample-bg:${palette.surface};--sample-text:${palette.text}">
      <p class="eyebrow">Palette ${String.fromCharCode(65 + index)}</p><h3>${palette.name}</h3><p>${palette.summary}</p>
      <div class="palette-sample"><span>MERKEN</span><span>Observation record</span></div>
      <dl class="swatch-list">${swatches.map(([name, color]) => `<div><dt><span class="swatch" style="background:${color}" aria-hidden="true"></span>${name}</dt><dd>${color}</dd></div>`).join("")}</dl>
      <div class="ratio-summary"><span>Text / surface <strong>${contrast(palette.text, palette.surface).toFixed(2)}:1</strong></span><span>Muted / surface <strong>${contrast(palette.muted, palette.surface).toFixed(2)}:1</strong></span><span>Border / raised <strong>${contrast(palette.border, palette.raised).toFixed(2)}:1</strong></span></div>
      <button class="button secondary" type="button" data-palette="${palette.id}" aria-pressed="${index === 0}">Preview ${palette.name}</button>
    </article>`;
  }).join("");
  element("#semantic-colors").innerHTML = semantics.map((semantic) => `<article class="semantic-swatch" style="color:${semantic.color};background:${semantic.background}">${icon(semantic.key)}<h4>${semantic.label}</h4><p>${semantic.color}<br />on ${semantic.background}</p><p>${contrast(semantic.color, semantic.background).toFixed(2)}:1 · AA text</p></article>`).join("");
  element("#contrast-matrix").innerHTML = palettes.map((palette) => `<details class="contrast-group"><summary>${palette.name} / ${pairsFor(palette).length} contrast pairs</summary><ul class="contrast-list">${pairsFor(palette).map((pair) => {
    const ratio = contrast(pair.foreground, pair.background);
    return `<li><span>${pair.label}<small>${pair.foreground} / ${pair.background}</small></span><strong>${ratio.toFixed(2)}:1</strong><span>${ratio >= pair.minimum ? "Pass" : "Fail"} · ${pair.minimum}:1</span></li>`;
  }).join("")}</ul></details>`).join("");
}

const typeOptions = [
  { id: "flight", name: "Flight Documentation", heading: "Barlow Semi Condensed", body: "Public Sans", data: "Roboto Mono", reason: "Approved. Disciplined headings, open body text and explicit instrument figures." },
  { id: "record", name: "Technical Record", heading: "Archivo", body: "Source Sans 3", data: "Source Code Pro", reason: "Broader headings and a compact record-like reading rhythm. Test perceived body size on device." },
  { id: "institutional", name: "Institutional Standard", heading: "Libre Franklin", body: "Libre Franklin", data: "Roboto Mono", reason: "A consistent public-service voice. Two families; hierarchy relies on size and weight." },
];

function renderType(): void {
  element("#type-options").innerHTML = typeOptions.map((option, index) => `<article class="panel type-option" style="--preview-heading:'${option.heading}';--preview-body:'${option.body}';--preview-data:'${option.data}'"><p class="eyebrow">Proposal ${String.fromCharCode(65 + index)} / OFL-1.1</p><h3>${option.name}</h3><p class="type-option-copy">${option.reason}</p><p class="type-option-data">0123456789<br />72 bpm · 14:32 UTC</p><dl class="type-family-list"><div><dt>Headings / 600</dt><dd>${option.heading}</dd></div><div><dt>Body / 400, 600</dt><dd>${option.body}</dd></div><div><dt>Figures / 400</dt><dd>${option.data}</dd></div></dl><button class="button secondary" type="button" data-type="${option.id}" aria-pressed="${index === 0}">Preview proposal ${String.fromCharCode(65 + index)}</button></article>`).join("");
}

function wordmark(mono: boolean): string {
  return `<svg viewBox="0 0 520 180" role="img" aria-label="MERKEN" xmlns="http://www.w3.org/2000/svg"><text x="52" y="126" fill="${mono ? "currentColor" : "#E8EDF2"}" font-family="Barlow Semi Condensed, Arial Narrow, sans-serif" font-size="100" font-weight="600" letter-spacing="6">MERKEN</text></svg>`;
}

function renderStateSpecimens(): void {
  element("#vital-grid").innerHTML = states.map((state) => `<article class="vital-card panel" data-state="${state.key}"><div class="vital-heading"><h4>Heart rate</h4>${chip(state.key)}</div><p class="reading">${state.value}${state.value !== "—" ? '<span class="unit">bpm</span>' : ""}</p><p class="value-caption">${state.valueLabel}</p><p>${state.description}</p><p class="caption">${state.key === "no_data" ? "No measurement time" : `Measured <time datetime="${state.key === "stale" ? "2026-11-10" : "2026-11-14"}T14:32:00Z">${state.key === "stale" ? "10" : "14"} Nov 2026, 14:32 UTC</time>`}</p><p class="source-label">DEMO · Demo rule, not medical advice.</p><code>${state.contract}</code></article>`).join("");
  element("#all-chips").innerHTML = states.map((state) => chip(state.key)).join("") + neutralChip("Insufficient history", "history") + neutralChip("Context only", "info") + neutralChip("DEMO", "demo") + neutralChip("Sync pending", "sync");
  element("#notice-grid").innerHTML = states.map((state) => `<article class="notice" data-state="${state.key}"><div class="notice-heading">${icon(state.key)}<h4>${state.title}</h4></div><p>${state.description}</p>${state.key === "alarm" ? '<p class="caption">Need help event specimen · Urgent<br />Saved locally · Synchronization pending<br />Ground receipt is not confirmed.</p>' : '<p class="caption">DEMO · Demo rule, not medical advice.</p>'}<button class="button secondary" type="button" data-preview="${state.key === "alarm" ? "Event preview only. No event service is connected." : "Detail preview only. No health service is connected."}">${state.key === "alarm" ? "View event" : "View details"}</button></article>`).join("");
  element("#state-options").innerHTML = states.map((state, index) => `<button class="button secondary" type="button" data-select-state="${state.key}" aria-pressed="${index === 0}">${icon(state.key)}${state.label}</button>`).join("");
}

const chartValues = [72, 73, 72, 74, 73, 72];
const chartTimes = ["14:07", "14:12", "14:17", "14:22", "14:27", "14:32"];

function renderHistory(state: SpecimenState): string {
  if (state.key === "no_data") return `${chip(state.key)}<div class="empty-plot">${icon("no_data")}<p>No samples to plot.</p></div><p class="caption">A missing reading is not a zero.</p>`;
  if (state.key === "warn" || state.key === "alarm") return `${chip(state.key)}<div class="empty-plot"><p>Numeric series omitted in this specimen.</p></div><p class="caption">In the app, plot only the records associated with the core evaluation. No clinical trigger values are invented here.</p>`;
  if (state.key === "not_evaluated") return `${chip(state.key)}<svg class="sparkline neutral-line" viewBox="0 0 360 100" role="img" aria-label="One illustrative sample: 72 beats per minute at 14:32 UTC. No trend can be plotted."><circle cx="180" cy="50" r="4" fill="currentColor" /></svg><p>One illustrative reading: 72 bpm at 14:32 UTC.</p><p class="caption">Need 3 previous readings (0/3). Counts are specimen values, not a new rule.</p>`;
  const historical = state.key === "stale";
  const points = chartValues.map((value, index) => `${24 + index * 62},${80 - (value - 72) * 30}`).join(" ");
  return `${chip(state.key)}<p class="caption">${historical ? "Historical samples / 10 Nov 2026" : "Illustrative samples / 14 Nov 2026"}<br />14:07–14:32 UTC · Heart rate (bpm)</p><svg class="sparkline" data-state="${state.key}" viewBox="0 0 360 100" role="img" aria-label="Illustrative readings in beats per minute: 72, 73, 72, 74, 73, 72, from 14:07 to 14:32 UTC. ${historical ? "Historical only; not current." : "Not real telemetry."}"><polyline points="${points}" fill="none" stroke="currentColor" stroke-width="2" ${historical ? 'stroke-dasharray="5 4"' : ""}/>${chartValues.map((value, index) => `<circle cx="${24 + index * 62}" cy="${80 - (value - 72) * 30}" r="3" fill="currentColor"/>`).join("")}</svg><div class="chart-axis"><span>14:07 UTC</span><span>14:32 UTC</span></div><p class="caption">Plotted extent: 72 to 74 bpm. Chart scale only, not a medical reference. ${historical ? "The trace stops at the last recorded sample." : "Six fictional readings; no clinical thresholds."}</p><details><summary>Read sample data</summary><table class="sample-table"><caption>Illustrative readings · ${historical ? "10" : "14"} Nov 2026</caption><thead><tr><th scope="col">Time (UTC)</th><th scope="col">Heart rate (bpm)</th></tr></thead><tbody>${chartValues.map((value, index) => `<tr><td>${chartTimes[index]}</td><td>${value}</td></tr>`).join("")}</tbody></table></details>`;
}

let selectedState: StateKey = "ok";
let dialogOpener: HTMLElement | null = null;

function currentState(): SpecimenState {
  return states.find((state) => state.key === selectedState) ?? states[0];
}

function updateExplorer(): void {
  const state = currentState();
  element("#crew-status").innerHTML = chip(state.key);
  element("#help-context").innerHTML = chip(state.key);
  element("#history-preview").innerHTML = renderHistory(state);
  let emptyHeading = "No additional history";
  let emptyCopy = "No additional records are available in this selected history view. The current evaluation remains visible.";
  if (state.key === "no_data") {
    emptyHeading = "No reading available";
    emptyCopy = "Record a reading to begin. No value or trend is inferred.";
  } else if (state.key === "not_evaluated") {
    emptyHeading = "Insufficient history";
    emptyCopy = "Need 3 previous readings (0/3). These counts illustrate the core fields; they do not define a requirement.";
  } else if (state.key === "stale") {
    emptyHeading = "No current reading";
    emptyCopy = "A historical record is available. It is too old for a current evaluation.";
  }
  element("#empty-preview").innerHTML = `${chip(state.key)}<div class="empty-state">${icon(state.key === "not_evaluated" ? "history" : "no_data")}<h5>${emptyHeading}</h5><p>${emptyCopy}</p><button class="button secondary" type="button" data-preview="${state.key === "no_data" ? "Entry preview only. No measurement will be saved." : "History preview only. No record service is connected."}">${state.key === "no_data" ? "Record a reading" : "View history"}</button></div>`;
  document.querySelectorAll<HTMLButtonElement>("[data-select-state]").forEach((button) => {
    button.setAttribute("aria-pressed", String(button.dataset.selectState === selectedState));
  });
}

function renderButtons(): void {
  const kinds = [
    { className: "primary", label: "Record a reading", busy: "Saving…", name: "Primary" },
    { className: "secondary", label: "View history", busy: "Loading…", name: "Secondary" },
    { className: "emergency", label: "Need help", busy: "Recording…", name: "Emergency" },
  ];
  const interactions = ["Default", "Hover", "Pressed", "Focus", "Busy", "Disabled"];
  element("#button-specimens").innerHTML = kinds.map((kind) => `<section class="button-family"><h4>${kind.name}</h4><div class="button-state-grid">${interactions.map((interaction) => {
    const unavailable = interaction === "Busy" || interaction === "Disabled";
    const label = interaction === "Busy" ? kind.busy : kind.label;
    return `<div class="button-state"><span class="caption">${interaction}</span><button class="button ${kind.className} specimen-${interaction.toLowerCase()}" type="button" ${unavailable ? "disabled" : 'data-preview="Interaction specimen only. No action was submitted."'} ${interaction === "Busy" ? 'aria-busy="true"' : ""}>${kind.className === "emergency" ? icon("alarm") : ""}${label}</button>${unavailable ? `<small>${interaction === "Busy" ? "Submission in progress; duplicate action blocked." : "Local service unavailable; action cannot be submitted."} Static specimen.</small>` : ""}</div>`;
  }).join("")}</div></section>`).join("");
  element("#dialog-results").innerHTML = [
    { icon: "sync" as const, title: "Recording request…", copy: "Busy specimen. Keep the dialog open and block duplicate submission. Do not report success yet." },
    { icon: "no_data" as const, title: "Help request not saved", copy: "Failure specimen. The request could not be saved. Keep entered content and provide Try again and Cancel." },
    { icon: "sync" as const, title: "Help request saved locally", copy: "Result copy specimen. Synchronization pending. Ground receipt is not confirmed. Show this only after a successful core response." },
  ].map((result) => `<article class="panel">${icon(result.icon)}<h4>${result.title}</h4><p>${result.copy}</p><p class="caption">Preview only / no request created</p></article>`).join("");
}

const voiceExamples: Array<{ context: string; heading: string; copy: string; note: string }> = [
  { context: "Stable", heading: "Stable", copy: "No change detected in the evaluated history.", note: "Avoid: All clear. Stable is scoped to an evaluation." },
  { context: "Change", heading: "Change detected", copy: "A change was detected in the recent readings.", note: "Review readings. Direction and persistence come from the core." },
  { context: "Alarm", heading: "Alarm", copy: "The core reported an alarm. Review the event details.", note: "Show escalation separately. Do not infer a diagnosis." },
  { context: "No data", heading: "No data", copy: "No reading is available for this indicator.", note: "Record a reading. Never substitute a zero." },
  { context: "Stale", heading: "Stale reading", copy: "Last reading: 10 Nov 2026, 14:32 UTC. It is too old for a current evaluation.", note: "Illustrative timestamp. Preserve the original measurement time." },
  { context: "Insufficient history", heading: "Insufficient history", copy: "Need {history_needed} previous readings ({history_count}/{history_needed}).", note: "Use core counts. The latest reading is not a previous reading." },
  { context: "Connection error", heading: "Connection unavailable", copy: "This request could not be completed. Local records remain available.", note: "Try again. Use this for a connection request, not a failed local save." },
  { context: "Need help / confirmation", heading: "Record an urgent help request?", copy: "This will save an urgent event on this device. Ground receipt is not confirmed.", note: "Cancel / Record help request. Never say: Help is on the way." },
  { context: "DEMO", heading: "DEMO", copy: "Simulated records. Not uploaded to ground.", note: "For demo_heuristic: Demo rule, not medical advice." },
  { context: "Pending sync", heading: "Synchronization pending", copy: "3 records pending. Saved locally; not yet synchronized.", note: "Illustrative count. Do not promise a delivery time." },
  { context: "Not evaluated", heading: "Not evaluated", copy: "An evaluation is not available for this indicator.", note: "Keep the recorded value when one exists. Do not imply Stable." },
  { context: "Context only", heading: "Context only", copy: "Recorded for context; no health evaluation applies.", note: "Neutral presentation. No health-status badge." },
  { context: "First real record", heading: "Demo data will be removed", copy: "Saving real data will remove the demo data.", note: "Cancel / Save real data. Include this consequence for real help events too." },
  { context: "Help event / pending", heading: "Help request saved locally", copy: "Synchronization pending. Ground receipt is not confirmed.", note: "Persistent alarm. No dismissal or resolution claim." },
  { context: "Local service error", heading: "Local service unavailable", copy: "The local service could not be reached. This action was not confirmed.", note: "Try again. Keep the user's entry." },
  { context: "Ground review", heading: "Flagged for ground review", copy: "Synchronization pending. Ground receipt is not confirmed.", note: "A flag is not a delivery receipt or a human review." },
];

function renderVoice(): void {
  element("#voice-grid").innerHTML = voiceExamples.map((example) => `<article class="voice-example"><p class="eyebrow">${escapeHtml(example.context)}</p><h3>${escapeHtml(example.heading)}</h3><p class="voice-copy">${escapeHtml(example.copy)}</p><p class="caption">${escapeHtml(example.note)}</p></article>`).join("");
}

function announce(message: string): void {
  element("#preview-feedback").textContent = message;
}

function bindInteractions(): void {
  document.addEventListener("click", (event: MouseEvent) => {
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest<HTMLButtonElement>("button");
    if (!button || button.disabled) return;
    const palette = palettes.find((entry) => entry.id === button.dataset.palette);
    if (palette) {
      const tokens: Record<string, string> = { bg: palette.bg, surface: palette.surface, "surface-raised": palette.raised, text: palette.text, "text-muted": palette.muted, border: palette.border, divider: palette.divider, accent: palette.accent, "accent-hover": palette.hover, "accent-pressed": palette.pressed, "on-accent": palette.on, focus: palette.accent };
      Object.entries(tokens).forEach(([key, value]) => document.documentElement.style.setProperty(`--mk-${key}`, value));
      document.querySelectorAll<HTMLButtonElement>("[data-palette]").forEach((control) => control.setAttribute("aria-pressed", String(control.dataset.palette === palette.id)));
      announce(`Palette selected: ${palette.name}. Semantic colors are unchanged.`);
    }
    const typography = typeOptions.find((entry) => entry.id === button.dataset.type);
    if (typography) {
      document.documentElement.style.setProperty("--mk-font-heading", `"${typography.heading}", Arial, sans-serif`);
      document.documentElement.style.setProperty("--mk-font-body", `"${typography.body}", Arial, sans-serif`);
      document.documentElement.style.setProperty("--mk-font-data", `"${typography.data}", Consolas, monospace`);
      document.querySelectorAll<HTMLButtonElement>("[data-type]").forEach((control) => control.setAttribute("aria-pressed", String(control.dataset.type === typography.id)));
      announce(`Typography selected: ${typography.name}. The wordmark is unchanged.`);
    }
    const state = states.find((entry) => entry.key === button.dataset.selectState);
    if (state) {
      selectedState = state.key;
      updateExplorer();
      announce(`State specimen selected: ${state.label}. No health evaluation was performed.`);
    }
    if (button.dataset.preview) announce(button.dataset.preview);
  });

  element<HTMLSelectElement>("#crew-select").addEventListener("change", (event) => {
    const control = event.currentTarget;
    if (!(control instanceof HTMLSelectElement)) return;
    const crewId = control.value === "02" ? "02" : "01";
    element("#crew-summary").textContent = `Crew member ${crewId} · Simulation`;
    announce(`Fictional crew member ${crewId} selected. No real data loaded.`);
  });

  const dialog = element<HTMLDialogElement>("#help-dialog");
  const openButton = element<HTMLButtonElement>("#help-open");
  openButton.innerHTML = `${icon("alarm")}Need help`;
  openButton.addEventListener("click", () => {
    dialogOpener = openButton;
    element("#dialog-crew").textContent = element("#crew-summary").textContent;
    element("#dialog-context").innerHTML = chip(selectedState);
    dialog.showModal();
    element<HTMLButtonElement>("#help-cancel").focus();
  });
  element("#help-confirm").addEventListener("click", () => {
    dialog.close("preview-confirmed");
    announce("Confirmation preview complete. No help request was saved or sent.");
  });
  dialog.addEventListener("close", () => {
    if (dialog.returnValue !== "preview-confirmed") announce("Help preview cancelled. No request was created.");
    dialog.returnValue = "";
    dialogOpener?.focus();
  });
}

element("#wordmark-primary").innerHTML = wordmark(false);
element("#wordmark-mono").innerHTML = wordmark(true);
renderPalettes();
renderType();
renderStateSpecimens();
renderButtons();
renderVoice();
updateExplorer();
bindInteractions();
