import type { AstronautProfile, BloodPressure } from "../types";

export interface FeedState {
  crew: AstronautProfile[];
  loading: boolean;
  live: boolean;
  lastSuccess: Date | null;
}
function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
export function reading(value: unknown): number | null {
  if (typeof value !== "number" && typeof value !== "string") return null;
  if (typeof value === "string" && !value.trim()) return null;
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}
export function parsePressure(value: unknown): BloodPressure {
  if (typeof value !== "string") return { systolic: null, diastolic: null };
  const parts = value.split("/");
  if (parts.length !== 2) return { systolic: null, diastolic: null };
  return { systolic: reading(parts[0]), diastolic: reading(parts[1]) };
}
export function parseCrew(payload: unknown): AstronautProfile[] {
  if (!Array.isArray(payload)) throw new Error("Invalid crew response");
  const seen = new Set<string>();
  return payload.map((row: unknown) => {
    if (!object(row) || (typeof row.id !== "number" && typeof row.id !== "string") || String(row.id).trim() === "") {
      throw new Error("Invalid crew identity");
    }
    const id = String(row.id);
    if (seen.has(id)) throw new Error("Duplicate crew identity");
    seen.add(id);
    const name = typeof row.astronaut_name === "string" && row.astronaut_name.trim() ? row.astronaut_name.trim() : "Unnamed crew member";
    return { id, name, telemetry: {
      astronaut_name: name, blood_pressure: parsePressure(row.blood_pressure),
      heart_rate: reading(row.heart_rate), oxygen_in_blood: reading(row.oxygen_in_blood),
      blood_sugar: reading(row.blood_sugar), body_temperature: reading(row.body_temperature),
      sleep_hours: reading(row.sleep_hours), nutrition: reading(row.nutrition),
      space_radiation: reading(row.space_radiation), distance_from_earth: reading(row.distance_from_earth),
      mood: typeof row.mood === "string" && row.mood.trim() ? row.mood.trim() : null,
    } };
  });
}
export function createTelemetryFeed(onUpdate: (state: FeedState) => void) {
  let state: FeedState = { crew: [], loading: true, live: false, lastSuccess: null };
  let inFlight = false;
  let stopped = false;
  let controller: AbortController | null = null;
  async function refresh(): Promise<void> {
    if (inFlight || stopped) return;
    inFlight = true;
    controller = new AbortController();
    const timeout = window.setTimeout(() => controller?.abort(), 8000);
    try {
      const response = await fetch("/api/astronauts", { signal: controller.signal, cache: "no-store" });
      if (!response.ok) throw new Error("Crew request failed");
      const crew = parseCrew(await response.json());
      state = { crew, loading: false, live: true, lastSuccess: new Date() };
    } catch {
      state = { ...state, loading: false, live: false };
    } finally {
      window.clearTimeout(timeout);
      inFlight = false;
      if (!stopped) onUpdate(state);
    }
  }
  onUpdate(state);
  void refresh();
  const timer = window.setInterval(() => { void refresh(); }, 2000);
  return {
    refresh,
    stop(): void { stopped = true; window.clearInterval(timer); controller?.abort(); },
  };
}
