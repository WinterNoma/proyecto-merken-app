/// <reference types="vite/client" />
import "@fontsource/barlow-semi-condensed/latin-600.css";
import "@fontsource/public-sans/latin-400.css";
import "@fontsource/public-sans/latin-600.css";
import "@fontsource/roboto-mono/latin-400.css";
import "../../docs/brand/tokens.css";
import "./styles.css";
import { isTauri } from "@tauri-apps/api/core";
import { element } from "./ui/dom";
import { createVitals } from "./ui/vitals";
import { createCrewSelector } from "./ui/crew";
import { createTelemetryFeed } from "./ui/telemetry";
import { createCorePanel } from "./ui/core-panel";
import { setupHelp } from "./ui/help";

import { createDashboard, setupPages } from "./ui/dashboard";
import { setupVoice } from "./ui/voice";

const available = isTauri();
const dashboard = createDashboard(available);
setupPages();
const vitals = createVitals();
const crew = createCrewSelector(member => { vitals.render(member); dashboard.member(member); });
const feed = createTelemetryFeed(state => {
  crew.update(state);
  vitals.feed(state);
  dashboard.feed(state);
});
element("#feed-retry").addEventListener("click", () => { void feed.refresh(); });
const panel = createCorePanel(available, dashboard.core);
setupHelp(available, panel);
setupVoice(available, panel);
window.addEventListener("pagehide", event => {
  if (!event.persisted) { feed.stop(); panel.stop(); }
});
