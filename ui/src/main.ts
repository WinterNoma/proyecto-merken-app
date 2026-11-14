import type { AstronautProfile } from "./types";
import {
  OVERALL_LABEL,
  bloodPressureStatus,
  glucoseStatus,
  heartRateStatus,
  oxygenStatus,
  sleepStatus,
  temperatureStatus,
  worstLevel,
  type Level,
  type VitalStatus,
} from "./vital-status";

const LEVELS: Level[] = ["ok", "warn", "alarm"];

/** Pone el texto y el color (clase ok / warn / alarm) de un elemento. */
function paint(el: Element | null, text: string, level: Level) {
  if (!el) return;
  el.textContent = text;
  el.classList.remove(...LEVELS);
  el.classList.add(level);
}

// ============================================================================
// ESTRUCTURA DE FILA RETORNADA POR MYSQL
// ============================================================================
interface MysqlAstronautRow {
  id: number;
  astronaut_name: string;
  blood_pressure: string; // ej. "120/80"
  blood_sugar: number;
  body_temperature: number;
  distance_from_earth: number;
  garmin_last_sync?: string;
  garmin_linked?: { type: string; data: number[] };
  heart_rate: number;
  mood: string;
  nutrition: number;
  oxygen_in_blood: number;
  sleep_hours: number;
  space_radiation: number;
}

// Colores de acento visual para los astronautas
const ASTRONAUT_COLORS = ["#00e5ff", "#10b981", "#f59e0b", "#a855f7", "#ec4899", "#38bdf8"];
const ASTRONAUT_ROLES = [
  "ESPECIALISTA EVA // BIOMONITOR",
  "OFICIAL MÉDICO A BORDO (CMO)",
  "COMANDANTE DE MISIÓN",
  "INGENIERO SOPORTE VITAL",
];

// Helper para parsear la presión arterial tipo "120/80"
function parseBloodPressure(bpStr: string): { systolic: number; diastolic: number } {
  if (!bpStr || typeof bpStr !== "string") {
    return { systolic: 120, diastolic: 80 };
  }
  const parts = bpStr.split("/");
  const sys = parseInt(parts[0] || "120", 10);
  const dia = parseInt(parts[1] || "80", 10);
  return {
    systolic: isNaN(sys) ? 120 : sys,
    diastolic: isNaN(dia) ? 80 : dia,
  };
}

// Helper para convertir fila de MySQL a perfil de nuestra aplicación
// Número de MySQL: conserva el 0 (un sueño de 0 h no es "sin dato") y solo
// usa el valor por defecto si no es un número finito.
function num(value: unknown, fallback: number): number {
  const n = Number(value);
  return value !== null && value !== undefined && value !== "" && Number.isFinite(n) ? n : fallback;
}

function mapRowToProfile(row: MysqlAstronautRow, index: number): AstronautProfile {
  const bp = parseBloodPressure(row.blood_pressure);
  const color = ASTRONAUT_COLORS[index % ASTRONAUT_COLORS.length] || "#00e5ff";
  const role = ASTRONAUT_ROLES[index % ASTRONAUT_ROLES.length] || "TRIPULANTE DE MISIÓN";

  return {
    id: `astronaut-${row.id}`,
    name: row.astronaut_name.toUpperCase(),
    role: role,
    badge: `AST-${row.id.toString().padStart(2, "0")}`,
    color: color,
    telemetry: {
      astronaut_name: row.astronaut_name,
      space_radiation: num(row.space_radiation, 0),
      distance_from_earth: num(row.distance_from_earth, 384400),
      blood_pressure: bp,
      heart_rate: num(row.heart_rate, 75),
      blood_sugar: num(row.blood_sugar, 90),
      body_temperature: num(row.body_temperature, 36.8),
      sleep_hours: num(row.sleep_hours, 6.5),
      mood: row.mood || "NEUTRAL",
      nutrition: num(row.nutrition, 2000),
      oxygen_in_blood: num(row.oxygen_in_blood, 98.0),
    },
  };
}

// Helper para generar el SVG del avatar con el color distintivo de cada astronauta
function generateAvatarSvg(color: string, size = 80): string {
  return `
    <svg class="astronaut-avatar" viewBox="0 0 80 80" width="${size}" height="${size}" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="80" height="80" rx="12" fill="#0c1829" />
      <rect x="1" y="1" width="78" height="78" rx="11" stroke="${color}" stroke-opacity="0.5" stroke-width="1.5" />
      <path d="M14 74C14 62 26 56 40 56C54 56 66 62 66 74V80H14V74Z" fill="#e2e8f0" />
      <path d="M30 56V65M50 56V65" stroke="${color}" stroke-width="1.5" stroke-linecap="round" />
      <rect x="33" y="62" width="14" height="6" rx="2" fill="${color}" fill-opacity="0.25" stroke="${color}" stroke-width="1" />
      <ellipse cx="40" cy="54" rx="20" ry="7" fill="#64748b" />
      <ellipse cx="40" cy="53" rx="18" ry="5" fill="#334155" />
      <circle cx="40" cy="34" r="23" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5" />
      <ellipse cx="40" cy="35" rx="17" ry="14" fill="#0a192f" stroke="${color}" stroke-width="1.8" />
      <ellipse cx="40" cy="35" rx="15" ry="12" fill="${color}" fill-opacity="0.35" />
      <path d="M28 32C30 27 36 26 42 26" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round" stroke-opacity="0.75" />
    </svg>
  `;
}

// Lista en memoria de la tripulación conectada a la base de datos
let crewMembers: AstronautProfile[] = [];
let activeCrewIndex = 0;
let lastSyncTimestamp: string | null = null;

// ============================================================================
// INICIALIZACIÓN DE LA APLICACIÓN
// ============================================================================
window.addEventListener("DOMContentLoaded", async () => {
  // Elementos del Header
  const coreStatusEl = document.querySelector("#core-status");
  const headerBtn = document.querySelector("#astronaut-header-btn");
  const headerAvatarEl = document.querySelector("#header-avatar-svg");
  const headerNameEl = document.querySelector("#header-astronaut-name");
  const headerRoleEl = document.querySelector("#header-astronaut-role");

  // Elementos de Métricas Vitales (Los 11 indicadores de la base de datos)
  const heartRateEl = document.querySelector("#val-heart-rate");
  const oxygenEl = document.querySelector("#val-oxygen");
  const bpSysEl = document.querySelector("#val-bp-sys");
  const bpDiaEl = document.querySelector("#val-bp-dia");
  const glucoseEl = document.querySelector("#val-glucose");
  const tempEl = document.querySelector("#val-temp");
  const sleepEl = document.querySelector("#val-sleep");
  const moodEl = document.querySelector("#val-mood");
  const nutritionEl = document.querySelector("#val-nutrition");
  const radiationEl = document.querySelector("#val-radiation");
  const distanceEl = document.querySelector("#val-distance");
  const footerRadiationEl = document.querySelector("#footer-radiation");
  const footerLinkEl = document.querySelector("#footer-link");

  // Barras visuales e indicadores
  const meterBarEl = document.querySelector<HTMLElement>(".meter-bar");
  const rangePointEl = document.querySelector<HTMLElement>(".range-point");

  // Botones de acción
  const btnEmergency = document.querySelector("#btn-emergency");

  // Elementos del Modal / Carrusel
  const modalEl = document.querySelector<HTMLElement>("#astronaut-modal");
  const modalCloseBtn = document.querySelector("#modal-close-btn");
  const carouselTrack = document.querySelector<HTMLElement>("#carousel-track");
  const carouselPrevBtn = document.querySelector("#carousel-prev-btn");
  const carouselNextBtn = document.querySelector("#carousel-next-btn");
  const carouselDotsEl = document.querySelector("#carousel-indicators");

  // ==========================================================================
  // RENDERIZADO DE LAS 11 MÉTRICAS EN PANTALLA
  // ==========================================================================
  function renderActiveAstronaut() {
    if (crewMembers.length === 0) return;
    const current = crewMembers[activeCrewIndex] || crewMembers[0];
    if (!current) return;
    const t = current.telemetry;

    // Actualizar Header
    if (headerNameEl) headerNameEl.textContent = current.name;
    if (headerRoleEl) headerRoleEl.textContent = current.role;
    if (headerAvatarEl && headerAvatarEl.parentElement) {
      headerAvatarEl.parentElement.innerHTML = `
        ${generateAvatarSvg(current.color, 48)}
        <span class="badge-online"></span>
        <span class="avatar-switch-hint">⇄</span>
      `;
    }

    // 1. Ritmo Cardíaco (HeartRate de MySQL)
    if (heartRateEl) heartRateEl.textContent = t.heart_rate.toString();

    // 2. Oxígeno en Sangre (OxygenInBlood de MySQL)
    if (oxygenEl) oxygenEl.textContent = t.oxygen_in_blood.toFixed(1);
    if (meterBarEl) {
      meterBarEl.style.width = `${Math.min(100, Math.max(0, t.oxygen_in_blood))}%`;
    }

    // 3. Presión Arterial (BloodPressure de MySQL)
    if (bpSysEl) bpSysEl.textContent = t.blood_pressure.systolic.toString();
    if (bpDiaEl) bpDiaEl.textContent = t.blood_pressure.diastolic.toString();

    // 4. Glucosa en Sangre (BloodSugar de MySQL)
    if (glucoseEl) glucoseEl.textContent = Math.round(t.blood_sugar).toString();
    if (rangePointEl) {
      const pct = Math.min(100, Math.max(0, ((t.blood_sugar - 60) / 80) * 100));
      rangePointEl.style.left = `${pct}%`;
    }

    // 5. Temperatura Corporal (BodyTemperature de MySQL)
    if (tempEl) tempEl.textContent = t.body_temperature.toFixed(1);

    // 6. Horas de Sueño (SleepHours de MySQL)
    if (sleepEl) sleepEl.textContent = t.sleep_hours.toFixed(1);

    // Etiquetas, color y línea inferior de cada tarjeta según la lectura
    // (mismos umbrales que el núcleo; ver vital-status.ts).
    const cards: Array<[string, string, VitalStatus]> = [
      ["#badge-heart-rate", ".heart-card", heartRateStatus(t.heart_rate)],
      ["#badge-oxygen", ".oxygen-card", oxygenStatus(t.oxygen_in_blood)],
      ["#badge-bp", ".bp-card", bloodPressureStatus(t.blood_pressure.systolic, t.blood_pressure.diastolic)],
      ["#badge-glucose", ".glucose-card", glucoseStatus(t.blood_sugar)],
      ["#badge-temp", ".temp-card", temperatureStatus(t.body_temperature)],
    ];
    for (const [badgeSel, cardSel, s] of cards) {
      paint(document.querySelector(badgeSel), s.label, s.level);
      paint(document.querySelector(`${cardSel} .vital-trend`), s.detail, s.level);
    }
    const sleep = sleepStatus(t.sleep_hours);
    paint(document.querySelector("#badge-sleep"), sleep.detail, sleep.level);
    const overall = worstLevel([...cards.map(([, , s]) => s.level), sleep.level]);
    paint(document.querySelector("#scan-status-pill"), OVERALL_LABEL[overall], overall);

    // 7. Estado de Ánimo (Mood de MySQL)
    if (moodEl) moodEl.textContent = t.mood;

    // 8. Nutrición (Nutrition de MySQL)
    if (nutritionEl) nutritionEl.textContent = Math.round(t.nutrition).toLocaleString("es-ES");

    // 9. Radiación Espacial (SpaceRadiation de MySQL)
    if (radiationEl) radiationEl.innerHTML = `${t.space_radiation.toFixed(1)} <small>µSv/h</small>`;
    if (footerRadiationEl) footerRadiationEl.textContent = `${t.space_radiation.toFixed(1)} µSv/h`;

    // 10. Distancia de la Tierra (DistanceFromEarth de MySQL)
    if (distanceEl) distanceEl.innerHTML = `${Math.round(t.distance_from_earth).toLocaleString("es-ES")} <small>km</small>`;
    if (footerLinkEl) {
      const timeStr = lastSyncTimestamp ? new Date(lastSyncTimestamp).toLocaleTimeString() : "En vivo";
      footerLinkEl.textContent = `Sync: ${timeStr} · MySQL Live`;
    }

    updateCarouselSelectionUI();
  }

  // ==========================================================================
  // RENDERIZADO DEL CARRUSEL DE ASTRONAUTAS
  // ==========================================================================
  function buildCarousel() {
    if (!carouselTrack || !carouselDotsEl) return;

    carouselTrack.innerHTML = "";
    carouselDotsEl.innerHTML = "";

    crewMembers.forEach((member, index) => {
      const slide = document.createElement("div");
      slide.className = `astronaut-slide ${index === activeCrewIndex ? "active" : ""}`;
      slide.setAttribute("data-index", index.toString());

      slide.innerHTML = `
        <div class="slide-avatar-frame" style="color: ${member.color};">
          ${generateAvatarSvg(member.color, 60)}
        </div>
        <div class="slide-meta">
          <div class="slide-name">${member.name}</div>
          <div class="slide-role">${member.role}</div>
        </div>
        <div class="slide-stats">
          <div class="slide-stat-item">
            <span class="slide-stat-lbl">PULSO</span>
            <span class="slide-stat-val">${member.telemetry.heart_rate} bpm</span>
          </div>
          <div class="slide-stat-item">
            <span class="slide-stat-lbl">SpO₂</span>
            <span class="slide-stat-val">${member.telemetry.oxygen_in_blood}%</span>
          </div>
          <div class="slide-stat-item">
            <span class="slide-stat-lbl">Á NIMO</span>
            <span class="slide-stat-val">${member.telemetry.mood}</span>
          </div>
        </div>
        <button class="slide-select-btn" type="button">
          ${index === activeCrewIndex ? "MONITOREANDO" : "SELECCIONAR"}
        </button>
      `;

      slide.addEventListener("click", () => {
        selectAstronaut(index);
      });

      carouselTrack.appendChild(slide);

      const dot = document.createElement("span");
      dot.className = `carousel-dot ${index === activeCrewIndex ? "active" : ""}`;
      dot.addEventListener("click", () => {
        scrollToSlide(index);
      });
      carouselDotsEl.appendChild(dot);
    });
  }

  function updateCarouselSelectionUI() {
    const slides = document.querySelectorAll(".astronaut-slide");
    const dots = document.querySelectorAll(".carousel-dot");

    slides.forEach((slide, idx) => {
      const btn = slide.querySelector(".slide-select-btn");
      if (idx === activeCrewIndex) {
        slide.classList.add("active");
        if (btn) btn.textContent = "MONITOREANDO";
      } else {
        slide.classList.remove("active");
        if (btn) btn.textContent = "SELECCIONAR";
      }
    });

    dots.forEach((dot, idx) => {
      if (idx === activeCrewIndex) dot.classList.add("active");
      else dot.classList.remove("active");
    });
  }

  function selectAstronaut(index: number) {
    activeCrewIndex = index;
    renderActiveAstronaut();
    closeModal();
  }

  function scrollToSlide(index: number) {
    if (!carouselTrack) return;
    const slides = carouselTrack.querySelectorAll<HTMLElement>(".astronaut-slide");
    if (slides[index]) {
      slides[index].scrollIntoView({ behavior: "smooth", inline: "center", block: "nearest" });
    }
  }

  // ==========================================================================
  // CONTROL DEL MODAL
  // ==========================================================================
  function openModal() {
    if (!modalEl) return;
    buildCarousel();
    modalEl.style.display = "flex";
    setTimeout(() => {
      scrollToSlide(activeCrewIndex);
    }, 50);
  }

  function closeModal() {
    if (!modalEl) return;
    modalEl.style.display = "none";
  }

  headerBtn?.addEventListener("click", (e) => {
    e.preventDefault();
    openModal();
  });

  modalCloseBtn?.addEventListener("click", closeModal);

  modalEl?.addEventListener("click", (e) => {
    if (e.target === modalEl) closeModal();
  });

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && modalEl?.style.display === "flex") {
      closeModal();
    }
  });

  carouselPrevBtn?.addEventListener("click", () => {
    if (crewMembers.length === 0) return;
    const nextIdx = (activeCrewIndex - 1 + crewMembers.length) % crewMembers.length;
    scrollToSlide(nextIdx);
  });

  carouselNextBtn?.addEventListener("click", () => {
    if (crewMembers.length === 0) return;
    const nextIdx = (activeCrewIndex + 1) % crewMembers.length;
    scrollToSlide(nextIdx);
  });

  // Botón de emergencia médica
  btnEmergency?.addEventListener("click", () => {
    const current = crewMembers[activeCrewIndex];
    alert(
      `🚨 ALERTA MÉDICA ACTIVADA PARA ${current?.name || "ASTRONAUTA"}:\n` +
      `• Pulso actual: ${current?.telemetry.heart_rate} BPM\n` +
      `• SpO₂: ${current?.telemetry.oxygen_in_blood}%\n` +
      `• Presión: ${current?.telemetry.blood_pressure.systolic}/${current?.telemetry.blood_pressure.diastolic} mmHg\n` +
      `• Notificando a Oficial Médico a bordo (CMO) y transmitiendo telemetría a Tierra.`
    );
  });

  // ==========================================================================
  // CONEXIÓN EN TIEMPO REAL CON LA BASE DE DATOS MYSQL (AWS ***REMOVED-HOST***)
  // ==========================================================================
  async function fetchLiveAstronautsFromDb() {
    try {
      const response = await fetch("/api/astronauts");
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const rows: MysqlAstronautRow[] = await response.json();

      if (rows && rows.length > 0) {
        // Mapear los astronautas reales provenientes de MySQL
        const updatedCrew = rows.map((r, i) => mapRowToProfile(r, i));

        // Detectar si cambió la lista de tripulación
        const listChanged = updatedCrew.length !== crewMembers.length ||
          updatedCrew.some((m, idx) => m.id !== crewMembers[idx]?.id);

        crewMembers = updatedCrew;
        if (activeCrewIndex >= crewMembers.length) {
          activeCrewIndex = 0;
        }

        // Guardar última sincronización si viene en la fila
        if (rows[activeCrewIndex]?.garmin_last_sync) {
          lastSyncTimestamp = rows[activeCrewIndex].garmin_last_sync!;
        }

        // Refrescar pantalla con los datos reales de MySQL
        renderActiveAstronaut();

        if (listChanged) {
          buildCarousel();
        }

        // Actualizar etiqueta del núcleo / base de datos
        if (coreStatusEl) {
          coreStatusEl.innerHTML = `<span style="color: #34d399;">● MySQL Live (***REMOVED-HOST***)</span>`;
        }
      }
    } catch (err) {
      console.warn("Error consultando MySQL:", err);
      if (coreStatusEl && crewMembers.length === 0) {
        coreStatusEl.textContent = "Conectando con MySQL Tierra...";
      }
    }
  }

  // Primera consulta inmediata a MySQL
  await fetchLiveAstronautsFromDb();

  // Polling automático en tiempo real cada 2 segundos a MySQL
  setInterval(() => {
    fetchLiveAstronautsFromDb();
  }, 2000);
});
