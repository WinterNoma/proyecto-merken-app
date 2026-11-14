# Rediseño de Merken

Fecha: 14 de noviembre de 2026. Implementación local de la dirección aprobada: **B / Basalt Standard + Flight Documentation**.

## Decisiones visuales y de contenido

- La app importa los tokens existentes de `docs/brand/tokens.css`. Barlow Semi Condensed 600 se usa en títulos y MERKEN; Public Sans 400/600 en texto y controles; Roboto Mono 400 en lecturas y tiempos. Las cuatro hojas de fuentes son locales.
- Superficies opacas, divisores discretos, contornos de estado y SVG de trazo. Se retiraron los brillos, degradados, emojis, escaneo animado y ECG ficticia.
- Diseño de una columna en 375–430 px, con 16 px de margen. Desde 900 px se separan el feed y el registro del dispositivo en dos columnas. Las tarjetas ganan su segunda columna cuando hay espacio. No se oculta el desbordamiento horizontal para disimular problemas de tamaño.
- Todo el texto de interfaz está en inglés. Nombres, mensajes escritos por la persona y textos del núcleo se muestran mediante `textContent`. El único `innerHTML` nuevo contiene geometría SVG fija del catálogo de iconos.
- Las referencias mantienen los números originales: 50-100 bpm, ≥ 94%, 90-139 / 60-89, 70-180 mg/dL y 36.1 - 38.0 °C.
- El esquema de MySQL contiene diez grupos de indicadores; presión sistólica y diastólica aportan dos lecturas. Se conservan los once valores en diez tarjetas, sin inventar un undécimo grupo. Nutrición, radiación y distancia son contexto sin color sanitario. Ánimo es un autoinforme sin regla de evaluación en `vital-status.ts`.
- No se agregaron datos de batería, misión, comunicaciones, roles ni valores sustitutivos. Los datos ausentes aparecen como `--` / `No data`.
- Los estados de las lecturas MySQL y la evaluación histórica del núcleo se presentan por separado. Un resultado `ok` de una lectura no se convierte en `Stable` del núcleo.

## Mapa de módulos

| Archivo | Responsabilidad |
| --- | --- |
| `ui/index.html` | Estructura semántica, tarjetas, referencias, panel local y diálogos. |
| `ui/src/main.ts` | Importación de marca y fuentes; composición de módulos; detección de Tauri y limpieza del sondeo. |
| `ui/src/styles.css` | Estilo Basalt, distribución adaptable, foco, controles, estados y movimiento reducido. |
| `ui/src/types.ts` | Datos MySQL anulables; identidad real sin roles ni colores inventados. |
| `ui/src/ui/dom.ts` | Creación segura de nodos, texto, valores y tiempos UTC. |
| `ui/src/ui/icons.ts` | SVG propios de estado, navegación e identidad. |
| `ui/src/ui/telemetry.ts` | Validación de filas, presión, sondeo MySQL y último resultado correcto. |
| `ui/src/ui/status-copy.ts` | Traducción de los resultados de `vital-status.ts` y presentación de estados del núcleo. |
| `ui/src/ui/crew.ts` | Selector nativo dentro del carrusel modal, flechas, puntos, teclado e identidad por ID. |
| `ui/src/ui/vitals.ts` | Tarjetas MySQL, vocabulario de ánimo y disponibilidad del feed. |
| `ui/src/ui/dialog.ts` | Modal nativo, cierre, ciclo de Tab y devolución del foco. |
| `ui/src/ui/core-format.ts` | Nombres, unidades y extracción de valores del contrato Rust. |
| `ui/src/ui/core-panel.ts` | Perfil, estado, historial, alertas, sincronización y carga de DEMO. |
| `ui/src/ui/body-map.ts` | Figura con las 14 zonas, leyenda y equivalencia textual de cada estado. |
| `ui/src/ui/sparkline.ts` | SVG del historial real y tabla accesible de muestras. |
| `ui/src/ui/help.ts` | Confirmación de ayuda, aviso DEMO, guardado y mensajes de resultado. |

## MySQL y selector

Se mantiene `fetch("/api/astronauts")` con intervalo de 2 segundos y sin consultas superpuestas. Cada petición tiene cancelación a los 8 segundos; se limpia al salir de la página. El middleware no se abrió ni se modificó.

Los campos numéricos admiten números y cadenas numéricas; no convierten vacíos, booleanos ni datos malformados en lecturas. Se conserva el cero, incluido sueño de 0 h. Una presión parcial conserva su componente disponible y no se evalúa como presión completa.

`Live` aparece únicamente después de un sondeo correcto. Un fallo conserva las últimas lecturas, muestra `Last update hh:mm:ss UTC` y explica que son el último resultado recibido. Una respuesta correcta pero vacía elimina la tripulación anterior y muestra el estado vacío. La hora de recepción no se presenta como hora de medición: el contrato de este feed no aporta un timestamp de medición para cada indicador.

La selección se mantiene por ID ante reordenamientos del feed. El modal ofrece un selector nativo, flechas, puntos con botones de 44 px, ArrowLeft/ArrowRight y Home/End fuera del selector nativo, Escape, clic fuera, selección explícita y devolución del foco. La vista previa presenta nombre, ID, lecturas y estado con icono y texto. La cantidad de tripulantes procede de la respuesta, sin fijar seis en el código.

Las seis funciones de `vital-status.ts` siguen calculando los niveles. La traducción interpreta sus resultados y la dirección de su texto, sin reproducir los umbrales. Se usa `worstLevel` sobre las lecturas disponibles. Si faltan lecturas, se declara la falta: una colección incompleta de lecturas normales no se presenta como un agregado verde; un nivel warn/alarm disponible se conserva.

El ánimo de MySQL es texto libre, sin catálogo documentado. Se traducen términos conocidos de español e inglés. Para un término desconocido se muestra `Recorded` sin inferir su significado; queda pendiente ampliar el vocabulario con los valores reales de la base. Los nombres propios no se traducen.

## Integración del núcleo

`isTauri` se importa de `@tauri-apps/api/core`; no se cambiaron firmas ni tipos de `core-api.ts`.

### Navegador

No se invoca el núcleo. Se muestra:

> Local service unavailable  
> Open the Merken app to use on-device records.

MySQL sigue funcionando de forma independiente. El diálogo Need help explica la limitación y, al intentar confirmar, declara `Local service unavailable. This request was not recorded.` No se guarda ni se simula un evento.

### Tauri

- Se consulta `getStatus()` aproximadamente cada 10 segundos, junto con `getProfile()`. Las acciones refrescan el estado al terminar. Se evitan consultas superpuestas y se invalidan respuestas anteriores a un cambio local.
- La identidad local se muestra explícitamente; no se reasigna al tripulante elegido en MySQL. El contrato no expone selección local por aquel ID.
- Se presentan `overall.status`, `escalation`, `worst`, `met`, `sync.pending`, `sync.failed` y la hora de evaluación. Un error conserva el último resultado confirmado y sus alarmas con un aviso de disponibilidad.
- Cada indicador conserva valor, unidad, hora, resumen, fuente, dirección y persistencia cuando el núcleo las devuelve. Se distinguen Stable, Change detected, Alarm, No data, Stale reading, Insufficient history (n/m), Not evaluated y Context only.
- Los resultados `demo_heuristic` incluyen `Demo rule, not medical advice`. DEMO aparece en el panel, lecturas y eventos correspondientes.
- Se muestran `status.symptoms` y las 14 entradas de `status.body_zones`. El cuerpo no deriva estados de MySQL. Cabeza, cuello, pecho, abdomen, brazos, manos, piernas y pies tienen zonas separadas; espalda y otra zona se muestran como referencias laterales.
- Las alertas son persistentes, sin botón de resolución. Se anuncia una alerta nueva una sola vez por identidad. Se mantienen los detalles abiertos cuando no cambia su contenido y se ofrecen IDs, regla, fuente y, cuando está disponible el evento, mensaje, hora y sincronización.
- La carga DEMO se ofrece solo si se confirma que no hay perfil, registros, eventos, síntomas ni valores de indicadores. `loadDemo("routine")` sigue siendo la autoridad final: se captura `real_data_present` y se oculta la acción si hay datos reales.

### Historial

Se consulta `listRecords({ indicator, limit: 40 })` para pulso, temperatura, SpO₂, glucosa y sueño. Solo se representan registros efectivos, sin timestamps futuros marcados por el núcleo y con fecha interpretable. Se ordenan por hora de medición.

No hay series de ejemplo en la app. Un punto se dibuja como punto, una serie vacía muestra `No history` y una consulta fallida muestra `History unavailable`. Se muestra la escala numérica, unidad, inicio y fin, además de la tabla con las muestras exactas. Se preserva el carácter histórico de un estado stale y no se extrapola hasta ahora.

La discontinuidad gráfica usa una regla exclusivamente visual: se interrumpe el trazo ante un valor ausente o un intervalo superior a tres veces la mediana de los intervalos observados. No es un umbral sanitario ni modifica estados. No se añadió la sparkline opcional de sesión MySQL.

### Need help

1. El diálogo identifica el perfil del dispositivo y explica que el guardado es local; no confirma recepción en Tierra.
2. Se consulta el estado actual y se muestra el aviso `Demo data will be removed` cuando corresponde.
3. Antes de guardar se vuelve a consultar DEMO. Si apareció mientras el diálogo estaba abierto, se exige otra activación del botón con el aviso visible.
4. La confirmación llama a `raiseEvent({ kind: "need_help", message })`, con mensaje opcional de hasta 2.000 caracteres.
5. Se bloquea la doble activación durante el guardado y después de un resultado correcto. La cancelación no crea eventos.
6. Solo tras la respuesta se muestra `Help request saved locally` y el estado de sincronización devuelto. Un fallo conserva el mensaje y permite reintentar.
7. La alerta devuelta queda visible aunque falle el refresco posterior. Si el evento real eliminó DEMO, se retira la presentación anterior de esos datos.

## Accesibilidad

Se incluyeron `lang="en"`, enlace de salto, jerarquía de encabezados, unidades legibles, tablas de muestras, texto además del color, estados con `aria-live`, alarmas nuevas anunciadas y foco visible. Los diálogos usan `showModal()`, `role="dialog"`, `aria-modal`, nombre, descripción, foco inicial en Cancel/Close, ciclo de Tab, Escape y devolución al botón de apertura.

El viewport conserva zoom. Los controles tienen al menos 44 px de alto; los puntos del carrusel también tienen un objetivo de 44 × 44 px. Se usan los pares de color AA del brandbook sin bajar la opacidad de texto secundario o de controles deshabilitados. Las transiciones son solo de color/borde y se eliminan con `prefers-reduced-motion`.

Esto describe la implementación, no certifica conformidad WCAG ni sustituye una prueba con lector de pantalla y dispositivos reales.

## Verificación realizada

El sandbox no permite lanzar procesos. Se usó el intérprete Node disponible para lectura/escritura y comprobaciones en memoria; no se ejecutaron comandos del sistema ni se inició servidor alguno.

- API del compilador TypeScript instalado, con el `tsconfig.json` del proyecto, `noEmit`, `strict`, `noUnusedLocals` y `noUnusedParameters`: sin diagnósticos.
- 107 comprobaciones de funciones puras: ausencia/malformación, ceros, presión parcial, paridad exacta de niveles en límites de las funciones originales, traducciones, agregados incompletos y formato de valores del núcleo.
- 10 comprobaciones aisladas del sondeo con fetch simulado: carga inicial, éxito, error, conservación del último dato, respuesta vacía, intervalo de 2 segundos, endpoint y limpieza.
- 13 comprobaciones de integración del panel con un núcleo y DOM mínimos simulados: navegador sin llamadas Rust, estado local, disponibilidad de DEMO, persistencia de alarmas ante fallo, `real_data_present` y retirada de DEMO con conservación del evento real.
- 11 comprobaciones del flujo de ayuda con servicios simulados: navegador sin guardado, aviso DEMO, argumento del evento, éxito local, sincronización pendiente, duplicados, conservación del mensaje, reintento y aparición de DEMO antes de confirmar.
- Parseo sintáctico de CSS, comprobación de IDs únicos y selectores estáticos, existencia de las cuatro importaciones de fuentes y revisión de los usos de `innerHTML`.
- Hashes sin cambios para los archivos protegidos leídos: core-api, vital-status, brandbook, tokens, brand.ts, brand.css, brand.html y package.json.

Las pruebas de integración anteriores verifican lógica con dobles de prueba; **no son pruebas de renderizado ni de Tauri real**. Los datos de esas pruebas existieron solo en memoria, no se insertaron en el producto ni en MySQL.

## Pendiente de verificar por el equipo

1. Ejecutar `npx tsc --noEmit` y la compilación Vite/Tauri en el entorno habitual. La API del compilador pasó, pero no se ejecutó el comando de consola ni el empaquetado.
2. Revisar visualmente a 375, 390, 430, 900 y 1440 px; nombres largos, valores grandes, zoom 200 %, viewport bajo y teclado virtual.
3. Probar con la base MySQL real: seis tripulantes, cambios de orden, pérdida/recuperación de conexión y vocabulario real de ánimo.
4. Probar Tauri con almacén vacío, DEMO, datos reales, historial y fallos de almacenamiento. Confirmar el aviso y la retirada de DEMO en el primer evento real.
5. Probar Tab/Shift+Tab, Escape, clic fuera, foco restaurado, selector nativo, lector de pantalla, contraste final y movimiento reducido en WebView móvil.
6. Comprobar todas las zonas con síntomas y la conservación de alarmas durante una indisponibilidad.
7. La recepción/sincronización con Tierra y la resolución de eventos siguen dependiendo del contrato/backend existente; no se implementan ni se prometen desde esta interfaz.

## Alcance de archivos

Solo se modificaron `ui/index.html`, `ui/src/main.ts`, `ui/src/styles.css` y `ui/src/types.ts`; se añadieron módulos en `ui/src/ui/` y este documento. No se abrieron ni modificaron `vite.config.ts`, `crates/` o `src-tauri/`. No se alteraron dependencias, la muestra de marca ni los archivos del brandbook.
