# Rediseño de Pulsys · Pantalla principal

14 de noviembre de 2026. Segunda ronda: implementación de `docs/front/concepto-pantalla.md`, sobre Basalt Standard + Flight Documentation. Interfaz en inglés; informe en español.

## Resultado

La principal reúne el selector de tripulante, Need help, estado general, cuerpo con ocho tarjetas, contexto y entrada manual/voz. El historial extenso pasa a una segunda pantalla. Las dos se alcanzan con puntos de paginación de 44 px o con un deslizamiento horizontal; se ignoran los gestos iniciados sobre controles y los desplazamientos predominantemente verticales. No hay pestañas fijas.

En móvil, las tarjetas se distribuyen en dos columnas y el cuerpo ocupa una columna central de aproximadamente 98 px a 375 px de viewport. La cuadrícula usa columnas laterales flexibles, sin mínimos que provoquen desbordamiento. A 375 px, los márgenes de 16 px y los dos espacios de 8 px dejan aproximadamente 115 px por columna de tarjetas. Las tarjetas priorizan etiqueta, valor, unidad y estado con icono. Las referencias quedan en el detalle. La altura puede crecer con avisos, estados largos, zoom o un viewport bajo: se permite desplazamiento vertical, sin recortar información para forzar una altura fija.

Desde 900 px se mantiene la composición, con cuerpo de 170 px, más separación y tipografía mayor. La segunda pantalla sigue siendo independiente; su contenido se distribuye en dos columnas. Se eligió esta opción permitida por el concepto para conservar una principal tranquila también en escritorio.

Se conservan tokens y fuentes locales: Barlow Semi Condensed, Public Sans y Roboto Mono. Los controles tienen borde visible y foco; la selección usa contorno y, en el cuerpo, trazo discontinuo. El color de estado no se usa para la conexión ni la sincronización. Las cajas de radiación, distancia y sincronización son neutrales.

## Fuentes de información

- Las tarjetas compactas muestran al tripulante seleccionado en MySQL. No se reasigna el perfil local: el contrato no ofrece ese enlace.
- En navegador, el estado general conserva la evaluación de lecturas de `vital-status.ts`, traducida por `status-copy.ts`. No se llama al núcleo. El cuerpo permanece neutral y se muestra “Local service unavailable”.
- En Tauri, el estado general procede de `status.overall` y el cuerpo exclusivamente de `status.body_zones`. La caja identifica el perfil del dispositivo, muestra DEMO cuando corresponde y conserva la información sobre evaluaciones ausentes, `worst` y acceso a alertas persistentes.
- La línea bajo el cuerpo aclara que las tarjetas son MySQL y las zonas son síntomas del dispositivo. El detalle vuelve a identificar ambas fuentes.
- En navegador, “Live” corresponde a una recepción correcta del feed. En Tauri, corresponde al último estado local confirmado y muestra su hora de evaluación. Un fallo conserva el estado anterior como “Last update”; las alarmas no se degradan por desconexión.
- Las referencias conservan los números originales: “Ref: 50-100 bpm”, “Ref: ≥ 94%”, “Ref: 90-139 / 60-89”, “Ref: 70-180 mg/dL” y “Ref: 36.1 - 38.0 °C”.

## Body map

SVG frontal sin cuadrícula, efectos HUD ni recursos remotos. Geometría curva para cabeza, cuello, tronco y extremidades; izquierda/derecha se refieren al cuerpo de la persona, no al observador.

Las 14 claves están declaradas con `Record<BodyZone, string | null>`:

| BodyZone | Presentación |
| --- | --- |
| head | Cabeza frontal |
| neck | Cuello |
| chest | Pecho |
| abdomen | Abdomen y pelvis |
| left_arm / right_arm | Brazos independientes |
| left_hand / right_hand | Manos independientes |
| left_leg / right_leg | Piernas independientes |
| left_foot / right_foot | Pies independientes |
| back / other | Botones rotulados bajo la figura |

Cada zona tiene `role="button"`, `tabindex="0"`, nombre y estado en `aria-label`, selección mediante `aria-pressed` y activación con Enter/Espacio. Los controles HTML de Back/Other usan activación nativa. “All zones” abre una lista alternativa con objetivos de al menos 44 px, útil para las partes pequeñas del SVG.

La relación de navegación es explícita en `readingLinks`: pulso/SpO₂ → pecho; presión → brazo izquierdo; temperatura/ánimo/sueño → cabeza; glucosa/nutrición → abdomen; contexto → other. Estas asociaciones no son evaluaciones médicas y nunca calculan colores. Una zona sin tarjeta asociada lo indica. Tocar una zona resalta las tarjetas vinculadas; tocar una tarjeta selecciona la zona y abre su detalle.

## Detalle e historial

La hoja muestra la lectura, referencia, etiqueta de estado y hora de recepción MySQL. Explica que el feed no entrega hora de medición. En Tauri consulta estado, perfil e historial del indicador al abrir: evaluación del núcleo, valor local, fecha de medición, resumen, fuente, calificación DEMO y sparkline con tabla de muestras. Un fallo muestra indisponibilidad y no introduce ejemplos.

El panel histórico anterior se reutiliza en “History / On-device record”: estado, perfil, alertas persistentes, detalle de eventos, evaluaciones, síntomas, DEMO y sparklines. Sus consultas periódicas y conservación del último estado confirmado se mantienen. La lista de zonas se trasladó a la principal.

Las sparklines solo usan registros efectivos, fechas interpretables y muestras que el núcleo no marcó como futuras. Un punto se muestra como punto; sin datos se muestra “No history”. En el detalle de presión se representa la sistólica, identificada expresamente como tal. No se extrapolan datos. La segunda pantalla conserva los cinco historiales que ofrecía antes; el detalle permite consultar los demás indicadores.

El hueco “Explain (assistant not loaded)” está en el detalle. No contiene explicaciones generadas ni promete un asistente activo.

## Micrófono y animación

El botón circular “Hold to talk” responde a pointerdown y a mantener Enter/Espacio. La hoja aparece desde abajo con desplazamiento vertical de **48 px a 0 y opacidad de 0 a 1 en 180 ms**, con `cubic-bezier(0.2, 0, 0, 1)`. El fondo oscurecido entra en el mismo tiempo. No hay rebote, brillo, escaneo ni sonido.

Durante la pulsación, nueve barras monocromas varían suavemente su escala vertical en ciclos alternos de 900 ms. Es una excepción acotada al movimiento general del brandbook, solicitada para este flujo: **actividad del botón, no forma de onda de audio**. La propia hoja dice “Button activity only. No audio is being captured.” No se solicita acceso al micrófono, no se captura audio y no se simula una transcripción.

Al soltar aparece “Check what was understood” y “Voice input is not available in this build”, con “Type instead”. Cancelación del puntero, pérdida de foco de ventana y ocultación de la página también terminan la pulsación. Un clic de tecnología asistiva sin secuencia down/up abre directamente esa revisión. Escape, clic exterior y Discard cierran la hoja cuando no hay guardado en curso. Con `prefers-reduced-motion` desaparecen todas las animaciones; las barras quedan estáticas y el flujo funciona igual.

“Type a reading” ofrece acceso directo desde la principal sin mantener pulsado el botón.

## Registro manual

El formulario soporta los indicadores del contrato, incluidos presión con dos campos, sueño con ventana UTC obligatoria y síntomas con una de las 14 zonas. Los valores se dejan vacíos; la hora se inicializa con la hora UTC actual. La validación clínica permanece en Rust.

1. Identifica el perfil local y advierte que es independiente del tripulante MySQL. En navegador, indica que no se registra y deshabilita Save.
2. En Tauri consulta DEMO al abrir y de nuevo antes de guardar. Si DEMO apareció mientras se editaba, muestra el aviso y exige otra activación; no guarda en ese primer intento.
3. Llama a `core.logRecord` con `source: "form"`. Si devuelve `needs_confirmation`, muestra literalmente sus avisos y ofrece “Confirm warnings and save”.
4. La segunda activación envía `confirm_warnings: true` solo para el mismo contenido revisado. Cualquier edición invalida esa confirmación.
5. Bloquea duplicados y cierre durante la operación. Los errores conservan la entrada. Solo `saved` muestra “Reading saved locally”; presenta el estado de sincronización devuelto sin afirmar recepción humana.
6. El guardado real invalida la vista DEMO anterior aunque falle el refresco. Una ayuda confirmada conserva su alarma sin fabricar un perfil o una cola de sincronización cuando faltan esos datos.

No se implementó motor de voz, asistente SLM, sincronización con Tierra ni resolución de alarmas: no existen esas capacidades en el contrato utilizado.

## Conservado

- `fetch("/api/astronauts")` y sondeo cada 2 s, validación de datos, cancelación y conservación del último resultado: `telemetry.ts` intacto.
- Carrusel/modal de tripulantes, selector nativo, flechas, puntos, teclado, Escape, clic fuera y selección por ID: `crew.ts` y `dialog.ts` intactos.
- Funciones originales de estado: `vital-status.ts` intacto; traducciones de `status-copy.ts` intactas.
- Diálogo Need help, aviso DEMO y guardado de eventos: `help.ts` intacto. Se amplió la notificación de su resultado a la principal en `core-panel.ts`.
- Los datos MySQL, nombres, mensajes del núcleo y avisos se insertan con `textContent` o nodos de texto. No se usan como HTML.
- Contraste basado en los pares opacos AA del brandbook, foco visible, zoom disponible, movimiento reducido y alternativa de lista al mapa. Esto no certifica conformidad completa ni sustituye la prueba con lector de pantalla.

## Verificación de esta ronda

Sin procesos, servidor ni navegador iniciados. Se usó Node para lectura/escritura y comprobaciones en memoria.

- API de TypeScript instalada, con el `tsconfig.json` real y `noEmit`, `strict`, `noUnusedLocals`, `noUnusedParameters`: sin diagnósticos.
- **37 comprobaciones** con DOM y servicios simulados en memoria: 14 zonas, selección Enter/Espacio, color exclusivo del núcleo, pulsación pointer/teclado, revisión honesta, navegador sin llamadas Rust, aviso DEMO inicial y sobrevenido, confirmación de avisos e invalidación por edición, guardado confirmado, bloqueo de duplicados, conservación de entradas ante fallo, Escape, puntos/deslizamiento, distinción frente a scroll vertical, detalle con referencia, navegación recíproca, inserción textual de nombres y preservación de alarmas/retirada de DEMO ante fallo.
- Parseo de CSS con PostCSS; IDs HTML únicos; comprobación de selectores estáticos y destinos ARIA.
- Comparación de contenido de archivos conservados y hashes de los protegidos leídos. No se abrió `vite.config.ts`; no se accedió a `crates/` ni `src-tauri/`.

Las comprobaciones con dobles verifican lógica, **no renderizado, gestos nativos reales, lector de pantalla ni Tauri**. Los datos usados en esas comprobaciones no se escribieron en la app ni en MySQL.

## Pendiente para el equipo

Ejecutar `npx tsc --noEmit`, build y app real. Revisar 375/390/430/900/1440 px, nombres y valores largos, estados de varias líneas, zoom 200 %, orientación, teclado virtual y viewport bajo. Comprobar la proporción final del SVG, contraste renderizado y foco con teclado/lector de pantalla. Probar pulsación y cancelación en WebView, deslizamiento y `prefers-reduced-motion`. Verificar MySQL real y el ciclo completo de Tauri con DEMO, datos reales, avisos de plausibilidad, fallos de almacenamiento y pérdida/recuperación del servicio.

## Archivos cambiados

- `ui/index.html`, `ui/src/main.ts`, `ui/src/styles.css`.
- `ui/src/ui/body-map.ts`, `core-panel.ts`, `sparkline.ts`, `vitals.ts`.
- Nuevos: `ui/src/ui/dashboard.ts`, `ui/src/ui/voice.ts`.
- `docs/front/rediseno.md`.

No se modificaron tipos, contratos, reglas de evaluación, dependencias, configuración, backend ni archivos de marca.
