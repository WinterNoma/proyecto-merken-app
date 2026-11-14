import type { BodyZone, StatusResponse } from "../core-api";
import { element, el, setText } from "./dom";
import { badge, statusCopy, paintBadge } from "./status-copy";
import { modal } from "./dialog";

// Anatomical left is the viewer's right. Back and other have explicit controls.
const shapes: Record<BodyZone, string | null> = {
  head: "M90 10C76 10 69 21 70 36L73 48Q77 59 90 61Q103 59 107 48L110 36C111 21 104 10 90 10Z",
  neck: "M80 61L80 72L75 77Q90 83 105 77L100 72L100 61Q90 66 80 61Z",
  chest: "M73 79Q60 81 56 89L63 125L66 145Q90 152 114 145L117 125L124 89Q120 81 107 79Q90 88 73 79Z",
  abdomen: "M66 149Q90 156 114 149L115 178Q122 195 115 212L96 219L90 212L84 219L65 212Q58 195 65 178Z",
  right_arm: "M53 89Q43 95 40 116L32 149L22 182L34 187L49 155L58 127Z",
  left_arm: "M127 89Q137 95 140 116L148 149L158 182L146 187L131 155L122 127Z",
  right_hand: "M21 186L33 191L31 206L25 220Q22 224 19 219L14 213L15 199Z",
  left_hand: "M159 186L147 191L149 206L155 220Q158 224 161 219L166 213L165 199Z",
  right_leg: "M65 216L85 223L88 226L85 269L81 293L78 338L64 338L60 293L59 265Z",
  left_leg: "M115 216L95 223L92 226L95 269L99 293L102 338L116 338L120 293L121 265Z",
  right_foot: "M64 342L78 342L79 356Q78 362 69 362L51 362Q46 358 53 353Z",
  left_foot: "M102 342L116 342L127 353Q134 358 129 362L111 362Q102 362 101 356Z",
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
  svg.setAttribute("viewBox", "0 0 180 375");
  svg.setAttribute("class", "body-map");
  svg.setAttribute("role", "group");
  svg.setAttribute("aria-label", "Body zones, front view");
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
      node.setAttribute("d", geometry!); node.setAttribute("class", "body-zone"); svg.append(node);
      if (smallZones.has(zone)) {
        // Invisible, wider hit area behind every visible zone, so neighbours drawn
        // on top keep their own taps; the anatomy stays unchanged.
        const hit = document.createElementNS(ns, "path");
        hit.setAttribute("d", geometry!); hit.setAttribute("class", "zone-hit"); hit.setAttribute("aria-hidden", "true");
        hit.addEventListener("click", () => { select(zone); onSelect(zone); });
        svg.insertBefore(hit, svg.firstChild);
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
