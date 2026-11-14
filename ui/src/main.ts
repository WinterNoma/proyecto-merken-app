import { invoke } from "@tauri-apps/api/core";

// Datos de telemetría biométrica del astronauta
interface TelemetryState {
  heartRate: number; // BPM
  oxygen: number;    // % SpO2
  glucose: number;   // mg/dL
}

const currentVitals: TelemetryState = {
  heartRate: 72,
  oxygen: 98,
  glucose: 95,
};

window.addEventListener("DOMContentLoaded", async () => {
  const coreStatusEl = document.querySelector("#core-status");
  const heartRateEl = document.querySelector("#val-heart-rate");
  const oxygenEl = document.querySelector("#val-oxygen");
  const glucoseEl = document.querySelector("#val-glucose");
  const meterBarEl = document.querySelector<HTMLElement>(".meter-bar");
  const rangePointEl = document.querySelector<HTMLElement>(".range-point");
  const btnSimulate = document.querySelector("#btn-simulate");
  const btnEmergency = document.querySelector("#btn-emergency");

  // Conexión con el núcleo Rust si corre bajo Tauri
  if (coreStatusEl) {
    try {
      const status = await invoke<string>("core_status");
      coreStatusEl.textContent = `Core: ${status}`;
    } catch {
      // Si estamos en navegador web (Vite dev) sin backend Tauri
      coreStatusEl.textContent = "Core: Simulación Web Activa";
    }
  }

  // Actualizar valores en pantalla
  function renderVitals() {
    if (heartRateEl) heartRateEl.textContent = currentVitals.heartRate.toString();
    if (oxygenEl) oxygenEl.textContent = currentVitals.oxygen.toString();
    if (glucoseEl) glucoseEl.textContent = currentVitals.glucose.toString();

    if (meterBarEl) {
      meterBarEl.style.width = `${Math.min(100, Math.max(0, currentVitals.oxygen))}%`;
    }

    if (rangePointEl) {
      // Normalizar rango glucosa entre 60 y 140 mg/dL a 0% - 100%
      const pct = Math.min(100, Math.max(0, ((currentVitals.glucose - 60) / 80) * 100));
      rangePointEl.style.left = `${pct}%`;
    }
  }

  // Simulación interactiva con el botón
  btnSimulate?.addEventListener("click", () => {
    // Generar pequeñas variaciones fisiológicas
    currentVitals.heartRate = Math.floor(68 + Math.random() * 18);
    currentVitals.oxygen = Math.floor(96 + Math.random() * 4);
    currentVitals.glucose = Math.floor(88 + Math.random() * 16);
    renderVitals();
  });

  // Botón de emergencia médica
  btnEmergency?.addEventListener("click", () => {
    alert("🚨 ALERTA MÉDICA ACTIVADA:\nNotificando al CMO (Oficial Médico a Bordo) y encolando telemetría para enlace con Tierra.");
  });

  // Simulación pasiva de latidos y fluctuación sutil cada 4 segundos
  setInterval(() => {
    const deltaHr = Math.floor(Math.random() * 3) - 1;
    currentVitals.heartRate = Math.max(60, Math.min(105, currentVitals.heartRate + deltaHr));
    if (heartRateEl) heartRateEl.textContent = currentVitals.heartRate.toString();
  }, 4000);

  renderVitals();
});
