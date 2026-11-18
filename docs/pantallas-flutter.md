# Pantallas de Pulsys en Flutter (propuesta v1, 18-nov-2026)

Base: la app anterior, inventariada en [reviews/inventario-ui-anterior.md](../reviews/inventario-ui-anterior.md). Se conserva la estructura y la posición de cada bloque; el estilo es un placeholder simple hasta decidir el diseño final.

## Navegación

- Tres páginas en horizontal con deslizamiento (`PageView`). Abre en la del **centro**.
- Abajo, tres indicadores con nombre ("History", "Today", "Recommendations") que también son botones, para usarla con TalkBack y VoiceOver sin gestos.
- Cabecera fija en las tres: logo PULSYS (vuelve al centro), selector de tripulante y botón **Need help**.
- Atrás del sistema: desde izquierda o derecha vuelve al centro; desde el centro sale de la app.

## Izquierda — History (datos detallados)

1. Alertas persistentes, si las hay, con su detalle.
2. Resumen del perfil: estado general, escalado, indicadores a revisar, sincronización y fecha de evaluación.
3. Tendencia del pulso de las últimas horas (gráfica y tabla accesible), con filtro de rango (6 h, 24 h, 7 días).
4. Por indicador: último valor, estado de Rust, fuente y botón **Explain**.
5. Datos DEMO: aviso y selector de escenario cuando el almacén está vacío.

Ya existe: la tendencia y la tabla (etapa 3). Falta: filtro por horas, alertas, resumen e indicadores distintos del pulso.

## Centro — Today (lo esencial)

1. Estado general en una línea, con la hora de evaluación y la marca DEMO.
2. Figura del cuerpo en el centro, con cuatro tarjetas de signos a cada lado (como antes): izquierda pulso, presión, SpO₂ y ánimo; derecha temperatura, glucosa, sueño y nutrición. Tocar una tarjeta o una zona abre su ficha.
3. Fila de contexto: radiación, distancia y sincronización.
4. Abajo, la entrada del asistente: micrófono ("Hold to talk") y campo de texto ("Tell me what happened…").

Ya existe: la figura y el registro de pulso (que pasa a abrirse desde la tarjeta de pulso o desde el asistente). Falta: tarjetas de los demás signos, fila de contexto y asistente.

## Derecha — Recommendations

1. Sugerencia actual con su motivo, la acción del núcleo ("avisar a los médicos de Tierra" cuando corresponda) y las fuentes.
2. Mediciones usadas y aviso de que es orientación, no diagnóstico.
3. Preferencia de ocio opcional.
4. Pautas de ejercicio de la misión (texto fijo).

Ya existe: nada en Flutter. Primera versión: lo que ya decide Rust (estados, alertas, escalado). El texto del modelo llega con el asistente.

## Asistente (entrada del centro)

Problemas de la versión anterior, según el inventario:

- El chat rápido no era el modelo: era una expresión regular en TypeScript que solo entendía el pulso y **siempre pedía la hora** si no venía en formato UTC (`ui/src/ui/reading-chat.ts:47-53`).
- El modelo hacía una sola pasada sin conversación; tampoco hablaba: la síntesis de voz existía en Rust pero ninguna pantalla la usaba, y las funciones de voz venían desactivadas por defecto.
- El explicador nunca respondía: el catálogo de fichas está vacío (`crates/slm/fichas/fichas.json` = `[]`).

Propuesta:

1. Una sola conversación para texto y voz. Si no se dice la hora, se asume "ahora" y se muestra para corregir; solo se pregunta lo ambiguo o lo que cambia la recomendación.
2. Interpretación en dos capas como antes (reglas de Rust y modelo local solo para lo que queda sin entender), que termina en una tarjeta "esto es lo que entendí" con **Guardar** o **Corregir**.
3. Voz nativa del sistema y offline: reconocimiento y lectura en voz alta de la respuesta. Sin modelos pesados de voz dentro de la app.
4. Modelo cargado solo al abrir el asistente, con respuesta en streaming, cancelable y liberado al salir, para no saturar la memoria.
5. Lo médico lo decide Rust con reglas citadas; las frases salen de fichas autorizadas (RF-15). Hay que llenar las fichas con fuentes de la NASA (Sol + contraste de Hydrogen).
