import type { BodyZone, StatusResponse } from "../core-api";
import { element, el } from "./dom";
import { badge, statusCopy, paintBadge } from "./status-copy";

const shapes: Record<BodyZone, string> = {
  head: "M85 32a25 25 0 1 0 50 0a25 25 0 1 0-50 0",
  neck: "M99 61h22v16H99Z",
  chest: "M77 82h66l-5 67H82Z",
  abdomen: "M82 155h56l6 61-15 15H91l-15-15Z",
  right_arm: "M70 85 51 97 29 197 46 202 72 137Z",
  left_arm: "m150 85 19 12 22 100-17 5-26-65Z",
  right_hand: "m28 203 17 5-4 30-20-5Z",
  left_hand: "m192 203-17 5 4 30 20-5Z",
  right_leg: "M89 237h18l-6 129H80Z",
  left_leg: "M113 237h18l9 129h-21Z",
  right_foot: "M80 372h21v17H69v-9Z",
  left_foot: "M119 372h21l11 8v9h-32Z",
  back: "M230 100h30v56h-30Z",
  other: "M230 215h30v30h-30Z",
};
export function zoneName(zone: BodyZone): string {
  return zone.replace(/_/g, " ").replace(/^./, letter => letter.toUpperCase());
}
export function createBodyMap() {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 290 405");
  svg.setAttribute("class", "body-map");
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", "Symptom zone overview. Exact zone states are listed under View all body zones.");
  const nodes = new Map<BodyZone, SVGPathElement>();
  for (const [zone, geometry] of Object.entries(shapes) as [BodyZone, string][]) {
    const node = document.createElementNS(ns, "path");
    node.setAttribute("d", geometry); node.setAttribute("class", "body-zone");
    node.dataset.tone = "neutral";
    svg.append(node); nodes.set(zone, node);
  }
  for (const [name, y] of [["Back", 90], ["Other", 205]] as const) {
    const text = document.createElementNS(ns, "text");
    text.setAttribute("x", "245"); text.setAttribute("y", String(y));
    text.setAttribute("text-anchor", "middle"); text.setAttribute("class", "map-label");
    text.textContent = name; svg.append(text);
  }
  element("#body-map").append(svg);
  for (const status of ["stable", "change", "alarm", "not_evaluated"] as const) element("#body-legend").append(badge(statusCopy(status)));
  const rows = new Map<BodyZone, HTMLElement>();
  for (const zone of Object.keys(shapes) as BodyZone[]) {
    const row = el("li"); row.append(el("span", "", zoneName(zone)));
    const state = badge(statusCopy("not_evaluated")); row.append(state);
    rows.set(zone, state); element("#body-zone-list").append(row);
  }
  return (zones: StatusResponse["body_zones"]): void => {
    for (const zone of Object.keys(shapes) as BodyZone[]) {
      const status = zones.find(entry => entry.body_zone === zone)?.status ?? "not_evaluated";
      const copy = statusCopy(status);
      const node = nodes.get(zone);
      if (node) {
        node.dataset.tone = copy.tone;
        const title = document.createElementNS(ns, "title");
        title.textContent = zoneName(zone) + ": " + copy.label; node.replaceChildren(title);
      }
      const row = rows.get(zone);
      if (row) paintBadge(row, copy);
    }
  };
}
