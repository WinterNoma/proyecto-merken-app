import type { FeedState } from "./telemetry";
import { parseCrew } from "./telemetry";

/** Read-only fixtures for ?preview=1 in Vite dev. No network or native calls. */
export function createPreviewFeed(onUpdate: (state: FeedState) => void) {
  const state: FeedState = {
    preview: true, loading: false, live: true, lastSuccess: null,
    crew: parseCrew([
      { id: "sample-01", astronaut_name: "A. Morgan", heart_rate: 72, blood_pressure: "118/76",
        oxygen_in_blood: 98, blood_sugar: 95, body_temperature: 36.7, sleep_hours: 7.5,
        mood: "Calm", nutrition: 2100, space_radiation: 15, distance_from_earth: 384400 },
      { id: "sample-02", astronaut_name: "J. Rivera", heart_rate: 78, blood_pressure: "122/80",
        oxygen_in_blood: 97, blood_sugar: 102, body_temperature: 36.6, sleep_hours: 8,
        mood: "Good", nutrition: 2250, space_radiation: 15, distance_from_earth: 384400 },
    ]),
  };
  let stopped = false;
  const refresh = async (): Promise<void> => { if (!stopped) onUpdate(state); };
  void refresh();
  return { refresh, stop(): void { stopped = true; } };
}
