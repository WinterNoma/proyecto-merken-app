/** Native modal isolation plus an explicit Tab loop for WebView keyboard use. */
export function modal(dialog: HTMLDialogElement, canClose: () => boolean = () => true) {
  let opener: HTMLElement | null = null;
  function close(): void { if (canClose()) dialog.close(); }
  dialog.addEventListener("cancel", event => {
    event.preventDefault();
    close();
  });
  let pointerOutside = false;
  const outside = (event: MouseEvent): boolean => {
    const rect = dialog.getBoundingClientRect();
    return event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom;
  };
  dialog.addEventListener("pointerdown", event => { pointerOutside = event.target === dialog && outside(event); });
  dialog.addEventListener("click", event => {
    if (pointerOutside && event.target === dialog && outside(event)) close();
    pointerOutside = false;
  });
  dialog.addEventListener("keydown", event => {
    if (event.key !== "Tab") return;
    const controls = Array.from(dialog.querySelectorAll<HTMLElement>(
      'button:not(:disabled), select:not(:disabled), textarea:not(:disabled), input:not(:disabled), a[href], [tabindex="0"]',
    )).filter(node => !node.hidden && node.getClientRects().length > 0);
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (!first || !last) { event.preventDefault(); dialog.focus(); return; }
    if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog)) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialog)) {
      event.preventDefault(); first.focus();
    }
  });
  dialog.addEventListener("close", () => { opener?.focus(); });
  return {
    open(initialFocus: HTMLElement): void {
      if (dialog.open) return;
      opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
      initialFocus.focus();
    },
    close,
  };
}
