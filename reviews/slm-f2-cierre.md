# Fase 2 del SLM — cierre (15-nov-2026, trabajo nocturno autónomo)

**Estado: cerrada por el orquestador, pendiente de una última confirmación de Sol.**

El acuerdo con el usuario era un máximo de dos rondas de corrección por fase, y continuar después solo con P0 claros. La fase lleva cinco revisiones de Sol (`slm-f2-revision-1.md` a `-5.md`). Todos los P0 encontrados se han corregido con pruebas de regresión. A partir de la revisión 3, los P0 eran variantes de una misma clase: el residuo perdía símbolos con contenido junto a un número (signos separados, guiones largos, espacios Unicode, unidades simbólicas). La corrección final (commit siguiente a `571468a`, del orquestador) cierra la clase con dos reglas generales simétricas:

- **Prefijo.** Un fragmento pendiente incorpora cualquier secuencia de símbolos inmediatamente anterior a su primera palabra.
- **Sufijo.** Un fragmento pendiente incorpora cualquier secuencia de símbolos inmediatamente posterior a su última palabra.

En ambos casos:

- los espacios horizontales (incluidos los Unicode) pueden unir símbolos;
- la puntuación de frase y los saltos de línea cortan la secuencia;
- un símbolo pegado a una palabra cubierta pertenece a esa palabra.

Regresiones: `pulse – 72`, `pulse —72`, `pulso + setenta`, `pulso ± 72`, `pulse −⍽72` (NBSP), `pulso 72%`, `temperatura 36℉`, `temperatura 36 ℉.`.

Verificación: clippy `-D warnings` limpio en el workspace y con `llama`/`slm-llama`/`voice-tts`; tests del workspace en verde.

## Pendiente para la integración (P1/P2 de las revisiones)

- **P1.** Granite 350M como intérprete aporta poco: recall 2/40 en la medición directa. La base Qwen3 del v5 sin LoRA llega a 13/40. Hay que decidir el intérprete; mientras tanto, la capa 1 y el formulario cubren el flujo.
- **P2.** `granite_eval` mide el motor, no el aporte efectivo del servicio. Falta una evaluación de `InterpretResponse` tras `filter_layer2`.
- **P2.** La lista de alias compuestos del residuo debe mantenerse sincronizada con el núcleo.
- **P2.** Faltan pruebas negativas de cabeceras GGUF manipuladas y de LoRA sobre Granite. El código ya las rechaza.
- **UI.** Mostrar el texto original, las citas, las dos propuestas, `unresolved` y `uninterpreted`, y confirmar registro a registro. Ignorar eventos de solicitudes abandonadas.
- **Android.** Medir memoria, latencia y ciclo de vida en un móvil real. Coordinar con Whisper (un modelo cada vez: `slm_release`/`voice_release`).
