# Etapa 3 · Figura corporal de Pulsys

## 1. Qué hice

Implementé `BodyScreen(api: ...)` con una consulta a `zoneStatus()`, carga, reintento y marca DEMO en la pantalla y en la ficha.
Porté a Dart el contorno frontal y las 12 regiones de la referencia. Espalda y Otra zona tienen controles propios debajo de la figura. Las 14 zonas también se pueden abrir desde la lista.
La figura combina color, iconos y una leyenda de texto que permanece visible al desplazar el cuerpo. Cada control anuncia el nombre anatómico, el estado y la selección.
La ficha conserva el estado de zona que devuelve Rust. Distingue la hora UTC del resumen de las evaluaciones individuales, con sus motivos, horas, fuentes, referencias, reglas y versiones.

## 2. Archivos que toqué

| Archivo | Contenido |
| --- | --- |
| `app/lib/features/body/body_screen.dart` | Entrada pública, figura/lista, selección, ficha y ciclo de sesión. |
| `app/lib/features/body/body_controller.dart` | Consulta asíncrona y estados de carga, respuesta y fallo. |
| `app/lib/features/body/body_geometry.dart` | Contorno Dart y polígonos frontales recortados a la silueta. |
| `app/lib/features/body/body_figure.dart` | Dibujo, detección de toques en el contorno y marcadores accesibles. |
| `app/lib/features/body/body_presentation.dart` | IDs anatómicos, nombres, botones semánticos, iconos y tokens de estado. |
| `app/lib/features/body/body_detail.dart` | Ficha desplazable, evidencia de cada síntoma y campos ausentes. |
| `app/test/widgets/body_screen_test.dart` | Siete pruebas de widget, incluidas las acciones semánticas sobre las 14 zonas en las dos vistas, lateralidad, procedencia, sesión, error/reintento, DEMO y texto al 200 %. |
| `app/test/goldens/body_figure_test.dart` | Golden propuesto a 390 × 1100 con fuentes locales, cuatro estados de Rust y DEMO. |
| `reviews/etapa3-cuerpo-astra.md` | Entrega, comandos y catálogo para integración. |

## 3. Comandos y resultados

La tarea prohíbe ejecutar procesos. Esta instrucción prevalece sobre la ejecución de lint y pruebas que pide AGENTS.md. Revisé los archivos directamente. El cotejo de coordenadas da 12 blancos frontales de 48 × 48 dp sin intersecciones entre ellos. La tabla siguiente cubre los 46 getters `body*` usados. Estas comprobaciones son estáticas.

Ejecutar los comandos desde `app/` después de incorporar las claves. La generación inicial del PNG requiere revisión visual en Windows antes de aceptarlo como referencia.

**NO EJECUTADO** — `flutter gen-l10n`

**NO EJECUTADO** — `dart format --output=none --set-exit-if-changed .`

**NO EJECUTADO** — `dart analyze --fatal-infos`

**NO EJECUTADO** — `flutter test --no-pub test/widgets/body_screen_test.dart` (7 pruebas escritas).

**NO EJECUTADO** — `flutter test --no-pub --tags golden --update-goldens test/goldens/body_figure_test.dart` (genera `test/goldens/baselines/body_figure.png`).

**NO EJECUTADO** — `flutter test --no-pub --tags golden test/goldens/body_figure_test.dart` (1 prueba visual contra el PNG revisado).

## 4. Pendientes y decisiones de integración

- Añadir al catálogo las claves de la tabla y regenerar las localizaciones antes de compilar. Todas son cadenas simples, sin parámetros.
- Ejecutar los comandos anteriores. Quedan pendientes el render real, la comparación visual y el recorrido con TalkBack en Android y VoiceOver en iOS, tanto con tamaño normal como al 200 %.
- La lateralidad corresponde al astronauta. El cuello se puede tocar directamente y también mediante un marcador lateral unido por una línea, de modo que el blanco de 48 dp deja libres la cabeza y el pecho.
- `crates/core/src/evaluate.rs:405–417` agrega síntomas por zona. La interfaz utiliza `ZoneState.status` y presenta cada `SymptomEvaluation` por separado. Si falta la zona completa, muestra “Status unavailable”. Si la zona tiene estado pero carece de evaluaciones, muestra ese estado y declara la ausencia de evidencia. La fecha común está rotulada como hora del resumen.
- Para el golden y las pruebas se utilizan respuestas sintéticas del adaptador de pruebas. La regla y la procedencia que se presentan en producción vienen del DTO.

### Claves para app_en.arb

| Clave | Texto inglés | Descripción |
| --- | --- | --- |
| `bodyTitle` | Body zones | Título de pantalla. |
| `bodyFront` | Front view | Encabezado de la figura anatómica frontal. |
| `bodyList` | Zone list | Encabezado de la lista equivalente. |
| `bodyShowList` | Show zone list | Abre la lista desde la figura. |
| `bodyShowFigure` | Show body figure | Vuelve a la figura desde la lista. |
| `bodyDemo` | DEMO | Marca datos simulados en pantalla y ficha. |
| `bodyLoading` | Loading body zones… | Lectura de zonas en curso. |
| `bodyLoadError` | Body zones could not be loaded. | Fallo de lectura de las zonas. |
| `bodyRetry` | Try again | Reintenta la consulta. |
| `bodyClose` | Close | Cierra la ficha de zona. |
| `bodyStable` | Stable | Estado estable devuelto por Rust. |
| `bodyChange` | Change | Cambio devuelto por Rust. |
| `bodyAlarm` | Alarm | Alarma devuelta por Rust. |
| `bodyNotEvaluated` | Not evaluated | Estado sin evaluar devuelto por Rust. |
| `bodyStatusUnavailable` | Status unavailable | La respuesta no contiene esta zona. |
| `bodySnapshotTime` | Snapshot time (UTC) | Fecha del resumen completo. |
| `bodyEvaluation` | Symptom evaluation | Encabezado de cada evaluación individual. |
| `bodyReason` | Reason | Motivo devuelto en la evaluación. |
| `bodyNoData` | No data | Motivo no_data. |
| `bodyStale` | Stale reading | Motivo stale. |
| `bodyTooFewData` | Insufficient history | Motivo too_few_data. |
| `bodyNotApplicable` | Context only | Motivo not_applicable. |
| `bodyMissing` | Not provided | El campo no aparece en la respuesta. |
| `bodyEvaluationTime` | Evaluation time (UTC) | Fecha de la evaluación individual. |
| `bodySourceKind` | Source type | Tipo de procedencia de la evaluación. |
| `bodySourceRef` | Source reference | Referencia de la fuente devuelta por el núcleo. |
| `bodyPublished` | Published source | Tipo de fuente published. |
| `bodyApprovedProtocol` | Approved protocol | Tipo de fuente approved_protocol. |
| `bodyDemoHeuristic` | Demo rule, not medical advice. | Tipo de fuente demo_heuristic. |
| `bodyRuleId` | Rule ID | Identificador de regla sin traducir. |
| `bodyRuleVersion` | Rule version | Versión de la regla sin traducir. |
| `bodyNoEvidence` | No symptom evaluation was provided for this zone. | La zona carece de evaluaciones individuales y sus metadatos. |
| `bodyHead` | Head | Nombre anatómico de head. |
| `bodyNeck` | Neck | Nombre anatómico de neck. |
| `bodyChest` | Chest | Nombre anatómico de chest. |
| `bodyAbdomen` | Abdomen | Nombre anatómico de abdomen. |
| `bodyRightArm` | Right arm | Nombre anatómico de right_arm. |
| `bodyLeftArm` | Left arm | Nombre anatómico de left_arm. |
| `bodyRightHand` | Right hand | Nombre anatómico de right_hand. |
| `bodyLeftHand` | Left hand | Nombre anatómico de left_hand. |
| `bodyRightLeg` | Right leg | Nombre anatómico de right_leg. |
| `bodyLeftLeg` | Left leg | Nombre anatómico de left_leg. |
| `bodyRightFoot` | Right foot | Nombre anatómico de right_foot. |
| `bodyLeftFoot` | Left foot | Nombre anatómico de left_foot. |
| `bodyBack` | Back | Nombre anatómico de back. |
| `bodyOther` | Other | Nombre anatómico de other. |
