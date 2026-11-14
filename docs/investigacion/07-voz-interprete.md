# Investigación — Voz e intérprete en el dispositivo

## Resumen y recomendación

- **Elección para el piloto: pulsar para hablar → Whisper base multilingüe Q5_1 → intérprete Rust → Granite 4.0 350M Q4_K_M como respaldo → validación y confirmación.** Sus pesos suman **≈297 MB**; ambos modelos tienen licencias permisivas. La precisión conjunta en Merken todavía debe medirse. [Whisper](https://huggingface.co/ggerganov/whisper.cpp/tree/main), [Granite](https://huggingface.co/ibm-granite/granite-4.0-350m-GGUF).
- Usaría el reconocimiento del sistema como optimización para móviles previamente comprobados; su disponibilidad depende del servicio y de los idiomas instalados. [Android](https://developer.android.com/reference/android/speech/SpeechRecognizer).
- **Moonshine Streaming ES/EN merece competir en la prueba:** ya tiene modelos españoles con licencia MIT; no debe confundirse con su antiguo modelo español sin streaming. [Catálogo v0.1.6](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/docs/models/available-models.md).
- Para comandos cerrados, Vosk con vocabulario restringido es una alternativa razonable; para dictado libre hay que comprobar especialmente acentos y números. [Modelos y evaluaciones](https://alphacephei.com/vosk/models).
- No adoptaría Picovoice como dependencia de una misión desconectada sin un acuerdo que elimine la validación de licencia por Internet. [Documentación del proveedor](https://github.com/Picovoice/pico-cookbook/blob/main/recipes/microcontroller-voice-assistant/mcu/stm32f411/README.md).
- **GBNF garantiza estructura, no extracción correcta:** el respaldo necesita evaluación semántica y probablemente ajuste con ejemplos propios; no está demostrado que “con gramática basta”. [GBNF](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md), [ajuste de FunctionGemma](https://ai.google.dev/gemma/docs/functiongemma/finetuning-with-functiongemma).
- Revisión documental al **14-nov-2026**, sin ejecutar modelos ni modificar archivos. RF-14 exige confirmación; RF-18 es opcional y el v5 existente no extrae registros. [Requisitos locales](../../docs/requisitos-app-movil.md), [revisión v5](../../reviews/slm-space-health-v5.md).

## Referencia: Hex

**Hex es una aplicación de dictado local de escritorio:** mantener una tecla, hablar y soltar para insertar la transcripción. Descarga el modelo inicialmente y después puede transcribir localmente. [Repositorio actual](https://github.com/anomalyco/hex).

Hay dos implementaciones que conviene distinguir:

- **Original, Swift para Mac Apple Silicon:** Whisper mediante **WhisperKit/Core ML** y Parakeet TDT v3 mediante **FluidAudio**. El repositorio anuncia que esta implementación dejó de mantenerse. [Hex original](https://github.com/kitlangton/Hex).
- **Actual, Rust, macOS/Windows/Linux:** versión **3.4.0**, publicada el 11-nov-2026. Usa `transcribe-cpp` 0.4.1 con parches fijados; su catálogo incluye Parakeet y Whisper. No es una aplicación Android. [Versión](https://github.com/anomalyco/hex/releases/tag/v3.4.0), [dependencias](https://github.com/anomalyco/hex/blob/v3.4.0/Cargo.toml), [catálogo](https://github.com/anomalyco/hex/blob/v3.4.0/src/transcription/models.rs).

**Aplicación a Merken — propuesta de diseño:** botón grande, estado visible de grabación, cancelar y revisar antes de confirmar. Añadir modo tocar para iniciar/tocar para terminar para quien no pueda mantener pulsado. Conviene copiar esta interacción; el rendimiento de Core ML en Mac no demuestra rendimiento equivalente en Android.

## Tabla comparativa de reconocimiento de voz

**Convenciones:** MB decimales salvo indicación; pesos sin bibliotecas ni WebView. `NF` = **no encontrado** para esa variante en Android de gama media. “Madurez” es valoración técnica. Los WER publicados usan corpus distintos y **no permiten ordenar directamente los motores**.

| Opción | Tamaño | RAM | Latencia | Español | Streaming | Licencia | Integración Rust/Tauri | Madurez |
|---|---|---|---|---|---|---|---|---|
| [Android on-device](https://developer.android.com/reference/android/speech/SpeechRecognizer) | Sin pesos dentro del APK; paquetes externos: NF | NF | NF comparable | Depende del proveedor/paquete; WER: NF | Parciales según servicio | API Android; proveedor con condiciones propias | Plugin Kotlin | API estable; disponibilidad variable |
| [Whisper tiny/base/small Q5_1](https://huggingface.co/ggerganov/whisper.cpp/tree/main) | **32,2 / 59,7 / 190,1 MB** | Q5 Android: NF; referencia sin cuantizar ≈273/388/852 MB [memoria](https://github.com/ggml-org/whisper.cpp#memory-usage) | NF comparable | Multilingües; no usar `.en`; WER de esta cuantización en ES: NF | No nativo; ventanas con solapamiento | MIT | `whisper-rs` o C ABI; captura nativa | Alta; [ejemplo Android](https://github.com/ggml-org/whisper.cpp/tree/master/examples/whisper.android) |
| [Vosk small ES 0.42 / EN-US 0.15](https://alphacephei.com/vosk/models) | **39 / 40 MB descargados**; instalados: NF | ≈300 MB por modelo pequeño, orientación del autor | NF; API incremental | ES: WER 16,02 % Common Voice; 11,21 % MLS | Sí | Apache 2.0 en esos modelos | [Bindings Rust y Android](https://github.com/alphacep/vosk-api); C ABI | Consolidada; calidad limitada por modelo |
| [Zipformer Kroko ES/EN](https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-es-kroko-2025-08-06) mediante sherpa | ES ≈155,8 MB; [EN ≈71,1 MB](https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-en-kroko-2025-08-06/tree/main), suma ONNX | NF | NF comparable | Modelo español específico; WER móvil: NF | Sí | Runtime Apache 2.0; pesos CC-BY-SA según [ficha Kroko](https://huggingface.co/Banafo/Kroko-ASR); versión exacta: no encontrada | Bindings Rust/Kotlin de sherpa | Runtime consolidado; revisar licencia del paquete |
| [SenseVoiceSmall INT8](https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09/tree/main) / [Paraformer ZH INT8](https://huggingface.co/csukuangfj/sherpa-onnx-paraformer-zh-2024-03-09/tree/main) | ≈237,1 / 227,3 MB | NF | NF comparable | Esos checkpoints **no cubren ES** | SenseVoice: no; Paraformer según variante | SenseVoice: [FunASR Model License](https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE); licencia del paquete Paraformer: NF | sherpa Rust/Kotlin | Útiles para otros idiomas; descartados aquí |
| [Moonshine Streaming Tiny / Small](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/docs/models/available-models.md) | Tiny ES+EN ≈**77,7 MB**; Small ES+EN ≈**264,3 MB**, cálculo sin alineación opcional desde [manifiesto](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/core/moonshine-model-file-metadata.generated.cpp) | NF | ES en gama media: NF | WER publicado Tiny 6,2 % / Small 4,9 %, muestra de 400 clips FLEURS+MLS | Sí, nativo | MIT; antiguo Base ES: Community no comercial | SDK Android o C ABI hacia Rust | Prometedor; versiones/modelos recientes |
| [Parakeet TDT 0.6B v3 INT8](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html) | ≈670,6 MB, suma de [archivos ONNX](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/tree/main) | NF | NF comparable | Sí, entre 25 idiomas; WER Android ES: NF | Esta variante: no; sherpa ofrece streaming simulado | Pesos [CC-BY-4.0](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3); runtime Apache 2.0 | sherpa Rust/Kotlin; APK de ejemplo | Disponible en Android; pesado |
| [Canary 180M Flash INT8](https://k2-fsa.github.io/sherpa/onnx/nemo/canary.html) | ≈207,2 MB, suma de [ONNX y tokens](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8/tree/main) | NF | NF comparable | Sí: ES/EN/DE/FR | No nativo en esta integración | Pesos [CC-BY-4.0](https://huggingface.co/nvidia/canary-180m-flash); runtime Apache 2.0 | sherpa Rust/Kotlin | Alternativa vigente para comparar |
| [Qwen3-ASR 0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) | BF16: 1,88 GB; ONNX INT8 ≈0,98 GB + tokenizador, cálculo desde [listado](https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/export.html) | NF | NF comparable | Sí | Integración sherpa consultada: por segmentos | Apache 2.0 | sherpa; distinto del Qwen textual | Vigente, fuera del objetivo ligero |
| [Cheetah](https://picovoice.ai/docs/cheetah/) / [Leopard](https://picovoice.ai/docs/leopard/) | NF para paquete Android ES+EN | NF | NF comparable | Ambos ofrecen ES/EN; evaluación Merken: NF | Cheetah sí; Leopard por lotes | SDKs con Apache 2.0; uso sujeto a AccessKey/plan | SDK Android o C ABI con puente propio | Comercial; dependencia contractual |

**sherpa-onnx es un runtime, no un modelo:** soporta Rust, Kotlin y Android, y ya incluye ejemplos Tauri. Su licencia no sustituye la licencia de los pesos. [Proyecto y matriz](https://github.com/k2-fsa/sherpa-onnx), [ejemplo Tauri con micrófono](https://github.com/k2-fsa/sherpa-onnx/tree/v2.0.1/tauri-examples/non-streaming-speech-recognition-from-microphone).

Moonshine dispone de SDK Android `ai.moonshine:moonshine-voice:0.1.6`. Para sus modelos `.ort` actuales usaría ese SDK o su C ABI; **no está verificada su compatibilidad directa con la integración Moonshine de sherpa**. [Quickstart oficial](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/docs/quickstart.md).

## Voz del sistema Android: pros, contras y cómo integrarla

**Ventaja:** evita incorporar pesos ASR propios. **Límite:** Android no garantiza que todos los fabricantes expongan el servicio local ni los mismos idiomas. Tener dictado en Gboard no demuestra que una aplicación pueda acceder al mismo motor. [API](https://developer.android.com/reference/android/speech/SpeechRecognizer), [servicios y diferencias prácticas](https://github.com/jamsch/expo-speech-recognition).

Flujo recomendado para un plugin propio:

1. **API 31+:** comprobar `isOnDeviceRecognitionAvailable()` y crear exclusivamente `createOnDeviceSpeechRecognizer()`. Si falta, presentar alternativa local empaquetada o formulario. [Android](https://developer.android.com/reference/android/speech/SpeechRecognizer).
2. **API 33+:** consultar `checkRecognitionSupport()` para el idioma solicitado; distinguir instalado, pendiente y descargable. Preparar ES/EN antes de quedar sin red mediante `triggerModelDownload()`. [RecognitionSupport](https://developer.android.com/reference/android/speech/RecognitionSupport).
3. Kotlin solicita `RECORD_AUDIO`, recibe inicio/parciales/final/error y expone comandos/eventos al plugin Tauri; Rust interpreta el texto final. Tauri permite clases `Plugin` con `@TauriPlugin` y comandos nativos. [Plugins móviles](https://v2.tauri.app/develop/plugins/develop-mobile/).
4. Crear/controlar `SpeechRecognizer` desde el hilo principal y llamar a `destroy()` al terminar. No está pensado para reconocimiento continuo permanente. [Contrato Android](https://developer.android.com/reference/android/speech/SpeechRecognizer).

**`EXTRA_PREFER_OFFLINE` es insuficiente:** la documentación permite que el proveedor lo ignore. Tampoco basta con que la app carezca de permiso de Internet: el reconocimiento puede ejecutarse en otro servicio. La decisión debe exigir el servicio on-device, sin retorno automático al reconocedor convencional. [RecognizerIntent](https://developer.android.com/reference/android/speech/RecognizerIntent).

**Móviles e idioma:** no encontré una matriz oficial completa de modelos comerciales que garantice SpeechRecognizer local en español. Hay una demostración del autor en un RedMagic, dentro de Tauri y en modo avión, pero en inglés; es evidencia de ese montaje, **no verificado** independientemente ni extensible a ES. [Prueba publicada](https://github.com/hherb/primer/blob/main/docs/superpowers/specs/2026-06-18-android-native-voice-poc-design.md). También existen reportes de OEM Android 15 con `isOnDeviceRecognitionAvailable() == false`. [Issue](https://github.com/openclaw/openclaw/issues/157149).

**Pitidos y cortes:** el proveedor puede emitir tonos y finalizar por silencio. No encontré un interruptor público universal para eliminar los pitidos; un proyecto documenta un recurso con `EXTRA_AUDIO_SOURCE` en Android 13+, que requiere probar compatibilidad. Los extras de duración pueden ser ignorados. [Implementación y explicación](https://github.com/jamsch/expo-speech-recognition#muting-the-beep-sound-on-android), [extras oficiales](https://developer.android.com/reference/android/speech/RecognizerIntent).

**Plugins existentes:** `tauri-plugin-stt` es MIT, pero en Android delega en `SpeechRecognizer`; su Whisper empaquetado corresponde a escritorio. `tauri-plugin-device-ai-apis` reconoce posibles dependencias de servicios remotos. Ninguno debe aceptarse como garantía offline sin revisar su Kotlin. [STT](https://github.com/brenogonzaga/tauri-plugin-stt), [Device AI](https://github.com/hypothesi/tauri-plugin-device-ai-apis).

**Novedad 2026:** ML Kit ofrece reconocimiento GenAI `1.0.0-alpha2`: Basic usa reconocimiento tradicional local; Advanced usa Gemini Nano en dispositivos seleccionados. Incluye español, pero añade requisitos de AICore/dispositivo y sigue en alpha; no lo elegiría como base universal. [Documentación y dispositivos](https://developers.google.com/ml-kit/genai/speech-recognition/android).

## Voz a intención y modelos de audio

- **Rhino:** produce intención y slots directamente dentro de un contexto cerrado, sin exigir una transcripción intermedia a la aplicación. Admite contextos españoles y Android; sería adecuado para frases como “registra pulso setenta y dos”. No equivale a comprender cualquier narración de síntomas. [Rhino](https://picovoice.ai/docs/rhino/), [idiomas y contextos](https://picovoice.ai/docs/model-api/rhino/).
- **Coste/licencia Picovoice:** la FAQ actual ofrece prueba empresarial y no planes personales/no comerciales dedicados; precio contractual exacto: **no encontrado**. Rhino contabiliza usuarios activos; Cheetah/Leopard, audio procesado. El proveedor documenta validación de AccessKey mediante Internet: inferencia local no significa independencia de sus servidores. [FAQ](https://picovoice.ai/docs/faq/general/), [AccessKey](https://github.com/Picovoice/pico-cookbook/blob/main/recipes/microcontroller-voice-assistant/mcu/stm32f411/README.md).
- **Vosk:** `vosk_recognizer_new_grm()` acepta una lista JSON de frases y puede devolver `[unk]`. Restringe reconocimiento; **no entrega slots automáticamente ni implementa una gramática JSON de extracción**. Rust debe mapear las palabras a campos. Incluir rechazo y lenguaje fuera de vocabulario, no solo comandos válidos. [C API](https://github.com/alphacep/vosk-api/blob/master/src/vosk_api.h).
- **Gemma 3n y Gemma 4:** aceptan audio, pero no pertenecen al presupuesto de un extractor diminuto. Gemma 3n E2B carga más de 5B parámetros en ejecución estándar; “E2B” describe parámetros efectivos bajo técnicas específicas. Gemma 4 E2B tiene 5,1B con embeddings y soporta audio. Los descartaría aquí por alcance y recursos. [Gemma 3n](https://ai.google.dev/gemma/docs/gemma-3n), [Gemma 4](https://ai.google.dev/gemma/docs/core/model_card_4).
- **Moonshine Micro:** su modelo de ≈1,3 MiB reconoce letras, dígitos y comandos ingleses aislados; no es dictado continuo ES/EN para los ejemplos de Merken. [Alcance exacto](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/micro/stt/README.md).

## SLM diminuto para extraer (tabla)

Los tamaños siguientes son archivos concretos, **no RAM de ejecución**. Para todos, rendimiento de extracción sanitaria ES/EN en Merken: **no encontrado**.

| Modelo | Bilingüismo y adecuación | Tamaño cuantizado | RAM Android | Gramática y ajuste | Licencia |
|---|---|---|---|---|---|
| [Granite 4.0 350M](https://huggingface.co/ibm-granite/granite-4.0-350m) | ES/EN declarados; candidato inicial para extracción | **237,0 MB Q4_K_M**, [GGUF oficial](https://huggingface.co/ibm-granite/granite-4.0-350m-GGUF) | No encontrado | Evaluar con GBNF; decidir LoRA según errores | Apache 2.0 |
| [LFM2 350M](https://huggingface.co/LiquidAI/LFM2-350M) | ES/EN; el autor recomienda especialización para tareas estrechas | **229,3 MB Q4_K_M**, [GGUF oficial](https://huggingface.co/LiquidAI/LFM2-350M-GGUF) | No encontrado | Candidato de extracción con ajuste; comprobar backend Android | LFM Open License 1.0; condiciones comerciales propias |
| [Gemma 3 270M IT](https://ai.google.dev/gemma/docs/core/model_card_3) | Familia multilingüe; evaluación específica ES de 270M: no encontrada | **253,1 MB Q4_K_M**, [archivo](https://huggingface.co/unsloth/gemma-3-270m-it-GGUF/blob/main/gemma-3-270m-it-Q4_K_M.gguf) | No encontrado para GGUF | No presumir competencia por el esquema; comparar con LoRA ES/EN | Gemma Terms |
| [FunctionGemma 270M](https://ai.google.dev/gemma/docs/functiongemma/model_card) | Especializado en funciones; calidad ES publicada: no encontrada | **253,1 MB Q4_K_M**, [archivo](https://huggingface.co/unsloth/functiongemma-270m-it-GGUF/blob/main/functiongemma-270m-it-Q4_K_M.gguf) | Referencia: **551 MB RSS**, LiteRT INT8 en S25 Ultra; no extrapolable a GGUF/gama media | Google indica ajuste específico; su formato nativo de llamadas no es JSON común | Gemma Terms |
| [Qwen3 0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) | Multilingüe; candidato para frases más variadas | **484,2 MB Q4_K_M**, [archivo](https://huggingface.co/bartowski/Qwen_Qwen3-0.6B-GGUF) | No encontrado comparable | Desactivar thinking; GBNF y evaluación antes de decidir LoRA | Apache 2.0 |
| [SmolLM2 135M / 360M](https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct) | Principalmente inglés; no los elegiría como base bilingüe sin adaptación | **105,5 / 270,6 MB Q4_K_M**, [135M](https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF), [360M](https://huggingface.co/bartowski/SmolLM2-360M-Instruct-GGUF) | No encontrado | Exigir entrenamiento/evaluación ES; gramática insuficiente para añadir comprensión | Apache 2.0 |

**Conclusión:** el intervalo “200–400 MB para 0,3–0,6B” de RF-14 es orientativo: Qwen3-0.6B Q4 lo supera. Compararía primero Granite y Qwen; FunctionGemma es interesante si el compañero entrenará una tarea específica. Google publica una mejora de **58 % a 85 %** tras especializar FunctionGemma en Mobile Actions; eso no prueba extracción médica ni español. [Evaluación](https://ai.google.dev/gemma/docs/functiongemma/model_card).

**Contrato propuesto:** GBNF con claves y enums cerrados, valores ausentes como `null`, fragmento textual que respalda cada campo y lista de partes sin interpretar. Rust valida unidades, negaciones y coherencia. “Desde esta mañana” conserva su ambigüedad hasta confirmación; el modelo no debe inventar una hora. La restricción sintáctica se implementa en el adaptador de inferencia, no reemplaza la validación del dominio. [Capacidades de GBNF](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md).

## Lo que dice la comunidad (con enlaces)

Estos testimonios son **no verificados independientemente**; sirven para diseñar pruebas, no para fijar prestaciones:

- **Android real, resultados contradictorios:** usuarios de Phone Whisper informan funcionamiento con Parakeet 110M en Z Fold 5 y cierres con 0.6B; otro comunica funcionamiento en Pixel 5. No se demuestra que la causa sea memoria. [r/LocalLLaMA, marzo–abril 2026](https://www.reddit.com/r/LocalLLaMA/comments/1s1ov3u/phone_whisper_pushtotalk_dictation_for_android/).
- **Modelos pequeños y otros idiomas:** un desarrollador encuentra Vosk y Whisper pequeño inconsistentes fuera del inglés; posteriormente dice que Vosk fue lo más viable con su presupuesto de cómputo. [r/LocalLLaMA, febrero–junio 2026](https://www.reddit.com/r/LocalLLaMA/comments/1raste0/fast_voice_to_text_looking_for_offline_mobile/).
- **Whisper y silencio:** hay reportes de subtítulos y frases inventadas sobre silencio, música y ruido, también en español. VAD y filtros reducen el problema; no constituyen garantía. [Discusión original](https://github.com/openai/whisper/discussions/679), [Hacker News](https://news.ycombinator.com/item?id=36973400).
- **Rendimiento anecdótico:** un usuario comunica 11 s de audio en 1,5 s con tiny en un Motorola de 12 GB; faltan modelo exacto del móvil y protocolo comparable. Otro comenta que modelos mayores mejoran el polaco, pero resultan lentos. [Issue whisper.cpp](https://github.com/ggml-org/whisper.cpp/issues/2370).
- **Fragilidad del sistema:** se documentaron problemas de permisos en Pixel con Android 16 tras una actualización de seguridad. Es un incidente concreto, no prueba de fallo general actual. [Issue y seguimiento](https://github.com/jamsch/expo-speech-recognition/issues/117).
- **Calor y batería:** usuarios de LLM locales denuncian calentamiento y consumo sostenido. Son modelos y cargas diferentes; no encontré una comparación controlada de consumo ASR ES/EN en gama media que permita asignar cifras a Merken. [Testimonios](https://www.reddit.com/r/LocalLLaMA/comments/1pgrevr/pro_tip_for_local_llm_usage_on_the_phone/).

## Dos combinaciones propuestas: "mínima" (lo más ligero) y "mejor calidad", con peso total estimado

**Presupuesto:** suma de pesos publicados, antes de un eventual adaptador. `B` = aplicación, bibliotecas y recursos adicionales, cuyo tamaño todavía **no está encontrado/medido**. Se añaden los ≈1,1 GB del v5 informados en el encargo; no significa mantener los modelos simultáneamente en RAM.

| Combinación | Componentes | Pesos añadidos | Con v5 existente |
|---|---|---|---|
| **Mínima autónoma** | Whisper **tiny Q5_1** + Rust + **Granite 350M Q4_K_M** | **≈269,1 MB** | **≈1,369 GB + B** |
| **Mejor calidad, candidata** | Whisper **small Q5_1** + Rust + **Qwen3-0.6B Q4_K_M**, sin thinking | **≈674,3 MB** | **≈1,774 GB + B** |

Cálculos a partir de [pesos Whisper](https://huggingface.co/ggerganov/whisper.cpp/tree/main), [Granite](https://huggingface.co/ibm-granite/granite-4.0-350m-GGUF) y [Qwen](https://huggingface.co/bartowski/Qwen_Qwen3-0.6B-GGUF). “Mejor calidad” expresa la hipótesis a probar; no una superioridad demostrada en Merken.

**Ajuste que elegiría inicialmente:** sustituir tiny por **base** cuesta solo **≈27,6 MB adicionales**: queda en **≈296,7 MB**, o **≈1,397 GB + B** con v5. Para un móvil controlado, sustituir ASR empaquetado por el sistema reduce los pesos propios a ≈237 MB, pero añade paquetes externos de tamaño desconocido y dependencia del proveedor. Es la variante más ligera del APK, condicionada a comprobar ese dispositivo. [Archivos](https://huggingface.co/ggerganov/whisper.cpp/tree/main), [disponibilidad Android](https://developer.android.com/reference/android/speech/SpeechRecognizer).

**Gestión propuesta:** ejecutar ASR y extracción secuencialmente; cargar el extractor solo ante fallo del parser y mantener el explicador v5 descargado de memoria durante el registro. Medir el coste de recarga antes de decidir qué conservar residente.

## Riesgos y cómo probarlo (prueba de 1 día: qué medir y en qué móvil)

**Dispositivo propuesto:** Samsung Galaxy A54 de 6/8 GB si está disponible; registrar variante, Android y versión del servicio de voz. Añadir un Pixel como contraste del proveedor si el equipo ya dispone de él. Son dispositivos de prueba propuestos, no certificaciones de compatibilidad.

**Plan de ocho horas:**

1. **Hora 0–1:** instalar/preparar modelos y paquetes ES/EN, fijar versiones y hashes. Reiniciar; comprobar funcionamiento sin red desde arranque frío. Repetir con red disponible y observar tráfico para detectar un retorno remoto oculto.
2. **Horas 1–3:** grabar 120 frases: tres hablantes × veinte frases ES/EN × dos condiciones, tranquila y ventilador. Añadir silencio, solo ventilador y una selección con mascarilla/micrófono de casco. El ventilador es una aproximación de laboratorio; no reproduce automáticamente una cabina espacial.
3. **Horas 3–6:** comparar sistema, Whisper base/small y Moonshine Tiny; incluir Vosk si priorizamos comandos cerrados. Usar exactamente el mismo audio cuando la API admita entrada externa; separar las pruebas que exijan repetir la locución.
4. **Hora 6–7:** probar Granite y Qwen sobre transcripciones idénticas, con parser y GBNF. Así se distingue error acústico de error de interpretación. El entrenamiento LoRA y su validación independiente quedan fuera de esta jornada.
5. **Hora 7–8:** repetir sesiones consecutivas, evaluar cortes/cierres y seleccionar el conjunto por precisión de campos, latencia y recursos. Fijar entonces los límites de RNF-05.

**Mediciones:** WER por idioma/acento; exactitud de intención y de cada slot; números/unidades/negaciones; omisiones; campos inventados; tasa de rechazo y uso del SLM; tiempo desde soltar hasta propuesta p50/p95; carga fría/caliente; RAM RSS/PSS total, incluido WebView y servicio externo; tamaño instalado; energía y estado térmico durante sesiones repetidas.

**Casos imprescindibles:** “seis” frente a “dieciséis”; “setenta y dos” frente a “ciento setenta y dos”; “no me duele”; “dormí cinco horas y media”; múltiples indicadores; correcciones; mezcla ES/EN y voz débil. No hay evidencia encontrada que garantice estos modelos con casco o mascarilla: debe probarse el micrófono real.

**Diseño recomendado:** pulsar para hablar y detener al soltar, con alternativa de pulsación alternada. Aplazar palabra de activación: obligaría a mantener una escucha y probar falsos despertares. VAD puede filtrar silencio, pero debe conservar palabras débiles; no asumir que elimina alucinaciones. [Limitaciones observadas](https://github.com/openai/whisper/discussions/679).

**Privacidad y ayuda:** audio efímero por defecto, sin transcripciones en logs ni copias automáticas de audio; modelos preparados antes de la desconexión. “Necesito ayuda” abre el flujo determinista de ayuda sin esperar al SLM; mantener siempre el botón manual. Ningún registro se guarda sin confirmación, conforme a RF-14.

**Criterios de descarte para la demo:** dependencia de red tras preparación, escrituras sin confirmación, números inventados aceptados silenciosamente o fallos recurrentes de memoria. La ausencia de fallos en esta prueba breve no demuestra fiabilidad operacional.

## Fuentes

- Contexto revisado: [requisitos §5.4, RF-14/RF-18](../../docs/requisitos-app-movil.md) y [revisión del SLM v5](../../reviews/slm-space-health-v5.md).
- Versiones comprobadas: [whisper.cpp 1.9.5, 6-oct-2026](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.5); [whisper-rs 0.16.0](https://docs.rs/whisper-rs/0.16.0/whisper_rs/); [sherpa-onnx 2.0.1, 26-oct-2026](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v2.0.1). El binding Rust puede incorporar una revisión distinta del motor C++; debe fijarse y registrarse.
- Distribuciones comprobadas: [Vosk Android 0.3.75 en Maven](https://repo.maven.apache.org/maven2/com/alphacephei/vosk-android/maven-metadata.xml); [Moonshine 0.1.6, 6-nov-2026](https://github.com/moonshine-ai/moonshine/releases/tag/v0.1.6); [Hex 3.4.0](https://github.com/anomalyco/hex/releases/tag/v3.4.0).
- Licencias particulares: [Gemma Terms](https://ai.google.dev/gemma/terms), [LFM Open License 1.0](https://huggingface.co/LiquidAI/LFM2-350M/blob/main/LICENSE), [Moonshine](https://github.com/moonshine-ai/moonshine/blob/v0.1.6/LICENSE). Las fichas y archivos enlazados en cada tabla permiten identificar el checkpoint y la cuantización exactos.