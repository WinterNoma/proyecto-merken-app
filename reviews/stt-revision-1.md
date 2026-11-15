# Revisión 1 del dictado por voz offline (STT) — Sol + pruebas reales

Rama `claude/dictado-stt-ef20c9`. Revisión estática de Sol (solo lectura) y ejecución real del ejemplo `transcribe` por el orquestador. Los commits solo existen en local: no hay push ni PR.

**Veredicto: sin P0.** Sol encontró dos P1 y cinco P2. Los dos P1 y cuatro de los P2 se corrigieron en una ronda de corrección. El P2 restante se documenta abajo.

## Hallazgos de Sol

| # | Gravedad | Hallazgo | Estado |
|---|---|---|---|
| 1 | P1 | Falta `MODIFY_AUDIO_SETTINGS` en el manifiesto de Android. Wry 0.57.2 pide los dos permisos a la vez y deniega la WebView si falla la petición conjunta. | Corregido en `AndroidManifest.xml` |
| 2 | P1 | Al cambiar de idioma se creaba el reconocedor nuevo antes de liberar el anterior, así que convivían dos modelos en memoria durante la carga (riesgo de matar la app en Android). | Corregido: `SttEngine` guarda `Option<OfflineRecognizer>`, lo libera antes de recrear y se recupera si la recarga falla |
| 3 | P2 | `transcribe_audio` validaba duración y amplitud después de cargar el modelo (podía devolver `model_missing` por un audio inválido). | Corregido: `validate_audio` se llama antes de reservar el motor |
| 4 | P2 | La transcripción no se puede cancelar y el código `cancelled` documentado no era alcanzable. | Documentado en `docs/api/comandos-core.md` y en el TSDoc de `dictate()`; no se añade comando de cancelación |
| 5 | P2 | El límite de 30 s no detenía el micrófono. | Corregido: la captura se cierra sola y `recordDictation()` expone `limitReached` |
| 6 | P2 | `cancel()` y `stop()` conservaban los bloques de PCM. | Corregido: se vacían `chunks` y el contador |
| 7 | P2 | El ejemplo `transcribe` lee el WAV completo antes de recortarlo a 30 s (un WAV de una hora a 48 kHz son unos 691 MB). | **Pendiente (P2)**: `sherpa_onnx::Wave::read` no admite lectura acotada. Es una herramienta de desarrollo, no va en la app |

## Cambios derivados de decisiones del equipo

- Whisper es el modelo por defecto y Canary queda secundario: Canary escribe los números en letras y el core solo entiende cifras.
- Orden de búsqueda de carpetas de modelo bajo la raíz de modelos: `stt-whisper-base`, `stt-whisper-small`, `stt-canary-180m`. Gana la primera que exista.
- Whisper usa `task = "transcribe"` y `tail_paddings = -1`.
- `SttPaths::from_dir` acepta cualquier prefijo `<x>-encoder.int8.onnx`, `<x>-decoder.int8.onnx` y `<x>-tokens.txt`, así que sirven `base-` y `small-`.
- Canary con idioma `auto` usa español (no autodetecta).

## Pruebas reales con el ejemplo `transcribe`

Release, 2 hilos, PC Windows. `load` es la carga del modelo y `trans` la transcripción; audio de 4,7–6 s salvo que se indique. Idioma forzado (`en` o `es`).

| Audio | Whisper base | Whisper small | Canary 180m |
|---|---|---|---|
| en-1-zira (pulso y temperatura) | "My hard rate is 72 and my temperature is 36.5." (0,52 s) | "My heart rate is 72 and my temperature is 36.5." (1,78 s) | "...seventy two ... thirty six point five." (0,51 s) |
| en-2-david (tensión y oxígeno) | "Blood pressure 120/80, oxygen 98%." (0,44 s) | "Blood pressure 120/80, Oxygen 98%" (1,49 s) | "...one hundred and twenty over eighty. Oxygen ninety eight percent." (0,70 s) |
| en-4-david (pulso y peso) | "Pulse 65. Weight 71kg. No nausea." (0,51 s) | "Pulse 65. Weight 71 kilos. No nausea." (1,84 s) | "Pull sixty five. Weight seventy one kilos. No nausea." (0,53 s) |
| es-kroko-0 (español, 5,3 s) | "No preguntes si puedes hacer tu país por ti, ¿degunta que puedes hacer que un corto país?" (0,80 s) | "No preguntes que puede hacer tu país por ti. Te pregunta que puedes hacer que importe tu país." (2,29 s) | "No preguntes qué puede hacer tu país por ti. Pregunta qué puedes hacerte por tu país." (0,60 s) |

Carga del modelo: base 0,6–1,0 s, small 2,0–2,9 s, Canary 1,3–2,0 s.

Otros WAV de prueba de los paquetes (idioma `auto`): Whisper base transcribe `0.wav` (6,6 s) en 0,87 s y `1.wav` (16,7 s) en 2,10 s con puntuación y mayúsculas correctas. Whisper small tarda 2,4 s y 6,2 s. Canary con `auto` fuerza español, así que traduce el audio en inglés a español.

Conclusiones:

- Solo Whisper devuelve cifras ("72", "36.5", "120/80", "98%"), que es lo que necesita el intérprete. Canary escribe los números en letras.
- Whisper base falla en español ("¿degunta que puedes hacer que un corto país?"). Small mejora, pero no es perfecto. Canary es el mejor en español, con números en letras.
- Small corrige "heart rate" y "71 kilos", pero tarda unas 3,5 veces más que base.
- Los resultados son de PC. Falta medir en el dispositivo Android.

## Comprobaciones ejecutadas

- `cargo clippy -p merken-voice --all-targets -- -D warnings` sin features, con `stt`, con `tts` y con `tts,stt`: sin avisos.
- `cargo clippy -p merken --all-targets -- -D warnings` sin features, con `voice-stt`, con `voice-tts` y con `voice-tts,voice-stt`: sin avisos tras la última corrección. `cargo test -p merken`: 13 pasan sin feature y 15 con `voice-stt` (también con `voice-tts,voice-stt`).
- `cargo test -p merken-voice --features stt`: 9 pasan, 1 ignorado (necesita el paquete Kokoro).
- `npx tsc --noEmit`: sin errores.

## Pendientes

- P2 7: lectura acotada del WAV en el ejemplo.
- Probar la captura (`AudioWorklet` y el respaldo `ScriptProcessorNode`) y el diálogo de permiso en un Android real: no se ha probado fuera del PC.
- Medir tiempos de transcripción en el dispositivo y decidir allí entre base y small.
- Sin cancelación de la transcripción: una petición nueva recibe `busy` hasta que termina la anterior.
- El dictado nunca se guarda solo: el texto debe pasar por `interpret_text` con `source: "voice"` y el usuario confirma campo por campo. La UI que lo conecta es de otra sesión.
