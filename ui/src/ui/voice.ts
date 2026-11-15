import { core, isCoreError, type Indicator, type LogRecordInput, type ProposedRecord, type RecordValue } from "../core-api";
import type { CorePanel } from "./core-panel";
import { element, el, setText, utc } from "./dom";
import { modal } from "./dialog";
import { bodyZones, zoneName } from "./body-map";
import { indicatorName } from "./core-format";
import { setupAssistant, type AssistSource } from "./assistant";

const fieldSelectors: Record<Indicator, Record<string, string>> = {
  heart_rate: { bpm: "#manual-a" },
  blood_pressure: { systolic: "#manual-a", diastolic: "#manual-b" },
  body_temperature: { celsius: "#manual-a" },
  glucose: { mg_dl: "#manual-a" },
  sleep: { hours: "#manual-a" },
  mood: { score: "#manual-a" },
  oxygen_in_blood: { percentage: "#manual-a" },
  nutrition: { kcal: "#manual-a" },
  space_radiation: { usv_h: "#manual-a" },
  distance_from_earth: { km: "#manual-a" },
  symptom: { body_zone: "#manual-zone", score: "#manual-a" },
};

export function setupVoice(available: boolean, panel: CorePanel, onHolding?: (active: boolean) => void): void {
  const dialog = element<HTMLDialogElement>("#voice-dialog");
  const hold = element<HTMLButtonElement>("#talk-hold");
  const entry = element(".voice-entry");
  const inlineStatus = element("#voice-inline-status");
  const close = element<HTMLButtonElement>("#voice-close");
  const form = element<HTMLFormElement>("#manual-form");
  const fields = element<HTMLFieldSetElement>("#manual-fields");
  const select = element<HTMLSelectElement>("#manual-indicator");
  const values = element("#manual-values");
  const time = element<HTMLInputElement>("#manual-time");
  const note = element<HTMLTextAreaElement>("#manual-note");
  const save = element<HTMLButtonElement>("#manual-save");
  const result = element("#manual-result");
  let holding = false;
  let pointer: number | null = null;
  let busy = false;
  let saved = false;
  let warnedDemo = false;
  let confirmedDraft: LogRecordInput | null = null;
  let entrySource: "form" | AssistSource = "form";
  let opening = 0;
  const controls = modal(dialog, () => !busy);
  const indicators: Indicator[] = ["heart_rate", "blood_pressure", "oxygen_in_blood", "mood", "body_temperature", "glucose", "sleep", "nutrition", "symptom", "space_radiation", "distance_from_earth"];
  for (const indicator of indicators) { const option = el("option", "", indicatorName(indicator)); option.value = indicator; select.append(option); }
  function input(id: string, label: string, type = "number"): HTMLInputElement {
    const caption = el("label", "", label); caption.htmlFor = id;
    const node = el("input"); node.id = id; node.type = type; node.required = true;
    if (type === "number") { node.step = "any"; node.inputMode = "decimal"; }
    if (type === "datetime-local") node.step = "1";
    values.append(caption, node); return node;
  }
  function renderFields(): void {
    values.replaceChildren();
    switch (select.value as Indicator) {
      case "blood_pressure": input("manual-a", "Systolic (mmHg)"); input("manual-b", "Diastolic (mmHg)"); break;
      case "sleep":
        input("manual-a", "Sleep duration (h)"); input("manual-start", "Sleep window start (UTC)", "datetime-local"); input("manual-end", "Sleep window end (UTC)", "datetime-local"); break;
      case "symptom": {
        const label = el("label", "", "Body zone"); label.htmlFor = "manual-zone";
        const zone = el("select"); zone.id = "manual-zone";
        for (const key of bodyZones) { const option = el("option", "", zoneName(key)); option.value = key; zone.append(option); }
        values.append(label, zone); input("manual-a", "Symptom score (0–10)"); break;
      }
      default: {
        const labels: Partial<Record<Indicator, string>> = { heart_rate: "Pulse (bpm)", oxygen_in_blood: "SpO₂ (%)", mood: "Mood score (0–10)", body_temperature: "Temperature (°C)", glucose: "Glucose (mg/dL)", nutrition: "Nutrition (kcal)", space_radiation: "Radiation (µSv/h)", distance_from_earth: "Distance (km)" };
        input("manual-a", labels[select.value as Indicator] ?? "Value");
      }
    }
  }
  function resetConfirmation(): void {
    confirmedDraft = null; result.replaceChildren();
    setText(save, warnedDemo ? "Save real data" : "Save reading");
  }
  function applyProposal(record: ProposedRecord, source: AssistSource): void {
    if (busy) return;
    if (saved) {
      saved = false; fields.disabled = false; save.disabled = !available;
      result.replaceChildren(); confirmedDraft = null;
      if (!time.value) time.value = new Date().toISOString().slice(0, 19);
      setText(close, "Discard");
    }
    select.value = record.indicator;
    renderFields();
    const mapping = fieldSelectors[record.indicator];
    for (const field of record.fields) {
      if (field.value === null) continue;
      const selector = mapping[field.key];
      if (!selector) continue;
      const target = values.querySelector<HTMLInputElement | HTMLSelectElement>(selector);
      if (!target) continue;
      if (target instanceof HTMLSelectElement) {
        if (typeof field.value === "string" && bodyZones.some(zone => zone === field.value)) {
          target.value = field.value;
        }
      } else if (typeof field.value === "number" && Number.isFinite(field.value)) {
        target.value = String(field.value);
      } else if (typeof field.value === "string") {
        target.value = field.value;
      }
    }
    entrySource = source;
    form.dispatchEvent(new Event("input", { bubbles: true }));
    const firstEmpty = Array.from(
      fields.querySelectorAll<HTMLInputElement | HTMLSelectElement>("input[required], select[required]"),
    ).find(inputNode => !inputNode.value.trim());
    if (firstEmpty) firstEmpty.focus();
    else if (!save.disabled) save.focus();
  }
  const assistant = setupAssistant({ available, onUseProposal: applyProposal });
  select.addEventListener("change", () => { entrySource = "form"; renderFields(); resetConfirmation(); });
  form.addEventListener("input", resetConfirmation);
  renderFields();
  function resetSaved(): void {
    if (!saved) return;
    saved = false; fields.disabled = false; form.reset(); renderFields(); result.replaceChildren();
    confirmedDraft = null; time.value = ""; entrySource = "form"; assistant.reset();
  }
  function openSheet(): void {
    resetSaved(); entrySource = "form"; assistant.reset(); opening += 1; warnedDemo = false;
    element("#manual-demo").hidden = true;
    close.disabled = false; save.disabled = !available; setText(close, "Discard");
    controls.open(close);
  }
  function release(cancelled = false): void {
    if (!holding) return;
    const captured = pointer;
    holding = false; pointer = null; hold.setAttribute("aria-pressed", "false");
    entry.dataset.voiceState = "idle";
    if (captured !== null && hold.hasPointerCapture(captured)) hold.releasePointerCapture(captured);
    onHolding?.(false);
    setText(inlineStatus, cancelled ? "Preview stopped · No audio captured" : "Voice unavailable · Use Type");
  }
  function begin(): boolean {
    if (holding || busy || dialog.open) return false;
    hold.focus({ preventScroll: true }); holding = true; hold.setAttribute("aria-pressed", "true");
    entry.dataset.voiceState = "holding";
    setText(inlineStatus, "Visual preview · No audio captured");
    onHolding?.(true);
    return true;
  }
  hold.addEventListener("pointerdown", event => {
    if (!event.isPrimary || event.button !== 0) return;
    event.preventDefault();
    if (!begin()) return;
    pointer = event.pointerId;
    hold.setPointerCapture(pointer);
  });
  window.addEventListener("pointerup", event => { if (pointer === event.pointerId) release(); }, true);
  window.addEventListener("pointercancel", event => { if (pointer === event.pointerId) release(true); }, true);
  hold.addEventListener("lostpointercapture", () => { if (pointer !== null) release(true); });
  hold.addEventListener("keydown", event => {
    if (event.key === " " || event.key === "Enter") { event.preventDefault(); if (!event.repeat) begin(); }
  });
  window.addEventListener("keydown", event => { if (holding && event.key === "Escape") { event.preventDefault(); release(true); } });
  window.addEventListener("keyup", event => {
    if (holding && pointer === null && (event.key === " " || event.key === "Enter")) { event.preventDefault(); release(); }
  }, true);
  // An AT-generated click has no down/up sequence; announce availability inline.
  hold.addEventListener("click", event => { if (event.detail === 0 && begin()) release(); });
  window.addEventListener("blur", () => release(true));
  window.addEventListener("pagehide", () => release(true));
  document.addEventListener("visibilitychange", () => { if (document.hidden) release(true); });
  close.addEventListener("click", controls.close);
  dialog.addEventListener("close", () => { holding = false; pointer = null; opening += 1; hold.setAttribute("aria-pressed", "false"); onHolding?.(false); assistant.reset(); });
  async function manual(): Promise<void> {
    if (!dialog.open) openSheet();
    const revision = opening;
    release(true); entry.dataset.voiceState = "idle"; setText(inlineStatus, "");
    form.hidden = false;
    setText(element("#voice-title"), "Type a reading");
    if (!time.value) time.value = new Date().toISOString().slice(0, 19);
    setText(element("#manual-profile"), available ? "On-device profile · Checking…" : "Browser mode");
    setText(element("#manual-availability"), available ? "Saved to the on-device profile, separate from the selected MySQL crew member." : "Local service unavailable. Entries are not recorded in the browser.");
    save.disabled = !available; select.focus();
    if (!available) return;
    const [status, identity] = await Promise.allSettled([core.getStatus(), core.getProfile()]);
    if (revision !== opening || !dialog.open || busy || saved) return;
    setText(element("#manual-profile"), "On-device profile · " + (identity.status === "fulfilled" ? identity.value.profile?.astronaut.display_name ?? "No profile available" : "Profile unavailable"));
    if (status.status === "fulfilled") {
      warnedDemo = status.value.is_demo; element("#manual-demo").hidden = !warnedDemo;
      setText(save, confirmedDraft ? "Confirm warnings and save" : warnedDemo ? "Save real data" : "Save reading");
    } else setText(result, "Local service unavailable. Your entry has been kept; try saving again.");
  }
  element("#manual-open").addEventListener("click", () => { void manual(); });
  function date(id: string): string {
    const raw = element<HTMLInputElement>(id).value;
    const parsed = new Date(raw + "Z");
    if (!raw || !Number.isFinite(parsed.getTime())) throw new Error("Enter a valid UTC time.");
    return parsed.toISOString();
  }
  function draft(): LogRecordInput {
    const indicator = select.value as Indicator;
    const number = (id: string): number => {
      const field = element<HTMLInputElement>(id);
      if (!field.value.trim() || !Number.isFinite(field.valueAsNumber)) throw new Error("Enter a numeric reading.");
      return field.valueAsNumber;
    };
    const n = number("#manual-a");
    let value: RecordValue;
    switch (indicator) {
      case "heart_rate": value = { bpm: n }; break;
      case "blood_pressure": value = { systolic: n, diastolic: number("#manual-b") }; break;
      case "body_temperature": value = { celsius: n }; break;
      case "glucose": value = { mg_dl: n }; break;
      case "sleep": value = { hours: n, quality: null, window_start: date("#manual-start"), window_end: date("#manual-end") }; break;
      case "mood": value = { score: n }; break;
      case "oxygen_in_blood": value = { percentage: n }; break;
      case "nutrition": value = { kcal: n }; break;
      case "space_radiation": value = { usv_h: n }; break;
      case "distance_from_earth": value = { km: n }; break;
      case "symptom": {
        const zone = bodyZones.find(key => key === element<HTMLSelectElement>("#manual-zone").value);
        if (!zone) throw new Error("Choose a body zone.");
        value = { body_zone: zone, score: n }; break;
      }
    }
    return { indicator, value, measured_at: date("#manual-time"), source: entrySource, note: note.value.trim() || undefined };
  }
  form.addEventListener("submit", async event => {
    event.preventDefault(); if (busy || saved || !available || !form.reportValidity()) return;
    let entry: LogRecordInput;
    try { entry = draft(); } catch (error) { setText(result, error instanceof Error ? error.message : "Check your entry."); return; }
    const confirmed = confirmedDraft !== null && JSON.stringify(confirmedDraft) === JSON.stringify(entry);
    busy = true; fields.disabled = true; save.disabled = true; close.disabled = true;
    dialog.setAttribute("aria-busy", "true"); setText(save, "Saving…");
    try {
      const status = await core.getStatus();
      element("#manual-demo").hidden = !status.is_demo;
      if (status.is_demo && !warnedDemo) {
        warnedDemo = true;
        setText(result, "Demo data will be removed. Review the notice, then choose Save real data."); return;
      }
      warnedDemo = status.is_demo;
      const response = await core.logRecord({ ...entry, confirm_warnings: confirmed });
      if (response.status === "needs_confirmation") {
        confirmedDraft = entry;
        result.replaceChildren(el("p", "", "Review these warnings before saving:"));
        const warnings = el("ul");
        for (const warning of response.warnings) warnings.append(el("li", "", warning.message));
        result.append(warnings); return;
      }
      saved = true; confirmedDraft = null; panel.acceptRecord(response);
      const sync = response.record.sync === "synced" ? "Marked as synchronized. Ground review is not confirmed."
        : response.record.sync === "pending" ? "Synchronization pending. Ground receipt is not confirmed."
        : response.record.sync === "failed" ? "Not synchronized. The reading remains saved locally." : "Saved on this device only.";
      result.replaceChildren(el("p", "", "Reading saved locally"), el("p", "caption", sync), el("p", "caption", "Measured " + utc(response.record.measured_at)));
      element("#manual-demo").hidden = true; setText(close, "Close");
      await panel.refresh(true);
    } catch (error) {
      setText(result, isCoreError(error) ? error.message + " Your entry has been kept." : "Local service unavailable. The save was not confirmed. Your entry has been kept.");
    } finally {
      busy = false; fields.disabled = saved; close.disabled = false; save.disabled = saved || !available;
      dialog.setAttribute("aria-busy", "false");
      setText(save, saved ? "Saved locally" : confirmedDraft ? "Confirm warnings and save" : warnedDemo ? "Save real data" : "Save reading");
      if (saved) close.focus();
    }
  });
}
