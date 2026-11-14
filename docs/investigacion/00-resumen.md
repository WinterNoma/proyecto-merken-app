# Resumen: problemas de salud de astronautas

Investigación de Terra (14-nov-2026). Detalle y fuentes en los archivos 01–06.

## Lo más frecuente

| Problema | Frecuencia | Archivo |
|---|---|---|
| Cefalea (primeros días) | hasta 90 % | 06 |
| Mareo espacial / náuseas | hasta 73 % los primeros 2–3 días | 03, 06 |
| SANS (cambios en el ojo) | ~70 % | 03, 06 |
| Dolor lumbar | 52 % los primeros días | 03, 06 |
| Sueño corto | ~6 h por noche | 02, 06 |
| Pérdida de masa corporal 5–10 % | 45 de 79 astronautas | 05 |
| Reactivación de herpesvirus | 14 de 23 (casi siempre sin síntomas) | 04 |
| Erupciones en la piel | 1,12 episodios/año de vuelo | 04, 06 |
| Síntomas respiratorios | 0,97 episodios/año de vuelo | 04, 06 |
| Pérdida ósea | 1–1,5 % al mes en huesos de carga | 03 |

## Hallazgo clave para el diseño

Casi ningún problema tiene un umbral oficial de alarma. Los únicos claros: CO₂ ≤ 3 mmHg de promedio horario y radiación ≤ 600 mSv en la carrera. Las señales de alarma concretas son eventos: coágulo detectado, desmayo, pérdida de campo visual.
→ La app debe comparar cada valor con **la línea base personal del astronauta** y detectar tendencias, no usar umbrales fijos inventados.

## Lo que ya se mide a bordo (datos realistas para la app)

Temperatura, presión, pulso, respiración · ecografía guiada desde Tierra · OCT y agudeza visual · actígrafo de muñeca (sueño) · masa corporal (SLAMMD) · dosímetro personal · CO₂ de cabina · pruebas cognitivas de tiempo de reacción · audiometría.

## Ideas para la app que se repiten

1. Registro de síntomas con escala 0–10 y fotos para el médico de vuelo.
2. Cruzar datos entre sí: CO₂ con cefalea, radiación con síntomas, sueño con rendimiento.
3. Funcionamiento sin conexión con sincronización diferida (Marte: hasta 22 min de retraso por trayecto).
4. Chequeo privado de ánimo y estrés, con contacto confidencial.
5. Inventario de medicamentos con alertas de caducidad.
6. Vista por fase de misión: llegada, estancia larga, regreso a gravedad y Luna/Marte.
