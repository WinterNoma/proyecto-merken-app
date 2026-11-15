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
let telemetryMod;
let historyMod;

before(async () => {
  tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "merken-telemetry-ui-test-"));
  const build = await rolldown({
    input: {
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
  telemetryMod = await import(pathToFileURL(path.join(tempDir, "telemetry.mjs")).href);
  historyMod = await import(pathToFileURL(path.join(tempDir, "history.mjs")).href);
});

after(() => {
  if (tempDir) {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

describe("telemetría local — parseCrew, crewToReadings y cola IPC", () => {
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

  test("createTelemetryFeed fuera de Tauri publica el feed sin invocar IPC", async () => {
    delete globalThis.window.__TAURI_INTERNALS__;
    let invokeCount = 0;
    globalThis.fetch = async () => ({
      ok: true,
      json: async () => [sampleRawCrewRow("a1")],
    });

    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s));
    await flushMicrotasks();
    feed.stop();

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
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s));
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
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s));
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

  test("cola de guardado: tiempo máximo de 10 s marca error en state.persist y libera la cola", async t => {
    t.mock.timers.enable({ apis: ["setTimeout", "setInterval", "Date"] });

    let callIndex = 0;
    globalThis.window.__TAURI_INTERNALS__ = {
      invoke: (_cmd, args) => {
        callIndex += 1;
        if (callIndex === 1) {
          // Simula IPC colgado que nunca responde.
          return new Promise(() => {});
        }
        return Promise.resolve({
          stored: args.input.readings.length,
          skipped: 0,
          batch_id: "recovered-batch",
        });
      },
    };

    globalThis.fetch = async () => ({
      ok: true,
      json: async () => [sampleRawCrewRow("a1")],
    });

    const states = [];
    const feed = telemetryMod.createTelemetryFeed(s => states.push(s));
    await flushMicrotasks();

    assert.equal(callIndex, 1);
    assert.equal(states.at(-1).live, true);

    // Avanzamos 10 s: el primer IPC vence por timeout, registra error y libera la cola.
    t.mock.timers.tick(10_000);
    await flushMicrotasks();

    const failedState = states.find(s => s.persist && s.persist.ok === false);
    assert.ok(failedState, "Debe haberse publicado un estado con persist.ok === false tras vencer 10 s");
    assert.match(failedState.persist.lastError, /timed out/i);

    // La cola quedó liberada: un guardado posterior (por intervalo o refresco) completa correctamente.
    await feed.refresh();
    await flushMicrotasks();
    feed.stop();

    assert.ok(callIndex >= 2);
    assert.equal(states.at(-1).persist.ok, true);
    assert.equal(states.at(-1).persist.lastBatchId, "recovered-batch");
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
    assert.equal(host.querySelector(".tele-hist-status").textContent, "No readings stored yet");

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
      cmd === "list_telemetry" ? hang2.promise : Promise.resolve({ ok: true, checked: 0, series_checked: 0, first_bad_reading_id: null, problem: null });
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

    const crew = [
      { id: "a1", name: "Alice" },
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
    assert.equal(traceLine.textContent, "15 readings · chain consistent ✓");
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
    assert.equal(traceLine.textContent, "15 readings · chain consistent ✓");

    // Botón "Verify chain" deshabilitado mientras corre y sin dos verificaciones simultáneas.
    nextVerifyDeferred = deferred();
    const slowVerify = nextVerifyDeferred;
    verifyResponse = {
      ok: false,
      checked: 10,
      series_checked: 1,
      first_bad_reading_id: 42,
      problem: "hash_mismatch",
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
    assert.equal(traceLine.textContent, "chain problem: hash mismatch at #42");

    // Tras >= 60 s, cambiar a "a2" vuelve a verificar y muestra texto legible sin #id si es null.
    t.mock.timers.tick(65_000);
    verifyResponse = {
      ok: false,
      checked: 9,
      series_checked: 1,
      first_bad_reading_id: null,
      problem: "missing_tail",
    };
    crewSelect.value = "a2";
    crewSelect.dispatchEvent(new Event("change"));
    t.mock.timers.tick(250);
    await flushMicrotasks();
    assert.equal(verifyCalls.length, 4);
    assert.equal(traceLine.textContent, "chain problem: missing tail");

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
      };
      verifyBtn.click();
      await flushMicrotasks();
      assert.equal(traceLine.textContent, expected);
    }

    handle.destroy();
  });
});
