# Revisión del frontend de Pulsys / Merken

La revisión detectó problemas confirmados de coherencia entre lecturas, estados y datos guardados.
Los más graves permiten conservar “Stable” tras guardar una alarma, mostrar valores imposibles como normales y ocultar historial local al iniciar sin conexión.
También hay errores al cambiar filtros, recuperar gráficas y descartar formularios.
La composición móvil tiene texto muy pequeño y algunas zonas táctiles inferiores a 44 px; la integración del feed y la exportación requieren verificación en la app instalada.

**Alcance y evidencia:** revisión en solo lectura del código indicado, las pruebas y los documentos de contexto. Se reprodujeron casos mediante módulos y DOM simulado en memoria, sin comandos nativos ni escritura de datos. No se ejecutó la suite completa ni se verificó el renderizado en un teléfono; no se certifican contraste renderizado, gestos, teclado virtual ni ausencia de desbordamientos.

## Hallazgos, de mayor a menor gravedad

### 1. Alta — Una alarma recién guardada puede dejar el resumen en “Stable”

**Archivos:** `ui/src/ui/core-panel.ts:264`, `ui/src/ui/core-panel.ts:293`, `ui/src/ui/dashboard.ts:48`.

**Qué pasa:** `acceptRecord()` actualiza la evaluación individual, pero conserva el agregado anterior en `current`. Pasa la alarma confirmada al dashboard, aunque este prioriza `status.overall` siempre que exista.

**Por qué falla:** después de guardar una lectura con alarma, si falla `getStatus()`, puede quedar la tarjeta individual en alarma y el resumen principal en “Stable”. Se reprodujo en memoria: el callback entregó simultáneamente `overall: stable` y `confirmedAlert: alarm`.

**Arreglo propuesto:** invalidar explícitamente el resumen anterior tras una mutación y mostrar la alarma confirmada mientras se actualiza el agregado. Separar las alarmas de lecturas de las alertas persistentes por eventos: actualmente ambas pueden acabar activando “View persistent alerts”.

### 2. Alta — Valores imposibles reciben estados favorables

**Archivos:** `ui/src/ui/telemetry.ts:146`, `ui/src/ui/telemetry.ts:120`, `ui/src/vital-status.ts:42`, `ui/src/vital-status.ts:77`, `ui/src/ui/status-copy.ts:26`.

**Qué pasa:** el feed admite números finitos sin comprobar límites físicos o de dominio. `oxygenStatus()` no tiene límite superior y `sleepStatus()` considera adecuado cualquier valor igual o superior a seis.

**Por qué falla:** se reprodujeron estos resultados:

- SpO₂ de **101 %** → **“In range”**, verde.
- Sueño de **30 h** → **“Adequate rest”**, verde.
- Ambos pasan la validación previa a persistencia sin exclusiones.

**Arreglo propuesto:** distinguir datos inválidos de lecturas válidas fuera de rango. Validar el dominio antes de clasificar y mostrar “Invalid reading” o “Not evaluated” cuando corresponda. Usar límites compartidos con el contrato, sin añadir otra colección independiente de reglas clínicas.

### 3. Alta — El historial local depende de haber recibido la tripulación por red

**Archivos:** `ui/src/main.ts:51`, `ui/src/main.ts:70`, `ui/src/ui/telemetry-history.ts:615`, `ui/src/ui/telemetry-history.ts:675`.

**Qué pasa:** las opciones del historial proceden exclusivamente de `state.crew`. Al iniciar sin conexión, esa lista está vacía; sin `selectedCrewId`, el componente no consulta SQLite.

**Por qué falla:** puede haber lecturas guardadas y, aun así, aparecer **“No readings stored yet”**. Se reprodujo con una fuente local simulada que tenía datos: no hubo ninguna consulta. También quedan inaccesibles los tripulantes que desaparezcan del feed.

**Arreglo propuesto:** obtener el catálogo de tripulantes desde los registros locales y complementarlo con el feed. Diferenciar “No crew list available” de “No readings stored yet”; la segunda afirmación requiere consultar el almacenamiento.

### 4. Alta — El endpoint del feed depende del origen web — **a verificar**

**Archivo:** `ui/src/ui/telemetry.ts:486`.

**Qué pasa:** la única carga del feed usa `fetch("/api/astronauts")`, una URL relativa al origen que sirve la interfaz.

**Por qué puede fallar:** el código revisado no selecciona un endpoint para la aplicación móvil empaquetada. Si `/api` funciona mediante un proxy de desarrollo, ese comportamiento no queda garantizado en el origen de producción de Tauri. Los comandos registrados en `src-tauri/src/lib.rs:474` tampoco proporcionan ese feed directamente.

**Arreglo propuesto:** verificar la petición en una instalación de producción y definir explícitamente su transporte y URL. No basta con comprobarla mediante Vite. **No se declara roto el build móvil sin revisar su configuración y ejecución.**

### 5. Media — “Live” puede referirse al núcleo local mientras las tarjetas muestran un feed desconectado

**Archivos:** `ui/src/ui/vitals.ts:72`, `ui/src/ui/dashboard.ts:56`, `ui/src/main.ts:74`.

**Qué pasa:** `vitals.feed()` escribe la frescura del feed en `#feed-status`; inmediatamente después, `dashboard.feed()` vuelve a llamar a `overall()`, que en Tauri sustituye ese texto por la hora de evaluación local.

**Por qué no tiene sentido:** el dashboard puede mostrar **“Live”** y, a la vez, el aviso **“Connection unavailable. Showing the last successfully received readings.”** Los valores antiguos conservan además sus etiquetas favorables. La documentación describe esta separación, pero la pantalla reutiliza un mismo indicador visual para dos fuentes.

**Arreglo propuesto:** separar “Crew feed received…” de “Device evaluation updated…”. Mantener visible la antigüedad del feed junto a sus tarjetas y distinguir su última evaluación de una medición vigente.

### 6. Media — Al cambiar de astronauta permanecen visibles los valores del anterior

**Archivos:** `ui/src/ui/telemetry-history.ts:699`, `ui/src/ui/telemetry-history.ts:815`, `ui/src/ui/telemetry-history.ts:823`.

**Qué pasa:** cambiar los selectores invalida respuestas pendientes, pero no limpia ni identifica el contenido ya dibujado. La siguiente consulta solo añade “Loading history…” y `aria-busy`.

**Por qué falla:** durante una consulta lenta, el selector dice B mientras la gráfica y la lista todavía pertenecen a A. Se reprodujo en memoria. El mismo problema ocurre al cambiar de indicador.

**Arreglo propuesto:** retirar el contenido anterior al cambiar el ámbito, o conservarlo con una identificación inequívoca de su astronauta e indicador. No presentarlo bajo los nuevos filtros.

### 7. Media — Cambiar la ventana de una gráfica oculta un error de refresco

**Archivo:** `ui/src/ui/telemetry-trend.ts:100`.

**Qué pasa:** `reload()` limpia el cuerpo, pero conserva `items` cuando falla. Después habilita el selector, cuyo listener llama directamente a `render()`.

**Por qué falla:** tras “Could not load saved crew readings”, seleccionar otra ventana vuelve a pintar los datos antiguos y reemplaza el error por “N saved readings”. Se reprodujo sin una nueva consulta exitosa.

**Arreglo propuesto:** conservar un estado explícito de error o datos antiguos. Cambiar la ventana no debe eliminar ese estado; alternativamente, vaciar `items` al fallar y mantener disponible el reintento.

### 8. Media — Un guardado confirmado sigue bloqueando el cierre por consultas secundarias

**Archivo:** `ui/src/ui/voice.ts:204`.

**Qué pasa:** después de recibir `saved` y mostrar “Reading saved locally”, el formulario espera `await panel.refresh(true)`. Solo después libera `busy` y habilita Close.

**Por qué falla:** el usuario no puede salir de un guardado ya terminado mientras se actualizan estado, perfil e historiales. Una consulta secundaria que no resuelva deja el diálogo bloqueado; `modal()` también impide cerrarlo con Escape o desde fuera.

**Arreglo propuesto:** habilitar el cierre al confirmar el guardado y refrescar el panel en segundo plano. Mantener deshabilitada la repetición del guardado, pero separar esa protección de la navegación.

### 9. Media — “Discard” no descarta el borrador manual

**Archivos:** `ui/src/ui/voice.ts:68`, `ui/src/ui/voice.ts:110`, `ui/src/ui/voice.ts:128`.

**Qué pasa:** cerrar el diálogo cancela el dictado e incrementa `opening`, pero no borra valores, nota, hora ni confirmación pendiente. `resetSaved()` solo limpia cuando ya hubo un guardado exitoso.

**Por qué no tiene sentido:** tras pulsar **“Discard”**, volver a abrir y elegir el formulario recupera la lectura anterior. La hora tampoco se actualiza porque conserva un valor, con riesgo de registrar una nueva medición con la fecha del borrador descartado.

**Arreglo propuesto:** implementar el descarte real del borrador y sus confirmaciones. Si se quiere conservarlo, llamar al botón “Close” y hacer explícita esa conservación.

### 10. Media — La exportación termina en una ruta sin una salida móvil utilizable — **a verificar**

**Archivos:** `ui/src/ui/telemetry-history.ts:847`, `ui/src/ui/telemetry-history.ts:873`; contexto: `src-tauri/src/lib.rs:391`.

**Qué pasa:** el archivo se escribe en `app_data_dir()/exports`. El frontend muestra la ruta y, cuando existe Clipboard API, ofrece únicamente “Copy path”.

**Por qué puede fallar:** copiar una ruta interna no proporciona un flujo para recuperar o compartir el JSON en un teléfono. No hay selector de destino, acción de compartir ni apertura del archivo. Además, un fallo de copia se silencia.

**Arreglo propuesto:** ofrecer guardado en un destino elegido o compartir mediante la plataforma, y comunicar el resultado de copiar. **Debe verificarse la accesibilidad real de esa ruta en Android/iOS; no se comprobó en dispositivo.**

### 11. Media — Los anuncios urgentes están dentro de una pantalla que se vuelve inaccesible

**Archivos:** `ui/index.html:55`, `ui/src/ui/page-navigation.ts:37`, `ui/src/ui/core-panel.ts:143`.

**Qué pasa:** `#alarm-announcement`, con `role="alert"`, está dentro de History. Cuando Overview está activo, History recibe `inert` y `aria-hidden="true"`.

**Por qué falla:** la región destinada a anunciar nuevas alarmas queda excluida del árbol accesible. El resumen visible puede anunciar un cambio a “Alarm”, pero no sustituye el anuncio de una nueva alerta cuando el resumen ya estaba en ese estado.

**Arreglo propuesto:** colocar la región global de anuncios fuera de las pantallas ocultables. Verificar recepción y ausencia de duplicados con TalkBack y VoiceOver.

### 12. Media — La zona táctil del cuello no alcanza 44 px

**Archivos:** `ui/src/ui/body-map.ts:13`, `ui/src/ui/body-map.ts:81`, `ui/src/styles.css:170`, `ui/src/hybrid.css:374`.

**Qué pasa:** el cuello usa una región de 44 × 19 unidades y una ampliación por trazo de 36 unidades. A 390 px, la columna central mide aproximadamente 120,9 px para un SVG de 180 unidades de ancho.

**Por qué falla:** incluso contando toda la ampliación, la altura máxima calculada ronda **37 px**, antes de considerar las regiones vecinas que se superponen. La lista “All zones” ofrece una alternativa, pero no corrige el objetivo directo del mapa.

**Arreglo propuesto:** proporcionar áreas táctiles efectivas de al menos 44 × 44 px con arbitraje entre zonas, o reservar las zonas pequeñas para la lista accesible. Verificar la geometría efectiva en el WebView.

### 13. Media — Información importante queda en texto de 10 px

**Archivos:** `ui/src/hybrid.css:369`, `ui/src/hybrid.css:384`, `ui/src/hybrid.css:401`, `ui/src/hybrid.css:257`.

**Qué pasa:** a 390 px y con tamaño raíz estándar, unidades, estados de las tarjetas, procedencia, frescura y textos del detalle usan 10 px; varios controles y etiquetas usan 11 px.

**Por qué es problemático:** la información que permite interpretar correctamente una lectura queda entre los textos más pequeños. Esto incluye distinguir unidades, fuentes y estados. El tamaño está confirmado por CSS; la legibilidad efectiva y el contraste sobre transparencias están **a verificar** visualmente.

**Arreglo propuesto:** aumentar la tipografía informativa y permitir que el contenido crezca verticalmente. Priorizar unidades, estados y procedencia antes que mantener una composición compacta.

## Cosas raras / sin sentido

### 14. Baja — Las referencias escritas no coinciden exactamente con las funciones

**Archivos:** `ui/index.html:34`, `ui/index.html:38`, `ui/src/vital-status.ts:52`, `ui/src/vital-status.ts:68`.

**Qué pasa y por qué:** **38,05 °C** obtiene “In range”, aunque el detalle dice `Ref: 36.1 - 38.0 °C`. También **139,5/89,5 mmHg** se considera normal mientras la referencia termina en `139 / 89`. El feed admite decimales.

**Arreglo propuesto:** generar el texto desde los mismos límites o expresar las desigualdades reales. Si el dominio exige determinada precisión, validarla explícitamente.

### 15. Baja — La demo genera SpO₂ de hasta 102 %

**Archivo:** `ui/src/ui/visual-preview.ts:9`.

**Qué pasa y por qué:** se aplica el mismo patrón aditivo a todos los indicadores. Para A. Morgan, los 98 % iniciales producen muestras de 101 % y 102 %. Se confirmó en memoria.

**Arreglo propuesto:** usar fixtures específicas por indicador. Los datos están correctamente marcados como sintéticos, pero no deberían introducir incoherencias accidentales en una demostración.

### 16. Baja — El detalle conserva un hueco visible de funcionalidad pendiente

**Archivo:** `ui/index.html:100`.

**Qué pasa y por qué:** todos los detalles muestran **“Explain (assistant not loaded)”** dentro de un `div` sin acción. Está previsto expresamente en la documentación, por lo que no es un botón roto; aun así, expone un pendiente de implementación en la interfaz final.

**Arreglo propuesto:** ocultarlo hasta que exista la función o presentar una explicación breve de disponibilidad si aporta información útil.

### 17. Baja — Mood cambia de idioma o pierde información según dónde se consulte

**Archivos:** `ui/src/ui/vitals.ts:20`, `ui/src/ui/telemetry-history.ts:129`, `ui/src/ui/telemetry-trend.ts:25`.

**Qué pasa y por qué:** la tarjeta traduce vocabulario conocido y sustituye cualquier otro texto por “Recorded”; los historiales muestran la etiqueta original. Un valor `triste` aparece como “Sad” en Overview y como `triste` en History; un término inglés no contemplado también pierde su contenido en la tarjeta.

**Arreglo propuesto:** unificar la presentación y distinguir la traducción del texto original registrado. Conservar el dato original sin convertirlo accidentalmente en texto de interfaz sin contexto.

### 18. Baja — Las escalas manuales no explican qué significan sus extremos

**Archivo:** `ui/src/ui/voice.ts:53`.

**Qué pasa y por qué:** el formulario dice “Mood score (0–10)” y “Symptom score (0–10)”, pero no explica que una puntuación alta significa mejor ánimo y peor síntoma. Además, todos los campos numéricos usan `step="any"`, aunque esas puntuaciones deben ser enteras.

**Arreglo propuesto:** mostrar `0 = very bad / 10 = very good` y `0 = none / 10 = worst`; configurar `min`, `max` y `step="1"` para esas escalas.

### 19. Baja — El alcance de Export no coincide visualmente con los filtros que lo rodean

**Archivo:** `ui/src/ui/telemetry-history.ts:856`.

**Qué pasa y por qué:** el botón está junto a los filtros de astronauta e indicador, pero solo envía `include_telemetry`; no envía `astronaut_key`. Exporta el ámbito general del dispositivo, sin aclararlo antes de ejecutarse.

**Arreglo propuesto:** etiquetarlo como **“Export all device data”** y describir su alcance, o ofrecer una exportación explícitamente filtrada. No asumir que elegir un astronauta filtra el archivo.

### 20. Baja — Restos claros del flujo de voz anterior

**Archivos:** `ui/src/styles.css:206`, `ui/src/styles.css:221`, `ui/src/vital-status.ts:90`.

**Qué pasa y por qué:** permanecen `.voice-wave`, `#voice-held` y `held-wave`, sin elementos que los utilicen en el HTML/TypeScript revisado. `OVERALL_LABEL` tampoco tiene consumidores.

**Arreglo propuesto:** retirar esos restos cuando se autorice editar. Las cadenas españolas de `vital-status.ts` sí usadas por `status-copy.ts` no deben confundirse con una fuga general de español: actualmente se traducen antes de mostrarse.