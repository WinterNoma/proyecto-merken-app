# Etapa 2 de Pulsys · UI y contrato Dart del adaptador (Hydrogen)

## 1. Qué hice

- Definí el contrato Dart `PulsysApi` con tipos de presentación inmutables, clases selladas (`SaveOutcome`, `PulseTarget`), el tipo con unidad `Bpm`, marcas de tiempo en UTC y errores tipados `ApiError` con `code` y `field`, además de `FakePulsysApi` en memoria con respuestas/fallos programables e idempotencia por clave.
- Traduje los tokens activos del brandbook (**Basalt Standard + Flight Documentation**) a `app/lib/design/tokens/` y `app/lib/design/theme.dart`.
- Implementé el recorrido de pulso (`PulseController`, `PulseScreen`, diálogo de confirmación con avisos/DEMO y lista simple de historial), ampliando `app/lib/l10n/app_en.arb`, creando `PulsysApp`, el enrutamiento inicial y un adaptador en memoria aislado en `createPulsysApi()` dentro de `app/lib/main.dart`.
- Añadí pruebas de widgets y semántica en `app/test/widgets/pulse_screen_test.dart` y actualicé el smoke test en `app/test/widget_test.dart` para `PulsysApp`.

## 2. Archivos que tocaste

- `app/lib/platform/pulsys_api.dart`
- `app/lib/design/tokens/colors.dart`
- `app/lib/design/tokens/spacing.dart`
- `app/lib/design/tokens/typography.dart`
- `app/lib/design/theme.dart`
- `app/lib/l10n/app_en.arb`
- `app/lib/l10n/generated/app_localizations.dart`
- `app/lib/l10n/generated/app_localizations_en.dart`
- `app/lib/features/pulse/pulse_controller.dart`
- `app/lib/features/pulse/pulse_format.dart`
- `app/lib/features/pulse/pulse_confirmation_dialog.dart`
- `app/lib/features/pulse/pulse_screen.dart`
- `app/lib/routing/app_router.dart`
- `app/lib/app.dart`
- `app/lib/main.dart`
- `app/test/fakes/fake_pulsys_api.dart`
- `app/test/widget_test.dart`
- `app/test/widgets/pulse_screen_test.dart`
- `reviews/etapa2-ui-hydrogen.md`

## 3. Comandos que ejecutaste, con su resultado

### Resultados informados por Claude (ronda 1)

- `flutter gen-l10n`: regeneró `app/lib/l10n/generated/`.
- `flutter analyze`: 11 infos (8 imports de `package:flutter/material.dart` obsoletos en Flutter 3.50 frente a `package:material_ui/material_ui.dart`, 2 `prefer_initializing_formals` en `pulse_controller.dart:26` y `fake_pulsys_api.dart:34`, y 1 `prefer_final_fields` en `fake_pulsys_api.dart:66`).
- `dart format`: reformateó `app/test/widgets/pulse_screen_test.dart`.
- `flutter test`: 5 pruebas pasaron y 3 fallaron (`displays validation errors on the matching form field`, `exposes accessible semantic labels for fields and action` y `renders form and dialog at 2.0x text without overflow`).

### Resultados informados por Claude (ronda 2)

- `dart analyze --fatal-infos`: sin avisos.
- `flutter test`: 7 pruebas pasaron y 1 falló (`renders form and dialog at 2.0x text without overflow`): tras `enterText`, el scroll implícito de `EditableText` dejaba una `DrivenScrollActivity` activa en `Scrollable` (`IgnorePointer` bloqueaba el `tap` posterior cuando `_scrollAndTap` solo esperaba un `pump()`).

### Verificación tras la corrección (ronda 3)

**NO EJECUTADO** `dart analyze --fatal-infos` (en `app/`, por indicación de la tarea: entorno headless).

**NO EJECUTADO** `dart format --output=none --set-exit-if-changed lib test` (en `app/`, por indicación de la tarea: entorno headless).

**NO EJECUTADO** `flutter test` (en `app/`, por indicación de la tarea: entorno headless).

## 4. Lo que queda pendiente o en duda

- Ejecutar desde `app/` `dart analyze --fatal-infos`, `dart format --output=none --set-exit-if-changed lib test` y `flutter test` tras esta corrección.
- `app/pubspec.yaml` no registra todavía los archivos locales de las fuentes `Barlow Semi Condensed`, `Public Sans` y `Roboto Mono` en `assets/fonts/` (no se modificó `pubspec.yaml` por estar fuera de alcance); los estilos en `PulsysTypography` declaran sus nombres y respaldos del sistema hasta que el integrador empaquete las fuentes.
- Falta conectar `createPulsysApi()` en `app/lib/main.dart` con `RustAdapter` cuando Sol entregue el puente FRB.
- Para la generación de claves de idempotencia se usó un generador UUID v4 puro en Dart (`Random.secure()`) sin añadir paquetes externos; si más adelante se requiere UUID v7 con reloj o el paquete `uuid`, el generador es inyectable en `PulseController`.
