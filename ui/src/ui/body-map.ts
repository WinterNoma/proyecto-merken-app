import type { BodyZone, StatusResponse } from "../core-api";
import { element, el, setText } from "./dom";
import { badge, statusCopy, paintBadge } from "./status-copy";
import { modal } from "./dialog";
import humanBodyUrl from "../assets/body-reference.webp";
import humanGeometry from "../assets/body-reference-geometry.json";

// Invisible regions over the generated cutout. Its alpha-derived contour clips
// highlights; the illustration itself is never reconstructed as vector art.
// Anatomical left is the viewer's right.
const shapes: Record<BodyZone, string | null> = {
  head: "M58 0H122V59H58Z",
  neck: "M68 59H112V78H68Z",
  chest: "M48 78H132L124 110L122 149H58L56 110Z",
  abdomen: "M58 149H122L135 222H45Z",
  right_arm: "M48 78L56 110L58 149L37 208H12L17 144L28 87Z",
  left_arm: "M132 78L124 110L122 149L143 208H168L163 144L152 87Z",
  right_hand: "M0 208H40V254H0Z",
  left_hand: "M140 208H180V254H140Z",
  right_leg: "M45 222H90V393H46Z",
  left_leg: "M90 222H135L134 393H90Z",
  right_foot: "M45 393H90V435H45Z",
  left_foot: "M90 393H135V435H90Z",
  back: null, other: null,
};
export const bodyZones = Object.keys(shapes) as BodyZone[];
const smallZones = new Set<BodyZone>(["neck", "right_hand", "left_hand", "right_foot", "left_foot"]);
export function zoneName(zone: BodyZone): string {
  return zone.replace(/_/g, " ").replace(/^./, letter => letter.toUpperCase());
}
export function createBodyMap(onSelect: (zone: BodyZone) => void) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `0 0 ${humanGeometry.width} ${humanGeometry.height}`);
  svg.setAttribute("class", "body-map");
  svg.setAttribute("role", "group");
  svg.setAttribute("aria-label", "Body zones, front view");
  const defs = document.createElementNS(ns, "defs");
  const clip = document.createElementNS(ns, "clipPath");
  clip.id = "human-body-clip";
  const outline = document.createElementNS(ns, "path");
  outline.setAttribute("d", humanGeometry.outline); clip.append(outline); defs.append(clip);
  const image = document.createElementNS(ns, "image");
  image.setAttribute("href", humanBodyUrl);
  image.setAttribute("width", String(humanGeometry.width));
  image.setAttribute("height", String(humanGeometry.height));
  image.setAttribute("preserveAspectRatio", "xMidYMid meet");
  image.setAttribute("class", "body-image");
  image.setAttribute("aria-hidden", "true"); image.setAttribute("pointer-events", "none");
  const fallback = outline.cloneNode() as SVGPathElement;
  fallback.setAttribute("class", "body-fallback");
  fallback.setAttribute("aria-hidden", "true"); fallback.setAttribute("pointer-events", "none");
  svg.append(defs, image, fallback);
  const hits = document.createElementNS(ns, "g");
  hits.setAttribute("aria-hidden", "true");
  const regions = document.createElementNS(ns, "g");
  regions.setAttribute("clip-path", "url(#human-body-clip)");
  svg.append(hits, regions);
  const nodes = new Map<BodyZone, SVGPathElement | HTMLButtonElement>();
  const rows = new Map<BodyZone, HTMLButtonElement>();
  const labels = new Map<BodyZone, HTMLElement>();
  let selected: BodyZone | null = null;
  const dialog = element<HTMLDialogElement>("#zones-dialog");
  const controls = modal(dialog);
  element("#zones-open").addEventListener("click", () => controls.open(element("#zones-close")));
  element("#zones-close").addEventListener("click", controls.close);
  function select(zone: BodyZone): void {
    selected = zone;
    for (const [key, node] of nodes) node.setAttribute("aria-pressed", String(key === zone));
    for (const [key, node] of rows) node.setAttribute("aria-pressed", String(key === zone));
    setText(element("#zone-selection"), zoneName(zone));
  }
  for (const zone of bodyZones) {
    const geometry = shapes[zone];
    const node = geometry ? document.createElementNS(ns, "path") : el("button", "zone-extra", zoneName(zone));
    node.setAttribute("role", "button"); node.setAttribute("tabindex", "0");
    node.setAttribute("aria-pressed", "false"); node.dataset.zone = zone;
    node.dataset.tone = "neutral";
    if (node instanceof SVGElement) {
      node.setAttribute("d", geometry!); node.setAttribute("class", "body-zone"); regions.append(node);
      if (smallZones.has(zone)) {
        // Invisible, wider hit area behind every visible zone, so neighbours drawn
        // on top keep their own taps; the anatomy stays unchanged.
        const hit = document.createElementNS(ns, "path");
        hit.setAttribute("d", geometry!); hit.setAttribute("class", "zone-hit"); hit.setAttribute("aria-hidden", "true");
        hit.addEventListener("click", () => { select(zone); onSelect(zone); });
        hits.append(hit);
      }
      node.addEventListener("keydown", event => {
        if (event.key === "Enter" || event.key === " ") { event.preventDefault(); select(zone); onSelect(zone); }
      });
    } else { node.type = "button"; element("#body-extra").append(node); }
    node.addEventListener("click", () => { select(zone); onSelect(zone); });
    nodes.set(zone, node);
    const row = el("button", "zone-row"); row.type = "button";
    row.append(el("span", "", zoneName(zone)));
    const label = badge(statusCopy("not_evaluated")); row.append(label);
    row.addEventListener("click", () => { select(zone); onSelect(zone); controls.close(); });
    rows.set(zone, row); labels.set(zone, label); element("#body-zone-list").append(row);
  }
  element("#body-map").append(svg);
  for (const status of ["stable", "change", "alarm", "not_evaluated"] as const) element("#body-legend").append(badge(statusCopy(status)));
  function update(zones: StatusResponse["body_zones"]): void {
    for (const zone of bodyZones) {
      const status = zones.find(entry => entry.body_zone === zone)?.status ?? "not_evaluated";
      const copy = statusCopy(status);
      const node = nodes.get(zone)!;
      node.dataset.tone = copy.tone;
      node.setAttribute("aria-label", zoneName(zone) + ": " + copy.label);
      node.setAttribute("aria-pressed", String(selected === zone));
      rows.get(zone)!.setAttribute("aria-label", zoneName(zone) + ": " + copy.label);
      paintBadge(labels.get(zone)!, copy);
    }
  }
  update([]);
  return { update, select };
}
