# Revisión del rediseño de front de Merken

**Veredicto: aprobado con cambios.**

La entrega respeta los archivos protegidos, elimina los valores de respaldo ficticios y mantiene separadas las evaluaciones de MySQL y del núcleo. Hay dos problemas mayores que deben corregirse antes de cerrar la entrega: una evaluación de alarma devuelta por el registro manual puede quedar sin mostrar y el bucle de foco impide acceder por Tab al historial del diálogo.

Revisión de `d49b669..95ffd185`, en modo solo lectura. No modifiqué archivos. No identifiqué hallazgos bloqueantes.

## Hallazgos por gravedad

### 1. M — El registro manual descarta la evaluación confirmada por el núcleo

**Archivos:** `ui/src/ui/core-panel.ts:260–269`; `ui/src/ui/voice.ts:183–189`.

**Problema:** `acceptRecord(result)` utiliza `result.record.is_demo`, pero ignora `result.evaluation`. La pantalla depende del siguiente `getStatus()` para mostrar el nuevo estado.

Si el núcleo guarda una lectura y devuelve una evaluación de alarma, pero falla ese refresco, puede seguir mostrándose el estado anterior —incluso `Stable`— acompañado del aviso de indisponibilidad. La evaluación confirmada ya estaba disponible en la respuesta del guardado.

Reproduje en memoria la secuencia: estado inicial `stable`, registro guardado con evaluación `alarm` y fallo posterior del servicio. Las notificaciones conservaron `stable` y no transmitieron la nueva evaluación.

**Arreglo:** conservar y presentar `result.evaluation` inmediatamente después de `saved`. Mientras no llegue un estado agregado actualizado, mostrar esa evaluación confirmada y marcar el resumen anterior como pendiente de actualización. No fabricar un nuevo `overall` en la UI. Mantener también esta información cuando el guardado sustituya DEMO y falle el refresco.

### 2. M — El foco atrapado excluye los controles de historial

**Archivos:** `ui/src/ui/dialog.ts:21–30`; `ui/src/ui/sparkline.ts:52–53`; `ui/src/ui/dashboard.ts:113–116`.

**Problema:** la lista de controles del bucle de Tab no incluye `<summary>`.

En el diálogo de lectura, `Close` puede ser el único elemento reconocido por ese selector. Al pulsar Tab desde él, el manejador devuelve el foco al mismo botón e impide alcanzar los controles nativos del historial, incluido `Read sample data`.

La reproducción en memoria confirmó que Tab se cancela y vuelve a enfocar `Close`.

**Arreglo:** incluir `summary` en el cálculo de elementos navegables y comprobar correctamente su visibilidad. Verificar Tab y Shift+Tab con los detalles abiertos y cerrados, además de Escape y devolución del foco.

### 3. m — `loadDemo` carece de protección explícita de disponibilidad

**Archivo:** `ui/src/ui/core-panel.ts:226–234`.

**Problema:** el manejador comprueba `actionBusy`, pero llama a `core.loadDemo()` sin comprobar `available`. El botón queda oculto en el navegador durante el flujo normal; la protección depende de esa presentación, mientras que la regla exige proteger toda llamada al núcleo.

**Arreglo:** comenzar el manejador con una comprobación de `!available || stopped || actionBusy` y mantener deshabilitado el botón cuando el núcleo no esté disponible.

### 4. m — La confirmación de ayuda aparece habilitada en navegador

**Archivos:** `ui/src/ui/help.ts:43–54`; `ui/src/ui/help.ts:72–78`; `ui/index.html:91`.

**Problema:** confirmado en código. Al abrir el diálogo con `available === false`, se muestra la explicación de indisponibilidad y se retorna antes de deshabilitar `Record help request`.

El clic no llama al núcleo ni afirma un guardado: muestra otro aviso y oculta el botón. Aun así, ofrece inicialmente una acción que sabe que no puede ejecutar.

**Arreglo:** establecer `confirm.disabled = !available` antes de abrir el diálogo y conservar la explicación visible. Mantener accesible `Need help` para que el usuario pueda consultar esa limitación.

### 5. m — El flujo de ayuda pierde el detalle de `CoreError`

**Archivo:** `ui/src/ui/help.ts:110–113`.

**Problema:** el `catch` descarta el error y presenta siempre un mensaje genérico. Se conserva la entrada, pero un rechazo con `code`, `field` y `message` pierde la explicación que permitiría corregirlo.

**Arreglo:** capturar el error, reconocerlo mediante `isCoreError()` y mostrar su mensaje con inserción textual. Utilizar `field` para señalar el campo afectado cuando corresponda y reservar el mensaje genérico para errores desconocidos.

### 6. m — El estado de sueño puede partir palabras y desalinear las filas

**Archivos:** `ui/src/styles.css:130`; `ui/src/styles.css:154–163`; `ui/src/ui/status-copy.ts:20–21`.

**Problema:** la causa del hallazgo es compatible con el código: `overflow-wrap: anywhere` permite cortar dentro de `Insufficient`, y cada columna calcula sus cuatro filas de manera independiente. Un estado más alto en la columna derecha puede desalinearla respecto de la izquierda.

No hay un guion introducido en el texto fuente; el corte visual exacto a 320 px queda pendiente de reproducción.

**Arreglo:** permitir que icono y texto se redistribuyan conservando palabras completas y compartir las filas entre ambas columnas, por ejemplo mediante una cuadrícula común con `subgrid`. Verificar también `Very insufficient rest`, sin recortar el estado ni imponer una altura que oculte contenido.

### 7. m — La cabecera reserva un ancho fijo insuficientemente robusto para la marca

**Archivos:** `ui/src/styles.css:138–139`; `ui/index.html:12`.

**Problema:** la columna móvil reserva 112 px para una marca a 36 px con espaciado entre letras. No se adapta al ancho real del texto, lo que permite que invada la columna del selector.

La explicación de Claude sobre «un SVG más ancho que su enlace» **no corresponde al código actual**: la cabecera contiene texto `MERKEN`, sin SVG. El riesgo de solapamiento sigue siendo consistente con la cuadrícula fija.

**Arreglo:** dimensionar la columna según el ancho intrínseco de la marca, conservar su proporción y espacio de separación, y dejar que el selector absorba la reducción disponible. Revisar 320 y 375 px con la fuente cargada y durante su carga.

### 8. m — Las zonas pequeñas del mapa no cumplen el objetivo táctil del brandbook

**Archivos:** `ui/src/ui/body-map.ts:14–19`; `ui/src/styles.css:154`; `ui/src/styles.css:167–168`.

**Problema:** las manos y los pies utilizan directamente su geometría visible como área de activación. Al reducir la figura en móvil, los objetivos quedan muy por debajo de los 44 × 44 px establecidos en el brandbook.

La lista `All zones`, con filas de altura mínima de 44 px, ofrece una alternativa funcional y reduce el impacto. No confirma que tocar directamente la silueta sea cómodo.

**Arreglo:** ampliar las áreas de activación sin alterar la anatomía visible ni solapar zonas competidoras. Donde no quepan objetivos separados, utilizar selección ampliada o acercamiento y conservar la lista alternativa. Comprobar las dimensiones finales en navegador.

### 9. m — Las cachés de eventos y anuncios retienen entradas retiradas

**Archivos:** `ui/src/ui/core-panel.ts:88–89`; `ui/src/ui/core-panel.ts:135–139`; `ui/src/ui/core-panel.ts:182`; `ui/src/ui/core-panel.ts:262–279`.

**Problema:** `announced` y `knownEvents` incorporan entradas, pero no eliminan las que dejan de ser relevantes. Las vistas y sus firmas sí se retiran; estas otras colecciones permanecen, incluso al sustituir DEMO por datos reales.

Es una retención acumulativa en sesiones prolongadas; no medí su impacto de memoria.

**Arreglo:** depurar ambas colecciones usando el estado autoritativo vigente y limpiar las entradas DEMO al confirmar su sustitución. Conservar los eventos todavía referenciados por alertas persistentes.

## Contraste de los hallazgos de Claude

| Hallazgo | Resultado |
|---|---|
| Marca recortada a 375/320 px | Confirmación parcial: cuadrícula fija problemática; la causa atribuida al SVG no corresponde a este `HEAD`. Véase hallazgo 7. |
| Temperatura `37 °C` en lugar de `37.0 °C` | Confirmado: `vitals.ts:40` usa `numberText()`, que omite ceros decimales finales (`dom.ts:32–33`). Antes se utilizaba `toFixed(1)`. |
| Corte del estado de sueño | Causa confirmada en CSS; corte exacto pendiente de renderizado. Véase hallazgo 6. |
| Objetivos diminutos del mapa | Confirmado estructuralmente; las dimensiones exactas aportadas no fueron remedidas. Véase hallazgo 8. |
| Confirmación de ayuda activa en navegador | Confirmado. Véase hallazgo 4. |
| Pecho resalta pulso y SpO₂, pero no presión | Comportamiento intencional y documentado: `dashboard.ts:12–13` vincula presión con `left_arm`; `rediseno.md:44` describe ese mismo mapeo. No constituye un defecto respecto del contrato entregado. |

**Sobre la temperatura:** el cambio de formato existe, pero `37` y `37.0` representan el mismo valor numérico. El brandbook pide preservar precisión y no añadir decimales ficticios (`docs/brand/brandbook.md:225`). Si el equipo requiere recuperar una décima fija, debe documentar esa política de presentación y aplicarla solo a temperatura; no cambiar globalmente `numberText()`.

## Comprobaciones satisfactorias

- **Archivos protegidos:** los 52 archivos comprendidos por `vital-status.ts`, `core-api.ts`, `vite.config.ts`, `crates/` y `src-tauri/` mantienen el mismo contenido entre base y `HEAD`.
- **Referencias:** coinciden los cinco conjuntos numéricos originales: `50–100`, `≥94`, `90–139 / 60–89`, `70–180` y `36.1–38.0`.
- **Clasificación MySQL:** `status-copy.ts:26–35` delega en las funciones de `vital-status.ts`; no encontré umbrales clínicos duplicados en los componentes nuevos.
- **Datos y seguridad:** no encontré valores médicos de respaldo ni transcripciones simuladas en la app. Los datos externos se insertan mediante texto. Los usos de `innerHTML` revisados corresponden a iconos estáticos y al espécimen de marca, aislado y rotulado como ilustrativo.
- **Mapa en navegador:** permanece neutral. Sus colores de evaluación se actualizan exclusivamente desde `body_zones` del núcleo.
- **Ayuda:** advierte sobre la eliminación de DEMO, vuelve a comprobar ese estado antes del guardado y no afirma recepción en Tierra.
- **Registro manual:** trata `needs_confirmation`, muestra los avisos y envía `confirm_warnings: true` únicamente para el contenido previamente revisado. Las ediciones invalidan la confirmación.
- **Sondeo:** existen protecciones contra consultas simultáneas dentro de cada sondeo; el feed cancela solicitudes al detenerse y los intervalos tienen limpieza. Queda la retención descrita en el hallazgo 9.
- **Accesibilidad:** se usan diálogos nativos con `showModal()`, devolución del foco, nombres accesibles y teclado para las 14 zonas, regiones de anuncio y movimiento reducido. El bucle de foco requiere la corrección del hallazgo 2.
- **Marca y textos:** la app importa Basalt y las tres familias de Flight Documentation. Los textos de interfaz están en inglés y mantienen, en general, el tono prudente del brandbook.
- **Contraste calculado:** texto/superficie ≈13,48:1; texto secundario/superficie ≈8,33:1; borde/superficie elevada ≈4,39:1; foco/superficie elevada ≈8,59:1. Estos cálculos no certifican el renderizado completo.
- **TypeScript:** comprobación con la API de TypeScript 6.0.3, el `tsconfig.json` real y sin emisión: **sin diagnósticos**.

## Lo que no pude verificar

- Renderizado actual a 320/375 px, medidas táctiles, zoom, teclado virtual, lector de pantalla y contraste renderizado: el acceso al navegador no estuvo disponible.
- Los resultados visuales, de consola y de interacción comunicados por Claude se consideran evidencia aportada; no los reproduje en esta sesión.
- Build de Vite, empaquetado y ejecución real de Tauri: la creación de procesos fue denegada por el entorno.
- Integración real con MySQL y el ciclo completo de almacenamiento, DEMO, errores y sincronización. Las reproducciones nuevas utilizaron dobles en memoria.
- Consumo de memoria prolongado y funcionamiento en WebView2/Android.
- Vigencia externa de enlaces, licencias, cifras de actividad y demás afirmaciones de la investigación. La revisión contrastó su aplicación al código, sin una auditoría web independiente.
---

## Respuesta de Claude (14-nov-2026)

Todos los cambios se verificaron con `npx tsc --noEmit` y en el navegador (puerto 1430, a 320 px, 375 px y 1280 px).

| # | Estado | Cambio |
|---|---|---|
| 1 M | Corregido | `acceptRecord` pinta al momento `result.evaluation` en su fila, marca el resumen como «Summary update pending», anuncia la alarma y la pasa a la cabecera (`confirmedAlert`). No recalcula `overall`. Si se sustituye DEMO, limpia el perfil y el resumen anteriores. |
| 2 M | Corregido | `dialog.ts` incluye `summary` en el bucle de Tab y excluye los controles ocultos o los que están dentro de un `<details>` cerrado. |
| 3 m | Corregido | `loadDemo` comprueba `!available \|\| stopped \|\| actionBusy` y el botón está deshabilitado sin núcleo. |
| 4 m | Corregido | «Record help request» sale deshabilitado en el navegador; la explicación sigue visible y «Need help» sigue accesible. Verificado. |
| 5 m | Corregido | El `catch` de ayuda muestra el `message` de `CoreError` y enfoca el mensaje si `field === "message"`. |
| 6 m | Corregido | `overflow-wrap: break-word` en vez de `anywhere`, y filas `minmax(86px, 1fr)`. Verificado a 320 px: «Very insufficient rest» sin cortes y las dos columnas con las filas alineadas (174/299/423/548 px). |
| 7 m | Corregido | La cabecera usa `auto minmax(0,1fr) auto` y el logotipo `clamp(24px, 7.5vw, 36px)` en una sola línea. A 320 px, el logotipo acaba en 105 px y el selector empieza en 117 px. |
| 8 m | Corregido | Cuello, manos y pies tienen un área táctil invisible más ancha (trazo transparente de 36 unidades) detrás de las zonas visibles, que conservan la prioridad. Comprobado con `elementFromPoint`. La lista «All zones» se mantiene. |
| 9 m | Corregido | `announced` y `knownEvents` se depuran con las alertas vigentes y se vacían al sustituir DEMO. |
| Temperatura | Sin cambio | Coincido con Sol: «37» es el valor exacto de MySQL y el brandbook no quiere decimales ficticios. |
| Pecho y presión | Sin cambio | Es intencional: la presión se asocia a `left_arm`, como en `rediseno.md`. |

Pendiente: probar en Tauri real (núcleo, DEMO, `logRecord`, `need_help`) y en Android.
