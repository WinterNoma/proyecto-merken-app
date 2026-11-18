# Figura generada y recortada — iteración actual

El usuario rechazó el acabado del cuerpo SVG dibujado y pidió generar una imagen recortada más cercana a la figura de la referencia híbrida. Esta iteración sustituye la ilustración visible anterior; el producto sigue siendo exclusivamente móvil.

## Archivos

- `cuerpo-generado-recortado.png`: PNG RGBA de 664 × 1594 px, con transparencia real, para revisar o reutilizar. El recorte mantiene las proporciones y deja margen junto a manos y pies.
- `ui/src/assets/body-reference.webp`: imagen que carga la app, 106292 bytes; conserva transparencia. No requiere ninguna dependencia de ejecución nueva.
- `ui/src/assets/body-reference-geometry.json`: contorno obtenido del canal alfa, utilizado únicamente para recortar selección y estados. No vuelve a dibujar la ilustración.
- `ui/src/assets/body-human.svg`: intento anterior conservado para comparación, ya no se usa como ilustración de la app.

La generación y la eliminación de fondo se hicieron con image_gen integrado. Después se recortaron los márgenes transparentes con FFmpeg (`crop=664:1594:161:0`), sin estirar el cuerpo, y se codificó WebP con calidad 90. El PNG de origen se conserva bajo el directorio generado de Codex.

La visualización usa el bitmap dentro de un SVG con áreas de interacción transparentes. Las doce zonas del frente se alinearon a la nueva figura; Back y Other conservan sus controles. La selección se recorta al contorno real de la imagen. Los estados de salud siguen viniendo del núcleo.

## Comprobación

- Build de TypeScript y Vite correcto; diff sin errores de whitespace.
- PNG con canal alfa entre 0 y 255; WebP conserva alfa.
- Las doce regiones del frente se seleccionaron mediante clic en navegador; Chest también mediante Enter.
- Revisión en tamaños móviles de 320, 375 y 420 px, sin desbordamiento horizontal observado.
- Capturas: [pantalla móvil](cuerpo-generado-375.jpg) y [detalle de figura y tarjetas](cuerpo-generado-detalle.jpg).
- No se añadieron efectos de fondo, captura de audio ni interpretación. Android físico sigue pendiente.

## Prompt completo

```text
Use case: background-extraction / product-mockup asset. The provided image is a PULSYS app mockup. ISOLATE AND RECREATE AT HIGH RESOLUTION ONLY THE CENTRAL FULL-BODY HUMAN FIGURE FROM THAT EXACT REFERENCE. This is an asset extraction, not a new app screenshot. Output a single transparent-background PNG cutout, alpha channel, with NO interface, NO text, NO metric cards, NO buttons, NO background, NO ground or cast shadow, NO bounding box, NO checkerboard painted into the image. Keep the look of the reference central figure very closely: a continuous anatomically plausible human form with natural proportions, smooth smoky translucent charcoal-gray material, a soft satin surface, subtle thin pearly silver rim lighting, soft volumetric shading, restrained dimensional depth. Front-facing orthographic view, upright natural relaxed stance, head looking forward, arms angled slightly away from the waist, elbows relaxed, hands open and fingers pointing down, feet a small distance apart. Preserve the slim natural silhouette, gently shaped shoulders, chest, waist, pelvis, thighs, calves and feet from the reference. It must look like the figure IN THE REFERENCE, not a new robotic mannequin. Neutral adult body, non-sexual educational medical-app visualization, no nipples or genital detail, no clothing, no hair, smooth understated facial structure without eyes drawn on. Seamless neck and limb transitions. No joints, no armor, no mechanical panels, no outlines around body parts, no muscle chart, no skeleton, no halos or large glow. Fully include crown, all fingers and feet with only a small transparent safety margin; figure should occupy roughly 90% of canvas height. This is a high quality rendered bitmap meant to be composited over the dark mobile app, not hand-drawn SVG art. Transparent background is essential. Retain natural proportions without stretching.
```
