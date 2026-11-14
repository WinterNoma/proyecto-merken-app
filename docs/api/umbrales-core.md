# Umbrales del núcleo (merken-core)

Versión de reglas: **`rules-2026.11.14`** · Estado: **contrastado con Hydrogen** ([informe](../../reviews/umbrales-hydrogen.md), 14-nov-2026). Las citas las aportó Hydrogen; Sol las revisa en [core-v1](../../reviews/core-v1.md).
Fuente de verdad en código: `crates/core/src/rules.rs`. Los `source_ref` van en inglés porque la app los muestra.
Diseño: [decisión 003](../decisiones/003-nucleo-rust.md). Unidades del catálogo §5 de [`modelo-datos-backend.md`](modelo-datos-backend.md).

`source_kind`: `published` (evidencia publicada, con cita exacta), `approved_protocol` (protocolo aprobado por el equipo) o `demo_heuristic` (sin fuente: solo para la demo).
**Ninguna regla es diagnóstico.** Los umbrales para personas en la Tierra se usan como orientación; no hay rangos específicos de vuelo validados para esta app.

## 1. Plausibilidad (pide confirmar; nunca rechaza)

| `rule_id` | Condición | `source_kind` | Fuente |
|---|---|---|---|
| `plaus.heart_rate` | `bpm < 30` o `> 200` | `demo_heuristic` | — |
| `plaus.blood_pressure` | `systolic < 70` o `> 230`; `diastolic < 35` o `> 140` | `demo_heuristic` | — |
| `plaus.bp_order` | `diastolic ≥ systolic` | `demo_heuristic` | — |
| `plaus.body_temperature` | `celsius < 34.0` o `> 41.5` | `demo_heuristic` | — |
| `plaus.glucose` | `mg_dl < 40` o `> 450` | `demo_heuristic` | — |
| `plaus.sleep` | `hours > 14` | `demo_heuristic` | — |
| `plaus.future_timestamp` | `measured_at > now + 10 min` | `approved_protocol` | Contrato §6 |
| `plaus.old_measurement` | `measured_at < now − 7 d` | `demo_heuristic` | — |

## 2. Evaluación

| Indicador | Regla | Condición | Estado | Escalado mínimo | `source_kind` | Fuente |
|---|---|---|---|---|---|---|
| heart_rate | `heart_rate.alarm_high` | `bpm ≥ 150` | alarm | urgent | published | AHA 2020 CPR/ECC, Part 3, *Adult Tachycardia With a Pulse Algorithm* (Circulation 2020;142:S366) |
| heart_rate | `heart_rate.alarm_low` | `bpm ≤ 40` | alarm | urgent | published | RCP NEWS2 (2017), Chart 1 |
| heart_rate | `heart_rate.attention_high` | `bpm > 100` | change ↑ | self_care | published | AHA: taquicardia en reposo > 100 lpm |
| heart_rate | `heart_rate.attention_low` | `bpm < 50` | change ↓ | self_care | published | 2018 ACC/AHA/HRS Bradycardia Guideline §3 (Circulation 2019;140:e382) |
| blood_pressure | `blood_pressure.alarm_high` | `systolic > 180` o `diastolic > 120` | alarm | urgent | published | 2025 AHA/ACC High Blood Pressure Guideline, *severe hypertension* (Circulation 2025;152:e114) |
| blood_pressure | `blood_pressure.alarm_low` | `systolic < 80` | alarm | urgent | demo_heuristic | — |
| blood_pressure | `blood_pressure.attention_high` | `systolic ≥ 140` o `diastolic ≥ 90` | change ↑ | self_care | published | 2017 ACC/AHA estadio 2; NASA OCHMO-STD-100.1A |
| blood_pressure | `blood_pressure.attention_low` | `systolic < 90` o `diastolic < 60` | change ↓ | self_care | published | NHLBI: hipotensión < 90/60 |
| body_temperature | `body_temperature.alarm_high` | `celsius > 40.0` | alarm | urgent | published | Bouchama & Knochel, *Heat Stroke*, NEJM 2002;346:1978 |
| body_temperature | `body_temperature.alarm_low` | `celsius < 35.0` | alarm | urgent | published | Hipotermia < 35 °C; NASA-STD-3001 Vol. 1 Rev. C, Appendix C |
| body_temperature | `body_temperature.attention_high` | `celsius ≥ 38.1` (valor tal cual se anota, precisión 0,1 °C) | change ↑ | self_care | published | RCP NEWS2 (2017), Chart 1: 38.1–39.0 °C puntúa 1 (corregido tras Sol) |
| body_temperature | `body_temperature.attention_low` | `celsius ≤ 36.0` | change ↓ | self_care | published | RCP NEWS2 (2017), Chart 1 |
| glucose | `glucose.alarm_low` | `mg_dl < 54` | alarm | urgent | published | ADA Standards of Care 2026, §6, Tabla 6.2 (nivel 2) |
| glucose | `glucose.alarm_high` | `mg_dl > 250` | alarm | urgent | published | ADA Standards of Care 2026, §6, Tabla 6.2 (hiperglucemia nivel 2) |
| glucose | `glucose.attention_low` | `mg_dl < 70` | change ↓ | self_care | published | ADA 2026, §6, Tabla 6.2 (nivel 1) |
| glucose | `glucose.attention_high` | `mg_dl > 180` | change ↑ | self_care | published | ADA 2026, §6: rango objetivo 70–180 |
| sleep | `sleep.attention_low` | `hours < 6` | change ↓ | self_care | demo_heuristic | Adultos ≥ 7 h (AASM/SRS 2015); NASA-STD-3001 Vol. 1 Rev. C [V1 6001] planifica 8,5 h; ISS media 6,09 h (Barger 2014) |
| sleep | `sleep.severe_low` | `hours < 5` | change ↓ | flag_for_ground | demo_heuristic | ISS: < 5 h asociado a síntomas somáticos (Jones et al., *Sleep* 2022) |
| mood | `mood.attention_low` | `score ≤ 3` | change ↓ | self_care | demo_heuristic | — |
| mood | `mood.severe_low` | `score ≤ 1` | change ↓ | flag_for_ground | demo_heuristic | — |
| headache | `headache.attention_high` | `score ≥ 4` | change ↑ | self_care | published | NRS 0–10, 4–6 moderado (Krebs et al., JGIM 2007) |
| headache | `headache.severe_high` | `score ≥ 7` | change ↑ | flag_for_ground | published | NRS 0–10, 7–10 grave (Serlin et al., *Pain* 1995) |
| symptom | `symptom.attention_high` | `score ≥ 4` | change ↑ | self_care | demo_heuristic | — |
| symptom | `symptom.severe_high` | `score ≥ 7` | change ↑ | flag_for_ground | demo_heuristic | — |
| events | `event.alarm` | `need_help`, `syncope`, `visual_field_loss`, `suspected_clot`, **sin caducidad** | alarm | urgent | approved_protocol | RF-10 (requisitos) |

**Alarma con dato caducado:** si la última lectura disparaba una regla de alarma, la serie sigue en `alarm` aunque el dato haya caducado, hasta que haya una lectura nueva (recomendación crítica de Hydrogen).

**Limitaciones conocidas** (Hydrogen, pendientes):
- No se registra si la medida es en reposo o tras ejercicio, ni la postura: `heart_rate ≥ 150`, fiebre o `< 90/60` pueden dar falsas alarmas o esconder intolerancia ortostática.
- La alarma por evento **no caduca** (Sol, B2): queda activa hasta que el contrato tenga una resolución explícita. Hay que acordarla con backend.
- La glucosa no distingue ayunas y posprandial.

### Historial personal (`<ind>.personal_change`, `demo_heuristic`)
Ventana de 14 días, mínimo 3 registros previos, mediana. Diferencias que cuentan como cambio:

| Indicador | `delta` | Dirección que cuenta |
|---|---|---|
| heart_rate | 15 bpm | ↑ ↓ |
| blood_pressure | 20 mmHg sistólica o 10 diastólica | ↑ ↓ |
| body_temperature | 0.8 °C | ↑ ↓ |
| glucose | 30 mg/dL | ↑ ↓ |
| sleep | 1.5 h | ↓ |
| mood | 3 puntos | ↓ |
| headache, symptom | 3 puntos | ↑ |

### Datos caducados (`stale_after`, `demo_heuristic`)
heart_rate, blood_pressure, body_temperature, sleep, mood, headache: 48 h · glucose, symptom: 72 h.
