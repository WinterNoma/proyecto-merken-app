# Investigación de front-end para Merken

**Consulta: 14 de noviembre de 2026.** Investigación sin cambios en código ni instalaciones. Estrellas aproximadas y actividad observada cambian; «sin verificar» indica que la fuente no permitió confirmar el dato. Los pesos deben medirse con la versión y el build Vite elegidos: tamaño instalado, comprimido y servido son cosas diferentes.

**Estado revisado:** Tauri 2, Vite y TypeScript vanilla. La pantalla usa tarjetas oscuras, figura SVG, carrusel y emergencia. El contrato de core-api.ts distingue ausencia, antigüedad, historial insuficiente y contexto; entrega fechas, evaluaciones, registros y sincronización. El README reserva las decisiones de salud al núcleo Rust. Sin embargo, main.ts contiene valores de respaldo para datos MySQL, vital-status.ts clasifica lecturas en la interfaz e index.html muestra «EN VIVO». Eso es una brecha de diseño observable, no una prueba de qué ruta está activa. No se debe presentar un valor de respaldo como una medición real.

## 1. Skills y reglas de front-end para agentes

- [Anthropic frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design): orientación para dirección visual, tipografía y composición; [LICENSE específico Apache-2.0](https://github.com/anthropics/skills/blob/main/skills/frontend-design/LICENSE.txt). Es útil ya para redactar un brief y revisar el resultado; adaptar la búsqueda de identidad a la lectura de datos y a TS vanilla.
- [Awesome Claude Skills](https://github.com/ComposioHQ/awesome-claude-skills): catálogo de descubrimiento, no garantía de calidad o licencia de cada entrada. Licencia del catálogo y de cada skill **sin verificar**; leer el archivo específico antes de copiar o ejecutar.
- [AGENTS.md de NASA Open MCT](https://github.com/nasa/openmct/blob/master/AGENTS.md) es un ejemplo de instrucciones de proyecto. Su [código principal es Apache-2.0 con componentes de terceros](https://github.com/nasa/openmct/blob/master/copyright-notice.html). La [guía de AGENTS.md de Codex](https://developers.openai.com/codex/guides/agents-md/) explica el mecanismo; licencia de la documentación **sin verificar**. Para Merken conviene fijar como instrucciones verificables: offline, sin framework, estados del núcleo, no inventar telemetría, accesibilidad y revisión visual.
- [Reglas de Cursor](https://cursor.com/docs/context/rules): contexto y reglas dirigidas por proyecto. Licencia del texto **sin verificar**; utilizar la idea de reglas acotadas como referencia, evitando duplicar instrucciones incompatibles.
- [Anthropic web-artifacts-builder](https://github.com/anthropics/skills/blob/main/skills/web-artifacts-builder/SKILL.md) incluye un flujo React/Tailwind; no es una plantilla adecuada aquí. Licencia específica **sin verificar**.

## 2. Repositorios, gráficas, animación e iconos

Actividad observada no significa necesariamente último commit. Los recursos con campos sin verificar no quedan aprobados automáticamente.

| Recurso | Estrellas y fecha de actividad | Licencia y utilidad |
|---|---|---|
| [Open Props](https://github.com/argyleink/open-props) | ≈5,6 mil; fecha **sin verificar** | [MIT](https://github.com/argyleink/open-props/blob/main/LICENSE). Variables CSS portables; consultar escalas y espacios sin adoptar todo. |
| [Radix Colors](https://github.com/radix-ui/colors) | ≈1,7 mil; actualizado 17-dic-2025 según [organización](https://github.com/radix-ui/) | [MIT](https://github.com/radix-ui/colors/blob/main/LICENSE). Escalas CSS sin React; comprobar contrastes concretos. |
| [Carbon de IBM](https://github.com/carbon-design-system/carbon) | ≈9,5 mil; versión 4-nov-2026 según [releases](https://github.com/carbon-design-system/carbon/releases) | [Apache-2.0](https://github.com/carbon-design-system/carbon/blob/main/LICENSE). Referencia de densidad y tokens; componentes React no necesarios. |
| [Tabler](https://github.com/tabler/tabler) | ≈42 mil; push 13-nov-2026 según [API](https://api.github.com/repos/tabler/tabler) | MIT según API; referencia de tarjetas. El kit completo usa Bootstrap y trae más de lo necesario. |
| [Astro UXDS](https://github.com/RocketCommunicationsInc/astro) | ≈151; fecha exacta **sin verificar**; README advierte que no se mantiene activamente | [Licencia propia](https://github.com/RocketCommunicationsInc/astro/blob/main/LICENSE): dominio público en EE. UU. y permiso mundial gratuito con aviso, exención e indemnidad. No es MIT. Web Components agnósticos de framework; patrones espaciales útiles como referencia. |
| [NASA Open MCT](https://github.com/nasa/openmct) | ≈13,2 mil; actualizado 12-nov-2026 según [NASA](https://github.com/orgs/nasa/repositories) | [Apache-2.0 y licencias de terceros](https://github.com/nasa/openmct/blob/master/copyright-notice.html). Referencia de telemetría, tiempo y contexto; su integración completa en Tauri offline no está verificada. |
| [Grafana](https://github.com/grafana/grafana) | ≈77,9 mil; push 14-nov-2026 según [API](https://api.github.com/repos/grafana/grafana) | AGPL-3.0 según API; revisar licencias por componente. Referencia de patrones, no copiar código sin revisar obligaciones. |
| [Chart.js](https://github.com/chartjs/Chart.js) | ≈67,8 mil; actualizado 9-nov-2026 según [organización](https://github.com/chartjs) | [MIT](https://github.com/chartjs/Chart.js/blob/master/LICENSE.md). Canvas, TS y vanilla; útil para historial interactivo. |
| [Apache ECharts](https://github.com/apache/echarts) | ≈67,7 mil; push 13-oct-2026 según [API](https://api.github.com/repos/apache/echarts) | Apache-2.0 según API. Canvas/SVG, modular y potente; probablemente excesivo para simples tendencias. |
| [uPlot](https://github.com/leeoniya/uPlot) | estrellas y fecha **sin verificar** | Series temporales Canvas con tipos TS. Licencia **sin verificar** en fuente consultada: revisar LICENSE de versión exacta antes de incorporarlo. |
| [GSAP](https://github.com/greensock/GSAP) | ≈28,4 mil; fecha **sin verificar** | El README anuncia uso comercial gratuito y [licencia GreenSock propia](https://gsap.com/standard-license), no MIT. Verificar versión y plugins. |
| [Tabler Icons](https://github.com/tabler/tabler-icons) | ≈22,7 mil; push 14-nov-2026 según [API](https://api.github.com/repos/tabler/tabler-icons) | MIT según repo y API; SVG individuales aptos para vanilla. |

Otras alternativas: [Motion](https://github.com/motiondivision/motion), [anime.js](https://github.com/juliangarnier/anime), [Lucide](https://github.com/lucide-icons/lucide) y [Phosphor](https://github.com/phosphor-icons/core): licencia, estrellas, última actividad y peso **sin verificar**. CSS y [View Transitions API](https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API) evitan dependencia de animación, pero el soporte exacto en WebView2 y Android WebView está **sin verificar**: usar detección y alternativa. Tailwind puede compilar CSS sin React, pero agrega configuración; licencia y tamaño **sin verificar** en esta revisión.

**Mapa corporal:** la silueta existente puede mantenerse si su procedencia interna está clara. [MuscleMap: procedencia](https://github.com/Jsplice/MuscleMap/blob/main/ASSET_PROVENANCE.md) identifica rutas SVG anatómicas originales MIT y assets de otra clase; revisar el archivo exacto y su adecuación antes de copiar. Preferir una silueta simplificada con las 14 zonas del contrato, etiquetas y teclado. Una licencia del código no prueba por sí sola la licencia de todos los SVG ni la validez médica de una figura.

## 3. Patrones de UI de salud

1. **Alarmas y prioridad.** La [IEC 60601-1-8 en ISO](https://www.iso.org/obp/ui?_escaped_fragment_=iso%3Astd%3Aiec%3A60601%3A-1-8%3Aed-2%3Av1%3Aen) trata sistemas de alarmas. Una [vista de la edición 2003](https://www.iteh.eu/catalog/standards/iec/8c7d1586-56b2-41a8-99ca-32947c93dabd/iec-60601-1-8-2003) muestra alta roja, media amarilla y baja cian o amarilla, diferenciadas también por comportamiento. Sirve de referencia visual histórica, no para afirmar cumplimiento: revisar edición aplicable, sonidos, tiempos y uso previsto con especialistas. En Merken: estado explícito con texto y símbolo; prioridad visible y alertas persistentes.
2. **Lectura con contexto.** [NEWS2 del Royal College of Physicians](https://www.rcp.ac.uk/resources/national-early-warning-score-news-2/) organiza observaciones y comunicación del deterioro. Inspirarse en jerarquía y secuencia, sin inventar puntuaciones ni afirmar que Merken calcula NEWS2. Los gráficos originales tienen sus propios derechos.
3. **Sparklines honestos.** Valor, unidad, hora de medición y estado del núcleo; una tendencia con ventana indicada y muestras reales de listRecords(), huecos si faltan registros y texto equivalente. No simular ECG desde una frecuencia ni exagerar variación con escala oculta.
4. **Sin dato y antiguo.** Representar de forma diferente «Sin datos», «Dato antiguo», «Historial insuficiente» y «No evaluado» según reason. Mostrar última medición cuando exista y distinguir su hora de sincronización. not_applicable es contexto, no alarma. «En vivo» solo con fuente y frescura demostrables.
5. **Wearables.** [Apple HealthKit HIG](https://developer.apple.com/design/human-interface-guidelines/healthkit) aporta orientación sobre datos de salud y marca. El patrón recomendado es lectura acompañada de historial y origen. Patrones específicos de Garmin y Oura y licencias de capturas **sin verificar**; no copiar sus marcas o pantallas.
6. **Accesibilidad y poca luz.** [WCAG: uso del color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html), [contraste de texto](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) y [contraste no textual](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html): señal adicional al color, foco visible, texto ampliable y controles accesibles. Fondo mate oscuro, destellos contenidos y prefers-reduced-motion; comprobar bajo luz y brillo reales. No ocultar alarmas al reducir movimiento.

## 4. Tabla de reutilización para este stack

Tamaño: **sin verificar** significa que no se midió una versión y build concretos. Cargar recursos empaquetados localmente y conservar los avisos exigidos.

| Recurso | Uso en Merken | Tamaño aproximado | Licencia | Compatibilidad |
|---|---|---|---|---|
| CSS propio | tokens y estados | solo CSS propio; sin medir | código del proyecto | Sí; sin dependencia. |
| Radix Colors | escalas oscuras | sin verificar | MIT | Sí, CSS local; probar contraste. |
| Open Props | espacios y curvas | sin verificar | MIT | Sí; opcional. |
| Tabler Icons | reloj, alerta y navegación | unos SVG; bytes sin verificar | MIT | Sí; copiar selección con aviso. |
| Chart.js | historial interactivo | sin verificar en build real | MIT | Sí, TS/Canvas; incluir alternativa textual y probar WebViews. |
| ECharts | gráficos complejos | sin verificar | Apache-2.0 | Sí, modularizar y medir. |
| uPlot | series temporales | sin verificar | **sin verificar** | TS/Canvas en principio; licencia y WebViews pendientes. |
| Astro UXDS | patrones, posibles Web Components | sin verificar | propia, condiciones en §2 | Compatible sin React en principio; soporte y condiciones pendientes de revisión. |
| Open MCT | referencia de misión | app completa; sin verificar | Apache-2.0 y terceros | Referencia; integración offline no verificada. |
| Grafana | referencia de paneles | app completa; sin verificar | AGPL-3.0; componentes pueden variar | Referencia; no dependencia recomendada. |
| GSAP | animación compleja | sin verificar | GreenSock propia | Vanilla sí; innecesaria para transición simple. |
| IBM Plex / Barlow | fuentes locales | WOFF2 según pesos y subconjuntos; sin verificar | OFL 1.1 | Sí, empaquetar fuente y licencia. |
| MuscleMap | posible figura SVG | sin verificar | MIT para rutas indicadas en procedencia | Sí técnicamente; zonas y accesibilidad pendientes. |

## 5. Generación de imágenes

Licencia del modelo, derechos de salida y términos del servicio son distintos. Generar fuera de la app y guardar los assets localmente permite uso offline. Registrar modelo, proveedor, fecha y plan. Revisar y simplificar SVG; texto, controles y estados deben seguir siendo código accesible.

| Herramienta | Uso y condiciones |
|---|---|
| [fal + FLUX.2](https://fal.ai/flux-2) | Avatares/fondos raster. fal declara uso comercial de imágenes Pro y Dev vía API; pesos locales Dev tienen condiciones distintas. SVG nativo **sin verificar**. No extrapolar a otros modelos. |
| [fal + Recraft](https://fal.ai/ai-image-generator) | fal anuncia salida SVG vectorial. Términos comerciales del endpoint y plan fal **sin verificar**; no asumir que coinciden con Recraft directo. |
| [Recraft directo](https://www.recraft.ai/blog/ownership-and-commercial-use-faq) | Raster y vectores. FAQ distingue plan gratuito y pago, con restricciones comerciales para salidas gratuitas posteriores al cambio de 2024; revisar términos vigentes del plan. |
| [Ideogram](https://ideogram.ai/tos) | Ilustraciones raster; términos comerciales exactos y SVG nativo **sin verificar**. No aprobar uso comercial hasta contrastarlos. |
| GPT Image | Raster para avatares/fondos; SVG nativo **sin verificar**. El [acuerdo API de OpenAI](https://cdn.openai.com/osa/openai-services-agreement.pdf) asigna al cliente derechos de OpenAI sobre la salida, sujeto a ley y condiciones; no garantiza exclusividad ni derechos sobre terceros. |
| [Midjourney](https://docs.midjourney.com/hc/en-us/articles/27870375276557-Using-Images-Videos-Commercially) | Raster; SVG nativo **sin verificar**. Uso comercial sujeto a condiciones: empresas que superan USD 1 millón anual necesitan Pro o Mega; una ampliación de imagen ajena pertenece al creador original. |
| [Stable Diffusion 3.5 local](https://stability.ai/license) | Raster local tras descargar pesos; SVG nativo **sin verificar**. Community permite uso comercial gratuito a usuarios elegibles bajo su umbral de ingresos y condiciones; otros modelos pueden tener licencias distintas. |

**Fallo fal comunicado por el equipo:** falta FAL_KEY en el entorno del proceso que inicia el MCP. Se necesita una clave válida, configurarla para ese proceso y reiniciar la conexión; verificar permisos, créditos y red si persiste. [Autenticación de fal](https://fal.ai/models/openrouter/router/audio/api). No poner la clave en el frontend, repositorio ni binario. El estado real del MCP no se verificó en esta investigación.

## 6. Dirección visual y recomendación

Paletas iniciales **sin validar**: comprobar pares de contraste, foco, legibilidad y brillo real. Son tokens visuales, no rangos médicos. El rojo y ámbar indican prioridad acompañados de texto e icono.

| Token CSS | A. Control de misión — recomendada | B. Clínica orbital | C. Noche de cabina |
|---|---|---|---|
| --bg | #08131c | #101820 | #111216 |
| --surface | #112431 | #1b2a34 | #202126 |
| --border | #385160 | #52636c | #54545b |
| --text | #edf5f6 | #f0f4f2 | #f2efed |
| --muted | #b5c8ce | #c0ccc9 | #cbc5c2 |
| --accent | #70d5e2 | #8bd5c5 | #b4c8e4 |
| --stable | #81d8ad | #83d9ad | #9bd7b3 |
| --warning | #ffcb72 | #ffcb72 | #ffcb72 |
| --alarm | #ff9090 | #ff9090 | #ff9090 |

**A: Control de misión.** Conservar la identidad espacial con azul petróleo y cian moderado, menor resplandor, una silueta como índice corporal y tarjetas de valor grande, unidad, estado textual y fecha. Orden móvil propuesto: alertas, constantes y mapa. **B** es más clínica y calmada; **C** más neutra para cabina oscura, previa prueba de uso.

**Tipografía:** [IBM Plex](https://github.com/IBM/plex) Sans para texto y Mono para cifras y horas; familia OFL, con [licencia OFL de Plex Mono](https://github.com/google/fonts/blob/main/ofl/ibmplexmono/OFL.txt). Alternativa [Barlow](https://github.com/google/fonts/blob/main/ofl/barlow/METADATA.pb), declarada OFL, para títulos. Empaquetar solo pesos y caracteres necesarios como WOFF2 o vía Fontsource, con licencia y avisos. Licencia específica de cada paquete Fontsource y tamaño **sin verificar**; no usar CDN en runtime.

**Dependencias recomendadas, máximo seis:**

1. Tabler Icons: pocos SVG MIT de estado y navegación; puede copiarse una selección sin instalar paquete.
2. Chart.js: solo si se necesita historial interactivo; MIT y compatible con TS vanilla. Medir y proporcionar texto accesible.
3. IBM Plex Sans local: texto legible, OFL.
4. IBM Plex Mono local: cifras alineadas, OFL.

Se puede comenzar con CSS y SVG nativos, sin biblioteca de animación. Fuentes como paquetes cuentan separadamente solo si se usa Fontsource.

**Criterios para una futura implementación:** mostrar fechas y estados del núcleo sin valores de relleno como telemetría; identificar demo y no fabricar trazados; mantener alertas accesibles y persistentes; probar empaquetado sin red, teclado, contraste, escalado, movimiento reducido y WebViews de escritorio y Android. La apariencia no demuestra conformidad IEC ni validación clínica.

## Fuentes

Las fuentes específicas y licencias están enlazadas junto a cada recurso. Destacan [licencia Astro UXDS](https://github.com/RocketCommunicationsInc/astro/blob/main/LICENSE), [NASA Open MCT](https://github.com/nasa/openmct), [NEWS2 RCP](https://www.rcp.ac.uk/resources/national-early-warning-score-news-2/), [IEC en ISO](https://www.iso.org/obp/ui?_escaped_fragment_=iso%3Astd%3Aiec%3A60601%3A-1-8%3Aed-2%3Av1%3Aen), [WCAG uso del color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html), [Chart.js MIT](https://github.com/chartjs/Chart.js/blob/master/LICENSE.md), [Tabler Icons](https://github.com/tabler/tabler-icons) y [licencia Stability](https://stability.ai/license). Las licencias del software no se extienden automáticamente a capturas, marcas o documentación.
