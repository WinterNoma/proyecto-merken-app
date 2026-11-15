import type { AstronautProfile } from "../types";
import { element, el, numberText, setText, utc } from "./dom";
import { icon } from "./icons";
import { crewStatus, paintBadge } from "./status-copy";
import { modal } from "./dialog";
import type { FeedState } from "./telemetry";

export function createCrewSelector(
  onSelect: (member: AstronautProfile | null) => void,
  // Installed app only: loads the member's history saved on this device into the local core (as DEMO).
  importToCore?: (member: AstronautProfile) => Promise<string>,
) {
  const trigger = element<HTMLButtonElement>("#crew-open");
  const dialog = element<HTMLDialogElement>("#crew-dialog");
  const close = element<HTMLButtonElement>("#crew-close");
  const select = element<HTMLSelectElement>("#crew-select");
  const previous = element<HTMLButtonElement>("#crew-prev");
  const next = element<HTMLButtonElement>("#crew-next");
  const confirm = element<HTMLButtonElement>("#crew-confirm");
  const useInCore = element<HTMLButtonElement>("#crew-core");
  const coreStatus = element("#crew-core-status");
  let importing = false;
  let statusFor: string | null | undefined;
  useInCore.hidden = !importToCore;
  const dots = element("#crew-dots");
  const headerStatus = element("#header-crew-status");
  const previewStatus = element("#crew-preview-status");
  const controls = modal(dialog);
  let crew: AstronautProfile[] = [];
  let selectedId: string | null = null;
  let previewId: string | null = null;
  let listSignature = "";
  trigger.prepend(icon("crew"));
  trigger.append(icon("chevron"));
  close.append(icon("close"));
  previous.append(icon("left"));
  next.append(icon("right"));
  function selected(): AstronautProfile | null { return crew.find(member => member.id === selectedId) ?? null; }
  function renderPreview(): void {
    const member = crew.find(entry => entry.id === previewId);
    confirm.disabled = !member;
    useInCore.disabled = importing || !member;
    // Only reset the message when another member is shown: the feed re-renders every few seconds.
    if (importToCore && !importing && statusFor !== (member?.id ?? null)) {
      statusFor = member?.id ?? null;
      setText(coreStatus, "");
    }
    if (!member) {
      setText(element("#crew-preview-name"), "No crew available");
      setText(element("#crew-preview-id"), "--");
      paintBadge(previewStatus, { tone: "neutral", label: "No data", icon: "no_data" });
      setText(element("#crew-preview-values"), "No readings available.");
      setText(element("#crew-position"), "No crew available");
      return;
    }
    const index = crew.indexOf(member);
    select.value = member.id;
    setText(element("#crew-preview-name"), member.name);
    setText(element("#crew-preview-id"), "Crew member · ID " + member.id);
    paintBadge(previewStatus, crewStatus(member.telemetry));
    setText(element("#crew-preview-values"), "Heart rate " + numberText(member.telemetry.heart_rate) + " bpm · SpO₂ " + numberText(member.telemetry.oxygen_in_blood) + "%");
    setText(element("#crew-position"), (index + 1) + " of " + crew.length);
    setText(confirm, selectedId === member.id ? "Selected · View readings" : "Select crew member");
    dots.querySelectorAll<HTMLButtonElement>("button").forEach(button => {
      button.setAttribute("aria-pressed", String(button.dataset.crewId === member.id));
    });
  }
  function move(offset: number): void {
    if (!crew.length) return;
    const index = crew.findIndex(member => member.id === previewId);
    previewId = crew[(index + offset + crew.length) % crew.length]?.id ?? null;
    renderPreview();
  }
  trigger.addEventListener("click", () => { previewId = selectedId; renderPreview(); controls.open(close); });
  close.addEventListener("click", controls.close);
  previous.addEventListener("click", () => move(-1));
  next.addEventListener("click", () => move(1));
  select.addEventListener("change", () => { previewId = select.value; renderPreview(); });
  dialog.addEventListener("keydown", event => {
    // Keep native select arrow/Home/End behavior.
    if (event.target === select) return;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault(); move(event.key === "ArrowLeft" ? -1 : 1);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      previewId = (event.key === "Home" ? crew[0] : crew[crew.length - 1])?.id ?? null;
      renderPreview();
    }
  });
  useInCore.addEventListener("click", async () => {
    const member = crew.find(entry => entry.id === previewId);
    if (!importToCore || !member || importing) return;
    importing = true; useInCore.disabled = true;
    setText(coreStatus, "Loading " + member.name + "'s saved history into the on-device core…");
    try {
      setText(coreStatus, await importToCore(member));
    } catch (error) {
      setText(coreStatus, typeof error === "object" && error !== null && "message" in error ? String(error.message) : "The readings could not be saved in the core");
    } finally {
      importing = false; useInCore.disabled = false;
    }
  });
  confirm.addEventListener("click", () => {
    selectedId = previewId;
    renderHeader();
    onSelect(selected());
    controls.close();
  });
  function renderHeader(): void {
    const member = selected();
    setText(element("#crew-name"), member?.name ?? "No crew available");
    if (member) paintBadge(headerStatus, crewStatus(member.telemetry));
    else paintBadge(headerStatus, { tone: "neutral", label: "No data", icon: "no_data" });
  }
  return {
    update(state: FeedState): void {
      crew = state.crew;
      if (!crew.some(member => member.id === selectedId)) selectedId = crew[0]?.id ?? null;
      if (!crew.some(member => member.id === previewId)) previewId = selectedId;
      const signature = JSON.stringify(crew.map(member => [member.id, member.name]));
      if (signature !== listSignature) {
        listSignature = signature;
        select.replaceChildren();
        dots.replaceChildren();
        crew.forEach((member, index) => {
          const option = el("option", "", member.name + " · ID " + member.id);
          option.value = member.id; select.append(option);
          const dot = el("button", "crew-dot");
          dot.type = "button";
          dot.dataset.crewId = member.id;
          dot.setAttribute("aria-label", "Show " + member.name + " (" + (index + 1) + " of " + crew.length + ")");
          dot.append(el("span", "dot-mark"));
          dot.addEventListener("click", () => { previewId = member.id; renderPreview(); });
          dots.append(dot);
        });
      }
      const empty = crew.length === 0;
      select.disabled = empty; previous.disabled = crew.length < 2; next.disabled = crew.length < 2;
      if (empty) select.replaceChildren(el("option", "", state.loading ? "Loading crew…" : "No crew available"));
      const availability = state.preview ? "DEMO · Fixed sample readings" : state.loading ? "Loading crew…" : state.live
        ? (empty ? "No crew available" : "Live")
        : state.lastSuccess ? "Last update " + utc(state.lastSuccess, true) : "Connection unavailable · No data";
      setText(element("#crew-availability"), availability);
      renderHeader();
      if (state.loading) setText(element("#crew-name"), "Loading crew…");
      renderPreview();
      onSelect(selected());
    },
  };
}
