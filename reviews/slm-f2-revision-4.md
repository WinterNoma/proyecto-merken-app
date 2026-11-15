# Fase 2 del SLM — verificación 4 de Sol (commit e9d152a)

**Veredicto: no acepto la Fase 2 en `e9d152a`; persiste una variante del P0 de pérdida de signos.**

**Comprobación del P0.** La corrección conserva literalmente `- 0,5`, `− 72` y `+ 72`, incluidos tabuladores y combinaciones de espacios. Sin embargo, la detección sigue limitada a tres signos seguidos de dígitos ASCII.

**Hallazgos**

| Prioridad | Evidencia | Pendiente |
|---|---|---|
| **P0 — conservación incompleta del signo** | `pulse – 72` y `pulse —72` dejan `unresolved: ["72"]`. `pulso + setenta` deja `["setenta"]`. El núcleo rechaza estas expresiones completas, pero el residuo omite el signo. Son contraejemplos deducidos estáticamente. | Alinear la conservación con los signos y comienzos numéricos reconocidos por el núcleo; añadir regresiones. |

La discrepancia está en [service_residual.rs:35](C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-f0-core/crates/slm/src/service_residual.rs:35), frente a [lexicon.rs:89](C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-f0-core/crates/core/src/interpret/lexicon.rs:89) y [numbers.rs:490](C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-f0-core/crates/core/src/interpret/numbers.rs:490).

La comparación estática no muestra regresiones nuevas en guiones entre palabras, rangos `36-37`/`36 - 37`, fechas ni `120/80`.

Verificación íntegramente en solo lectura: comprobé HEAD y los archivos contra los objetos Git. La ejecución de procesos está bloqueada; Clippy y los **86/93 tests verdes** son evidencia aportada por el orquestador.