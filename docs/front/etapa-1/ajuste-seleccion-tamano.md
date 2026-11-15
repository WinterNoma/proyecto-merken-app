# Selección sin marco y figura más grande

El usuario pidió eliminar el recuadro transitorio de la selección y aumentar un poco la figura, aprovechando mejor el espacio del panel móvil.

- Las zonas del cuerpo no dibujan stroke ni outline en selección o foco. El resaltado se obtiene solo mediante un tinte recortado al cuerpo.
- Se desactiva `-webkit-tap-highlight-color` en el mapa y las zonas para evitar el rectángulo del navegador al tocar. La respuesta durante pulsación y el foco de teclado conservan un tinte visible.
- La figura y sus regiones aumentan juntas, sin estirar la imagen. A 375 px de viewport, su ancho pasa de 93.75 a 120 px, aproximadamente un 28 % más.
- Las tarjetas comparten filas entre ambos lados, con altura según el contenido de cada par. Esto evita que todas hereden el alto de la etiqueta más larga y reduce espacio vacío conservando la alineación.
- La misma imagen WebP y el mismo mapa de zonas siguen en uso; no se añadieron dependencias ni cambios al registro de salud.

Verificación: build correcto; revisión de desbordamiento y textos a 320 y 375 px en navegador. Selección por clic y Enter conserva el estado y tiene `stroke: none` y `outline: none`. Comprobada la alineación de las cuatro parejas de tarjetas. La pulsación táctil en Android físico sigue pendiente de prueba.

Captura: [figura ampliada y pecho resaltado](cuerpo-ampliado-sin-marco-detalle.jpg).
