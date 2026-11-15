/// <reference types="vite/client" />
import "@fontsource/barlow-semi-condensed/latin-600.css";
import "@fontsource/public-sans/latin-400.css";
import "@fontsource/public-sans/latin-600.css";
import "@fontsource/roboto-mono/latin-400.css";
import "../../docs/brand/tokens.css";
import "./styles.css";
import "./hybrid.css";
import { isTauri } from "@tauri-apps/api/core";
import { element } from "./ui/dom";
import { createVitals } from "./ui/vitals";
import { createCrewSelector } from "./ui/crew";
import { createTelemetryFeed } from "./ui/telemetry";
import { createCorePanel } from "./ui/core-panel";
import { setupHelp } from "./ui/help";

import { createDashboard, setupPages } from "./ui/dashboard";
import { setupVoice } from "./ui/voice";
import { icon, type IconName } from "./ui/icons";
import { createPreviewFeed } from "./ui/visual-preview";
import type { FeedState } from "./ui/telemetry";
import { createAmbientParticles } from "./ui/ambient-particles";

// A dev-only visual fixture. It never polls MySQL or invokes the local core.
const preview = import.meta.env.DEV && new URLSearchParams(location.search).get("preview") === "1";
const available = isTauri() && !preview;
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
setupPages(index => ambient.setActive(index === 0));
const vitals = createVitals();
const crew = createCrewSelector(member => { vitals.render(member); dashboard.member(member); });
const renderFeed = (state: FeedState): void => {
  crew.update(state);
  vitals.feed(state);
  dashboard.feed(state);
};
const feed = preview ? createPreviewFeed(renderFeed) : createTelemetryFeed(renderFeed);
element("#feed-retry").addEventListener("click", () => { void feed.refresh(); });
const panel = createCorePanel(available, dashboard.core);
setupHelp(available, panel);
setupVoice(available, panel, ambient.setHolding);
window.addEventListener("pagehide", event => {
  if (!event.persisted) { feed.stop(); panel.stop(); ambient.destroy(); }
});
import.meta.hot?.dispose(() => ambient.destroy());
