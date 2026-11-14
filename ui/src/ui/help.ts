import { core, isCoreError, type StatusResponse } from "../core-api";
import type { CorePanel } from "./core-panel";
import { element, el, setText, utc } from "./dom";
import { icon } from "./icons";
import { modal } from "./dialog";

export function setupHelp(available: boolean, panel: CorePanel): void {
  const dialog = element<HTMLDialogElement>("#help-dialog");
  const opener = element<HTMLButtonElement>("#help-open");
  const cancel = element<HTMLButtonElement>("#help-cancel");
  const confirm = element<HTMLButtonElement>("#help-confirm");
  const message = element<HTMLTextAreaElement>("#help-message");
  const result = element("#help-result");
  let busy = false;
  let saved = false;
  let warnedDemo = false;
  let opening = 0;
  dialog.addEventListener("close", () => { opening += 1; });
  const controls = modal(dialog, () => !busy);
  opener.prepend(icon("alarm"));
  function setBusy(value: boolean, label = "Recording…"): void {
    busy = value;
    confirm.disabled = value;
    cancel.disabled = value;
    message.disabled = value || saved;
    dialog.setAttribute("aria-busy", String(value));
    confirm.setAttribute("aria-busy", String(value));
    setText(confirm, value ? label : "Record help request");
  }
  function demoWarning(status: StatusResponse): void {
    element("#help-demo-warning").hidden = !status.is_demo;
    element("#help-demo-badge").hidden = !status.is_demo;
    warnedDemo = status.is_demo;
  }
  function feedback(title: string, detail: string): void {
    result.hidden = false;
    result.replaceChildren(el("h3", "", title), el("p", "", detail));
  }
  opener.addEventListener("click", async () => {
    if (dialog.open) return;
    const thisOpening = ++opening;
    saved = false; warnedDemo = false;
    confirm.hidden = false; cancel.disabled = false; message.disabled = false;
    setText(cancel, "Cancel"); setText(confirm, "Record help request");
    result.hidden = true;
    element("#help-demo-warning").hidden = true;
    element("#help-demo-badge").hidden = true;
    setText(element("#help-profile"), available ? "On-device profile · Checking…" : "Browser mode · No on-device profile");
    setText(element("#help-consequence"), available
      ? "This will save an urgent event on this device. Ground receipt is not confirmed."
      : "Local service unavailable. Requests cannot be recorded in browser mode. Open the Merken app to use on-device records.");
    // In browser mode the request cannot be recorded: keep the explanation, disable the action.
    confirm.disabled = !available;
    controls.open(cancel);
    if (!available) return;
    confirm.disabled = true;
    setText(confirm, "Checking local record…");
    try {
      const [status, profile] = await Promise.allSettled([core.getStatus(), core.getProfile()]);
      if (!dialog.open || thisOpening !== opening) return;
      setText(element("#help-profile"), "On-device profile · " + (profile.status === "fulfilled" ? profile.value.profile?.astronaut.display_name ?? "No profile available" : "Profile unavailable"));
      if (status.status === "fulfilled") demoWarning(status.value);
      else feedback("Local service unavailable", "The local service could not be reached. No request has been recorded. You can try again.");
    } catch {
      if (dialog.open && thisOpening === opening) feedback("Local service unavailable", "The local service could not be reached. No request has been recorded. You can try again.");
    } finally {
      if (dialog.open && thisOpening === opening) {
        confirm.disabled = false;
        setText(confirm, "Record help request");
      }
    }
  });
  cancel.addEventListener("click", controls.close);
  confirm.addEventListener("click", async () => {
    if (busy || saved) return;
    if (!available) {
      feedback("Local service unavailable", "This request was not recorded.");
      confirm.hidden = true; setText(cancel, "Close");
      cancel.focus();
      return;
    }
    setBusy(true);
    let attemptedSave = false;
    try {
      // Check DEMO again immediately before the mutation. If the store changed
      // while this dialog was open, require confirmation with the warning shown.
      const status = await core.getStatus();
      if (status.is_demo && !warnedDemo) {
        demoWarning(status);
        feedback("Demo data will be removed", "Review the demo-removal notice, then choose Record help request to continue.");
        return;
      }
      demoWarning(status);
      attemptedSave = true;
      const response = await core.raiseEvent({ kind: "need_help", message: message.value.trim() || undefined });
      saved = true;
      panel.acceptHelp(response);
      const syncCopy = response.event.sync === "pending"
        ? "Synchronization pending… Ground receipt is not confirmed."
        : response.event.sync === "synced"
        ? "This request is marked as synchronized. Ground review is not confirmed."
        : response.event.sync === "failed"
        ? "Not synchronized. The help request remains saved locally."
        : "Saved on this device only. Ground receipt is not confirmed.";
      feedback("Help request saved locally", syncCopy);
      result.append(el("p", "caption", "Recorded " + utc(response.event.logged_at)));
      if (response.event.is_demo) result.append(el("p", "caption", "DEMO · Simulated event. Not uploaded to ground."));
      element("#help-demo-warning").hidden = true;
      element("#help-demo-badge").hidden = !response.event.is_demo;
      message.value = "";
      confirm.hidden = true; setText(cancel, "Close");
    } catch (error) {
      if (isCoreError(error)) {
        feedback("Help request not saved", error.message + " Your message has been kept.");
        if (error.field === "message") message.focus();
      } else {
        feedback(attemptedSave ? "Help request not saved" : "Local service unavailable",
          attemptedSave ? "The request could not be saved. Try again. Your message has been kept."
            : "The local service could not be reached. This request was not recorded. Your message has been kept.");
      }
    } finally {
      setBusy(false);
      if (!saved) setText(confirm, "Record help request");
      if (saved) cancel.focus();
      // The result of the save is retained even if this subsequent refresh fails.
      await panel.refresh(true);
    }
  });
}
