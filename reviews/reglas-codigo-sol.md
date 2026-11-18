# Reglas de código para Pulsys · 18 nov 2026

**Recomendación:** usa `AGENTS.md` como fuente única de las reglas y exige que CI aplique las comprobaciones antes de fusionar. Empieza por el código nuevo o modificado de `crates/api`, `crates/bridge` y `app/`.

**Verificado en solo lectura:** el [Cargo.toml](C:/Users/Salva/Desktop/git/nasa.spaceapps/Cargo.toml) declara `unsafe_code = "forbid"` y los siete crates actuales heredan los lints del workspace. **NO VERIFICADO:** el proyecto Flutter y las tres rutas propuestas están ausentes de este checkout, que contiene Tauri. Tampoco probé los comandos ni confirmé las versiones locales de las CLI. No modifiqué archivos.

## A. Qué es lint y cómo aplicar las tres reglas

Un **lint** es una regla que examina el código sin ejecutar la aplicación. Por ejemplo, Clippy puede señalar un `.unwrap()` que provocaría un pánico si falla la operación. El analizador de Dart puede señalar un parámetro booleano posicional que deja una llamada como `upload(true)` sin contexto. [Clippy](https://doc.rust-lang.org/clippy/) · [Análisis de Dart](https://dart.dev/tools/analysis)

El **linter** ejecuta esas reglas y devuelve diagnósticos con su ubicación, código y gravedad. Al versionar la configuración y hacer fallar CI ante avisos, todos reciben el mismo criterio. El formateador uniforma la presentación y las pruebas comprueban el comportamiento. [Clippy en CI](https://doc.rust-lang.org/clippy/usage.html#elevating-warning-to-errors) · [Dart analyze](https://dart.dev/tools/dart-analyze)

| Regla | Cómo hacerla cumplir |
|---|---|
| Código autoexplicativo | Automatiza convenciones de nombres, longitud, anidamiento y argumentos. Exige revisión del diff para juzgar si los nombres y tipos expresan la intención. |
| Lint obligatorio | Ejecuta el mismo chequeo local y en cada PR. Configura su resultado como requisito de fusión. |
| Menos comentarios y anotaciones | Usa inferencia local cuando el tipo sea evidente. Pide en revisión retirar comentarios que repiten el código y conservar los que explican contratos o decisiones. Contar comentarios no mide si aportan. |

## B. Configuración inicial

**Rust.** Fusiona este bloque con el manifiesto existente. Cada crate que deba aplicarlo declara `[lints]` y `workspace = true`.

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
indexing_slicing = "deny"
todo = "warn"
too_many_lines = "warn"
too_many_arguments = "warn"
excessive_nesting = "warn"
many_single_char_names = "warn"
```

```toml
# clippy.toml
too-many-lines-threshold = 60
too-many-arguments-threshold = 6
excessive-nesting-threshold = 4
```

```toml
# rustfmt.toml
edition = "2024"
style_edition = "2024"
max_width = 100
```

Conserva `clippy::all`, activo por defecto. En Clippy estable, `too_many_arguments` y `excessive_nesting` son `complexity`. `too_many_lines`, `many_single_char_names` y `similar_names` son `pedantic`. Activa los dos primeros de ese grupo y deja `similar_names` fuera al inicio para evitar ruido. `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `todo` y `cognitive_complexity` son `restriction`. Deja `cognitive_complexity` fuera: Clippy recomienda usar longitud y anidamiento como señales más concretas. `use_self` es un ejemplo de `nursery` disponible en estable. Deja desactivados los grupos completos `pedantic`, `restriction` y `nursery`. [Catálogo estable](https://clippy.rust-lang.org/stable/index.html) · [Grupos](https://doc.rust-lang.org/clippy/lints.html) · [Cargo](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table) · [Rustfmt](https://github.com/rust-lang/rustfmt#configuring-rustfmt)

**`missing_docs` sí para una API pública estable, de forma selectiva.** Es un lint de `rustc`. Por ejemplo, usa `#![warn(missing_docs)]` en `crates/api/src/lib.rs` cuando su interfaz sea un contrato para consumidores. Documenta unidades, errores y garantías. Evita activarlo globalmente y deja fuera `clippy::missing_docs_in_private_items`. [Rustc](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#missing-docs) · [Clippy](https://clippy.rust-lang.org/stable/index.html#missing_docs_in_private_items)

**Flutter.** Añade `flutter_lints` a `dev_dependencies` y usa este `app/analysis_options.yaml`. `lints` ofrece `core` y `recommended` para Dart puro. `flutter_lints` amplía `recommended` con reglas de Flutter. `very_good_analysis` incorpora más reglas, incluida documentación pública generalizada, así que lo dejaría fuera de la primera adopción. [lints](https://pub.dev/packages/lints) · [flutter_lints](https://pub.dev/packages/flutter_lints) · [very_good_analysis](https://pub.dev/packages/very_good_analysis) · [Opciones del analizador](https://dart.dev/tools/analysis)

```yaml
include: package:flutter_lints/flutter.yaml
analyzer:
  language:
    strict-casts: true
    strict-inference: true
    strict-raw-types: true
linter:
  rules:
    always_declare_return_types: true
    avoid_positional_boolean_parameters: true
    unawaited_futures: true
    use_build_context_synchronously: true
    public_member_api_docs: false
```

**Puerta local y CI.** Fija las mismas versiones de Rust estable y Flutter en ambos entornos. Estos son los comandos para cuando el alcance esté limpio. En CI exige que pasen antes de fusionar.

```sh
# Desde la raíz Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings

# Desde app/, una vez instalada la versión de Flutter fijada
flutter pub get
dart format --output=none --set-exit-if-changed .
dart analyze --fatal-infos
```

Para corregir en local, usa `cargo fmt` o `dart format`. Con Dart, ejecuta `dart fix --dry-run`, revisa las propuestas y aplica las aceptadas con `dart fix --apply`. Durante la adopción, ejecuta Clippy para los dos paquetes piloto con `-p` y analiza `app/` completo. Si hay deuda, registra los diagnósticos existentes de la rama base y compara por regla, archivo y fragmento, teniendo en cuenta desplazamientos de líneas. Bloquea diagnósticos nuevos en las rutas piloto, incluso si un cambio hace que el aviso aparezca en la firma de una función intacta. Comprueba el formato de los archivos nuevos o modificados. Elimina entradas corregidas de la línea base y mantén los errores de compilación y `unsafe_code` como bloqueantes. Cuando el piloto llegue a cero, aplica los comandos estrictos a ese alcance y amplíalo por paquete. [Clippy](https://doc.rust-lang.org/clippy/usage.html) · [Diagnósticos de Cargo](https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages) · [Formato Dart](https://dart.dev/tools/dart-format) · [Dart fix](https://dart.dev/tools/dart-fix) · [Checks obligatorios](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets#require-status-checks-to-pass-before-merging)

## C. Guía breve de código autoexplicativo

Usa términos del dominio y explicita las unidades. Dale una responsabilidad a cada función. Extrae un bloque cuando puedas ponerle un nombre que explique su propósito. Usa `Option`, `Result`, `enum` o tipos con unidades cuando aclaren el contrato. Comenta el **porqué** de una decisión poco obvia. Para una regla médica, cita fuente, versión y sección. En FFI, documenta propiedad, vida útil y quién libera recursos. Si existe código `unsafe` en un componente donde esté permitido, explicita sus invariantes con `SAFETY`. [Effective Dart](https://dart.dev/effective-dart/design) · [Documentación](https://dart.dev/effective-dart/documentation) · [Rust FFI](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html)

```rust
// Antes
fn get(xs: &[Sample]) -> &Sample {
    xs.first().unwrap()
}

// Después
fn first_sample(samples: &[Sample]) -> Option<&Sample> {
    samples.first()
}
```

```dart
// Antes
upload(true); // Solo por wifi.

// Después
upload(mode: UploadMode.wifiOnly);
```

## D. Una fuente de verdad para los agentes

- **Codex CLI** lee primero `~/.codex` y luego desde la raíz del repo hasta el directorio de trabajo. Por directorio prioriza `AGENTS.override.md`, luego `AGENTS.md` y luego los nombres alternativos configurados. La guía más cercana al directorio de trabajo prevalece. [OpenAI](https://learn.chatgpt.com/docs/agent-configuration/agents-md)
- **Antigravity `agy`** lee `AGENTS.md`, `GEMINI.md` y reglas en `.agents/` desde el directorio de arranque hacia la raíz Git, y agrega las globales de `~/.gemini`. Son acumulativas. **NO VERIFICADO:** cuál prevalece ante contradicciones entre dos reglas del mismo nivel. Usa directamente el `AGENTS.md` raíz. [Reglas de Antigravity](https://antigravity.google/docs/rules) · [Descubrimiento](https://antigravity.google/docs/customization-discovery?tab=cli)
- **Gemini CLI**, si también se usa, concatena `GEMINI.md` global, de proyecto y de contexto específico bajo demanda. Configura `context.fileName` como `AGENTS.md` o usa `@./AGENTS.md` en `GEMINI.md`. [Gemini CLI](https://geminicli.com/docs/cli/gemini-md/)
- **OpenCode** carga `AGENTS.md` global y locales, con `CLAUDE.md` como respaldo. El `instructions` de `opencode.json` agrega archivos o patrones después. La documentación actual describe combinación sin resolver contradicciones y difiere de V1 en el orden de carga. Usa la detección directa del archivo raíz. [OpenCode actual](https://opencode.ai/docs/instructions/) · [OpenCode V1](https://opencode.ai/v1/docs/rules/)
- **Claude Code** concatena los `CLAUDE.md` ancestros de lo general a lo específico y carga los de subdirectorios al acceder. Desde v2.1.277 admite `AGENTS.md` directamente bajo ciertas condiciones. Como este checkout ya tiene `CLAUDE.md`, agrega allí `@AGENTS.md` en una línea propia y conserva las instrucciones específicas de Claude. [Claude Code](https://code.claude.com/docs/en/memory#agentsmd)

## E. Borrador para `AGENTS.md`

```markdown
## Reglas de código

- Usa nombres del dominio y explicita las unidades en nombres o tipos.
- Dale una responsabilidad a cada función. Reduce el anidamiento.
- Agrupa argumentos relacionados y usa parámetros nombrados para opciones en Dart.
- Representa ausencia, errores y estados con tipos. Infiere tipos locales evidentes.
- Elimina comentarios que repitan el código. Explica decisiones e invariantes.
- Cita fuente, versión y sección para cada umbral o regla médica.
- Documenta contratos públicos y propiedad, vida útil y liberación en FFI.
- Conserva `unsafe_code = "forbid"` y propaga errores con `?` o manéjalos.
- Ejecuta formato y lint en el alcance modificado antes de entregar.
- Corrige cada diagnóstico nuevo. Usa una excepción puntual solo con motivo verificable.
- Durante la adopción, aplica la puerta a `crates/api`, `crates/bridge` y `app/`.
- Retira de la línea base cada diagnóstico corregido.
- Informa qué comandos ejecutaste, su resultado y cualquier chequeo pendiente.
```