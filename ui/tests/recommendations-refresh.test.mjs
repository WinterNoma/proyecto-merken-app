import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { after, test } from "node:test";
import { rolldown } from "rolldown";

const root = path.resolve(import.meta.dirname, "../..");
const temp = await fs.mkdtemp(path.join(os.tmpdir(), "merken-refresh-"));
const bundle = await rolldown({ input: path.join(root, "ui/src/ui/recommendations.ts") });
await bundle.write({ dir: temp, format: "esm", entryFileNames: "recommendations.mjs" });
await bundle.close();
const { setupRecommendations } = await import(pathToFileURL(path.join(temp, "recommendations.mjs")));
after(() => fs.rm(temp, { recursive: true, force: true }));

class Node extends EventTarget {
  constructor() { super(); this.children = []; this.value = ""; this.textContent = ""; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
}
class OptionNode extends Node {
  constructor(text, value) { super(); this.textContent = text; this.value = value; }
}
function environment(invoke) {
  const nodes = new Map();
  const document = new EventTarget();
  document.hidden = false;
  document.querySelector = selector => {
    if (!nodes.has(selector)) nodes.set(selector, new Node());
    return nodes.get(selector);
  };
  document.createElement = () => new Node();
  const window = new EventTarget();
  window.__TAURI_INTERNALS__ = { invoke };
  globalThis.document = document; globalThis.window = window; globalThis.Option = OptionNode;
  return { document, window, get: selector => document.querySelector(selector) };
}
const snapshot = { received_at: "2026-11-16T12:00:00Z", crew: [{ id: 1, astronautName: "A", garminLinked: true }] };
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function submit(node) { node.dispatchEvent(new Event("submit", { cancelable: true })); }

test("sync starts refresh, dispatches snapshots, pauses and resumes, and stops on expiry", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  const calls = []; let expired = false; let snapshots = 0;
  const env = environment(async command => {
    calls.push(command);
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend") return snapshot;
    if (command === "recommendations_refresh_backend") {
      if (expired) throw { message: "Ground-station session expired; sign in again" };
      return snapshot;
    }
    throw Error(command);
  });
  env.window.addEventListener("merken:backend-snapshot", () => snapshots++);
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  assert.equal(snapshots, 1);
  assert.match(env.get("#recommendation-sync-status").textContent, /Connected · updates every 15 s/);
  t.mock.timers.tick(15000); await flush();
  assert.equal(calls.filter(c => c === "recommendations_refresh_backend").length, 1);
  assert.equal(snapshots, 2);
  env.document.hidden = true; env.document.dispatchEvent(new Event("visibilitychange"));
  t.mock.timers.tick(30000); await flush();
  assert.equal(calls.filter(c => c === "recommendations_refresh_backend").length, 1);
  env.document.hidden = false; env.document.dispatchEvent(new Event("visibilitychange")); await flush();
  assert.equal(calls.filter(c => c === "recommendations_refresh_backend").length, 2);
  expired = true; t.mock.timers.tick(15000); await flush();
  assert.match(env.get("#recommendation-sync-status").textContent, /sign in again/);
  t.mock.timers.tick(30000); await flush();
  assert.equal(calls.filter(c => c === "recommendations_refresh_backend").length, 3);
  env.window.dispatchEvent(new Event("pagehide"));
});

test("browser mode never invokes backend", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  const calls = []; const env = environment(async command => { calls.push(command); });
  setupRecommendations(false);
  submit(env.get("#recommendation-sync"));
  env.document.dispatchEvent(new Event("visibilitychange"));
  t.mock.timers.tick(30000); await flush();
  assert.deepEqual(calls, []);
  env.window.dispatchEvent(new Event("pagehide"));
});

test("null refresh is silent and pagehide stops polling", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  const calls = []; let events = 0;
  const env = environment(async command => {
    calls.push(command);
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend") return snapshot;
    if (command === "recommendations_refresh_backend") return null;
    throw Error(command);
  });
  env.window.addEventListener("merken:backend-snapshot", () => events++);
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  const status = env.get("#recommendation-sync-status").textContent;
  t.mock.timers.tick(15000); await flush();
  assert.equal(events, 1);
  assert.equal(env.get("#recommendation-sync-status").textContent, status);
  env.window.dispatchEvent(new Event("pagehide"));
  t.mock.timers.tick(30000); await flush();
  assert.equal(calls.filter(c => c === "recommendations_refresh_backend").length, 1);
});


test("session expiry from manual sync stops polling", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  let syncs = 0; let refreshes = 0;
  const env = environment(async command => {
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend") {
      if (++syncs === 2) throw { message: "Ground-station session expired; sign in again" };
      return snapshot;
    }
    if (command === "recommendations_refresh_backend") { refreshes++; return snapshot; }
    throw Error(command);
  });
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  assert.match(env.get("#recommendation-sync-status").textContent, /session expired/);
  t.mock.timers.tick(30000); await flush();
  env.document.dispatchEvent(new Event("visibilitychange")); await flush();
  assert.equal(refreshes, 0);
  env.window.dispatchEvent(new Event("pagehide"));
});

test("refresh preserves local or chosen source and reports a missing astronaut", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  let next = { ...snapshot, crew: [...snapshot.crew, { id: 2, astronautName: "B", garminLinked: true }] };
  const env = environment(async command => {
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend" || command === "recommendations_refresh_backend") return next;
    throw Error(command);
  });
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  const source = env.get("#recommendation-source");
  assert.equal(source.value, "1");
  source.value = "";
  t.mock.timers.tick(15000); await flush();
  assert.equal(source.value, "");
  source.value = "2";
  t.mock.timers.tick(15000); await flush();
  assert.equal(source.value, "2");
  next = snapshot;
  t.mock.timers.tick(15000); await flush();
  assert.equal(source.value, "");
  assert.match(env.get("#recommendation-sync-status").textContent, /Selected astronaut unavailable/);
  env.window.dispatchEvent(new Event("pagehide"));
});

test("manual sync waits for stale refresh and retains its own error", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  let resolveRefresh; let syncs = 0; let refreshes = 0; let events = 0;
  const env = environment(async command => {
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend") {
      if (++syncs === 2) throw { message: "Manual sync failed" };
      return snapshot;
    }
    if (command === "recommendations_refresh_backend") {
      refreshes++;
      return new Promise(resolve => { resolveRefresh = resolve; });
    }
    throw Error(command);
  });
  env.window.addEventListener("merken:backend-snapshot", () => events++);
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  t.mock.timers.tick(15000); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  assert.equal(syncs, 1);
  t.mock.timers.tick(30000); await flush();
  assert.equal(refreshes, 1);
  resolveRefresh(snapshot); await flush();
  assert.equal(syncs, 2);
  assert.equal(events, 1);
  assert.equal(env.get("#recommendation-sync-status").textContent, "Manual sync failed");
  env.window.dispatchEvent(new Event("pagehide"));
});

test("stale refresh that finds the session expired still stops polling after a failed sync", async t => {
  t.mock.timers.enable({ apis: ["setInterval"] });
  let rejectRefresh; let syncs = 0; let refreshes = 0;
  const env = environment(async command => {
    if (command === "recommendations_cached_crew") return null;
    if (command === "recommendations_sync_backend") {
      if (++syncs === 2) throw { message: "Ground station is unreachable; cached data remains available" };
      return snapshot;
    }
    if (command === "recommendations_refresh_backend") {
      refreshes++;
      return new Promise((_, reject) => { rejectRefresh = reject; });
    }
    throw Error(command);
  });
  setupRecommendations(true); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  t.mock.timers.tick(15000); await flush();
  submit(env.get("#recommendation-sync")); await flush();
  rejectRefresh({ message: "Ground-station session expired; sign in again" }); await flush();
  assert.equal(syncs, 2);
  assert.match(env.get("#recommendation-sync-status").textContent, /unreachable/);
  t.mock.timers.tick(45000); await flush();
  assert.equal(refreshes, 1);
  env.window.dispatchEvent(new Event("pagehide"));
});
