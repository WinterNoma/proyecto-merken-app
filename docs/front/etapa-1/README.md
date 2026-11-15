# Etapa 1 — base visual híbrida

**Estado más reciente:** [etapa 2: partículas y micrófono centrado](../etapa-2/README.md), sobre la [adaptación móvil a una pantalla](adaptacion-movil/README.md). El resto de este archivo documenta la primera iteración histórica.

Primera entrega acotada después de acordar avanzar por partes. Se conserva la rama `codex/hybrid-reactive-ui` sobre el rediseño integrado de `desarrollo`.

## Alcance implementado

- Material oscuro de vidrio, fondo abstracto estático y jerarquía visual del híbrido sin paisaje.
- Cabecera, título, tarjetas con iconos, panel corporal, contexto y micrófono en cápsula.
- El SVG mantiene sus 14 zonas y estados del núcleo; el acabado neutral usa un degradado propio.
- Misma selección de tripulante, detalles, lista de zonas, historial, ayuda y formulario manual.
- Hoja de estilos de esta iteración en `ui/src/hybrid.css`, separada de los estilos funcionales existentes.
- Vista de desarrollo en `http://127.0.0.1:1420/?preview=1`, con dos tripulantes ficticios, aviso visible de DEMO y valores fijos. No consulta MySQL ni invoca el núcleo. No guarda registros. Las fichas sintéticas se eliminan del JavaScript de producción mediante el indicador de Vite DEV.
- Sin dependencias nuevas.

## Verificación realizada

- `npm run build`: TypeScript y Vite correctos.
- `git diff --check`: correcto.
- Inspección de JavaScript de producción: no contiene los IDs ni nombres de los tripulantes sintéticos.
- Navegador: vista a 320, 375 y 420 px, y escritorio; sin desbordamiento horizontal observado. Se conserva el desplazamiento vertical.
- Las ocho tarjetas siguen visibles y los botones HTML visibles alcanzan al menos 44 px en la vista de 320 px. Las partes pequeñas del SVG mantienen la alternativa All zones; esto no es una certificación de accesibilidad completa.
- Cambiar a J. Rivera modifica pulso, presión y demás lecturas; abrir el detalle identifica la muestra como sintética.
- Seleccionar Chest desde All zones marca la zona y resalta Pulse/SpO₂; las zonas permanecen sin evaluación en el navegador.
- Formulario manual y ayuda se abren y mantienen deshabilitado el guardado sin núcleo.
- No se observaron errores o advertencias en la consola del navegador durante el recorrido.

Capturas: [móvil 375](movil-375.jpg) y [escritorio](escritorio.jpg).

El build final informa aproximadamente 49.39 kB JS (16.73 kB gzip) y 35.44 kB CSS (8.03 kB gzip), frente a 47.45/16.04 kB JS y 24.88/5.80 kB CSS antes de esta etapa. Son tamaños de build, no mediciones de memoria o consumo. Las fuentes locales no cambiaron.

## Límites y próximos pasos

Esta entrega no añade fondo animado, partículas reactivas, captura de audio ni interpretación de temas. El comportamiento previo del botón de voz sigue informando que no captura audio. No se modificaron Rust, evaluación médica ni contratos de guardado.

Tauri real, Android físico, rendimiento sostenido, consumo y contraste completo quedan sin verificar en esta etapa. El cuerpo conserva la geometría SVG existente; las imágenes generadas siguen siendo dirección estética, no una reproducción idéntica del volumen 3D.

Orden propuesto para continuar:

1. Revisar esta composición y sus proporciones.
2. Añadir únicamente el fondo animado y medirlo contra esta base.
3. Añadir reacción a pulsación del micrófono y selección de zonas.
4. Integrar audio real y después reacción semántica cuando exista transcripción.

No se publicó, subió ni hizo commit de esta entrega.
