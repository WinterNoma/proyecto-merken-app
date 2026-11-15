import type { AstronautProfile } from "../types";
import { el, element, numberText, setText, utc } from "./dom";
import { mysqlStatuses, paintBadge, type StatusCopy, type VitalKey } from "./status-copy";
import type { FeedState } from "./telemetry";

// The feed's mood is free text. Translate known vocabulary; unknown text is
// not assigned a health meaning or passed through as potentially non-English UI.
const moods: Record<string, string> = {
  feliz: "Happy", happy: "Happy", alegre: "Cheerful", cheerful: "Cheerful",
  tranquilo: "Calm", tranquila: "Calm", calm: "Calm", calmado: "Calm", calmada: "Calm",
  neutral: "Neutral", normal: "Neutral", bien: "Good", good: "Good", bueno: "Good",
  excelente: "Excellent", excellent: "Excellent", mal: "Poor", malo: "Poor", poor: "Poor",
  triste: "Sad", sad: "Sad", ansioso: "Anxious", ansiosa: "Anxious", anxious: "Anxious",
  estresado: "Stressed", estresada: "Stressed", stressed: "Stressed",
  cansado: "Tired", cansada: "Tired", tired: "Tired", fatigado: "Fatigued", fatigued: "Fatigued",
  irritable: "Irritable", motivado: "Motivated", motivated: "Motivated",
  estable: "Steady", steady: "Steady", preocupado: "Concerned", concerned: "Concerned",
  relajado: "Relaxed", relaxed: "Relaxed",
};
export function moodText(value: string | null): string {
  if (!value) return "--";
  return moods[value.toLocaleLowerCase("en-US").trim()] ?? "Recorded";
}
export function createVitals() {
  const keys = ["heart", "oxygen", "pressure", "glucose", "temperature", "sleep", "mood", "nutrition", "radiation", "distance"] as const;
  const cards = new Map(keys.map(key => [key, element("#vital-" + key)]));
  for (const card of cards.values()) {
    const status = card.querySelector<HTMLElement>(".vital-status");
    if (status) { status.classList.add("status-chip"); paintBadge(status, { tone: "neutral", label: "No data", icon: "no_data" }); }
  }
  return {
    render(member: AstronautProfile | null): void {
      setText(element("#selected-name"), member?.name ?? "No crew available");
      setText(element("#selected-id"), member ? "Crew member · ID " + member.id : "--");
      const t = member?.telemetry;
      const states = t ? mysqlStatuses(t) : null;
      const values: Record<typeof keys[number], string> = {
        heart: numberText(t?.heart_rate ?? null), oxygen: numberText(t?.oxygen_in_blood ?? null),
        pressure: numberText(t?.blood_pressure.systolic ?? null) + " / " + numberText(t?.blood_pressure.diastolic ?? null),
        glucose: numberText(t?.blood_sugar ?? null), temperature: numberText(t?.body_temperature ?? null),
        sleep: numberText(t?.sleep_hours ?? null), mood: moodText(t?.mood ?? null),
        nutrition: numberText(t?.nutrition ?? null), radiation: numberText(t?.space_radiation ?? null),
        distance: numberText(t?.distance_from_earth ?? null),
      };
      for (const key of keys) {
        const card = cards.get(key);
        if (!card) continue;
        const value = card.querySelector(".reading-value");
        if (value) setText(value, values[key]);
        let copy: StatusCopy = { tone: "neutral", label: "No data", icon: "no_data" };
        const contextual = ["nutrition", "radiation", "distance"].includes(key);
        if (states && key in states) copy = states[key as VitalKey];
        else if (values[key] !== "--") copy = { tone: "neutral", label: key === "mood" ? "Self-reported · Not evaluated" : "Context only", icon: "info" };
        card.dataset.tone = contextual ? "neutral" : copy.tone;
        const status = card.querySelector<HTMLElement>(".vital-status");
        if (status) {
          status.classList.toggle("context-label", contextual || key === "mood");
          paintBadge(status, copy);
          // Keep the full source/status wording for assistive technology and
          // reading details; the phone card uses the shorter evaluation label.
          if (key === "mood" && copy.label === "Self-reported · Not evaluated") {
            status.querySelector("span")?.classList.add("status-label-full");
            if (!status.querySelector(".status-label-compact")) {
              const compact = el("span", "status-label-compact", "Not evaluated");
              compact.setAttribute("aria-hidden", "true");
              status.append(compact);
            }
          }
        }
      }
    },
    feed(state: FeedState): void {
      const label = state.cached ? "Saved backend replay · " + (state.lastSuccess ? utc(state.lastSuccess, true) : "Synchronize a snapshot") : state.preview ? "DEMO · Fixed sample readings" : state.loading ? "Connecting…" : state.live ? "Live · " + (state.lastSuccess ? utc(state.lastSuccess, true) : "Awaiting feed")
        : state.lastSuccess ? "Last update " + utc(state.lastSuccess, true) : "Connection unavailable";
      setText(element("#feed-status"), label);
      setText(element("#feed-time"), state.lastSuccess ? "Received " + utc(state.lastSuccess) : "No data received.");
      const notice = element("#feed-notice");
      notice.hidden = state.loading || (state.live && state.crew.length > 0);
      setText(element("#feed-message"), state.cached ? "Saved backend replay. Synchronize manually to refresh; receipt time does not establish measurement timing or health readiness." : state.live ? "No crew available. The feed returned no crew records."
        : state.lastSuccess ? "Connection unavailable. Showing the last successfully received readings." : "Connection unavailable. No crew readings have been received.");
    },
  };
}
