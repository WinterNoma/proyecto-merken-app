# Ajuste acotado: cuerpo humano continuo

**Histórico: este intento fue reemplazado por la [figura generada y recortada](cuerpo-generado.md), a petición del usuario. El SVG se conserva como comparación, no como referencia elegida.**

El usuario pidió aproximar la figura a la referencia híbrida, con un cuerpo integrado y humano, conservando la selección por zonas. Aclaró además que el producto se entrega exclusivamente como app móvil: la vista de navegador es una herramienta de desarrollo.

## Cambio

`ui/src/assets/body-human.svg` es el recurso vectorial completo y reutilizable. Tiene una sola silueta exterior, manos, cuello y pies conectados, proporciones más alargadas y sombreado suave por degradados. No depende de imágenes raster, filtros de desenfoque ni recursos remotos.

`body-map.ts` lo incorpora y coloca doce áreas transparentes sobre el frente, recortadas a la silueta. Las fronteras no son visibles en reposo. La selección y el foco resaltan la región; los colores de evaluación siguen viniendo únicamente del núcleo. Back y Other conservan sus controles y las catorce zonas permanecen en All zones.

## Comprobación

- Build de TypeScript y Vite correcto; diff sin errores de whitespace.
- SVG válido y referencias internas verificadas.
- Las doce regiones del frente se seleccionaron mediante clic en navegador; Chest también mediante Enter y mantuvo los enlaces a Pulse/SpO₂.
- Revisión móvil a 320, 375 y 420 px, sin desbordamiento horizontal observado.
- Sin cambios a captura de audio, efectos de fondo, servicios médicos o contratos Rust.
- Sin dependencias nuevas. Aún pendiente la comprobación en Android físico.

Capturas: [cuerpo dentro de la app](cuerpo-continuo-375.jpg) y [detalle de la figura y las tarjetas](cuerpo-continuo-detalle.jpg).

El SVG también se copia en `entrega-diseno-merken/imagenes/14-cuerpo-humano-continuo.svg`; la guía compartida recoge el alcance móvil y esta forma de representar las zonas.
