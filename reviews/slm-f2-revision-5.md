# Fase 2 del SLM — verificación 5 de Sol (commit 571468a)

**Veredicto: no acepto la Fase 2 en `571468a`; persisten vías P0 de pérdida de símbolos con contenido.**

**Comprobación.** Revisión exclusivamente en lectura; archivos contrastados con los objetos del commit. Los casos de la revisión 4 quedan corregidos. El ajuste de `dolor-cabeza` es válido: `cabeza` queda cubierta como mención de síntoma; no veo un defecto real ocultado por ese cambio.

Los contraejemplos siguientes son deducciones estáticas. No pude ejecutar procesos; Clippy y los 89/96 tests verdes son evidencia aportada.

**Hallazgos**

| Prioridad | Evidencia | Causa |
|---|---|---|
| **P0 — signo separado por espacio Unicode** | `pulse −[U+00A0]72` deja `["72"]`: pierde `−`. U+00A0 representa un espacio no separable. El núcleo ignora ese espacio y rechaza la expresión con signo. | El recorrido solo atraviesa espacios ASCII y tabuladores; se detiene ante U+00A0 antes de recuperar el signo. [service_residual.rs:59](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-f0-core/crates/slm/src/service_residual.rs:59). |
| **P0 — unidad simbólica al final** | `pulso 72%` deja `["72"]`; `temperatura 36℉` deja `["36"]`. El núcleo rechaza las unidades incompatibles, pero el residuo elimina precisamente la unidad. | El fragmento termina en `word.end`; la nueva regla únicamente amplía su comienzo. [service_residual.rs:446](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-f0-core/crates/slm/src/service_residual.rs:446). |