import { invoke } from "@tauri-apps/api/core";
import { el, element } from "./dom";

type Crew = { id: number; astronautName: string; garminLinked?: boolean };
type Cache = { received_at: string; crew: Crew[] };
type Result = {
  provider: string; generated_at: string; source: string; core_action: string;
  snapshot_status: string; context_note: string; elapsed_ms: number;
  readings: { metric: string; value: number | null; unit: string; measured_at: string | null }[];
  recommendation: { activity_id: string; recommendation: string; caveat: string; source_ids: string[] };
};
export function setupRecommendations(available: boolean): void {
  const panel = el("section", "panel"); panel.setAttribute("aria-label", "Offline recommendations");
  panel.append(el("h3", "", "Recommendations"), el("p", "caption", "Generated on this device with the offline SLM. Synchronizing measurements never sends an inference request to Nova."));
  const target = document.createElement("select"); target.setAttribute("aria-label", "Recommendation snapshot");
  const preference = document.createElement("select"); preference.setAttribute("aria-label", "Activity preference");
  for (const [value, label] of [["", "No leisure preference"], ["READING", "Reading"], ["MUSIC", "Music"], ["PUZZLE", "Puzzle"], ["JOURNAL", "Journal"]]) {
    const option = document.createElement("option"); option.value = value ?? ""; option.textContent = label ?? ""; preference.append(option);
  }
  const cached = el("p", "caption", "On-device profile. No synchronized backend snapshot loaded.");
  const generate = el("button", "button", "Recommendations"); generate.type = "button"; generate.disabled = !available;
  const cancel = el("button", "button secondary", "Cancel generation"); cancel.type = "button"; cancel.hidden = true;
  const message = el("p", "caption"); message.setAttribute("role", "status");
  const output = el("div", "notice"); output.hidden = true; output.setAttribute("aria-live", "polite");
  let busy = false;
  function clear(): void { output.replaceChildren(); output.hidden = true; }
  function roster(cache?: Cache | null): void {
    target.replaceChildren();
    const local = document.createElement("option"); local.value = ""; local.textContent = "On-device profile"; target.append(local);
    for (const row of cache?.crew ?? []) {
      const option = document.createElement("option"); option.value = String(row.id); option.textContent = row.astronautName + (row.garminLinked ? " · saved backend replay" : " · source not supported by V7"); option.disabled = !row.garminLinked; target.append(option);
    }
    if (cache) { cached.textContent = "Backend snapshot saved " + new Date(cache.received_at).toLocaleString("en-US") + ". It remains available offline; this is not a sensor measurement time."; const first = cache.crew.find(r => r.garminLinked); if (first) target.value = String(first.id); }
  }
  roster();
  target.addEventListener("change", clear); preference.addEventListener("change", clear);
  generate.addEventListener("click", async () => {
    if (busy) return; busy = true; generate.disabled = true; target.disabled = true; preference.disabled = true; cancel.hidden = false; clear();
    message.textContent = "Preparing the verified offline model and generating a recommendation. The first request may take longer.";
    try {
      const result = await invoke<Result>("recommendations_generate", { input: { astronaut_id: target.value || null, preferred_activity: preference.value || null } });
      output.append(el("h3", "", result.provider === "SLM_OFFLINE" ? "Offline SLM recommendation" : result.provider === "CONTRACT_RULES" ? "Recommendation from approved cards" : "Core-directed action"));
      if (result.provider === "CONTRACT_RULES") output.append(el("p", "caption", "Chosen by the on-device rules from the approved activity cards; no AI model was used."));
      output.append(el("p", "", result.recommendation.recommendation), el("p", "caption", result.recommendation.caveat));
      output.append(el("p", "", "Snapshot status: " + result.snapshot_status + " · Core action: " + result.core_action.split("_").join(" ")));
      output.append(el("p", "caption", result.context_note), el("p", "caption", "Source: " + result.source + " · Generated " + new Date(result.generated_at).toLocaleString("en-US") + " · " + (result.elapsed_ms / 1000).toFixed(1) + " s"));
      const details = document.createElement("details"); details.append(el("summary", "", "Measurements used"));
      for (const reading of result.readings) details.append(el("p", "caption", reading.metric.split("_").join(" ") + ": " + (reading.value ?? "Unknown") + " " + reading.unit + " · measured: " + (reading.measured_at ?? "Unknown")));
      output.append(details, el("p", "caption", "References: " + result.recommendation.source_ids.join(", ") + ". Experimental support; not a diagnosis, prescription or exercise clearance."));
      output.hidden = false; message.textContent = "Recommendation generated locally. Core alerts remain independent.";
    } catch (error) { message.textContent = typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error); }
    finally { busy = false; generate.disabled = !available; target.disabled = false; preference.disabled = false; cancel.hidden = true; }
  });
  cancel.addEventListener("click", () => { void invoke("recommendations_cancel"); message.textContent = "Cancellation requested. Core alerts remain available."; });
  const sync = document.createElement("details"); sync.append(el("summary", "", "Synchronize backend data for offline replay"));
  sync.append(el("p", "caption", "Uses your HTTPS ground-station login to save measurements and existing core flags on this device. Passwords are not saved. The model runs locally after synchronization."));
  const form = document.createElement("form");
  const username = document.createElement("input"); username.value = "demo"; username.autocomplete = "username"; username.required = true; username.maxLength = 100; username.setAttribute("aria-label", "Ground-station username");
  const password = document.createElement("input"); password.type = "password"; password.autocomplete = "current-password"; password.required = true; password.maxLength = 512; password.setAttribute("aria-label", "Ground-station password");
  const connect = el("button", "button secondary", "Synchronize backend snapshot"); connect.type = "submit"; connect.disabled = !available;
  form.append(username, password, connect); sync.append(form);
  form.addEventListener("submit", async event => {
    event.preventDefault(); if (busy) return; connect.disabled = true; clear();
    const credentials = { username: username.value, password: password.value }; password.value = "";
    message.textContent = "Synchronizing measurements only; no cloud inference is requested.";
    try { const cache = await invoke<Cache>("recommendations_sync_backend", { input: credentials }); roster(cache); window.dispatchEvent(new Event("merken:backend-snapshot")); message.textContent = "Snapshot saved. You can disconnect and press Recommendations."; }
    catch (error) { message.textContent = typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error); }
    finally { credentials.password = ""; connect.disabled = !available; }
  });
  const exercise = document.createElement("details"); exercise.append(el("summary", "", "Space-mission exercise options"), el("p", "caption", "Cycling, treadmill and resistance equipment are mission-plan options. No personal intensity or duration is generated. Current clearance and a reviewed mission plan are required before exercise is recommended."));
  panel.append(target, preference, cached, generate, cancel, message, output, sync, exercise);
  if (!available) message.textContent = "Offline generation is available in the installed app, not this browser preview.";
  element("#overview").append(panel);
  if (available) void invoke<Cache | null>("recommendations_cached_crew").then(roster).catch(() => { message.textContent = "Cached backend data could not be loaded. The on-device profile remains available."; });
}
