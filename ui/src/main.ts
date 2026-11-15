/// <reference types="vite/client" />
import "@fontsource/barlow-semi-condensed/latin-600.css";
import "@fontsource/public-sans/latin-400.css";
import "@fontsource/public-sans/latin-600.css";
import "@fontsource/roboto-mono/latin-400.css";
import "../../docs/brand/tokens.css";
import "./styles.css";
import "./hybrid.css";
import "./assistant.css";
import { isTauri } from "@tauri-apps/api/core";
import { element } from "./ui/dom";
import { createVitals } from "./ui/vitals";
import { createCrewSelector } from "./ui/crew";
import { createTelemetryFeed } from "./ui/telemetry";
import { mountTelemetryHistory } from "./ui/telemetry-history";
import { createCorePanel } from "./ui/core-panel";
import { setupHelp } from "./ui/help";

import { createDashboard, setupPages } from "./ui/dashboard";
import { setupVoice } from "./ui/voice";
import { icon, type IconName } from "./ui/icons";
import { createPreviewFeed } from "./ui/visual-preview";
import type { FeedState } from "./ui/telemetry";
import { createAmbientParticles } from "./ui/ambient-particles";
import { setupRecommendations } from "./ui/recommendations";
import { core } from "./core-api";
import type { AstronautProfile } from "./types";

// A dev-only visual fixture. It never polls MySQL or invokes the local core.
const preview = import.meta.env.DEV && new URLSearchParams(location.search).get("preview") === "1";
const available = isTauri() && !preview;
setupRecommendations(available);
if (preview) {
  element("#visual-preview").hidden = false;
  element("#crew-dialog-description").textContent = "Browse synthetic sample crew. No device profile is changed.";
}
const cardIcons: Record<string, IconName> = {
  heart: "pulse", pressure: "pressure", oxygen: "oxygen", mood: "mood",
  temperature: "temperature", glucose: "glucose", sleep: "sleep", nutrition: "nutrition",
  radiation: "radiation", distance: "earth",
};
for (const [key, name] of Object.entries(cardIcons)) {
  element("#vital-" + key).querySelector(".card-label")?.prepend(icon(name));
}
element("#sync-open .card-label").prepend(icon("sync"));
const ambient = createAmbientParticles(element<HTMLCanvasElement>("#ambient-particles"));
// Read-only measurements and an A/B switch for local visual QA, absent in builds.
if (import.meta.env.DEV) Object.defineProperty(window, "__merkenAmbient", {
  configurable: true, value: { snapshot: ambient.snapshot, setEnabled: ambient.setEnabled },
});
const dashboard = createDashboard(available, zone => {
  ambient.pulseAt(document.querySelector(`.body-zone[data-zone="${zone}"]`) ?? element("#body-map"));
});
// Historial de telemetría guardada en el dispositivo (solo dentro de la app).
let latestCrew: { id: string; name: string }[] = [];
const historyHost = element("#telemetry-history");
const history = available ? mountTelemetryHistory(historyHost, () => latestCrew) : null;
historyHost.hidden = !available;
let historyTimer: number | null = null;
setupPages(index => {
  ambient.setActive(index === 0);
  if (!history) return;
  // El historial solo se refresca mientras su pantalla está a la vista.
  if (historyTimer !== null) { window.clearInterval(historyTimer); historyTimer = null; }
  if (index === 1) {
    void history.refresh();
    historyTimer = window.setInterval(() => { void history.refresh(); }, 10000);
  }
});
const vitals = createVitals();
// Copies the member's latest MySQL readings into the local core (DEMO), so the core
// evaluation, Explain and Recommendations can use them. Never automatic.
async function importToCore(member: AstronautProfile): Promise<string> {
  const result = await core.importCrewMember({ astronaut_key: member.id, display_name: member.name });
  await panel.refresh(true);
  return result.imported + " readings from this device's saved history are now in the on-device core as DEMO data"
    + (result.rejected ? " · " + result.rejected + " not accepted by the core (for example, sleep without its start and end time)" : "")
    + ". Mood is text and is not imported.";
}
const crew = createCrewSelector(member => { vitals.render(member); dashboard.member(member); }, available ? importToCore : undefined);
const renderFeed = (state: FeedState): void => {
  latestCrew = state.crew.map(member => ({ id: member.id, name: member.name }));
  // Si guardar en el dispositivo falla o va lento, el panel de historial lo dice.
  history?.setPersistStatus(state.persist ?? null);
  crew.update(state);
  vitals.feed(state);
  dashboard.feed(state);
};
const feed = preview ? createPreviewFeed(renderFeed) : createTelemetryFeed(renderFeed, available && !import.meta.env.DEV);
element("#feed-retry").addEventListener("click", () => { void feed.refresh(); });
const panel = createCorePanel(available, dashboard.core);
setupHelp(available, panel);
setupVoice(available, panel, ambient.setHolding);
window.addEventListener("pagehide", event => {
  if (!event.persisted) {
    feed.stop(); panel.stop(); ambient.destroy();
    if (historyTimer !== null) window.clearInterval(historyTimer);
    history?.destroy();
  }
});
import.meta.hot?.dispose(() => ambient.destroy());
