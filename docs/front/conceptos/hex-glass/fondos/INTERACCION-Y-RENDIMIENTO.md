# Particulas reactivas a voz y tema

## Direccion acordada

Base visual provisional: hibrido, sin paisaje rocoso. El usuario propone como efecto extra que las particulas reaccionen al tema del que habla y pide evaluar el costo. Se conserva la composicion exterior del hibrido. La [etapa 2](../../../etapa-2/README.md) ya implementa Canvas 2D, reaccion a pulsacion/seleccion y una primera muestra de costo en escritorio. El resto de este documento orienta la futura reaccion a audio y temas: captura, transcripcion y clasificador siguen pendientes.

## Dos senales independientes

- **Audio:** la energia de la voz modifica suavemente la amplitud y velocidad del movimiento. Web Audio / AnalyserNode puede obtener datos del audio cuando exista una captura real compatible con la plataforma.
- **Tema:** un segmento de transcripcion suficientemente estable propone una categoria visual. La categoria modifica el recorrido o la concentracion local de las particulas mediante una transicion gradual. No se ejecuta interpretacion por fotograma.

El analizador de audio no entiende temas. La respuesta semantica depende de la transcripcion y su latencia; si no hay parciales, solo podra actualizarse despues de transcribir al soltar el microfono. La captura, la transcripcion y el interprete siguen pendientes de integrar en Pulsys.

## Ejemplos para comparar en un prototipo

| Tema | Movimiento propuesto |
| --- | --- |
| Sueno o descanso | Ondas amplias con recorrido lento |
| Actividad o ejercicio | Flujo algo mas dirigido, con intensidad acotada |
| Animo | Formas suaves que se expanden y se agrupan |
| Zona corporal mencionada | Ligera concentracion junto a la zona correspondiente, sin tapar la figura |
| Tema desconocido o ambiguo | Mantener patron neutral |

Estos ejemplos son opciones de diseno, no reglas definitivas. Los colores permanecen neutrales y los cambios no asignan estados de salud. No se registran mediciones ni se confirman campos mediante este efecto.

## Arquitectura propuesta

1. Motor visual Canvas 2D con un conjunto pequeno y reutilizable de particulas.
2. Energia de audio suavizada para evitar saltos.
3. Adaptador de temas desde la transcripcion existente: comenzar con vocabulario acotado y reutilizar el interprete del registro cuando este disponible. Evitar cargar un modelo adicional solo para el fondo.
4. Una interfaz de estado visual: modo (reposo/pulsacion/captura/procesamiento), energia y tema provisional. El renderizador no accede a datos clinicos ni ejecuta inferencia.
5. Cambio de tema gradual, con persistencia suficiente para evitar cambios por cada palabra. No etiquetar como captura real un simple efecto de pulsacion.
6. Considerar OGL/shader solo si la comparacion visual y las mediciones justifican evolucionar desde Canvas 2D.

## Comparacion de costo pendiente

Medir en el mismo dispositivo y con la misma sesion:

1. Interfaz sin efectos.
2. Interfaz con particulas y temas simulados, sin captura.
3. Interfaz con particulas y captura/analisis de audio.
4. Mismo flujo con transcripcion e interpretacion reales, cuando existan.

Registrar tamano agregado al build, memoria del proceso, tiempos por fotograma (p50/p95), fotogramas lentos, CPU/GPU, respuesta del boton, latencia voz-a-tema y estabilidad de la transcripcion bajo carga grafica. Para consumo y temperatura, realizar sesiones sostenidas comparables en Android; no inferir gasto de bateria desde FPS o una prueba breve de escritorio. Medirlo todo con audio sintetico o consentido; las metricas no requieren guardar audio ni texto del usuario.

Punto de partida por evaluar: unas decenas de particulas, resolucion de dibujo acotada y objetivo inicial de 30 FPS para el fondo. Son parametros de prototipo, no resultados ni garantias. El marco de la interfaz puede refrescarse a otra frecuencia.

Reducir automaticamente cantidad/resolucion si el efecto interfiere con la interaccion o la captura. Pausar al ocultar la pantalla y respetar movimiento reducido. No animar blur de pantalla completa de forma continua sin medir su costo.

## Referencias tecnicas

- [Web Audio AnalyserNode](https://developer.mozilla.org/en-US/docs/Web/API/AnalyserNode)
- [requestAnimationFrame](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame)
- [Consejos de rendimiento de PixiJS](https://pixijs.com/8.x/guides/concepts/performance-tips): referencia de costos de escena, filtros y resolucion; no implica haber elegido PixiJS.
