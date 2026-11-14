import type { RecordItem, Indicator } from "../core-api";
import { el, numberText, timestamp, utc } from "./dom";
import { indicatorName, scalar, units } from "./core-format";

const ns = "http://www.w3.org/2000/svg";
export function sparkline(records: RecordItem[], indicator: Indicator, historical: boolean): HTMLElement {
  const figure = el("figure", "history-figure");
  const ordered = records.filter(record => record.effective && !record.future_timestamp && record.indicator === indicator && Number.isFinite(Date.parse(record.measured_at)))
    .sort((a, b) => Date.parse(a.measured_at) - Date.parse(b.measured_at));
  const points = ordered.map(record => ({ time: Date.parse(record.measured_at), value: "systolic" in record.value ? record.value.systolic : scalar(record.value), record }));
  const valid = points.filter((point): point is typeof point & { value: number } => point.value !== null);
  if (!valid.length) { figure.append(el("p", "empty-history", "No history")); return figure; }
  const first = valid[0];
  const last = valid[valid.length - 1];
  if (!first || !last) return figure;
  const min = Math.min(...valid.map(point => point.value));
  const max = Math.max(...valid.map(point => point.value));
  const unit = units[indicator];
  if (indicator === "blood_pressure") figure.append(el("p", "caption", "Systolic history · Full pressure readings are retained in the local record."));
  figure.append(el("figcaption", "", (historical ? "Historical readings" : "Recorded history") + " · " + valid.length + " samples · " + unit));
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 360 100");
  svg.setAttribute("class", "sparkline");
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", indicatorName(indicator) + ": " + valid.length + " recorded samples, " + numberText(min) + " to " + numberText(max) + " " + unit + ". Exact readings are listed below.");
  const x = (time: number): number => first.time === last.time ? 180 : 12 + ((time - first.time) / (last.time - first.time)) * 336;
  const y = (value: number): number => min === max ? 50 : 86 - ((value - min) / (max - min)) * 72;
  // A visual continuity rule only; it does not evaluate health. Break missing
  // samples and gaps above three times the median observed interval.
  const intervals = valid.slice(1).map((point, index) => point.time - valid[index].time).filter(delta => delta > 0).sort((a, b) => a - b);
  const gap = intervals.length ? intervals[Math.floor(intervals.length / 2)] * 3 : 0;
  let previous: { time: number; value: number } | null = null;
  for (const point of points) {
    if (point.value === null) { previous = null; continue; }
    if (previous && gap > 0 && point.time - previous.time <= gap) {
      const line = document.createElementNS(ns, "line");
      line.setAttribute("x1", String(x(previous.time))); line.setAttribute("y1", String(y(previous.value)));
      line.setAttribute("x2", String(x(point.time))); line.setAttribute("y2", String(y(point.value)));
      line.setAttribute("stroke", "currentColor"); line.setAttribute("stroke-width", "2");
      if (historical) line.setAttribute("stroke-dasharray", "4 3");
      svg.append(line);
    }
    const dot = document.createElementNS(ns, "circle");
    dot.setAttribute("cx", String(x(point.time))); dot.setAttribute("cy", String(y(point.value)));
    dot.setAttribute("r", "3"); dot.setAttribute("fill", "currentColor"); svg.append(dot);
    previous = { time: point.time, value: point.value };
  }
  figure.append(svg);
  const extent = el("p", "caption", "Plotted extent: " + numberText(min) + "–" + numberText(max) + " " + unit + ". Chart scale only, not a medical reference.");
  const period = el("p", "caption", utc(first.record.measured_at) + " — " + utc(last.record.measured_at));
  figure.append(period, extent, el("p", "caption", "Long gaps are left open. The trace ends at the last recorded sample."));
  const details = el("details");
  details.append(el("summary", "", "Read sample data"));
  const table = el("table", "sample-table");
  table.append(el("caption", "sr-only", indicatorName(indicator) + " recorded history"));
  const head = el("thead"); const header = el("tr");
  for (const title of ["Measured (UTC)", "Value (" + unit + ")"]) { const th = el("th", "", title); th.scope = "col"; header.append(th); }
  head.append(header); table.append(head);
  const body = el("tbody");
  for (const point of valid) {
    const row = el("tr"); const time = el("td");
    time.append(timestamp(point.record.measured_at)); row.append(time, el("td", "data", numberText(point.value))); body.append(row);
  }
  table.append(body); details.append(table); figure.append(details);
  return figure;
}
