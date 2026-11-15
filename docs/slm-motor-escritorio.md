# Motor SLM de escritorio: Fase 1

La feature opcional `llama` habilita `LlamaEngine` con `llama-cpp-2 = 0.1.161`; está desactivada por defecto. El crate incluye llama.cpp en el commit `d81235049384534c167caea52b85a694f6103d14` (2026-10-05). El v5 se evaluó con `9183a552e5fdcbdb5243f187f3a63204eca7dba9` (2026-11-13), 945 commits más reciente: no se debe asumir que ambos producen los mismos tokens.

Use `LlamaEngine::load_into` con `ModelStore`, `ModelSlot` y `LlamaEngineConfig` para mantener la exclusión desde la verificación hasta el fin de la carga. El motor carga la base y aplica el adaptador al contexto, sin fusionar pesos; rechaza errores de aplicación. Cada generación crea un contexto nuevo, usa ChatML con pensamiento desactivado, los tokens especiales del vocabulario, contexto 1024, cuatro hilos y greedy. La semilla 3407 no afecta a greedy. Se admiten gramáticas GBNF, cancelación y plazo entre tokens; una decodificación en curso no se interrumpe hasta recuperar el control.

## Windows, sin red

Desde la raíz del worktree, en PowerShell, con CMake, Ninja y LLVM instalados:

```powershell
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
cargo fmt -p merken-slm -- --check
cargo clippy --offline -p merken-slm --all-targets -- -D warnings
cargo test --offline -p merken-slm
cargo clippy --offline -p merken-slm --all-targets --features llama -- -D warnings
cargo test --offline -p merken-slm --features llama
$env:MERKEN_V5_DIR = '<ruta-al-paquete>\space-health-slm-v5-20261114'
cargo run --offline -p merken-slm --release --features llama --example v5_parity
cargo run --offline -p merken-slm --release --features llama --example v5_parity -- --limit 2
```

Las variables se asignan solo a la sesión actual. No se especifica otro `target-dir`. El ejemplo verifica tamaño y SHA-256 según `runtime.json`, lee los 40 casos ya normalizados y mide carga con verificación, latencia por caso, percentiles de rango más cercano y tokens de salida por segundo durante las llamadas. Las comparaciones exactas conservan espacios; las normalizadas reducen secuencias de espacios en blanco a uno. El ejemplo comprueba la inicialización de ambas gramáticas y que cada generación sea JSON válido. No mide memoria pico.

## Resultados (2026-11-15, Windows 11, CPU, 4 hilos, release)

| Medida | Valor |
|---|---|
| Paridad exacta con `prediction` del v5 | **40/40** |
| Paridad normalizada | 40/40 |
| Verificación SHA-256 + carga (base + LoRA) | 2,1–4,5 s |
| Latencia por caso p50 / p95 | 4,8 s / 5,5 s |
| Tokens generados por segundo | 12,2 |
| Memoria pico del proceso (working set, medido desde fuera) | ~2,0 GB (privada ~1,3 GB) |
| Gramática `PROPOSAL_GBNF` | se inicializa con el vocabulario del v5 y genera JSON válido |
| Gramática de selección (`build_selection_grammar`) | se inicializa y genera JSON válido |

Pese a la diferencia de 945 commits de llama.cpp, la salida greedy coincide byte a byte en los 40 casos. Los pesos del GGUF base se mapean; el adaptador se lee y copia a buffers asignados. La medida externa no determina el desglose exacto: es un dato a vigilar en Android (Fase 3). Clippy (con y sin `llama`) y los tests pasan. No se ha validado Android ni el comportamiento clínico.

Si hay diferencias frente a las predicciones guardadas, revisar el prompt y sus saltos de línea, el BOS y los tokens especiales, la aplicación y escala de LoRA, y la diferencia de versiones de llama.cpp. La evaluación original recorta espacios al recoger stdout; la columna exacta compara el texto devuelto íntegro, por lo que también debe inspeccionarse la coincidencia normalizada. No modificar salidas ni referencias para forzar una coincidencia.

## Modelo de amenazas

Los manifiestos de modelo son de confianza: viajan con la aplicación y más adelante estarán firmados. El SHA-256 vincula cada archivo con su manifiesto; por sí solo no autentica un manifiesto. Las gramáticas las genera únicamente Merken. Las comprobaciones de esta ronda son defensa en profundidad ante paquetes defectuosos o corruptos que aun así coincidan con su manifiesto.

Se aceptan dos riesgos residuales: si falla una asignación durante el análisis nativo de una gramática válida, llama.cpp puede imprimir la gramática completa mediante `fprintf`; las gramáticas de Merken no contienen datos de usuario. Además, un escritor externo que ignore el bloqueo del almacén puede sustituir archivos entre la verificación y su reapertura nativa.

## Seguridad

El motor admite únicamente `PROPOSAL_GBNF` exacta o una gramática de selección canónica reconstruida byte a byte a partir de hasta 32 IDs ASCII de 1 a 64 bytes, con caracteres `[A-Za-z0-9_./-]`. Cualquier otra se rechaza con `InvalidGrammar` antes del prevalidador GBNF conservador y del parser nativo. El constructor devuelve una cadena vacía si los IDs no cumplen esos límites; el explicador rechaza ofertas que los excedan.

Antes de cargar un adaptador, la lectura acotada de cabeceras exige base `qwen3` y `qwen3.block_count`. Todos los pares LoRA deben apuntar a una de las siete matrices Qwen3 de atención o FFN de una capa existente; debe haber al menos un par. Se comprueban los tipos de metadatos consumidos por el cargador, la arquitectura coincidente, la alineación, los strings sin NUL y los tipos válidos de arrays sin anidamiento.

Para cada tensor del adaptador se permiten solo F32 y F16, rango entre 1 y 256, formas compatibles con el destino base, offsets alineados y secuenciales sin solapamiento, y datos completos dentro del archivo. La suma de tamaños está limitada a 256 MiB y al tamaño del archivo. Las operaciones de tamaño y posición son comprobadas. La entrada pública mantiene el bloqueo del almacén desde la verificación hasta terminar la carga; esa exclusión cubre solo escritores que respetan el bloqueo. Estas medidas no constituyen validación clínica ni prueba de viabilidad en Android.

## Intérprete Granite y plantillas de chat

El manifiesto de confianza de la aplicación define Granite 4.0 350M Q4_K_M como intérprete de capa 2. El almacén busca el archivo en `<raíz de modelos>/granite-4.0-350m/granite-4.0-350m-Q4_K_M.gguf` y verifica tamaño y SHA-256 antes de cargarlo. No se usa un manifiesto del disco como autoridad. El servicio solo propone: el núcleo valida y la persona confirma cada campo.

El motor lee `general.architecture` de la cabecera GGUF verificada: `qwen3` usa ChatML sin pensamiento y `granite` usa sus delimitadores de roles; otras arquitecturas devuelven `unsupported_architecture`. Los paquetes LoRA siguen limitados a Qwen3.

Desde la raíz del repositorio, con la feature y dependencias disponibles, ejecuta:

```powershell
cargo run -p merken-slm --example granite_eval --features llama --offline -- "C:\Users\Salva\Desktop\git\nasa.spaceapps\externo\granite-4.0-350m\granite-4.0-350m-Q4_K_M.gguf" crates/slm/examples/data/granite_eval.jsonl --dump granite-dump.jsonl
```

La tabla separa JSON válido, validación y citas de la salida cruda, validación final tras el saneado, recall de pares indicador-valor, tokens y latencia. El resumen cuenta también casos sin valores esperados que devolvieron valores. El recall se calcula sobre la propuesta saneada; el núcleo puede descartar valores incluso si el modelo los extrajo. Los casos negativos no aportan al denominador del recall. Fallos individuales se miden sin alterar el código de salida; errores de configuración, lectura o carga sí detienen el ejemplo.

El volcado opcional es JSONL y contiene texto sintético, salida cruda y problemas de `Proposal::check`; `raw_check_issues: null` indica que la salida no pudo deserializarse como `Proposal`. Se exige una ruta nueva para evitar sobrescribir archivos. Las clases `literal` y `words` separan cifras de números escritos; los controles sin número se agrupan en `literal`. Las frases añadidas son candidatas a casos difíciles para capa 1 y deben medirse, pues no se ha verificado su comportamiento.

El núcleo sí acepta ciertos números escritos: por ejemplo, `quote_number("setenta y dos")` devuelve 72. Por ello `words` mantiene sus valores esperados y su recall no se fuerza a cero. `invented` cuenta valores numéricos finales cuya cita literal no respalda el valor según `quote_number`; `no_digit_match` cuenta los que no aparecen en cifras, incluso si están respaldados por palabras. `unresolved_cases` cuenta propuestas con `uninterpreted` no vacío, sin verificar que cubra toda la frase. El volcado puede revelar copia de ejemplos, pero solo al ejecutar el modelo.
