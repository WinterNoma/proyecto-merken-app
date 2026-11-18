# Concepto de pantalla principal (14-nov-2026)

Acordado con el usuario a partir de las maquetas de Claude. Es la base de la siguiente ronda de Astra.

## Principio

Una sola pantalla desde la que se maneja todo el panel. Puede haber pantallas secundarias que se abren deslizando (los puntos de abajo), pero lo principal se resuelve aquí. Nada de interfaz recargada.

## Distribución (móvil, de arriba abajo)

1. **Barra superior:** logotipo PULSYS, selector de tripulante (abre el carrusel) y botón **Need help**, siempre visible y con diálogo de confirmación.
2. **Estado general:** una caja con la barra lateral del color de estado, el texto del estado («Change detected»…) y la frescura («Live · hh:mm UTC» o «Last update…»).
3. **Centro: body map** con las tarjetas de datos **en cajas, como en el brandbook** (superficie, borde, barra de estado de 3 px, etiqueta, valor en Roboto Mono y estado con icono y texto), a los dos lados del cuerpo:
   - izquierda: pulso, presión, SpO₂ y ánimo;
   - derecha: temperatura, glucosa, sueño y nutrición.
   - Al tocar una zona del cuerpo se resaltan sus tarjetas; al tocar una tarjeta se resalta su zona. El «Explain» del SLM saldría de ahí, cuando exista.
4. **Contexto** en cajas pequeñas: radiación, distancia y sincronización. No llevan color de estado.
5. **Micrófono debajo del cuerpo:** un botón redondo «Hold to talk». Al mantenerlo pulsado aparece desde abajo una hoja de grabación con la onda. Al soltar, muestra «Check what was understood», con lo interpretado y los botones «Discard» y «Save N readings». **La animación de aparición se define con Astra.**
6. **Puntos de paginación** para las pantallas secundarias (historial, registros, asistente…), sin pestañas fijas.

## Body map bien implementado

- La silueta es SVG limpia y proporcionada, sin cuadrícula ni efectos HUD.
- Las zonas se alinean con las 14 de `body_zones` del núcleo (`crates/core`): cabeza, cuello, pecho, abdomen, espalda, brazos, manos, piernas, pies y otros. Se pueden agrupar visualmente, pero el mapeo es explícito.
- Cada zona es un control accesible: `role="button"`, nombre y estado en `aria-label`, y navegable con teclado.
- No se mezclan las fuentes del color: en Tauri, el color de la zona viene de `status.body_zones` (síntomas); en el navegador, la relación tarjeta ↔ zona es solo de navegación y no pinta estados que el núcleo no dio.

## Escritorio (≥ 900 px)

La misma composición, más ancha. El cuerpo y las tarjetas en el centro; las pantallas secundarias pueden verse como columna lateral.
