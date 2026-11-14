# Revisión — Primer diseño de la app móvil (commit de592be)

Revisor: Claude · 14-nov-2026 · Archivos: `ui/index.html`, `ui/src/main.ts`, `ui/src/styles.css`
Probado en el navegador a 375×812 (tamaño de móvil) con `npm run dev`.

## Veredicto
**Buena base visual.** El estilo HUD, la figura humana y las tarjetas se ven bien y caben en un móvil. Ahora hay que convertirla en una pantalla que **muestre y registre datos reales**: sin valores inventados, en inglés y con los 7 indicadores acordados.

## Lo que está bien y se queda
- Figura humana con marcadores: es una de las ideas que propone el reto. Hay que hacerla **clicable por zona**.
- Tarjetas por indicador con valor grande, unidad, estado y tendencia: buen patrón para repetirlo en los 7 indicadores.
- La llamada a `invoke("core_status")`, con alternativa si se abre en el navegador.
- Botón de emergencia bien separado y visible.
- Adaptación a pantallas estrechas (`@media (max-width: 380px)`).

## Hay que cambiar (por orden de importancia)

1. **Datos inventados.** `Simular Telemetría`, el `setInterval` que mueve el pulso cada 4 s y los valores iniciales (72/98/95) muestran como reales datos que nadie midió. En una app de salud con datos de una persona real esto es peligroso: el astronauta puede creer que tiene pulso cuando no hay medición. **Quitar todo lo aleatorio.** Si no hay dato, mostrar `No data`. Para una demo, usar datos de ejemplo marcados como `DEMO` y siempre los mismos.
2. **Todo debe ir en inglés** (lo exige la NASA, RNF-14): `lang="en"`, títulos, botones, estados (`NORMAL` → `Stable`…), la alerta y el `<title>`.
3. **Indicadores acordados:** pulso, presión arterial, temperatura, glucosa, sueño, ánimo y cefalea, más síntomas por zona del cuerpo. Falta todo menos el pulso y la glucosa. **SpO₂ no está en la lista**: o se quita o se añade al catálogo de forma explícita (hay que decidirlo).
4. **Registrar datos (RF-29).** Cada tarjeta necesita un botón `Log` que abra un formulario manual con la unidad del catálogo (p. ej., glucosa en `mg/dL`) y un "medido a las" (por defecto, ahora). Esto es lo central de la entrega.
5. **La interfaz no decide si un valor es normal.** Los estados `NORMAL`, `ÓPTIMO` y `ESTADO GENERAL: ESTABLE`, los rangos (`Ref: 60-100 BPM`, `Ayuno: 70-100 mg/dL`, `> 95%`) y las tendencias (`▲ +2 bpm`) están escritos a mano. Todo eso lo calcula el núcleo en Rust y la interfaz solo lo pinta. Los rangos médicos además necesitan fuente. Mientras no exista el núcleo, mostrar `Not evaluated`.
6. **Botón de ayuda.** El `alert()` menciona al CMO (lo quitamos: no hay médico a bordo en nuestro planteamiento) y dice que "encola telemetría", pero no hace nada. Debe crear un evento `need_help` (ver `docs/api/modelo-datos-backend.md`), con confirmación, y mostrar si quedó enviado o pendiente.
7. **Pie de página inventado.** `BATERÍA TRAJE 94%`, `TRAJE O₂ 0.31 bar` y `+18m retardo` no vienen de ningún sitio. El entorno quedó fuera por ahora, y 18 minutos de retardo corresponde a Marte, no a Artemis. Sustituir por: **hora UTC + MET**, estado de sincronización (`Synced 2 min ago` / `3 pending`) y conexión (`Offline`/`Online`).
8. **Perfil.** `D. VÁSQUEZ`, `ESPECIALISTA EVA` y `MISIÓN: ARTEMIS · HAB-1` deben venir del perfil (`GET /v1/me`), no del HTML.
9. **Figura humana clicable.** Las zonas deben coincidir con el enum `body_zone` (`head`, `chest`, `abdomen`, `left_foot`…). Al pulsar una zona: registrar un síntoma o ver lo registrado ahí. La etiqueta `ESTADO GENERAL` tapa las piernas: moverla fuera de la figura.

## Rendimiento en un móvil de gama media-baja
- Hay 5 animaciones infinitas (escáner, anillos, puntos), dos `backdrop-filter: blur()` y un `filter: drop-shadow` sobre el SVG. En una WebView barata esto gasta batería y puede ir a tirones. Recomendación: dejar una sola animación, quitar `backdrop-filter` y respetar `@media (prefers-reduced-motion: reduce)`.
- `maximum-scale=1, user-scalable=no` impide hacer zoom: quitarlo por accesibilidad.
- Emojis en los botones (`⚡`, `🆘`): mejor iconos SVG como el resto, porque los emojis cambian según el móvil.

## Siguiente paso propuesto
1. Rodrigo: puntos 1, 2, 7 y 8 (limpieza y textos), que no dependen del núcleo.
2. Nosotros: los comandos de Tauri del núcleo (`get_profile`, `log_record`, `list_records`, `get_status`, `raise_event`), para que la interfaz los llame en vez de inventar.
3. Después: formulario de registro (punto 4) y figura clicable (punto 9) conectados a esos comandos.
