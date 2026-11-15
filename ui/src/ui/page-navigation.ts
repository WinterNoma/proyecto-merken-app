import { el, element, setText } from "./dom";

const screens = [
  { id: "core-panel", label: "History" },
  { id: "overview", label: "Daily overview" },
  { id: "recommendations-page", label: "Recommendations" },
] as const;
type ScreenId = (typeof screens)[number]["id"];
type Drag = {
  pointer: number;
  x: number;
  y: number;
  offset: number;
  lastX: number;
  lastTime: number;
  velocity: number;
  horizontal: boolean;
};

export function setupPages(onChange: (page: ScreenId) => void): void {
  const viewport = element("#screens");
  const rail = el("div", "page-rail");
  const label = element("#page-name");
  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
  const pages = screens.map(screen => {
    const page = element("#" + screen.id);
    const dot = element<HTMLButtonElement>(`[data-page][aria-controls="${screen.id}"]`);
    const slot = el("div", "page-slot");
    slot.append(page);
    rail.append(slot);
    page.hidden = false;
    return { ...screen, page, dot, slot };
  });
  const overview = pages.findIndex(page => page.id === "overview");
  const history = pages.findIndex(page => page.id === "core-panel");
  let active = overview;
  let drag: Drag | null = null;
  let frame = 0;
  let suppressClickUntil = 0;

  function position(offset = 0): void {
    const translate = offset ? "calc(" + (-active * 100) + "% + " + offset + "px)" : (-active * 100) + "%";
    rail.style.transform = "translate3d(" + translate + ", 0, 0)";
  }

  function updateNavigation(): void {
    pages.forEach(({ slot, dot }, index) => {
      slot.inert = index !== active;
      if (index === active) {
        slot.removeAttribute("aria-hidden");
        dot.setAttribute("aria-current", "page");
      } else {
        slot.setAttribute("aria-hidden", "true");
        dot.removeAttribute("aria-current");
      }
    });
    setText(label, pages[active].label);
  }

  function endDrag(): void {
    const pointer = drag?.pointer;
    drag = null;
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    viewport.classList.remove("is-page-dragging");
    if (pointer !== undefined && viewport.hasPointerCapture(pointer)) viewport.releasePointerCapture(pointer);
  }

  function show(index: number, focus = false): void {
    const next = Math.max(0, Math.min(pages.length - 1, index));
    const changed = next !== active;
    const moveFocus = focus || (changed && pages[active].page.contains(document.activeElement));
    endDrag();
    active = next;
    position();
    if (changed) {
      updateNavigation();
      onChange(pages[active].id);
    }
    if (moveFocus) pages[active].page.focus({ preventScroll: true });
  }

  position();
  updateNavigation();
  viewport.append(rail);
  viewport.classList.add("pager-ready");
  element(".app-shell").classList.add("has-page-rail");
  pages.forEach(({ dot }, index) => dot.addEventListener("click", () => show(index)));
  element("#sync-open").addEventListener("click", () => show(history, true));
  element("#overview-alerts").addEventListener("click", () => show(history, true));
  element(".wordmark").addEventListener("click", event => {
    event.preventDefault();
    show(overview, true);
  });

  viewport.addEventListener("pointerdown", event => {
    if (drag || !event.isPrimary || event.button !== 0 || rail.getAnimations().some(animation => animation.playState === "running")) return;
    suppressClickUntil = 0;
    const target = event.target as Element;
    if (target.closest("input, select, textarea, summary, a, [contenteditable], .voice-entry, .body-map")) return;
    drag = {
      pointer: event.pointerId, x: event.clientX, y: event.clientY, offset: 0,
      lastX: event.clientX, lastTime: event.timeStamp, velocity: 0, horizontal: false,
    };
  });

  viewport.addEventListener("pointermove", event => {
    const current = drag;
    if (!current || current.pointer !== event.pointerId) return;
    const dx = event.clientX - current.x;
    const dy = event.clientY - current.y;
    if (!current.horizontal) {
      if (Math.hypot(dx, dy) < 8) return;
      if (Math.abs(dx) <= Math.abs(dy) * 1.2) {
        endDrag();
        return;
      }
      current.horizontal = true;
      viewport.setPointerCapture(event.pointerId);
      viewport.classList.add("is-page-dragging");
    }
    event.preventDefault();
    const elapsed = event.timeStamp - current.lastTime;
    if (elapsed > 0) current.velocity = (event.clientX - current.lastX) / elapsed;
    current.lastX = event.clientX;
    current.lastTime = event.timeStamp;
    const neighbor = active - Math.sign(dx);
    current.offset = neighbor >= 0 && neighbor < pages.length
      ? Math.sign(dx) * Math.min(Math.abs(dx), viewport.clientWidth)
      : dx * .12;
    if (!frame && !reducedMotion.matches) {
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (drag === current) position(current.offset);
      });
    }
  });

  function release(event: PointerEvent, canceled: boolean): void {
    const current = drag;
    if (!current || current.pointer !== event.pointerId) return;
    if (!current.horizontal) {
      endDrag();
      return;
    }
    suppressClickUntil = performance.now() + 400;
    const direction = Math.sign(current.offset);
    const threshold = Math.max(36, Math.min(60, viewport.clientWidth * .16));
    const flick = Math.abs(current.offset) > 16 && event.timeStamp - current.lastTime < 100
      && current.velocity * direction > .45;
    const next = !canceled && (Math.abs(current.offset) >= threshold || flick) ? active - direction : active;
    show(next, next !== active && next >= 0 && next < pages.length);
  }

  viewport.addEventListener("pointerup", event => release(event, false));
  viewport.addEventListener("pointercancel", event => release(event, true));
  viewport.addEventListener("lostpointercapture", event => {
    if (event.target === viewport) release(event, true);
  });
  viewport.addEventListener("click", event => {
    if (event.detail === 0 || performance.now() >= suppressClickUntil) return;
    event.preventDefault();
    event.stopImmediatePropagation();
  }, true);
  const cancelDrag = () => { if (drag) show(active); };
  window.addEventListener("blur", cancelDrag);
  window.addEventListener("resize", cancelDrag);
  document.addEventListener("visibilitychange", () => { if (document.hidden) cancelDrag(); });
  reducedMotion.addEventListener("change", cancelDrag);
}
