import { invoke } from "@tauri-apps/api/core";
import { el, element, timestamp, utc } from "./dom";

type Crew = { id: number; astronautName: string; garminLinked?: boolean };
type Cache = { received_at: string; crew: Crew[] };
type Result = {
  provider: string; generated_at: string; source: string; core_action: string;
  snapshot_status: string; context_note: string; elapsed_ms: number;
  readings: { metric: string; value: number | null; unit: string; measured_at: string | null }[];
  recommendation: { recommendation: string; caveat: string; source_ids: string[] };
};

function errorMessage(error: unknown): string {
  return typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error);
}

function contextLabel(value: string): string {
  return value.replace(/_/g, " ").toLowerCase();
}

function resultContent(result: Result): HTMLElement[] {
  const provider = result.provider === "SLM_OFFLINE" ? "Offline SLM"
    : result.provider === "CONTRACT_RULES" ? "Approved activity cards" : "Core-directed action";
  const context = el("dl", "recommendation-context");
  context.append(el("dt", "", "Snapshot"), el("dd", "", contextLabel(result.snapshot_status)),
    el("dt", "", "Core action"), el("dd", "", contextLabel(result.core_action)));

  const readings = el("details", "recommendation-result-details");
  readings.append(el("summary", "", "Measurements used"));
  if (result.readings.length) {
    const list = el("dl", "recommendation-readings");
    for (const reading of result.readings) {
      const value = reading.value === null ? "Unknown" : String(reading.value) + (reading.unit ? " " + reading.unit : "");
      const row = el("div");
      const definition = el("dd", "", value);
      definition.append(timestamp(reading.measured_at));
      row.append(el("dt", "", contextLabel(reading.metric)), definition);
      list.append(row);
    }
    readings.append(list);
  } else {
    readings.append(el("p", "caption", "No measurements were included."));
  }

  const sources = el("details", "recommendation-result-details");
  sources.append(el("summary", "", "Source and references"),
    el("p", "caption", result.context_note));
  const metadata = el("p", "caption", "Source: " + result.source + " · Generated ");
  metadata.append(timestamp(result.generated_at), " · " + (result.elapsed_ms / 1000).toFixed(1) + " s");
  sources.append(metadata);
  if (result.recommendation.source_ids.length) {
    sources.append(el("p", "caption", "References: " + result.recommendation.source_ids.join(", ")));
  }

  return [
    el("p", "recommendation-provider", provider),
    el("p", "recommendation-copy", result.recommendation.recommendation),
    el("p", "recommendation-caveat", result.recommendation.caveat),
    context, readings, sources,
    el("p", "caption", "Experimental support. Diagnosis, prescriptions and exercise clearance require clinical review."),
  ];
}

export function setupRecommendations(available: boolean): void {
  const form = element<HTMLFormElement>("#recommendation-form");
  const target = element<HTMLSelectElement>("#recommendation-source");
  const preference = element<HTMLSelectElement>("#recommendation-preference");
  const sourceNote = element("#recommendation-source-note");
  const generate = element<HTMLButtonElement>("#recommendation-generate");
  const cancel = element<HTMLButtonElement>("#recommendation-cancel");
  const status = element("#recommendation-status");
  const empty = element("#recommendation-empty");
  const output = element("#recommendation-output");
  const sync = element<HTMLFormElement>("#recommendation-sync");
  const username = element<HTMLInputElement>("#recommendation-username");
  const password = element<HTMLInputElement>("#recommendation-password");
  const connect = element<HTMLButtonElement>("#recommendation-connect");
  const syncStatus = element("#recommendation-sync-status");
  let cache: Cache | null = null;
  let task: "generate" | "sync" | null = null;
  let sessionActive = false;
  let closed = false;
  let timer: ReturnType<typeof setInterval> | null = null;
  let refreshPending: Promise<void> | null = null;
  let generation = 0;

  function clearResult(): void {
    output.replaceChildren();
    output.hidden = true;
    empty.hidden = false;
  }

  function describeSource(): void {
    sourceNote.textContent = target.value && cache
      ? "Ground snapshot saved " + utc(cache.received_at) + ". The original measurement times are shown with the result."
      : "Uses the profile and readings stored on this device.";
  }

  function roster(next: Cache | null, selectSnapshot = true): boolean {
    cache = next;
    const selected = target.value;
    target.replaceChildren(new Option("On-device profile", ""));
    for (const row of next?.crew ?? []) {
      const option = new Option(row.astronautName + (row.garminLinked ? " · saved snapshot" : " · unsupported source"), String(row.id));
      option.disabled = !row.garminLinked;
      target.append(option);
    }
    const existing = next?.crew.find(row => row.garminLinked && String(row.id) === selected);
    const missing = Boolean(selected && !existing);
    const preferred = existing ?? (selectSnapshot ? next?.crew.find(row => row.garminLinked) : undefined);
    target.value = preferred ? String(preferred.id) : "";
    describeSource();
    return missing;
  }

  function setTask(next: typeof task): void {
    task = next;
    generate.disabled = !available || task !== null;
    connect.disabled = !available || task !== null;
    target.disabled = task !== null;
    preference.disabled = task !== null;
    username.disabled = !available || task === "sync";
    password.disabled = !available || task === "sync";
    cancel.hidden = task !== "generate";
    cancel.disabled = false;
  }

  roster(null);
  setTask(null);
  function changeSetup(): void {
    clearResult();
    if (available) status.textContent = "";
  }
  target.addEventListener("change", () => { describeSource(); changeSetup(); });
  preference.addEventListener("change", changeSetup);

  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (!available || task) return;
    const input = { astronaut_id: target.value || null, preferred_activity: preference.value || null };
    setTask("generate");
    clearResult();
    status.textContent = "Generating on this device. The first request may take longer.";
    try {
      const result = await invoke<Result>("recommendations_generate", { input });
      output.replaceChildren(...resultContent(result));
      empty.hidden = true;
      output.hidden = false;
      status.textContent = "Recommendation ready.";
    } catch (error) {
      status.textContent = errorMessage(error);
    } finally {
      setTask(null);
    }
  });

  cancel.addEventListener("click", () => {
    if (!available) return;
    cancel.disabled = true;
    status.textContent = "Cancellation requested.";
    void invoke("recommendations_cancel").catch(error => {
      if (task !== "generate") return;
      status.textContent = errorMessage(error);
      cancel.disabled = false;
    });
  });

  function stopTimer(): void {
    if (timer !== null) clearInterval(timer);
    timer = null;
  }

  function acceptSnapshot(next: Cache, manual: boolean): void {
    const missing = roster(next, manual);
    window.dispatchEvent(new Event("pulsys:backend-snapshot"));
    syncStatus.textContent = "Connected · updates every 15 s · Last snapshot " + utc(next.received_at)
      + (!manual && missing ? " · Selected astronaut unavailable; using on-device profile." : "");
  }

  function handleSessionError(error: unknown): string {
    const message = errorMessage(error);
    if (message.includes("Ground-station session expired")) {
      sessionActive = false;
      stopTimer();
    }
    return message;
  }

  function refresh(): Promise<void> {
    if (!available || !sessionActive || closed || document.hidden || task === "sync") return Promise.resolve();
    if (refreshPending) return refreshPending;
    const current = generation;
    const pending = (async () => {
      try {
        const next = await invoke<Cache | null>("recommendations_refresh_backend");
        if (next && sessionActive && !closed && current === generation) acceptSnapshot(next, false);
      } catch (error) {
        if (!sessionActive || closed) return;
        // A stale refresh may not overwrite the manual Sync's status, but a session
        // it found expired is still gone: always drop it so the timer stays off.
        const message = handleSessionError(error);
        if (current === generation) syncStatus.textContent = message;
      }
    })();
    refreshPending = pending;
    void pending.finally(() => { if (refreshPending === pending) refreshPending = null; });
    return pending;
  }

  function startTimer(): void {
    stopTimer();
    if (available && sessionActive && !closed && !document.hidden && task !== "sync") {
      timer = setInterval(() => { void refresh(); }, 15_000);
    }
  }

  document.addEventListener("visibilitychange", () => {
    if (document.hidden) stopTimer();
    else if (sessionActive && !closed) { void refresh(); startTimer(); }
  });
  window.addEventListener("pagehide", () => { closed = true; stopTimer(); });

  sync.addEventListener("submit", async event => {
    event.preventDefault();
    if (!available || task) return;
    ++generation;
    stopTimer();
    const input = { username: username.value, password: password.value };
    password.value = "";
    setTask("sync");
    clearResult();
    status.textContent = "";
    syncStatus.textContent = "Synchronizing measurements…";
    try {
      if (refreshPending) await refreshPending;
      const next = await invoke<Cache>("recommendations_sync_backend", { input });
      if (!closed) {
        sessionActive = true;
        acceptSnapshot(next, true);
      }
    } catch (error) {
      if (!closed) syncStatus.textContent = handleSessionError(error);
    } finally {
      input.password = "";
      setTask(null);
      startTimer();
    }
  });

  if (!available) {
    status.textContent = "Open the installed app to generate a recommendation.";
    return;
  }
  const initialGeneration = generation;
  void invoke<Cache | null>("recommendations_cached_crew").then(saved => {
    if (!cache && initialGeneration === generation) roster(saved, task === null);
  }).catch(() => {
    if (initialGeneration === generation) syncStatus.textContent = "The saved ground snapshot could not be loaded.";
  });
}
