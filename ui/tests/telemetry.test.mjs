import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, before, describe, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { rolldown } from "rolldown";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../..");

/** Crea un DOM mínimo en memoria suficiente para probar `telemetry-history.ts`. */
function createDomEnvironment() {
  class FakeNode {
    constructor(nodeType, tagName = "") {
      this.nodeType = nodeType;
      this.tagName = tagName.toUpperCase();
      this.children = [];
      this.childNodes = [];
      this.parentNode = null;
      this._attrs = new Map();
      this._listeners = new Map();
      this._text = "";
      this.className = "";
      this.id = "";
      this.title = "";
      this.type = "";
      this.htmlFor = "";
      this.dateTime = "";
      this.hidden = false;
      this.disabled = false;
      this._value = "";
      this.dataset = new Proxy(
        {},
        {
          get: (_, prop) => this._attrs.get(`data-${String(prop)}`),
          set: (_, prop, val) => {
            this._attrs.set(`data-${String(prop)}`, String(val));
            return true;
          },
          deleteProperty: (_, prop) => {
            this._attrs.delete(`data-${String(prop)}`);
            return true;
          },
        },
      );
    }

    get value() {
      if (this.tagName === "SELECT") {
        if (this._value) return this._value;
        const firstOpt = this.children.find(c => c.tagName === "OPTION");
        return firstOpt ? firstOpt.value : "";
      }
      return this._value;
    }

    set value(v) {
      this._value = String(v);
    }

    get textContent() {
      if (this.childNodes.length === 0) return this._text;
      return this.childNodes.map(c => c.textContent).join("");
    }

    set textContent(v) {
      this.children = [];
      this.childNodes = [];
      this._text = v === null || v === undefined ? "" : String(v);
    }

    setAttribute(name, value) {
      const str = String(value);
      this._attrs.set(name, str);
      if (name === "class") this.className = str;
      if (name === "id") this.id = str;
      if (name === "title") this.title = str;
    }

    getAttribute(name) {
      if (name === "class") return this.className;
      if (name === "id") return this.id;
      if (name === "title") return this.title;
      return this._attrs.has(name) ? this._attrs.get(name) : null;
    }

    hasAttribute(name) {
      return this._attrs.has(name);
    }

    removeAttribute(name) {
      this._attrs.delete(name);
    }

    append(...nodes) {
      for (const node of nodes) {
        if (typeof node === "string") {
          const textNode = new FakeNode(3);
          textNode._text = node;
          textNode.parentNode = this;
          this.childNodes.push(textNode);
        } else if (node) {
          node.parentNode = this;
          this.childNodes.push(node);
          this.children.push(node);
          if (this.tagName === "SELECT" && node.tagName === "OPTION" && this._value === "") {
            this._value = node.value;
          }
        }
      }
    }

    replaceChildren(...nodes) {
      this.children = [];
      this.childNodes = [];
      this._text = "";
      if (this.tagName === "SELECT") this._value = "";
      this.append(...nodes);
    }

    addEventListener(type, fn) {
      if (!this._listeners.has(type)) this._listeners.set(type, new Set());
      this._listeners.get(type).add(fn);
    }

    removeEventListener(type, fn) {
      this._listeners.get(type)?.delete(fn);
    }

    dispatchEvent(event) {
      const evt = event || { type: "" };
      if (evt.target == null) {
        Object.defineProperty(evt, "target", { value: this, configurable: true });
      }
      const listeners = this._listeners.get(evt.type);
      if (listeners) {
        for (const fn of [...listeners]) {
          fn(evt);
        }
      }
      return true;
    }

    click() {
      if (this.disabled) return;
      this.dispatchEvent(new Event("click"));
    }

    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    }

    querySelectorAll(selector) {
      const results = [];
      const visit = node => {
        for (const child of node.children) {
          if (matchesSelector(child, selector)) results.push(child);
          visit(child);
        }
      };
      visit(this);
      return results;
    }
  }

  function matchesSelector(node, selector) {
    if (selector.startsWith(".")) {
      const cls = selector.slice(1);
      return node.className.split(/\s+/).includes(cls);
    }
    if (selector.startsWith("#")) {
      return node.id === selector.slice(1);
    }
    return node.tagName.toLowerCase() === selector.toLowerCase();
  }

  const document = {
    createElement(tag) {
      return new FakeNode(1, tag);
    },
    createElementNS(_ns, tag) {
      return new FakeNode(1, tag);
    },
  };

  const windowObj = {
    setTimeout: (...args) => globalThis.setTimeout(...args),
    clearTimeout: id => globalThis.clearTimeout(id),
    setInterval: (...args) => globalThis.setInterval(...args),
    clearInterval: id => globalThis.clearInterval(id),
  };

  globalThis.window = windowObj;
  globalThis.document = document;
  return {
    window: windowObj,
    document,
    createHost: () => new FakeNode(1, "div"),
  };
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushMicrotasks(turns = 40) {
  for (let i = 0; i < turns; i += 1) {
    await Promise.resolve();
  }
}

function sampleRawCrewRow(id, overrides = {}) {
  return {
    id,
    astronaut_name: `Crew ${String(id).trim()}`,
    heart_rate: 72,
    blood_pressure: "120/80",
    oxygen_in_blood: 98,
    blood_sugar: 94,
    body_temperature: 36.7,
    sleep_hours: 7.5,
    mood: "Focused",
    nutrition: 2100,
    space_radiation: 14,
    distance_from_earth: 384400,
    hrv: 55,
    body_battery: 82,
    ...overrides,
  };
}

let tempDir;
let coreMod;
let telemetryMod;
let historyMod;

before(async () => {
  tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "merken-telemetry-ui-test-"));
  const build = await rolldown({
    input: {
      core: path.join(repoRoot, "ui/src/core-api.ts"),
      telemetry: path.join(repoRoot, "ui/src/ui/telemetry.ts"),
      history: path.join(repoRoot, "ui/src/ui/telemetry-history.ts"),
    },
  });
  await build.write({
    dir: tempDir,
    format: "esm",
    entryFileNames: "[name].mjs",
  });
  await build.close();

  createDomEnvironment();
  coreMod = await import(pathToFileURL(path.join(tempDir, "core.mjs")).href);
  telemetryMod = await import(pathToFileURL(path.join(tempDir, "telemetry.mjs")).href);
  historyMod = await import(pathToFileURL(path.join(tempDir, "history.mjs")).href);
});

after(() => {
  if (tempDir) {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

// Explicit false keeps these existing queue tests on the web-feed transport.
// The production/default Tauri replay is covered in front-graves.test.mjs.
describe("telemetría local — parseCrew, crewToReadings y cola IPC", () => {

  test("Ronda 5: valores presentes inválidos se rechazan y los válidos se conservan", () => {
    for (const value of ["1e309", "abc", NaN]) {
      const crew = telemetryMod.parseCrew([sampleRawCrewRow("a1", { heart_rate: value })]);
      assert.deepEqual(crew[0].invalid, [{ indicator: "heart_rate", reason: "not a finite number" }]);
      const result = telemetryMod.partitionCrewReadings(crew);
      assert.equal(result.rejected, 1);
      assert.equal(result.lastRejection, "heart_rate: not a finite number");
      assert.equal(result.readings.some(r => r.indicator === "heart_rate"), false);
      assert.ok(result.readings.some(r => r.indicator === "oxygen_in_blood"));
    }
    for (const pressure of ["abc/80", "120/abc"]) {
      const result = telemetryMod.partitionCrewReadings(telemetryMod.parseCrew([sampleRawCrewRow("a1", { blood_pressure: pressure })]));
      assert.equal(result.rejected, 1);
      assert.equal(result.lastRejection, "blood_pressure: not a finite number");
      assert.equal(result.readings.some(r => r.indicator === "blood_pressure"), false);
      assert.ok(result.readings.some(r => r.indicator === "heart_rate"));
    }
    const both = telemetryMod.partitionCrewReadings(telemetryMod.parseCrew([sampleRawCrewRow("a1", { blood_pressure: "abc/xyz" })]));
    assert.equal(both.rejected, 2);
  });

  test("Ronda 5: ausencias no cuentan y NaN directo sí cuenta", () => {
    for (const value of [null, "", "  ", undefined]) {
      const row = sampleRawCrewRow("a1", { heart_rate: value, blood_pressure: value });
      if (value === undefined) { delete row.heart_rate; delete row.blood_pressure; }
      const result = telemetryMod.partitionCrewReadings(telemetryMod.parseCrew([row]));
      assert.equal(result.rejected, 0);
      assert.equal(result.lastRejection, null);
      assert.ok(result.readings.some(r => r.indicator === "oxygen_in_blood"));
    }
    const crew = telemetryMod.parseCrew([sampleRawCrewRow("a1")]);
    crew[0].telemetry.heart_rate = NaN;
    crew[0].telemetry.blood_pressure.diastolic = NaN;
    const result = telemetryMod.partitionCrewReadings(crew);
    assert.equal(result.rejected, 2);
    assert.equal(result.lastRejection, "blood_pressure: not a finite number");
    assert.ok(result.readings.some(r => r.indicator === "oxygen_in_blood"));
    assert.equal(result.readings.some(r => r.indicator === "heart_rate" || r.indicator === "blood_pressure"), false);
  });

  test("parseCrew normaliza una vez y rechaza duplicados tras normalizar ('a1' y ' a1 ')", () => {
    assert.throws(
      () => telemetryMod.parseCrew([sampleRawCrewRow("a1"), sampleRawCrewRow(" a1 ")]),
      /Duplicate crew identity/,
    );
    assert.throws(() => telemetryMod.parseCrew([sampleRawCrewRow("   ")]), /Invalid crew identity/);

    const parsed = telemetryMod.parseCrew([sampleRawCrewRow(" a1 ")]);
    assert.equal(parsed.length, 1);
    assert.equal(parsed[0].id, "a1");

    const readings = telemetryMod.crewToReadings(parsed);
    assert.equal(readings.length, 12);
    assert.ok(readings.every(r => r.astronaut_key === "a1"));

    const byIndicator = Object.fromEntries(readings.map(r => [r.indicator, r]));
    assert.deepEqual(byIndicator.heart_rate.value, { bpm: 72 });
    assert.deepEqual(byIndicator.oxygen_in_blood.value, { percentage: 98 });
    assert.deepEqual(byIndicator.blood_pressure.value, { systolic: 120, diastolic: 80 });
    assert.deepEqual(byIndicator.glucose.value, { mg_dl: 94 });
    assert.deepEqual(byIndicator.body_temperature.value, { celsius: 36.7 });
    assert.deepEqual(byIndicator.sleep.value, { hours: 7.5 });
    assert.deepEqual(byIndicator.nutrition.value, { kcal: 2100 });
    assert.deepEqual(byIndicator.space_radiation.value, { usv_h: 14 });
    assert.deepEqual(byIndicator.distance_from_earth.value, { km: 384400 });
    assert.deepEqual(byIndicator.mood.value, { label: "Focused" });
    assert.deepEqual(byIndicator.hrv.value, { ms: 55 });
    assert.deepEqual(byIndicator.body_battery.value, { value: 82 });

    // Lecturas inválidas o parciales se omiten sin inventar valores.
    const partial = telemetryMod.parseCrew([
      sampleRawCrewRow("b2", {
        heart_rate: null,
        blood_pressure: "120/invalid",
        mood: "   ",
        hrv: Number.NaN,
      }),
    ]);
    const partialReadings = telemetryMod.crewToReadings(partial);
    const partialKeys = new Set(partialReadings.map(r => r.indicator));
    assert.equal(partialKeys.has("heart_rate"), false);
    assert.equal(partialKeys.has("blood_pressure"), false);
    assert.equal(partialKeys.has("mood"), false);
    assert.equal(partialKeys.has("hrv"), false);
  });

  test("N1 contrato compartido: parseCrew + crewToReadings coincide con el fixture telemetry-record-batch.json", () => {
    const fixturePath = path.join(repoRoot, "docs/api/fixtures/telemetry-record-batch.json");
    const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));

    const byIndicator = (a, b) => a.indicator.localeCompare(b.indicator);
    const actual = telemetryMod
      .crewToReadings(telemetryMod.parseCrew([fixture.crew_row]))
      .slice()
      .sort(byIndicator);
    const expected = fixture.readings.slice().sort(byIndicator);

    assert.deepStrictEqual(actual, expected);
  });

  test("createTelemetryFeed fuera de Tauri publica el feed sin invocar IPC", async () => {
    delete globalThis.window.__TAURI_INTERNALS__;
    let invokeCount = 0;
    const origRecordTelemetry = coreMod.core.recordTelemetry;
    coreMod.core.recordTelemetry = (...args) => {
      invokeCount += 1;
      return origRecordTelemetry(...args);
    };
    globalThis.fetch = async () => ({
      ok: true,
      json: async () => [sampleRawCrewRow("a1")],
    });

    const states = [];
    try {
      const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
      await flushMicrotasks();
      feed.stop();
    } finally {
      coreMod.core.recordTelemetry = origRecordTelemetry;
    }

    const latest = states.at(-1);
    assert.equal(latest.live, true);
    assert.equal(latest.loading, false);
    assert.equal(latest.crew.length, 1);
    assert.equal(latest.crew[0].id, "a1");
    assert.equal(invokeCount, 0);
  });

  test("trocea lotes en 64 / 64 / 2 de forma secuencial", async () => {
    const chunkSizes = [];
    let inFlightIpc = 0;
    let maxConcurrentIpc = 0;

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        assert.equal(cmd, "record_telemetry");
        inFlightIpc += 1;
        maxConcurrentIpc = Math.max(maxConcurrentIpc, inFlightIpc);
        chunkSizes.push(args.input.readings.length);
        await Promise.resolve();
        inFlightIpc -= 1;
        return { stored: args.input.readings.length, skipped: 0, batch_id: `batch-${chunkSizes.length}` };
      },
    };

    // 13 astronautas * 10 indicadores = 130 lecturas => 64, 64, 2
    const bigCrew = Array.from({ length: 13 }, (_, i) =>
      sampleRawCrewRow(`astro-${i}`, {
        blood_pressure: null,
        mood: null,
        hrv: null,
        body_battery: null,
      }),
    );
    // sampleRawCrewRow tiene 8 numéricos base + 2 opcionales; añadimos hrv y body_battery para tener 10 por astronauta
    for (const row of bigCrew) {
      row.hrv = 50;
      row.body_battery = 80;
    }

    globalThis.fetch = async () => ({ ok: true, json: async () => bigCrew });
    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
    await flushMicrotasks(60);
    feed.stop();

    assert.deepEqual(chunkSizes, [64, 64, 2]);
    assert.equal(maxConcurrentIpc, 1);
    const latest = states.at(-1);
    assert.equal(latest.persist.ok, true);
    assert.equal(latest.persist.stored, 130);
    assert.equal(latest.persist.lastBatchId, "batch-3");
  });

  test("cola de guardado: el feed publica aunque el IPC no responda, gana la última y stop() deja la cola inerte", async () => {
    const ipcCalls = [];
    const firstIpc = deferred();
    const secondIpc = deferred();

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: (cmd, args) => {
        assert.equal(cmd, "record_telemetry");
        const keys = [...new Set(args.input.readings.map(r => r.astronaut_key))];
        ipcCalls.push(keys);
        if (ipcCalls.length === 1) return firstIpc.promise;
        if (ipcCalls.length === 2) return secondIpc.promise;
        return Promise.resolve({ stored: 1, skipped: 0, batch_id: "unexpected" });
      },
    };

    let fetchSeq = 0;
    globalThis.fetch = async () => {
      fetchSeq += 1;
      return {
        ok: true,
        json: async () => [sampleRawCrewRow(`crew-${fetchSeq}`)],
      };
    };

    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
    await flushMicrotasks();

    // El primer HTTP ya publicó inmediatamente aunque el primer IPC siga pendiente.
    assert.equal(states.at(-1).live, true);
    assert.equal(states.at(-1).crew[0].id, "crew-1");
    assert.equal(ipcCalls.length, 1);

    // Llegan dos refrescos mientras el guardado #1 está bloqueado: el #2 se descarta y gana el #3.
    await feed.refresh();
    assert.equal(states.at(-1).crew[0].id, "crew-2");
    await feed.refresh();
    assert.equal(states.at(-1).crew[0].id, "crew-3");
    assert.equal(ipcCalls.length, 1);

    // Al resolverse el #1, solo debe ejecutarse el #3 (gana la última).
    firstIpc.resolve({ stored: 12, skipped: 0, batch_id: "b1" });
    await flushMicrotasks();
    assert.deepEqual(ipcCalls, [["crew-1"], ["crew-3"]]);
    assert.equal(states.at(-1).persist.lastBatchId, "b1");

    // stop() mientras #2 está pendiente y llega otro intento: la cola queda inerte.
    feed.stop();
    const countBeforeResolve = states.length;
    secondIpc.resolve({ stored: 12, skipped: 0, batch_id: "b3" });
    await feed.refresh();
    await flushMicrotasks();
    assert.equal(states.length, countBeforeResolve);
    assert.equal(ipcCalls.length, 2);
  });

  test("N3 cola de guardado: timeout de 10 s avisa sin liberar la cola hasta resolver y envía solo el último lote pendiente", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });

    const ipcCalls = [];
    const firstIpc = deferred();
    let inFlightIpc = 0;
    let maxInFlightIpc = 0;

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        assert.equal(cmd, "record_telemetry");
        inFlightIpc += 1;
        maxInFlightIpc = Math.max(maxInFlightIpc, inFlightIpc);
        const keys = [...new Set(args.input.readings.map(r => r.astronaut_key))];
        ipcCalls.push(keys);
        try {
          if (ipcCalls.length === 1) {
            return await firstIpc.promise;
          }
          return {
            stored: args.input.readings.length,
            skipped: 0,
            batch_id: `batch-${ipcCalls.length}`,
          };
        } finally {
          inFlightIpc -= 1;
        }
      },
    };

    let fetchSeq = 0;
    globalThis.fetch = async () => {
      fetchSeq += 1;
      return {
        ok: true,
        json: async () => [sampleRawCrewRow(`crew-${fetchSeq}`)],
      };
    };

    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
    await flushMicrotasks();

    assert.equal(ipcCalls.length, 1);
    assert.equal(inFlightIpc, 1);
    assert.equal(states.at(-1).live, true);

    // Avanzamos 10 s: el timeout publica estado "slow" (lento != fallo, ok sigue true), sin lanzar un segundo invoke.
    t.mock.timers.tick(10_000);
    await flushMicrotasks();

    const slowState = states.find(s => s.persist && s.persist.status === "slow");
    assert.ok(slowState, "Debe haberse publicado un estado con persist.status === 'slow' tras vencer 10 s");
    assert.equal(slowState.persist.ok, true);
    assert.equal(slowState.persist.lastError, null);
    assert.equal(ipcCalls.length, 1);
    assert.equal(inFlightIpc, 1);

    // Mientras sigue colgado el primero, llegan lotes intermedios: ninguno dispara otro invoke.
    await feed.refresh();
    await feed.refresh();
    await feed.refresh();
    assert.equal(ipcCalls.length, 1);
    assert.equal(maxInFlightIpc, 1);
    const latestCrewKey = states.at(-1).crew[0].id;
    assert.notEqual(latestCrewKey, "crew-1");
    assert.notEqual(latestCrewKey, "crew-2");

    // Al resolverse la promesa original, se envía únicamente el último lote pendiente (gana la última).
    firstIpc.resolve({ stored: 12, skipped: 0, batch_id: "slow-1" });
    await flushMicrotasks(60);
    feed.stop();

    assert.equal(maxInFlightIpc, 1);
    assert.deepEqual(ipcCalls, [["crew-1"], [latestCrewKey]]);
    assert.equal(states.at(-1).persist.status, "ok");
    assert.equal(states.at(-1).persist.ok, true);
    assert.equal(states.at(-1).persist.lastError, null);
    assert.equal(states.at(-1).persist.lastBatchId, "batch-2");
  });

  test("R3-1: validateReading replica cotas de Rust (clave, nombre, unidad, mood, finitos y tamaño JSON)", () => {
    const base = {
      astronaut_key: "a1",
      astronaut_name: "Alice",
      indicator: "heart_rate",
      value: { bpm: 72 },
      unit: "bpm",
    };
    assert.deepEqual(telemetryMod.validateReading(base), { ok: true, reason: null });

    assert.equal(telemetryMod.validateReading({ ...base, astronaut_key: "   " }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_key: "k".repeat(65) }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_key: "a1\n" }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_key: "k".repeat(64) }).ok, true);

    assert.equal(telemetryMod.validateReading({ ...base, astronaut_name: "   " }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_name: "n".repeat(101) }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_name: "Alice\x00" }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, astronaut_name: "n".repeat(100) }).ok, true);

    assert.equal(telemetryMod.validateReading({ ...base, unit: "u".repeat(17) }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, unit: "u".repeat(16) }).ok, true);

    const moodBase = { ...base, indicator: "mood", value: { label: "Focused" }, unit: "" };
    assert.equal(telemetryMod.validateReading(moodBase).ok, true);
    assert.deepEqual(
      telemetryMod.validateReading({ ...moodBase, value: { label: "m".repeat(65) } }),
      { ok: false, reason: "mood: too long" },
    );
    assert.equal(telemetryMod.validateReading({ ...moodBase, value: { label: "calm\nnext" } }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...moodBase, value: { label: "   " } }).ok, false);

    assert.equal(telemetryMod.validateReading({ ...base, value: { bpm: Number.NaN } }).ok, false);
    assert.equal(telemetryMod.validateReading({ ...base, value: { bpm: Number.POSITIVE_INFINITY } }).ok, false);
    assert.equal(
      telemetryMod.validateReading({
        ...base,
        value: { bpm: 72, extra: "x".repeat(2048) },
      }).ok,
      false,
    );
  });

  test("R3-1: mood de 65, mood con salto de línea, nombre de 101 e id vacío tras trim excluyen solo su lectura y envían el resto", async () => {
    const sentReadings = [];
    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        assert.equal(cmd, "record_telemetry");
        sentReadings.push(...args.input.readings);
        return { stored: args.input.readings.length, skipped: 0, batch_id: "b-r3-1" };
      },
    };

    // Caso A: mood de 65 caracteres en crew-1 -> solo se excluye mood, las otras 11 lecturas se envían.
    const crewMoodLong = telemetryMod.parseCrew([
      sampleRawCrewRow("crew-1", { mood: "m".repeat(65) }),
    ]);
    const partLong = telemetryMod.partitionCrewReadings(crewMoodLong);
    assert.equal(partLong.readings.length, 11);
    assert.equal(partLong.rejected, 1);
    assert.equal(partLong.lastRejection, "mood: too long");
    assert.equal(partLong.readings.some(r => r.indicator === "mood"), false);

    // Caso B: mood con salto de línea en crew-2 -> solo se excluye mood.
    const crewMoodNewline = telemetryMod.parseCrew([
      sampleRawCrewRow("crew-2", { mood: "calm\nline" }),
    ]);
    const partNewline = telemetryMod.partitionCrewReadings(crewMoodNewline);
    assert.equal(partNewline.readings.length, 11);
    assert.equal(partNewline.rejected, 1);
    assert.match(partNewline.lastRejection, /^mood:/);

    // Caso C: nombre de 101 chars + id vacío tras trim + miembro válido en el mismo lote.
    const longNameMember = telemetryMod.parseCrew([
      sampleRawCrewRow("crew-long-name", { astronaut_name: "N".repeat(101) }),
    ])[0];
    const emptyIdMember = {
      ...telemetryMod.parseCrew([sampleRawCrewRow("temp")])[0],
      id: "   ",
    };
    const validMember = telemetryMod.parseCrew([sampleRawCrewRow("crew-ok")])[0];

    const mixedBatch = telemetryMod.partitionCrewReadings([
      longNameMember,
      emptyIdMember,
      validMember,
    ]);
    assert.equal(mixedBatch.readings.length, 12);
    assert.ok(mixedBatch.readings.every(r => r.astronaut_key === "crew-ok"));
    assert.equal(mixedBatch.rejected, 24);

    // Caso D: createTelemetryFeed registra state.persist.rejected y lastRejection al persistir.
    globalThis.fetch = async () => ({
      ok: true,
      json: async () => [
        sampleRawCrewRow("crew-valid"),
        sampleRawCrewRow("crew-long", { astronaut_name: "A".repeat(101) }),
        sampleRawCrewRow("crew-bad-mood", { mood: "x".repeat(65) }),
      ],
    });
    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
    await flushMicrotasks(60);
    feed.stop();

    // crew-valid (12) + crew-bad-mood sin mood (11) = 23 lecturas enviadas; 12 + 1 = 13 rechazadas.
    assert.equal(sentReadings.length, 23);
    assert.equal(sentReadings.some(r => r.astronaut_key === "crew-long"), false);
    assert.equal(
      sentReadings.some(r => r.astronaut_key === "crew-bad-mood" && r.indicator === "mood"),
      false,
    );
    const latest = states.at(-1);
    assert.equal(latest.persist.status, "ok");
    assert.equal(latest.persist.ok, true);
    assert.equal(latest.persist.stored, 23);
    assert.equal(latest.persist.rejected, 13);
    assert.equal(latest.persist.lastRejection, "mood: too long");
  });

  test("R3-4: lento transitions -> error tras timeout de 10 s manteniendo un único invoke en vuelo", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });
    const firstIpc = deferred();
    let invokeCount = 0;

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async () => {
        invokeCount += 1;
        return firstIpc.promise;
      },
    };
    globalThis.fetch = async () => ({
      ok: true,
      json: async () => [sampleRawCrewRow("crew-err")],
    });

    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s), false);
    await flushMicrotasks();

    t.mock.timers.tick(10_000);
    await flushMicrotasks();
    const slowState = states.at(-1);
    assert.equal(slowState.persist.status, "slow");
    assert.equal(slowState.persist.ok, true);
    assert.equal(slowState.persist.lastError, null);
    assert.equal(invokeCount, 1);

    firstIpc.reject(new Error("SQLite locked"));
    await flushMicrotasks(40);
    feed.stop();

    const errState = states.at(-1);
    assert.equal(errState.persist.status, "error");
    assert.equal(errState.persist.ok, false);
    assert.equal(errState.persist.lastError, "SQLite locked");
  });
});

describe("telemetría local — panel telemetry-history.ts", () => {
  test("fuera de Tauri desactiva controles y no deja aria-busy", async () => {
    delete globalThis.window.__TAURI_INTERNALS__;
    const { createHost } = createDomEnvironment();
    const host = createHost();

    const handle = historyMod.mountTelemetryHistory(host, () => [{ id: "a1", name: "Alice" }]);
    await flushMicrotasks();

    const body = host.querySelector(".tele-hist-body");
    const status = host.querySelector(".tele-hist-status");
    const verifyBtn = host.querySelector(".tele-hist-verify");
    assert.equal(body.hasAttribute("aria-busy"), false);
    assert.equal(status.textContent, "History is available in the app");
    assert.equal(verifyBtn.disabled, true);
    handle.destroy();
  });

  test("aria-busy se retira en tripulación vacía, error y respuesta obsoleta", async () => {
    const { createHost } = createDomEnvironment();
    const firstList = deferred();
    const errorList = deferred();
    let mode = "hang";

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: cmd => {
        if (cmd === "verify_telemetry") {
          return Promise.resolve({
            ok: true,
            checked: 1,
            series_checked: 1,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: 1,
          });
        }
        if (mode === "hang") return firstList.promise;
        if (mode === "error") return errorList.promise;
        return Promise.resolve([]);
      },
    };

    let crew = [{ id: "a1", name: "Alice" }];
    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => crew);

    const body = host.querySelector(".tele-hist-body");
    assert.equal(body.getAttribute("aria-busy"), "true");

    // Salida 1: tripulación vacía mientras listTelemetry sigue pendiente.
    crew = [];
    await handle.refresh();
    assert.equal(body.hasAttribute("aria-busy"), false);
    assert.equal(host.querySelector(".tele-hist-status").textContent, "No crew list available");

    // La respuesta obsoleta llega después: aria-busy debe seguir retirado.
    firstList.resolve([]);
    await flushMicrotasks();
    assert.equal(body.hasAttribute("aria-busy"), false);

    // Salida 2: error en listTelemetry.
    crew = [{ id: "a1", name: "Alice" }];
    mode = "error";
    const errRefresh = handle.refresh();
    assert.equal(body.getAttribute("aria-busy"), "true");
    errorList.reject(new Error("SQLite read error"));
    await errRefresh;
    assert.equal(body.hasAttribute("aria-busy"), false);
    assert.match(host.querySelector(".tele-hist-status").textContent, /Could not load history/);

    // Salida 3: paso a fuera de Tauri durante una carga.
    mode = "hang";
    const hang2 = deferred();
    globalThis.window.__TAURI_INTERNALS__.invoke = cmd =>
      cmd === "list_telemetry"
        ? hang2.promise
        : Promise.resolve({
            ok: true,
            checked: 0,
            series_checked: 0,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: null,
          });
    void handle.refresh();
    assert.equal(body.getAttribute("aria-busy"), "true");
    delete globalThis.window.__TAURI_INTERNALS__;
    await handle.refresh();
    assert.equal(body.hasAttribute("aria-busy"), false);
    hang2.resolve([]);
    await flushMicrotasks();
    assert.equal(body.hasAttribute("aria-busy"), false);

    handle.destroy();
  });

  test("refresco habitual, debounce 250 ms, cooldown 60 s, sin dos verificaciones simultáneas y textos de cadena", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });
    const { createHost } = createDomEnvironment();

    const listCalls = [];
    const verifyCalls = [];
    let verifyInFlightCount = 0;
    let maxVerifyInFlight = 0;
    let nextVerifyDeferred = null;
    let verifyResponse = {
      ok: true,
      checked: 15,
      series_checked: 2,
      first_bad_reading_id: null,
      problem: null,
      checked_through_reading_id: 15,
    };

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        if (cmd === "list_telemetry") {
          listCalls.push(args.input);
          return [
            {
              reading_id: 10,
              astronaut_key: args.input.astronaut_key,
              astronaut_name: "Alice",
              indicator: args.input.indicator,
              value: { bpm: 71 },
              unit: "bpm",
              observed_at: "2026-11-14T21:05:33.120Z",
              batch_id: "b1",
              source: "mysql",
              app_version: "0.1.0",
              prev_hash: "0",
              row_hash: "h1",
            },
          ];
        }
        if (cmd === "verify_telemetry") {
          verifyCalls.push(args.input);
          verifyInFlightCount += 1;
          maxVerifyInFlight = Math.max(maxVerifyInFlight, verifyInFlightCount);
          try {
            if (nextVerifyDeferred) {
              const d = nextVerifyDeferred;
              nextVerifyDeferred = null;
              return await d.promise;
            }
            return verifyResponse;
          } finally {
            verifyInFlightCount -= 1;
          }
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const parsedCrew = telemetryMod.parseCrew([sampleRawCrewRow(" a1 ")]);
    const crew = [
      { id: parsedCrew[0].id, name: parsedCrew[0].name },
      { id: "a2", name: "Bob" },
    ];
    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => crew);
    await flushMicrotasks();

    // Montaje inicial: 1 listTelemetry + 1 verifyTelemetry para "a1".
    assert.equal(listCalls.length, 1);
    assert.equal(listCalls[0].astronaut_key, "a1");
    assert.equal(verifyCalls.length, 1);
    assert.equal(verifyCalls[0].astronaut_key, "a1");

    const traceLine = host.querySelector(".tele-hist-trace");
    const helpLine = host.querySelector(".tele-hist-help");
    const verifyBtn = host.querySelector(".tele-hist-verify");
    assert.match(traceLine.textContent, /^Checked \d{2}:\d{2}:\d{2} UTC · 15 readings in chain · consistent ✓$/);
    assert.equal(/verified/i.test(traceLine.textContent), false);
    assert.equal(traceLine.title, "Detects edits and missing records on this device.");
    assert.equal(helpLine.textContent, "Detects edits and missing records on this device.");

    // Refresco habitual: solo llama a listTelemetry.
    await handle.refresh();
    assert.equal(listCalls.length, 2);
    assert.equal(verifyCalls.length, 1);

    // Cambios rápidos de indicador: agrupados por debounce de 250 ms.
    const [crewSelect, indicatorSelect] = host.querySelectorAll(".tele-hist-select");
    indicatorSelect.value = "glucose";
    indicatorSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(100);
    indicatorSelect.value = "sleep";
    indicatorSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(100);
    assert.equal(listCalls.length, 2);

    t.mock.timers.tick(150);
    await flushMicrotasks();
    assert.equal(listCalls.length, 3);
    assert.equal(listCalls[2].indicator, "sleep");
    assert.equal(verifyCalls.length, 1);

    // Cambio a "a2": dispara verifyTelemetry para "a2".
    crewSelect.value = "a2";
    crewSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();
    assert.equal(verifyCalls.length, 2);
    assert.equal(verifyCalls[1].astronaut_key, "a2");

    // Regreso a "a1" dentro de 60 s: NO vuelve a verificar.
    t.mock.timers.tick(5_000);
    crewSelect.value = "a1";
    crewSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();
    assert.equal(verifyCalls.length, 2);
    assert.match(traceLine.textContent, /^Checked \d{2}:\d{2}:\d{2} UTC · 15 readings in chain · consistent ✓$/);

    // Botón "Verify chain" deshabilitado mientras corre y sin dos verificaciones simultáneas.
    nextVerifyDeferred = deferred();
    const slowVerify = nextVerifyDeferred;
    verifyResponse = {
      ok: false,
      checked: 10,
      series_checked: 1,
      first_bad_reading_id: 42,
      problem: "hash_mismatch",
      checked_through_reading_id: 10,
    };
    verifyBtn.click();
    assert.equal(verifyBtn.disabled, true);
    assert.equal(verifyCalls.length, 3);

    // Un segundo clic o cambio de astronauta no lanza dos verificaciones simultáneas.
    verifyBtn.click();
    assert.equal(verifyCalls.length, 3);
    assert.equal(maxVerifyInFlight, 1);

    slowVerify.resolve(verifyResponse);
    await flushMicrotasks();
    assert.equal(verifyBtn.disabled, false);
    assert.match(traceLine.textContent, /^Checked \d{2}:\d{2}:\d{2} UTC · 10 readings in chain · chain problem: hash mismatch at #42$/);

    // Tras >= 60 s, cambiar a "a2" vuelve a verificar y muestra texto legible sin #id si es null.
    t.mock.timers.tick(65_000);
    verifyResponse = {
      ok: false,
      checked: 9,
      series_checked: 1,
      first_bad_reading_id: null,
      problem: "missing_tail",
      checked_through_reading_id: 10,
    };
    crewSelect.value = "a2";
    crewSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();
    assert.equal(verifyCalls.length, 4);
    assert.match(traceLine.textContent, /^Checked \d{2}:\d{2}:\d{2} UTC · 9 readings in chain · chain problem: missing tail$/);

    // Comprobación de los restantes tipos de problema.
    for (const [problem, expected] of [
      ["broken_link", "chain problem: broken link at #7"],
      ["missing_series", "chain problem: missing series at #7"],
      ["count_mismatch", "chain problem: count mismatch at #7"],
    ]) {
      verifyResponse = {
        ok: false,
        checked: 5,
        series_checked: 1,
        first_bad_reading_id: 7,
        problem,
        checked_through_reading_id: 10,
      };
      verifyBtn.click();
      await flushMicrotasks();
      assert.match(traceLine.textContent, new RegExp(`^Checked \\d{2}:\\d{2}:\\d{2} UTC · 5 readings in chain · ${expected}$`));
    }

    handle.destroy();
  });

  test("Hallazgo 6: tres refresh() seguidos coalescen a 1 en vuelo + 1 pendiente con la selección más reciente", async () => {
    const { createHost } = createDomEnvironment();
    const listCalls = [];
    const deferredLists = [];
    let listInFlight = 0;
    let maxListInFlight = 0;

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        if (cmd === "verify_telemetry") {
          return {
            ok: true,
            checked: 2,
            series_checked: 1,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: 2,
          };
        }
        if (cmd === "list_telemetry") {
          listInFlight += 1;
          maxListInFlight = Math.max(maxListInFlight, listInFlight);
          listCalls.push({ ...args.input });
          const d = deferred();
          deferredLists.push(d);
          try {
            return await d.promise;
          } finally {
            listInFlight -= 1;
          }
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => [{ id: "a1", name: "Alice" }]);

    // Resolvemos la carga inicial del montaje para partir en reposo.
    assert.equal(deferredLists.length, 1);
    deferredLists[0].resolve([]);
    await flushMicrotasks();
    listCalls.length = 0;
    deferredLists.length = 0;

    const [, indicatorSelect] = host.querySelectorAll(".tele-hist-select");

    // Tres refresh() seguidos cambiando selección entre el 2º y 3º.
    const p1 = handle.refresh();
    indicatorSelect.value = "glucose";
    indicatorSelect.dispatchEvent(new Event("change"));
    const p2 = handle.refresh();
    indicatorSelect.value = "hrv";
    indicatorSelect.dispatchEvent(new Event("change"));
    const p3 = handle.refresh();

    // Solo 1 llamada en vuelo mientras no termine la primera.
    assert.equal(listCalls.length, 1);
    assert.equal(listInFlight, 1);

    deferredLists[0].resolve([]);
    await flushMicrotasks();

    // Al terminar la 1ª, se lanza exactamente 1 pasada pendiente con la selección más reciente ("hrv").
    assert.equal(listCalls.length, 2);
    assert.equal(listCalls[1].indicator, "hrv");
    assert.equal(maxListInFlight, 1);

    deferredLists[1].resolve([]);
    await Promise.all([p1, p2, p3]);
    assert.equal(listCalls.length, 2);

    handle.destroy();
  });

  test("N5: la lista conserva el orden recibido [2, 1] aunque observed_at venga invertido", async () => {
    const { createHost } = createDomEnvironment();
    const backendItems = [
      {
        reading_id: 2,
        astronaut_key: "a1",
        astronaut_name: "Alice",
        indicator: "heart_rate",
        value: { bpm: 72 },
        unit: "bpm",
        observed_at: "2026-11-14T20:00:00.000Z",
        batch_id: "b2",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "h1",
        row_hash: "h2",
      },
      {
        reading_id: 1,
        astronaut_key: "a1",
        astronaut_name: "Alice",
        indicator: "heart_rate",
        value: { bpm: 88 },
        unit: "bpm",
        observed_at: "2026-11-14T22:00:00.000Z",
        batch_id: "b1",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "h1",
      },
    ];

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async cmd => {
        if (cmd === "verify_telemetry") {
          return {
            ok: true,
            checked: 2,
            series_checked: 1,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: 2,
          };
        }
        if (cmd === "list_telemetry") {
          return backendItems;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => [{ id: "a1", name: "Alice" }]);
    await flushMicrotasks();

    const rows = host.querySelectorAll(".tele-hist-item");
    assert.equal(rows.length, 2);
    assert.equal(rows[0].querySelector(".tele-hist-time").textContent, "20:00:00 UTC");
    assert.equal(rows[0].querySelector(".tele-hist-value").textContent, "72 bpm");
    assert.equal(rows[1].querySelector(".tele-hist-time").textContent, "22:00:00 UTC");
    assert.equal(rows[1].querySelector(".tele-hist-value").textContent, "88 bpm");

    // La gráfica ordena aparte cronológicamente sin alterar la lista.
    const chartWindow = host.querySelector(".tele-hist-window");
    assert.equal(chartWindow.textContent, "20:00:00 UTC — 22:00:00 UTC");

    handle.destroy();
  });

  test("N6: etiqueta de integridad muestra hora UTC, alcance y marca lecturas nuevas sin re-verificar sola", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });
    t.mock.timers.setTime(Date.UTC(2026, 10, 14, 10, 15, 30));

    const { createHost } = createDomEnvironment();
    let verifyCount = 0;
    let listedItems = [
      {
        reading_id: 5,
        astronaut_key: "a1",
        astronaut_name: "Alice",
        indicator: "heart_rate",
        value: { bpm: 70 },
        unit: "bpm",
        observed_at: "2026-11-14T10:15:00.000Z",
        batch_id: "b1",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "h5",
      },
    ];
    let checkedTotal = 5;

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async cmd => {
        if (cmd === "verify_telemetry") {
          verifyCount += 1;
          return {
            ok: true,
            checked: checkedTotal,
            series_checked: 1,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: checkedTotal,
          };
        }
        if (cmd === "list_telemetry") {
          return listedItems;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => [{ id: "a1", name: "Alice" }]);
    await flushMicrotasks();

    const traceLine = host.querySelector(".tele-hist-trace");
    const verifyBtn = host.querySelector(".tele-hist-verify");
    assert.equal(verifyCount, 1);
    assert.equal(traceLine.textContent, "Checked 10:15:30 UTC · 5 readings in chain · consistent ✓");

    // Llegan lecturas nuevas en un refresco: marca el aviso pero NO re-verifica automáticamente.
    t.mock.timers.tick(5_000);
    listedItems = [
      {
        ...listedItems[0],
        reading_id: 6,
        observed_at: "2026-11-14T10:15:35.000Z",
      },
      listedItems[0],
    ];
    await handle.refresh();
    assert.equal(verifyCount, 1);
    assert.equal(
      traceLine.textContent,
      "Checked 10:15:30 UTC · 5 readings in chain · consistent ✓ · newer readings not yet checked",
    );

    // El botón manual "Verify chain" actualiza la hora y limpia el aviso.
    checkedTotal = 6;
    verifyBtn.click();
    await flushMicrotasks();
    assert.equal(verifyCount, 2);
    assert.equal(traceLine.textContent, "Checked 10:15:35 UTC · 6 readings in chain · consistent ✓");

    handle.destroy();
  });

  test("R3-2: respuesta tardía de A tras cambiar a B se descarta y luego B se pinta bien", async () => {
    const { createHost } = createDomEnvironment();
    const listA = deferred();
    const listB = deferred();
    const listCalls = [];

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        if (cmd === "verify_telemetry") {
          return {
            ok: true,
            checked: 4,
            series_checked: 1,
            first_bad_reading_id: null,
            problem: null,
            checked_through_reading_id: 4,
          };
        }
        if (cmd === "list_telemetry") {
          listCalls.push({ ...args.input });
          if (args.input.astronaut_key === "A" && listCalls.length === 1) {
            return listA.promise;
          }
          if (args.input.astronaut_key === "B") {
            return listB.promise;
          }
          return [];
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => [
      { id: "A", name: "Alice" },
      { id: "B", name: "Bob" },
    ]);

    assert.equal(listCalls.length, 1);
    assert.equal(listCalls[0].astronaut_key, "A");

    // Cambiamos a astronauta B mientras la consulta de A sigue en vuelo y refrescamos.
    const [crewSelect, indicatorSelect] = host.querySelectorAll(".tele-hist-select");
    crewSelect.value = "B";
    crewSelect.dispatchEvent(new Event("change"));
    const refreshB = handle.refresh();

    // Llega tarde la respuesta de A (con reading_id 999 > 4 para detectar cualquier fuga a estadísticas).
    listA.resolve([
      {
        reading_id: 999,
        astronaut_key: "A",
        astronaut_name: "Alice",
        indicator: "heart_rate",
        value: { bpm: 177 },
        unit: "bpm",
        observed_at: "2026-11-14T11:00:00.000Z",
        batch_id: "ba",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "ha",
      },
    ]);
    await flushMicrotasks();

    const statusLine = host.querySelector(".tele-hist-status");
    const traceLine = host.querySelector(".tele-hist-trace");
    const body = host.querySelector(".tele-hist-body");
    assert.doesNotMatch(statusLine.textContent, /readings loaded/);
    assert.equal(body.querySelectorAll(".tele-hist-item").length, 0);
    assert.doesNotMatch(traceLine.textContent, /newer readings not yet checked/);

    // Ahora se resuelve B: B debe pintarse correctamente sin rastro de A.
    assert.equal(listCalls.length, 2);
    assert.equal(listCalls[1].astronaut_key, "B");
    listB.resolve([
      {
        reading_id: 3,
        astronaut_key: "B",
        astronaut_name: "Bob",
        indicator: "heart_rate",
        value: { bpm: 64 },
        unit: "bpm",
        observed_at: "2026-11-14T11:05:00.000Z",
        batch_id: "bb",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "hb",
      },
    ]);
    await refreshB;
    await flushMicrotasks();

    assert.equal(statusLine.textContent, "1 readings loaded · Heart rate");
    const rows = body.querySelectorAll(".tele-hist-item");
    assert.equal(rows.length, 1);
    assert.equal(rows[0].querySelector(".tele-hist-value").textContent, "64 bpm");
    assert.doesNotMatch(traceLine.textContent, /newer readings not yet checked/);

    // También invalida al cambiar de indicador con consulta activa en vuelo.
    const oldIndicator = deferred();
    const newIndicator = deferred();
    let mode = "old";
    globalThis.window.__TAURI_INTERNALS__.invoke = async cmd => {
      if (cmd === "verify_telemetry") {
        return {
          ok: true,
          checked: 4,
          series_checked: 1,
          first_bad_reading_id: null,
          problem: null,
          checked_through_reading_id: 4,
        };
      }
      return mode === "old" ? oldIndicator.promise : newIndicator.promise;
    };
    const pOld = handle.refresh();
    indicatorSelect.value = "glucose";
    indicatorSelect.dispatchEvent(new Event("change"));
    mode = "new";
    const pNew = handle.refresh();
    oldIndicator.resolve([
      {
        reading_id: 4,
        astronaut_key: "B",
        astronaut_name: "Bob",
        indicator: "heart_rate",
        value: { bpm: 99 },
        unit: "bpm",
        observed_at: "2026-11-14T11:06:00.000Z",
        batch_id: "b-old",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "ho",
      },
    ]);
    await flushMicrotasks();
    assert.doesNotMatch(statusLine.textContent, /Heart rate$/);
    newIndicator.resolve([
      {
        reading_id: 4,
        astronaut_key: "B",
        astronaut_name: "Bob",
        indicator: "glucose",
        value: { mg_dl: 95 },
        unit: "mg/dL",
        observed_at: "2026-11-14T11:07:00.000Z",
        batch_id: "b-new",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "hn",
      },
    ]);
    await Promise.all([pOld, pNew]);
    assert.equal(statusLine.textContent, "1 readings loaded · Blood glucose");
    assert.equal(body.querySelector(".tele-hist-value").textContent, "95 mg/dL");

    handle.destroy();
  });

  test("R3-3: vigencia usa checked_through_reading_id entre indicadores y muestra scope unknown si falta referencia", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });
    t.mock.timers.setTime(Date.UTC(2026, 10, 14, 12, 0, 0));
    const { createHost } = createDomEnvironment();

    let verifyPayload = {
      ok: true,
      checked: 12,
      series_checked: 2,
      first_bad_reading_id: null,
      problem: null,
      checked_through_reading_id: 12,
    };
    const itemsByIndicator = {
      heart_rate: [
        {
          reading_id: 12,
          astronaut_key: "a1",
          astronaut_name: "Alice",
          indicator: "heart_rate",
          value: { bpm: 71 },
          unit: "bpm",
          observed_at: "2026-11-14T11:59:00.000Z",
          batch_id: "b1",
          source: "mysql",
          app_version: "0.1.0",
          prev_hash: "0",
          row_hash: "h12",
        },
      ],
      glucose: [],
    };

    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        if (cmd === "verify_telemetry") return verifyPayload;
        if (cmd === "list_telemetry") return itemsByIndicator[args.input.indicator] ?? [];
        throw new Error(`Unexpected command: ${cmd}`);
      },
    };

    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => [{ id: "a1", name: "Alice" }]);
    await flushMicrotasks();

    const traceLine = host.querySelector(".tele-hist-trace");
    const verifyBtn = host.querySelector(".tele-hist-verify");
    const [, indicatorSelect] = host.querySelectorAll(".tele-hist-select");
    assert.equal(traceLine.textContent, "Checked 12:00:00 UTC · 12 readings in chain · consistent ✓");

    // Llega glucosa #13 y el usuario selecciona glucosa por primera vez: debe avisar pese a tener 1 sola fila.
    itemsByIndicator.glucose = [
      {
        reading_id: 13,
        astronaut_key: "a1",
        astronaut_name: "Alice",
        indicator: "glucose",
        value: { mg_dl: 102 },
        unit: "mg/dL",
        observed_at: "2026-11-14T12:00:10.000Z",
        batch_id: "b2",
        source: "mysql",
        app_version: "0.1.0",
        prev_hash: "0",
        row_hash: "h13",
      },
    ];
    indicatorSelect.value = "glucose";
    indicatorSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();

    assert.equal(
      traceLine.textContent,
      "Checked 12:00:00 UTC · 12 readings in chain · consistent ✓ · newer readings not yet checked",
    );

    // Si checked_through_reading_id es null con lecturas presentes: muestra "scope unknown".
    verifyPayload = {
      ok: true,
      checked: 0,
      series_checked: 0,
      first_bad_reading_id: null,
      problem: null,
      checked_through_reading_id: null,
    };
    verifyBtn.click();
    await flushMicrotasks();
    assert.equal(
      traceLine.textContent,
      "Checked 12:00:00 UTC · 0 readings in chain · consistent ✓ · scope unknown",
    );

    // Si checked_through_reading_id es null pero NO hay lecturas cargadas, no muestra "scope unknown".
    itemsByIndicator.heart_rate = [];
    itemsByIndicator.glucose = [];
    await handle.refresh();
    indicatorSelect.value = "heart_rate";
    indicatorSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();
    assert.equal(traceLine.textContent, "Checked 12:00:00 UTC · 0 readings in chain · consistent ✓");

    handle.destroy();
  });

  test("setPersistStatus pinta los tres estados (ok, slow, error) en el panel con role=status", async () => {
    delete globalThis.window.__TAURI_INTERNALS__;
    const { createHost } = createDomEnvironment();
    const host = createHost();

    const handle = historyMod.mountTelemetryHistory(host, () => []);
    await flushMicrotasks();

    const persistLine = host.querySelector(".tele-hist-persist");
    assert.ok(persistLine);
    assert.equal(persistLine.getAttribute("role"), "status");
    assert.equal(persistLine.hidden, true);

    handle.setPersistStatus({ status: "ok", ok: true, stored: 12, skipped: 0, lastError: null });
    assert.equal(persistLine.hidden, false);
    assert.equal(persistLine.dataset.tone, "ok");
    assert.equal(persistLine.dataset.state, "ok");
    assert.equal(persistLine.textContent, "Saving to device: OK · last batch stored 12");

    handle.setPersistStatus({ status: "slow", ok: true, stored: 12, skipped: 0, lastError: null });
    assert.equal(persistLine.hidden, false);
    assert.equal(persistLine.dataset.tone, "warn");
    assert.equal(persistLine.dataset.state, "slow");
    assert.equal(
      persistLine.textContent,
      "Saving to device is taking longer than usual; waiting for it to finish.",
    );

    handle.setPersistStatus({ status: "error", ok: false, stored: 0, skipped: 0, lastError: "disk full" });
    assert.equal(persistLine.hidden, false);
    assert.equal(persistLine.dataset.tone, "alarm");
    assert.equal(persistLine.dataset.state, "error");
    assert.equal(persistLine.textContent, "Saving to device failed: disk full");

    handle.setPersistStatus(null);
    assert.equal(persistLine.hidden, true);
    assert.equal(persistLine.textContent, "");

    handle.destroy();
  });
});


describe("exportación JSON", () => {
  test("el envoltorio omite input cuando no se proporciona", async () => {
    createDomEnvironment();
    const calls = [];
    window.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => { calls.push({ cmd, args }); return {}; } };
    await coreMod.core.exportJson();
    await coreMod.core.exportJson({ include_telemetry: false });
    assert.equal(calls[0].cmd, "export_json");
    assert.equal(calls[0].args == null || !("input" in calls[0].args), true);
    assert.deepEqual(calls[1].args.input, { include_telemetry: false });
  });

  test("fuera de Tauri está deshabilitado y no invoca", async () => {
    const { createHost } = createDomEnvironment();
    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => []);
    const button = host.querySelector(".tele-hist-export-button");
    assert.equal(button.disabled, true);
    assert.equal(host.querySelector(".tele-hist-export-checkbox").disabled, true);
    button.dispatchEvent(new Event("click"));
    await flushMicrotasks();
    assert.equal(host.querySelector(".tele-hist-export-status").textContent, "");
    assert.equal(host.querySelector(".tele-hist-empty").textContent, "History is available in the app");
    handle.destroy();
  });

  test("ocupado, una sola llamada, éxito seguro y error", async () => {
    const { createHost } = createDomEnvironment();
    const pending = deferred();
    const calls = [];
    window.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => {
      if (cmd === "export_json") { calls.push(args); return calls.length === 1 ? pending.promise : Promise.reject({ code: "storage_error", field: null, message: "disk full" }); }
      if (cmd === "list_telemetry") return [];
      if (cmd === "verify_telemetry") return { ok: true, checked: 0, series_checked: 0, first_bad_reading_id: null, problem: null, checked_through_reading_id: null };
    } };
    const host = createHost();
    const handle = historyMod.mountTelemetryHistory(host, () => []);
    const button = host.querySelector(".tele-hist-export-button");
    const checkbox = host.querySelector(".tele-hist-export-checkbox");
    const status = host.querySelector(".tele-hist-export-status");
    const path = host.querySelector(".tele-hist-export-path");
    assert.equal(checkbox.checked, true);
    checkbox.checked = false;
    button.click();
    button.dispatchEvent(new Event("click"));
    assert.equal(button.disabled, true);
    assert.equal(checkbox.disabled, true);
    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0].input, { include_telemetry: false });
    assert.equal(status.textContent, "Exporting…");
    pending.resolve({ path: "C:/<file>&.json", file_name: "<file>&.json", bytes: 10, exported_at: "now", counts: { records: 1, events: 2, evaluations: 3, telemetry_readings: 4, outbox_pending: 5 } });
    await flushMicrotasks();
    assert.equal(status.getAttribute("role"), "status");
    assert.equal(status.getAttribute("aria-live"), "polite");
    assert.equal(status.textContent, "Exported <file>&.json · 1 records · 2 events · 3 evaluations · 4 telemetry readings");
    assert.equal(path.textContent, "C:/<file>&.json");
    assert.equal(path.children.length, 0);
    assert.equal(status.children.length, 0);
    assert.equal(button.disabled, false);
    button.click();
    await flushMicrotasks();
    assert.equal(status.textContent, "Export failed: disk full");
    assert.equal(path.textContent, "");
    handle.destroy();
  });
});


test("3: missing local/feed catalog does not claim SQLite is empty", async () => {
  const { createHost } = createDomEnvironment();
  const calls = [];
  window.__TAURI_INTERNALS__ = { invoke: async cmd => {
    calls.push(cmd);
    if (cmd === "recommendations_cached_crew") return null;
    if (cmd === "list_telemetry") return [];
    if (cmd === "verify_telemetry") return { ok: true, checked: 0, series_checked: 0, checked_through_reading_id: null };
    throw Error("Unexpected command: " + cmd);
  } };
  let crew = [];
  const host = createHost();
  const handle = historyMod.mountTelemetryHistory(host, () => crew);
  try {
    await handle.refresh(); await flushMicrotasks();
    assert.ok(calls.includes("recommendations_cached_crew"));
    assert.equal(calls.includes("list_telemetry"), false);
    assert.equal(host.querySelector(".tele-hist-status").textContent, "No crew list available");
    assert.doesNotMatch(host.querySelector(".tele-hist-body").textContent, /No readings stored yet/);
    crew = [{ id: "a1", name: "Alice" }];
    await handle.refresh();
    assert.ok(calls.includes("list_telemetry"));
    assert.equal(host.querySelector(".tele-hist-status").textContent, "No readings stored yet", "empty storage requires a successful query");
  } finally { handle.destroy(); }
});

test("3: offline history merges saved snapshot IDs with the feed and queries SQLite", async () => {
  const { createHost } = createDomEnvironment();
  const calls = [];
  let catalogFails = false;
  window.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "recommendations_cached_crew") {
      if (catalogFails) throw Error("Snapshot unavailable");
      return { received_at: "2026-11-15T12:00:00.000Z", crew: [{ id: "saved", astronautName: "Saved crew" }] };
    }
    if (cmd === "list_telemetry") return [{ reading_id: 1, astronaut_key: "saved", astronaut_name: "Saved crew",
      indicator: "heart_rate", value: { bpm: 72 }, unit: "bpm", observed_at: "2026-11-15T12:00:00.000Z", source: "feed" }];
    if (cmd === "verify_telemetry") return { ok: true, checked: 1, series_checked: 1, checked_through_reading_id: 1 };
    throw Error("Unexpected command: " + cmd);
  } };
  let crew = [];
  const host = createHost();
  const handle = historyMod.mountTelemetryHistory(host, () => crew);
  try {
    await flushMicrotasks(40); await handle.refresh();
    assert.ok(calls.some(call => call.cmd === "list_telemetry" && call.args.input.astronaut_key === "saved"));
    assert.match(host.querySelector(".tele-hist-status").textContent, /1 readings loaded/);
    const select = host.querySelector(".tele-hist-select");
    assert.deepEqual(select.children.map(option => option.value), ["saved"]);
    crew = [{ id: "online", name: "Online crew" }, { id: "saved", name: "Renamed crew" }];
    await handle.refresh();
    assert.deepEqual(select.children.map(option => option.value), ["saved", "online"]);
    assert.equal(select.children[0].textContent, "Renamed crew");
    crew = []; catalogFails = true;
    await handle.refresh(); await flushMicrotasks();
    assert.deepEqual(select.children.map(option => option.value), ["saved"]);
    assert.match(host.querySelector(".tele-hist-status").textContent, /1 readings loaded/);
  } finally { handle.destroy(); }
});
