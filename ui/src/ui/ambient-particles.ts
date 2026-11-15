/** Decorative Canvas 2D layer. Receives UI activity, never health/audio data. */
export function createAmbientParticles(canvas: HTMLCanvasElement) {
  const context = canvas.getContext("2d", { alpha: true });
  const reduced = matchMedia("(prefers-reduced-motion: reduce)");
  const forcedColors = matchMedia("(forced-colors: active)");
  const frameMs = 1000 / 30;
  const maxParticles = 84;
  const samples = new Float32Array(120);
  const particles = Array.from({ length: maxParticles }, () => ({
    x: Math.random(), y: Math.random(), phase: Math.random() * Math.PI * 2,
    depth: .35 + Math.random() * .65, size: 2 + Math.random() * 4,
  }));
  // Bake the soft edge once; no shadow blur or gradient allocation per particle.
  const sprite = document.createElement("canvas");
  sprite.width = sprite.height = 32;
  const spriteContext = sprite.getContext("2d");
  if (spriteContext) {
    const glow = spriteContext.createRadialGradient(16, 16, 0, 16, 16, 16);
    glow.addColorStop(0, "rgba(225,233,241,.9)");
    glow.addColorStop(.15, "rgba(213,224,235,.65)");
    glow.addColorStop(.45, "rgba(190,207,224,.16)");
    glow.addColorStop(1, "rgba(190,207,224,0)");
    spriteContext.fillStyle = glow;
    spriteContext.fillRect(0, 0, 32, 32);
  }
  let width = 1, height = 1, resolution = 1, count = 0;
  let raf: number | null = null;
  let active = true, enabled = true, suspended = false, destroyed = false;
  let holding = false, energy = 0, elapsed = 0;
  let lastPaint = 0, lastStep = 0, frames = 0, sampleCount = 0;
  let sampleIndex = 0, slowIntervals = 0;
  let pulse: { x: number; y: number; started: number } | null = null;
  let holdPoint = { x: .5, y: .85 };
  const supported = context !== null && spriteContext !== null;
  function mode(): string {
    if (destroyed) return "destroyed";
    if (!supported) return "unavailable";
    if (!enabled) return "disabled";
    if (!active) return "inactive";
    if (document.hidden || suspended) return "paused";
    if (forcedColors.matches) return "forced-colors";
    return reduced.matches ? "reduced" : "running";
  }
  function draw(now: number, dt: number): void {
    if (!context || !spriteContext) return;
    const start = performance.now();
    const moving = !reduced.matches;
    if (moving) {
      energy += ((holding ? 1 : 0) - energy) * (1 - Math.exp(-dt * 5));
      if (!holding && energy < .0001) energy = 0;
      elapsed += dt * (1 + energy * 1.8);
    }
    const impulse = moving && pulse ? Math.max(0, 1 - (now - pulse.started) / 1600) : 0;
    context.clearRect(0, 0, width, height);
    // Broad, low contrast currents behind the glass, without a grid or neon.
    context.lineWidth = .8;
    context.strokeStyle = "#c5d4e3";
    for (let i = 0; i < 3; i++) {
      const sway = Math.sin(elapsed * .13 + i * 1.8) * height * .055;
      const base = height * (.35 + i * .21);
      context.globalAlpha = .055 + energy * .045;
      context.beginPath();
      context.moveTo(-width * .2, base + sway);
      context.bezierCurveTo(width * .2, base - height * .18, width * .65,
        base + height * .16 + sway, width * 1.2, base - height * .07);
      context.stroke();
    }
    for (let i = 0; i < count; i++) {
      const p = particles[i];
      if (moving) {
        const flow = Math.sin(p.y * 5 + elapsed * .12 + p.phase);
        p.x += (5 + flow * 5) * p.depth * dt * (1 + energy * 2.5) / width;
        p.y += Math.cos(p.x * 4 + elapsed * .09 + p.phase) * p.depth * dt * 5 / height;
        const target = holding ? holdPoint : impulse && pulse ? pulse : null;
        if (target) {
          const dx = target.x - p.x, dy = target.y - p.y;
          const distance = Math.hypot(dx * width, dy * height);
          const influence = Math.max(0, 1 - distance / 240) * (holding ? energy : impulse);
          p.x += (dx * .4 - dy * .15) * influence * dt;
          p.y += (dy * .4 + dx * .15) * influence * dt;
        }
        p.x = (p.x + 1) % 1;
        p.y = (p.y + 1) % 1;
      }
      const twinkle = .8 + Math.sin(elapsed * .5 + p.phase) * .2;
      const size = p.size * (1 + energy * .4);
      context.globalAlpha = (.24 + p.depth * .3 + energy * .16) * twinkle;
      context.drawImage(sprite, p.x * width - size / 2, p.y * height - size / 2, size, size);
    }
    if (impulse && pulse) {
      const radius = 65 + (1 - impulse) * 65;
      context.globalAlpha = impulse * .13;
      context.drawImage(sprite, pulse.x * width - radius, pulse.y * height - radius, radius * 2, radius * 2);
    }
    context.globalAlpha = 1;
    if (import.meta.env.DEV) {
      samples[sampleIndex] = performance.now() - start;
      sampleIndex = (sampleIndex + 1) % samples.length;
      sampleCount = Math.min(sampleCount + 1, samples.length);
    }
    frames += 1;
  }
  function frame(now: number): void {
    raf = null;
    if (mode() !== "running") return;
    raf = requestAnimationFrame(frame);
    const gap = now - lastPaint;
    if (gap < frameMs) return;
    const interval = now - lastStep;
    if (interval > frameMs * 2.1) slowIntervals += 1;
    const dt = Math.min(.05, Math.max(0, interval / 1000));
    lastPaint = now - gap % frameMs;
    lastStep = now;
    draw(now, dt);
  }
  function sync(): void {
    if (raf !== null) cancelAnimationFrame(raf);
    raf = null;
    const current = mode();
    if (current !== "running") { energy = 0; pulse = null; }
    canvas.dataset.motion = current;
    canvas.hidden = !["running", "reduced"].includes(current);
    if (current === "running") {
      lastStep = performance.now(); lastPaint = lastStep - frameMs;
      raf = requestAnimationFrame(frame);
    } else if (current === "reduced") {
      energy = 0; pulse = null;
      draw(performance.now(), 0);
    }
  }
  function resize(): void {
    if (destroyed) return;
    const rect = canvas.getBoundingClientRect();
    // A hidden canvas has no box; restore the size when it becomes active.
    if (!rect.width || !rect.height) return;
    width = rect.width; height = rect.height;
    resolution = Math.min(devicePixelRatio || 1, 1.5, Math.sqrt(1_600_000 / (width * height)));
    canvas.width = Math.round(width * resolution);
    canvas.height = Math.round(height * resolution);
    context?.setTransform(resolution, 0, 0, resolution, 0, 0);
    count = Math.min(maxParticles, Math.max(48, Math.round(width * height / 6000)));
    canvas.dataset.particles = String(count);
    if (mode() === "reduced") draw(performance.now(), 0);
  }
  function visibility(): void { if (document.hidden) { holding = false; energy = 0; pulse = null; } sync(); }
  function suspend(): void { suspended = true; holding = false; energy = 0; pulse = null; sync(); }
  function resume(): void { suspended = false; sync(); resize(); }
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  window.addEventListener("resize", resize);
  window.addEventListener("pagehide", suspend);
  window.addEventListener("pageshow", resume);
  document.addEventListener("visibilitychange", visibility);
  reduced.addEventListener("change", sync);
  forcedColors.addEventListener("change", sync);
  resize(); sync();
  return {
    setHolding(value: boolean): void {
      if (destroyed) return;
      holding = value;
      const rect = document.querySelector("#talk-hold")?.getBoundingClientRect();
      if (rect) holdPoint = { x: (rect.x + rect.width / 2) / width, y: (rect.y + rect.height / 2) / height };
    },
    pulseAt(target: Element): void {
      if (destroyed || mode() !== "running") return;
      const rect = target.getBoundingClientRect();
      pulse = { x: (rect.x + rect.width / 2) / width, y: (rect.y + rect.height / 2) / height, started: performance.now() };
    },
    setActive(value: boolean): void { if (!destroyed && active !== value) { active = value; sync(); resize(); } },
    setEnabled(value: boolean): void { if (!destroyed && enabled !== value) { enabled = value; sync(); resize(); } },
    snapshot() {
      const times = Array.from(samples.subarray(0, sampleCount)).sort((a, b) => a - b);
      return { mode: mode(), count, resolution, width, height, frames, targetFps: 30,
        holding, energy, pulseActive: !!pulse && performance.now() - pulse.started < 1600,
        sampleCount, renderMsP50: times[Math.floor(times.length * .5)] ?? 0,
        renderMsP95: times[Math.min(times.length - 1, Math.floor(times.length * .95))] ?? 0,
        slowIntervals, canvasPixels: canvas.width * canvas.height };
    },
    destroy(): void {
      if (destroyed) return;
      destroyed = true; holding = false; pulse = null;
      sync(); observer.disconnect();
      window.removeEventListener("resize", resize);
      window.removeEventListener("pagehide", suspend);
      window.removeEventListener("pageshow", resume);
      document.removeEventListener("visibilitychange", visibility);
      reduced.removeEventListener("change", sync);
      forcedColors.removeEventListener("change", sync);
      context?.clearRect(0, 0, width, height);
      canvas.width = canvas.height = 1;
    },
  };
}
