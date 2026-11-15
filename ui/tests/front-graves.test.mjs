import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { after, test } from "node:test";
import { rolldown } from "rolldown";

const root = path.resolve(import.meta.dirname, "../..");
const temp = await fs.mkdtemp(path.join(os.tmpdir(), "merken-front-graves-"));
const modules = {
  core: "core-api", panel: "ui/core-panel", dashboard: "ui/dashboard",
  statuses: "ui/status-copy", vital: "vital-status", telemetry: "ui/telemetry", vitals: "ui/vitals",
};
const bundle = await rolldown({
  input: Object.fromEntries(Object.entries(modules).map(([key, file]) => [key, path.join(root, "ui/src", file + ".ts")])),
  plugins: [{ name: "test-image", load(id) { if (id.endsWith(".webp")) return 'export default "body.webp";'; } }],
});
await bundle.write({ dir: temp, format: "esm", entryFileNames: "[name].mjs" });
await bundle.close();
const loaded = Object.fromEntries(await Promise.all(Object.keys(modules).map(async key => [key,
  await import(pathToFileURL(path.join(temp, key + ".mjs")))])));
const { core } = loaded.core;
const { createCorePanel } = loaded.panel;
const { createDashboard } = loaded.dashboard;
const { mysqlStatuses, crewStatus, evaluationCopy } = loaded.statuses;
const { parseCrew, partitionCrewReadings, validateReading, createTelemetryFeed } = loaded.telemetry;
after(() => fs.rm(temp, { recursive: true, force: true }));

// Only DOM infrastructure is simulated; panel, dashboard and copy logic are real.
class Node {
  constructor(tag = "div") {
    this.tagName = tag.toUpperCase(); this.children = []; this.dataset = {}; this.attrs = new Map();
    this.listeners = new Map(); this._text = ""; this.className = ""; this.hidden = false;
    this.classList = {
      toggle: (name, enabled) => { const set = new Set(this.className.split(/\s+/).filter(Boolean));
        if (enabled ?? !set.has(name)) set.add(name); else set.delete(name); this.className = [...set].join(" "); },
      add: name => this.classList.toggle(name, true),
    };
  }
  set textContent(value) { this.children = []; this._text = String(value ?? ""); }
  get textContent() { return this._text + this.children.map(node => node.textContent).join(""); }
  append(...nodes) { for (let node of nodes) { if (typeof node === "string") { const text = new Node(); text.textContent = node; node = text; }
    node.remove(); node.parent = this; this.children.push(node); } }
  replaceChildren(...nodes) { this.children.forEach(node => { node.parent = null; }); this.children = []; this._text = ""; this.append(...nodes); }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(node => node !== this); this.parent = null; }
  setAttribute(name, value) { this.attrs.set(name, String(value)); if (name === "class") this.className = value; }
  getAttribute(name) { return this.attrs.get(name) ?? null; }
  removeAttribute(name) { this.attrs.delete(name); }
  addEventListener(name, fn) { this.listeners.set(name, [...(this.listeners.get(name) ?? []), fn]); }
  querySelector(selector) { return this.querySelectorAll(selector)[0] ?? null; }
  querySelectorAll(selector) { return this.children.flatMap(node => [
    ...(selector.startsWith(".") ? node.className.split(/\s+/).includes(selector.slice(1))
      : node.tagName.toLowerCase() === selector.toLowerCase()) ? [node] : [], ...node.querySelectorAll(selector)]); }
  cloneNode() { return new this.constructor(this.tagName); }
}
class SvgNode extends Node {}
const html = await fs.readFile(path.join(root, "ui/index.html"), "utf8");
function dom() {
  const nodes = new Map([...html.matchAll(/id="([^"]+)"/g)].map(match => ["#" + match[1], new Node()]));
  globalThis.HTMLElement = Node; globalThis.SVGElement = SvgNode;
  globalThis.document = { querySelector: key => nodes.get(key) ?? null,
    createElement: tag => new Node(tag), createElementNS: (_ns, tag) => new SvgNode(tag),
    createTextNode: text => { const node = new Node(); node.textContent = text; return node; } };
  globalThis.window = { setInterval: () => 1, clearInterval() {} };
  return key => nodes.get(key);
}
const time = "2026-11-15T12:00:00.000Z";
const evaluation = (overrides = {}) => ({ indicator: "heart_rate", body_zone: null, status: "alarm", reason: null,
  direction: null, persistent: false, escalation: "urgent", rule_id: "test", rule_version: "1", source_kind: "published",
  source_ref: null, input_record_ids: ["saved"], input_event_ids: [], core_version: "test", evaluated_at: time,
  latest_value: { bpm: 180 }, latest_measured_at: time, baseline: null, history_count: 0, history_needed: 0,
  summary: "Confirmed alarm", ...overrides });
const stable = (overrides = {}) => ({ evaluated_at: time, met: null,
  overall: { status: "stable", escalation: "continue", worst: [] }, indicators: [], symptoms: [], alerts: [], body_zones: [],
  sync: { pending: 0, failed: 0 }, is_demo: false, ...overrides });
const saved = e => ({ status: "saved", record: { is_demo: false }, evaluation: e, warnings: [] });
function stubCore() {
  core.getStatus = async () => stable(); core.getProfile = async () => ({ profile: null });
  core.listRecords = async () => ({ records: [] }); core.listEvents = async () => ({ events: [] });
}

test("1: saved alarm invalidates Stable in both summaries and survives refresh failure", async () => {
  const get = dom(); stubCore(); const dashboard = createDashboard(true);
  const panel = createCorePanel(true, dashboard.core);
  try {
    await panel.refresh(); assert.equal(get("#overall-status").dataset.label, "Stable");
    panel.acceptRecord(saved(evaluation()));
    assert.equal(panel.snapshot(), null);
    for (const id of ["#overall-status", "#core-overall"]) assert.equal(get(id).dataset.label, "Alarm");
    assert.equal(get("#overview-alerts").hidden, true, "reading alarms are not persistent events");
    core.getStatus = async () => { throw Error("offline"); };
    await panel.refresh(true);
    for (const id of ["#overall-status", "#core-overall"]) assert.equal(get(id).dataset.label, "Alarm");
    // Further non-alarm mutations must not erase a pending confirmed alarm.
    panel.acceptRecord(saved(evaluation({ indicator: "sleep", status: "stable", latest_value: { hours: 7 } })));
    assert.equal(get("#overall-status").dataset.label, "Alarm");
    core.getStatus = async () => stable(); await panel.refresh(true);
    assert.equal(get("#overall-status").dataset.label, "Stable", "only a new authoritative status replaces the pending alarm");
  } finally { panel.stop(); }
});

test("1: stale in-flight status cannot overwrite a newly saved alarm", async () => {
  const get = dom(); stubCore(); let resolve;
  core.getStatus = () => new Promise(done => { resolve = done; });
  const dashboard = createDashboard(true); const panel = createCorePanel(true, dashboard.core);
  try {
    panel.acceptRecord(saved(evaluation())); resolve(stable()); await panel.refresh();
    assert.equal(panel.snapshot(), null); assert.equal(get("#overall-status").dataset.label, "Alarm");
  } finally { panel.stop(); }
});

test("1: persistent event navigation stays distinct from reading alarms", async () => {
  const get = dom(); stubCore(); const dashboard = createDashboard(true); const panel = createCorePanel(true, dashboard.core);
  try {
    await panel.refresh();
    panel.acceptHelp({ event: { event_id: "help", is_demo: false }, evaluation: evaluation({ indicator: "need_help", input_event_ids: ["help"], latest_value: null }) });
    assert.equal(get("#overview-alerts").hidden, false); assert.match(get("#overview-alerts").textContent, /\(1\)/);
    panel.acceptRecord(saved(evaluation()));
    assert.equal(get("#overview-alerts").hidden, false); assert.match(get("#overview-alerts").textContent, /\(1\)/);
    panel.acceptHelp({ event: { event_id: "help-2", is_demo: false }, evaluation: evaluation({ indicator: "need_help", input_event_ids: ["help-2"], latest_value: null }) });
    assert.equal(get("#core-alerts").children.length, 2, "both persistent events survive pending aggregate invalidation");
    assert.match(get("#overview-alerts").textContent, /\(2\)/);
    // A real save replaces DEMO data, including its persistent-event navigation.
    core.getStatus = async () => stable({ is_demo: true, alerts: [evaluation({ indicator: "need_help", input_event_ids: ["demo-help"], latest_value: null })] });
    await panel.refresh(true);
    panel.acceptRecord(saved(evaluation()));
    assert.equal(get("#overview-alerts").hidden, true);
    assert.equal(get("#overall-status").dataset.label, "Alarm");
  } finally { panel.stop(); }
});

const row = overrides => ({ id: "a1", astronaut_name: "Crew", heart_rate: 72, blood_pressure: "120/80", oxygen_in_blood: 98,
  blood_sugar: 94, body_temperature: 36.7, sleep_hours: 7, ...overrides });
test("2: impossible values are Invalid reading, never green, and excluded from persistence", () => {
  for (const [field, value, key, indicator] of [
    ["oxygen_in_blood", 101, "oxygen", "oxygen_in_blood"], ["sleep_hours", 30, "sleep", "sleep"],
    ["heart_rate", -1, "heart", "heart_rate"], ["blood_sugar", -1, "glucose", "glucose"],
    ["body_temperature", -1, "temperature", "body_temperature"], ["blood_pressure", "-2/80", "pressure", "blood_pressure"],
    ["nutrition", -1, "nutrition", "nutrition"], ["space_radiation", -1, "radiation", "space_radiation"],
    ["distance_from_earth", -1, "distance", "distance_from_earth"], ["heart_rate", "garbage", "heart", "heart_rate"],
  ]) {
    const crew = parseCrew([row({ [field]: value })]);
    assert.equal(mysqlStatuses(crew[0].telemetry)[key].label, "Invalid reading", field);
    assert.equal(mysqlStatuses(crew[0].telemetry)[key].tone, "neutral", field);
    assert.notEqual(crewStatus(crew[0].telemetry).tone, "ok", field);
    const batch = partitionCrewReadings(crew); assert.equal(batch.rejected, 1, field);
    assert.equal(batch.readings.some(reading => reading.indicator === indicator), false, field);
    assert.ok(batch.readings.some(reading => reading.indicator !== indicator));
  }
  assert.equal(evaluationCopy(evaluation({ status: "stable", indicator: "oxygen_in_blood", latest_value: { percentage: 101 } })).label, "Invalid reading");
});

test("2: boundaries and valid extremes preserve clinical classifications", () => {
  for (const [name, value, expected] of [["oxygenStatus", 0, "alarm"], ["oxygenStatus", 100, "ok"], ["oxygenStatus", 91, "alarm"], ["oxygenStatus", 93, "warn"],
    ["sleepStatus", 0, "warn"], ["sleepStatus", 24, "ok"], ["sleepStatus", 5, "warn"], ["sleepStatus", 6, "ok"],
    ["heartRateStatus", 210, "alarm"], ["glucoseStatus", 300, "alarm"], ["temperatureStatus", 41, "alarm"]]) {
    assert.equal(loaded.vital[name](value).level, expected, name + " " + value);
  }
  for (const name of ["oxygenStatus", "sleepStatus", "heartRateStatus", "glucoseStatus", "temperatureStatus"]) {
    for (const value of [NaN, Infinity, -Infinity]) assert.equal(loaded.vital[name](value).level, "invalid");
  }
  for (const [indicator, value] of [["body_battery", { value: 101 }], ["hrv", { ms: -1 }], ["sleep", { hours: -1 }]]) {
    assert.equal(validateReading({ astronaut_key: "a1", astronaut_name: "Crew", indicator, value, unit: "" }).ok, false);
  }
  assert.equal(partitionCrewReadings(parseCrew([row({ heart_rate: 210, oxygen_in_blood: 91 })])).rejected, 0);
});

test("4: Tauri reads the saved snapshot through IPC, never fetches or re-records replay", async () => {
  const originalFetch = globalThis.fetch;
  const events = new EventTarget();
  const calls = [];
  globalThis.window = { setTimeout, clearTimeout, setInterval, clearInterval,
    addEventListener: events.addEventListener.bind(events), removeEventListener: events.removeEventListener.bind(events),
    __TAURI_INTERNALS__: { invoke: async cmd => {
      calls.push(cmd);
      assert.equal(cmd, "recommendations_cached_crew");
      return { received_at: time, crew: [{ id: "a1", astronautName: "Crew", heartRate: 72,
        oxygenInBlood: 98, bloodPressure: "120/80", sleepHours: 7 }] };
    } } };
  globalThis.fetch = async () => { throw Error("Native replay must not fetch"); };
  const states = [];
  const feed = createTelemetryFeed(state => states.push(state));
  try {
    // Wait for the initial asynchronous refresh, then exercise a manual refresh.
    for (let i = 0; i < 10; i++) await Promise.resolve();
    await feed.refresh();
    const state = states.at(-1);
    assert.equal(state.cached, true); assert.equal(state.live, false);
    assert.equal(state.lastSuccess.toISOString(), time);
    assert.equal(state.crew[0].telemetry.heart_rate, 72);
    assert.equal(state.crew[0].name, "Crew");
    assert.ok(calls.length >= 2); assert.ok(calls.every(cmd => cmd === "recommendations_cached_crew"));
  } finally { feed.stop(); globalThis.fetch = originalFetch; }
});

test("4: browser development retains /api/astronauts without native IPC", async () => {
  const originalFetch = globalThis.fetch;
  const events = new EventTarget(); const urls = []; const states = [];
  globalThis.window = { setTimeout, clearTimeout, setInterval, clearInterval,
    addEventListener: events.addEventListener.bind(events), removeEventListener: events.removeEventListener.bind(events) };
  globalThis.fetch = async url => { urls.push(url); return { ok: true, json: async () => [row({})] }; };
  const feed = createTelemetryFeed(state => states.push(state));
  try {
    for (let i = 0; i < 15; i++) await Promise.resolve();
    assert.deepEqual(urls, ["/api/astronauts"]);
    assert.equal(states.at(-1).live, true); assert.equal(states.at(-1).crew[0].id, "a1");
  } finally { feed.stop(); globalThis.fetch = originalFetch; }
});

test("2: mobile cards render invalid feed values with a neutral badge", () => {
  const get = dom();
  for (const key of ["heart", "oxygen", "pressure", "glucose", "temperature", "sleep", "mood", "nutrition", "radiation", "distance"]) {
    const status = new Node(); status.className = "vital-status";
    const value = new Node(); value.className = "reading-value";
    get("#vital-" + key).append(status, value);
  }
  const view = loaded.vitals.createVitals();
  view.render(parseCrew([row({ oxygen_in_blood: 101, sleep_hours: 30, nutrition: -2 })])[0]);
  for (const key of ["oxygen", "sleep", "nutrition"]) {
    const card = get("#vital-" + key);
    assert.equal(card.dataset.tone, "neutral");
    assert.equal(card.querySelector(".vital-status").dataset.label, "Invalid reading");
  }
});
