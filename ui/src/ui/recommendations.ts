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

  function roster(next: Cache | null, selectSnapshot = true): void {
    cache = next;
    const selected = target.value;
    target.replaceChildren(new Option("On-device profile", ""));
    for (const row of next?.crew ?? []) {
      const option = new Option(row.astronautName + (row.garminLinked ? " · saved snapshot" : " · unsupported source"), String(row.id));
      option.disabled = !row.garminLinked;
      target.append(option);
    }
    const preferred = next?.crew.find(row => row.garminLinked && String(row.id) === selected)
      ?? next?.crew.find(row => row.garminLinked);
    if (preferred && selectSnapshot) target.value = String(preferred.id);
    describeSource();
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
    cancel.disabled = true;
    status.textContent = "Cancellation requested.";
    void invoke("recommendations_cancel").catch(error => {
      if (task !== "generate") return;
      status.textContent = errorMessage(error);
      cancel.disabled = false;
    });
  });

  sync.addEventListener("submit", async event => {
    event.preventDefault();
    if (!available || task) return;
    const input = { username: username.value, password: password.value };
    password.value = "";
    setTask("sync");
    clearResult();
    status.textContent = "";
    syncStatus.textContent = "Synchronizing measurements…";
    try {
      roster(await invoke<Cache>("recommendations_sync_backend", { input }));
      window.dispatchEvent(new Event("merken:backend-snapshot"));
      syncStatus.textContent = "Snapshot saved on this device.";
    } catch (error) {
      syncStatus.textContent = errorMessage(error);
    } finally {
      input.password = "";
      setTask(null);
    }
  });

  if (!available) {
    status.textContent = "Open the installed app to generate a recommendation.";
    return;
  }
  void invoke<Cache | null>("recommendations_cached_crew").then(saved => {
    if (!cache) roster(saved, task === null);
  }).catch(() => {
    syncStatus.textContent = "The saved ground snapshot could not be loaded.";
  });
}
