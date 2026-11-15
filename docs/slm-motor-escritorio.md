# Motor SLM de escritorio: Fase 1

La feature opcional `llama` habilita `LlamaEngine` con `llama-cpp-2 = 0.1.161`; está desactivada por defecto. El crate incluye llama.cpp en el commit `d81235049384534c167caea52b85a694f6103d14` (2026-10-05). El v5 se evaluó con `9183a552e5fdcbdb5243f187f3a63204eca7dba9` (2026-11-13), 945 commits más reciente: no se debe asumir que ambos producen los mismos tokens.

Use `ModelStore::verify_and_load` con `ModelSlot` y `LlamaEngine::from_verified` para mantener la exclusión desde la verificación hasta el fin de la carga. El motor carga la base y aplica el adaptador al contexto, sin fusionar pesos; rechaza errores de aplicación. Cada generación crea un contexto nuevo, usa ChatML con pensamiento desactivado, los tokens especiales del vocabulario, contexto 1024, cuatro hilos y greedy. La semilla 3407 no afecta a greedy. Se admiten gramáticas GBNF, cancelación y plazo entre tokens; una decodificación en curso no se interrumpe hasta recuperar el control.

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

Pese a la diferencia de 945 commits de llama.cpp, la salida greedy coincide byte a byte en los 40 casos. La memoria incluye el mapeo del GGUF base (1,06 GB) y del adaptador (33 MB): es el dato a vigilar en Android (Fase 3). Clippy (con y sin `llama`) y los tests pasan. No se ha validado Android ni el comportamiento clínico.

Si hay diferencias frente a las predicciones guardadas, revisar el prompt y sus saltos de línea, el BOS y los tokens especiales, la aplicación y escala de LoRA, y la diferencia de versiones de llama.cpp. La evaluación original recorta espacios al recoger stdout; la columna exacta compara el texto devuelto íntegro, por lo que también debe inspeccionarse la coincidencia normalizada. No modificar salidas ni referencias para forzar una coincidencia.
