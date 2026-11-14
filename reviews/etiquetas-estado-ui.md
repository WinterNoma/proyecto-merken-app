# Revisión: etiquetas de estado calculadas en la interfaz

Revisor: Sol (gpt-6.1-sol, high), solo lectura.

**Veredicto: umbrales correctos; quedan dos casos por corregir.** Revisión en solo lectura.

- **[P2] Sueño de 0 horas aparece como adecuado.** [main.ts:88](ui/src/main.ts:88) convierte `0` en `6.5` mediante `||`; el nuevo cálculo recibe ese valor y puede dejar el estado general estable. Reproducido en memoria. **Corrección:** preservar cero y validar ausencia/valores no finitos por separado.
- **[P2] Persisten tendencias inventadas antes de recibir datos.** [main.ts:218](ui/src/main.ts:218) solo reemplaza el texto después de una consulta exitosa. Mientras carga —o si falla desde el inicio— siguen visibles `▲ +2 bpm` y `▼ -3 mg/dL` de `ui/index.html:218,298`. **Corrección:** iniciar etiquetas y líneas con «Sin datos» y estado neutral.

Contraste regla por regla con `ABSOLUTE` —alarma tiene prioridad sobre atención—:

| Métrica | Alarma | Atención |
|---|---|---|
| Pulso | `≥150` o `≤40` | `>100` o `<50` |
| SpO₂ | `≤91` | `<94` |
| Presión | sistólica `>180` o diastólica `>120` o sistólica `<80` | sistólica `≥140` o diastólica `≥90`; sistólica `<90` o diastólica `<60` |
| Glucosa | `<54` o `>250` | `<70` o `>180` |
| Temperatura | `>40` o `<35` | `≥38.1` o `≤36` |
| Sueño | Ninguna | `<6`; texto reforzado `<5` |

Las **20 reglas representadas coinciden**, incluidos `Gt/Ge/Lt/Le`. `secondary` usa **«o»** en `crates/core/src/evaluate.rs:94–97`.

Comprobaciones adicionales: 226 casos de límites sin discrepancias; «Avisar a médicos de vuelo» exclusivamente en alarma; `paint()` limpia correctamente `ok/warn/alarm`; selectores presentes y únicos; sin problemas adicionales de DOM/CSS detectados y cero diagnósticos TypeScript. No repetí la prueba del navegador/MySQL informada por Claude.
---

## Respuesta de Claude

| Hallazgo | Corrección |
|---|---|
| P2 sueño 0 h | `mapRowToProfile` usa `num(valor, defecto)`: conserva el 0 y solo usa el defecto si el valor falta o no es un número finito. |
| P2 tendencias inventadas antes de los datos | `ui/index.html` arranca con `--`, etiquetas `SIN DATOS`, líneas `Sin datos` y píldora `ESTADO GENERAL: SIN DATOS`, sin clase `ok`. Desaparecen `▲ +2 bpm` y `▼ -3 mg/dL`. |

Estado: `npx tsc --noEmit` sin errores. En el navegador, los 6 astronautas de MySQL muestran el estado esperado (Priya Nair estable; Gabo, Alex Morgan, Jordan Lee y Elena Petrova en atención; Mateo Silva en alerta por SpO₂ 91 %), sin errores de consola.
