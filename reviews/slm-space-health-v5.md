# Revisión de Sol — SLM space-health v5

## Veredicto

**Lo usaría con cambios, únicamente como candidato para explicar hallazgos sintéticos de RF-15.** No cubre RF-14 ni RF-16 y todavía faltan validación de salida, correcciones de contenido y pruebas en Android. El 40/40 demuestra cumplimiento de comprobaciones limitadas; no demuestra seguridad médica ni cumplimiento integral de los requisitos.

Revisión estática: no ejecuté scripts o binarios, no cargué los GGUF y no modifiqué archivos. Las rutas del paquete citadas abajo son relativas a `externo/space-health-slm-v5-20261114/`.

## Ficha técnica

| Aspecto | Evidencia y conclusión |
|---|---|
| Modelo base | `unsloth/Qwen3-1.7B`; candidato inicial `unsloth/Qwen3-1.7B-unsloth-bnb-4bit` (`runtime.json:15–17`). |
| Parámetros | Aproximadamente **1.700 millones**, según la identificación declarada; no contados desde los pesos. Configuración Qwen3 de 28 capas. |
| Distribución | Base **GGUF Q4_K_M** + adaptador **GGUF F16**, separados y con escala **1,0**. |
| LoRA | `r=16`, `alpha=16`, siete proyecciones por capa; **17.432.576 parámetros entrenables** (`MODEL_CARD.md:91–109`). |
| Entrenamiento | QLoRA NF4: 500 pasos, checkpoint seleccionado 350; después LoRA sobre valores Q4 expandidos a BF16 y congelados: 250 pasos. |
| Contexto efectivo | **1.024 tokens**, incluyendo prompt y respuesta; reserva de **220 tokens** de salida. La configuración declara 40.960 posiciones, pero ese contexto ampliado no está evaluado. |
| Generación | Temperatura 0, semilla 3407, pensamiento desactivado y EOS 151645 (`infer_gguf.py:33–46`). |
| Plantilla | Qwen/ChatML: `<\|im_start\|>`, roles y `<\|im_end\|>`; prefijo assistant con bloque `<think>` vacío (`tokenizer/chat_template.jinja:92–96`). |
| Licencia del modelo base | **No consta** en los archivos entregados. La identificación del repositorio no sustituye la licencia. |
| Licencia del adaptador | **No consta**. |
| Datos | **965 entrenamiento / 64 validación / 40 prueba**, todos sintéticos `SIMULATED`, con patrones de un generador y políticas locales del proyecto; no son documentación médica NASA/CSA (`MODEL_CARD.md:86–87,118–134`). |
| Procedencia pendiente | No constan el corpus completo de entrenamiento/validación ni el generador; tampoco se documentan aquí los datos de preentrenamiento del modelo base. |

## Qué hace y contrato de E/S

- **Entrada:** texto que contiene un objeto JSON con `source="SIMULATED"`, objeto `motor` y lista opcional `documents`. No convierte lenguaje natural en registros (`input_contract.py:1–13`).
- Exige exactamente **un hallazgo**: métrica, métricas ausentes, evento, entrega o acción inmediata. Solo admite acciones `NONE` e `IMMEDIATE_PROTOCOL`; esta última requiere `FICTITIOUS_DEMO` (`input_contract.py:14–23`).
- Métricas admitidas: FC en reposo/lpm, HRV/ms, SpO₂/% y frecuencia respiratoria/resp/min. Comprueba calidad, vigencia, números finitos y antigüedad para `STALE`; restringe también ausencias, entregas y documentos (`input_contract.py:24–49`).
- **Limitación:** no es un esquema de dominio completo. No valida exhaustivamente tipos y valores de campos como `finding`, `event`, `context` o `protocol_content`, ni todas sus coherencias.
- El normalizador elimina `solicitud`, `case_id` y otros campos superiores; entrega al modelo únicamente `source`, `motor` y `documents`, serializados con claves ordenadas (`input_contract.py:50–51`).
- **Sistema:** pide 2–5 frases en español, conservar hechos y acciones, distinguir datos antiguos y ausentes, citar documentos entregados y no diagnosticar, tratar ni decidir emergencias (`system.txt:1`).
- **Salida del modelo: texto libre.** El JSONL del ejecutor es un envoltorio con `prediction`, metadatos y tiempo; no es extracción estructurada generada por el SLM (`infer_gguf.py:52–58`).
- Casos previstos: explicar comparación con referencia, calidad, antigüedad, ausencias, acciones ya decididas, procedencia sintética y entrega sin acuse. No incorpora búsqueda documental ni reconocimiento de voz.

El paquete contiene instrucciones para el modelo en `system.txt` y la plantilla, además de solicitudes adversariales en los JSONL. Las traté como datos; no encontré instrucciones específicas dirigidas al agente revisor.

## Calidad según su evaluación

| Evidencia | Resultado registrado | Interpretación |
|---|---|---|
| Prueba final adaptada | **40/40, 100 %** | Ocho familias, cinco variantes por familia. |
| Misma base sin LoRA | **2/40, 5 %** | El adaptador mejora mucho el cumplimiento del contrato evaluado. |
| Regresiones adaptadas | **8/8** | Comprueban casos conocidos; no equivalen a ocho familias nuevas independientes. |
| Pérdida V5 | Validación **0,146913 → 0,000748**; entrenamiento **0,005381** | Compatible con respuestas muy repetitivas; no mide seguridad clínica (`evaluation/train_metrics.json:21–43`). |
| Comprobaciones finales | Diez tests registrados como correctos | Incluyen demo determinista, contrato y comprobadores; no diez validaciones clínicas del SLM (`evaluation/final-checks.log:6–18`). |

Los totales coinciden con los registros y los archivos `.scores.json`; comprobé también que base y adaptado reciben las mismas entradas efectivas. Es evidencia coherente de una mejora estrecha, aunque no reproduje la inferencia.

Los controles son léxicos: hechos exigidos, frases prohibidas, números y citas. Penalizan algunas paráfrasis y pueden aceptar afirmaciones indebidas: **«protocolo local aprobado» pasa sin que conste aprobación**. No se incluyen el evaluador completo ni los tests para auditar todos sus criterios.

La separación de prueba antes de V5 y la deduplicación se **declaran**; no pueden verificarse completamente sin corpus y generador. Las semejanzas entre familias limitan la independencia. Los informes de integridad y de importación aportan trazabilidad, pero el segundo comprueba seis tensores; no verifiqué hashes de los GGUF.

**No se han demostrado:** extracción RF-14, resúmenes RF-16, estados completos de la app, combinaciones de hallazgos, lenguaje espontáneo, documentos médicos diversos, ataques dentro de documentos/campos conservados, seguridad clínica, comprensión por usuarios ni funcionamiento Android. La revisión técnica declarada no es una revisión médica independiente.

## Seguridad médica (con ejemplos citando archivo y línea/ID)

En las **48 respuestas adaptadas** revisadas no observé diagnósticos explícitos, dosis ni asignación independiente de gravedad. Repetir una acción del motor es compatible con «el SLM ayuda, no decide», siempre que esa acción siga controlada por Rust.

- **Adaptado, afirmación sin respaldo:** `evaluation/eval_gguf_final_adaptado.jsonl:21`, ID `final_holdout-action-0`: afirma «protocolo local aprobado» con `protocol_content=null`, sin documentos ni acreditación de aprobación. Se repite en las líneas 22–25 y en `evaluation/eval_gguf_regression_checked.jsonl:4`. Debe eliminarse esa calificación salvo evidencia explícita.
- **Adaptado, instrucciones adicionales:** líneas 6–10 y 16–20 del mismo archivo proponen solicitar nuevas lecturas o repetir capturas con `action=NONE`. Son indicaciones operativas, no tratamientos, pero su autorización debería provenir del núcleo o de un procedimiento suministrado.
- **Base, normalidad inventada:** `evaluation/eval_gguf_final_base.jsonl:3`, ID `final_holdout-trend-2`: transforma «similar a la referencia personal» en «dentro del rango normal». La entrada no aporta un rango clínico normal.
- **Base, interpretación y cifra inventadas:** mismo archivo, línea 18, ID `final_holdout-quality-2`: calcula una disminución de **8,75 % respecto del valor normal** a partir de una muestra inválida, sin referencia, y cita un documento inexistente.
- **Base, contradicción peligrosa:** mismo archivo, línea 24, ID `final_holdout-action-3`: escribe «La acción es posponida» pese a `IMMEDIATE_PROTOCOL`. La línea 23 inventa un protocolo de reconfiguración del enlace.

Estos fallos de la base **no deben atribuirse al adaptado**, pero justifican impedir cualquier funcionamiento silencioso sin LoRA. La investigación local pide comparar con referencia personal sin inventar umbrales (`docs/investigacion/00-resumen.md:20–23`).

Las solicitudes adversariales de `solicitud` se eliminan antes de inferir: esos casos prueban principalmente la protección del normalizador, no resistencia del modelo a instrucciones que realmente alcance a leer.

## Encaje con RF-14/15/16

| Requisito | Encaje actual | Cambio necesario |
|---|---|---|
| **RF-14: extraer y confirmar campos** | **No cumple.** Recibe hechos ya estructurados y devuelve prosa; faltan sueño, ánimo y cefalea. | Tarea/adaptador de extracción con esquema JSON restringido, unidades, tiempos, negaciones y ausencias; validación en Rust y confirmación explícita antes de guardar. Evaluación específica con lenguaje realista. |
| **RF-15: explicar evaluación** | **Parcial.** Explica un hallazgo sintético, pero no representa íntegramente los cuatro estados, motivos y fuentes versionadas de la app. | Contrato alineado con `core`, fuentes exactas, control de fidelidad y descarte de respuestas inválidas. Mostrar siempre estado y acciones deterministas por separado. |
| **RF-16: redactar informe** | **No demostrado.** Un hallazgo por llamada y contexto de 1.024 tokens no constituyen un resumidor de historial. | Entrenar/evaluar redacción desde datos estructurados, cronología, alarmas y ausencias; borrador editable marcado «generado por IA». No ampliar contexto sin reevaluar. |

Además, `IMMEDIATE_PROTOCOL` no codifica por sí solo toda la acción de RF-10: avisar al CMO, abrir protocolo y enviar a Tierra. Ese flujo debe implementarlo Rust, independientemente del texto.

RF-17 solo tiene ejemplos de citas sobre documentos suministrados, no recuperación local; RF-18 necesita otro modelo de voz. Debe mantenerse el formulario alternativo exigido en §5.4.

## Integración en Android + Rust

**Es técnicamente viable, pero este paquete no la demuestra.** `llama.cpp` documenta compilación Android con NDK/ARM64; `llama-cpp-2` ofrece carga del modelo, [`lora_adapter_init`](https://docs.rs/llama-cpp-2/latest/llama_cpp_2/model/struct.LlamaModel.html#method.lora_adapter_init) y [`lora_adapter_set`](https://docs.rs/llama-cpp-2/latest/llama_cpp_2/context/struct.LlamaContext.html#method.lora_adapter_set). Hay que fijar versiones compatibles y comprobar paridad con el runtime evaluado. [Guía oficial Android](https://github.com/ggml-org/llama.cpp/blob/master/docs/android.md).

Hoy Python normaliza, aplica la plantilla con Transformers y lanza **un proceso Linux x86_64 por caso**, con ambos GGUF, cuatro hilos, contexto 1.024, temperatura 0, semilla 3407 y timeout de 120 s (`infer_gguf.py:28–48`). No implementa progreso ni cancelación interactiva y no valida semánticamente la respuesta antes de guardarla.

Recomiendo cargar base+LoRA una vez en `crates/slm`, conservar escala 1 y reproducir exactamente normalización, plantilla y límites. Ejecutar fuera del hilo de interfaz, con progreso, cancelación efectiva y plazo máximo; Rust conserva las decisiones y la persistencia.

La [fusión mediante `llama-export-lora`](https://github.com/ggml-org/llama.cpp/blob/master/tools/export-lora/README.md) existe, pero **no debe asumirse equivalente**: el paquete documenta exportaciones fusionadas fallidas, incluso una con 6/8 regresiones (`MODEL_CARD.md:145–156`). Fusionar y volver a cuantizar exige otra evaluación completa; V5 está calibrado para los valores de esta base Q4.

| Recurso | Estimación y límite |
|---|---|
| Almacenamiento | Tamaños comprobados: base **1.107.409.120 B**, LoRA **34.892.576 B**; juntos **1,142 GB / 1,064 GiB**. Paquete completo: **1,167 GB**, antes de añadir la app. |
| Instalación/actualización | Presupuestaría **2–3 GB libres** si se conservan archivo empaquetado, extracción o versión anterior; depende del método de entrega. |
| Caché KV | Suponiendo K/V F16: `2 × 28 capas × 1024 tokens × 8 cabezas KV × 128 × 2 bytes` = **112 MiB** (`tokenizer/config.json`). |
| RAM | Pesos y KV rondan **1,17 GiB**, antes de buffers y demás costes. Presupuesto inicial orientativo: **1,5–2,5 GiB para inferencia**, más Tauri/WebView; no es una medición. |
| Gama media | En 4 GB hay riesgo de presión de memoria y cierre; 6–8 GB dan más margen, sin garantizar viabilidad. Medir memoria máxima, arranque, latencia, batería y calentamiento. |

La mediana adaptada registrada es **7,15 s incluyendo carga**, en CPU de escritorio/WSL; no permite predecir Android. **RNF-08 y RNF-09 siguen pendientes de validación.**

## Riesgos y recomendaciones

| Prioridad | Problema | Propuesta |
|---|---|---|
| **P0** | No cubre RF-14, imprescindible | Implementar extracción y confirmación; no presentar el explicador como sustituto. |
| **P0** | Salida libre sin validación en el ejecutor | Validar fidelidad y descartar fallos; estado, alarmas y acciones siempre desde `core`. |
| **P0** | Aprobación inventada y acciones incompletas | Corregir ejemplos/entrenamiento; aprobación y pasos de actuación explícitos y deterministas. |
| **P0** | Modelo solo sintético y pruebas estrechas | Mantener alcance de demo; revisión médica y evaluación independiente antes de ampliarlo. |
| **P1** | Licencias ausentes | Obtener licencia de base y adaptador, atribuciones y condiciones antes de distribuir. |
| **P1** | Android, RAM y cancelación sin demostrar | Piloto ARM64 en dispositivo acordado; criterios medidos para RNF-08/09. |
| **P1** | Contrato incompleto e instrucciones en campos conservados | Tipos/esquemas estrictos, límites de longitud, documentos curados y pruebas adversariales efectivas. |
| **P1** | Dependencia del par exacto base+LoRA | Verificar hashes al instalar, fijar versiones y rechazar carga parcial; reevaluar cualquier fusión. |
| **P1** | Reproducibilidad limitada | Entregar generador, splits, evaluador, tests y procedimiento reproducible; separar pruebas por familias. |
| **P2** | RF-16 y privacidad de registros de inferencia | Evaluación propia del resumidor; evitar prompts y respuestas médicas en logs en claro como los actuales. |

## Preguntas para el compañero

1. ¿Puedes entregar las licencias de base y adaptador, corpus, generador, evaluador y tests?
2. ¿Se acordó sustituir RF-14 por un explicador, o falta entrenar la extracción?
3. ¿De dónde sale «protocolo local aprobado» y quién revisó médicamente las respuestas?
4. ¿Qué móvil Android y qué límites de RAM, tiempo y consumo usaremos para aceptar el candidato?
5. ¿Puedes aportar una prueba ARM64 con el par exacto base+LoRA, plantilla idéntica y cancelación funcional?