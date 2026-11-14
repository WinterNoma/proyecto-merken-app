# Requisitos — App móvil de salud para astronautas (Rust)

Borrador v0 · 14-nov-2026 · Repo: https://github.com/WinterNoma/nasa-spaceapps-app
Base: [reto](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/) e [investigación](investigacion/00-resumen.md).

## 1. Objetivo

Una app móvil con la que el astronauta registra sus indicadores de salud en pocos segundos, ve cómo está respecto a **su propia línea base** y recibe una orientación clara: seguir igual, actuar (con un protocolo) o avisar al médico de vuelo. Tiene que funcionar sin conexión, porque en Marte el mensaje tarda hasta 22 min por trayecto.

## 2. Por qué Rust: qué exige eso a la app

Rust no se elige por moda. Se elige porque el problema se parece a software de misión: tiene que ser fiable, funcionar sin red y dar siempre el mismo resultado. Si la app no aprovecha eso, Rust sobra. Por eso pedimos esto:

| Intención del lenguaje | Requisito que genera |
|---|---|
| Seguridad de memoria, sin cuelgues | **RNF-01** Sin `panic!` ni `unwrap()` en el núcleo; todo error es un `Result` y se muestra al usuario. |
| Tipos fuertes | **RNF-02** Las unidades son tipos (`Mmhg`, `Bpm`, `Celsius`, `MgDl`…): no se puede guardar un pulso donde va una presión. Los valores imposibles se rechazan al crearlos. |
| Determinismo | **RNF-03** Con los mismos datos, el motor de evaluación da siempre el mismo resultado. Lógica pura, sin E/S, que se puede reproducir para auditarla. |
| Un núcleo, muchas plataformas | **RNF-04** Toda la lógica (dominio, evaluación, almacenamiento, sincronización) vive en un *crate* `core` independiente de la interfaz. Compila para Android, iOS, escritorio y **web/WASM, para la demo ante los jueces**. |
| Recursos limitados a bordo | **RNF-05** Binario pequeño, arranque en menos de 1 s y bajo consumo; nada de servidores ni de dependencias en la nube para funcionar. |
| Testeable | **RNF-06** Tests unitarios y *property tests* (`proptest`) del motor de evaluación; `cargo test`, `clippy -D warnings` y `fmt` en CI. |

Regla práctica: **la interfaz solo dibuja; las decisiones de salud las toma el núcleo en Rust.**

## 3. Usuarios y contexto

- **Astronauta** (usuario principal): tiene poco tiempo, puede llevar guantes o estar en microgravedad y no es médico.
- **Médico de vuelo** en Tierra: recibe informes con retraso; no usa la app en directo.
- **Entornos**: nave en tránsito, estación, rover, base lunar o marciana. Cambian la latencia, el riesgo (radiación, gravedad) y los recursos.
- **Fase de misión**: llegada (días 1–3), estancia larga, regreso a la gravedad.

## 4. Requisitos funcionales

Prioridad: **M** = imprescindible para la demo · **S** = debería estar · **C** = si sobra tiempo.

### Registro
- **RF-01 (M)** Registrar las constantes: presión arterial, pulso, temperatura, SpO₂, frecuencia respiratoria y masa corporal.
- **RF-02 (M)** Registrar síntomas en una escala de 0 a 10 (cefalea, náuseas, dolor lumbar, visión, piel, respiratorio) y el ánimo y el estrés.
- **RF-03 (M)** Registrar el sueño (horas y calidad) y la ingesta (aproximada).
- **RF-04 (S)** Registrar el contexto ambiental: CO₂ de cabina y dosis de radiación (manual o simulado).
- **RF-05 (S)** Recordatorios de medición configurables por indicador.
- **RF-06 (C)** Adjuntar una foto a un síntoma (piel, ojo) para el médico.

### Figura humana
- **RF-07 (M)** Una figura humana interactiva: al tocar una zona se abren sus síntomas e indicadores, y cada zona se colorea según su estado.
- **RF-08 (C)** Versión 3D, rotable.

### Evaluación (núcleo Rust)
- **RF-09 (M)** Calcular la **línea base personal** de cada indicador (p. ej., mediana y desviación de los primeros N registros o de los datos previos al vuelo).
- **RF-10 (M)** Clasificar cada valor en **verde / ámbar / rojo** según la desviación respecto a esa línea base y la tendencia. Los únicos umbrales fijos son los que tienen fuente oficial (CO₂ ≤ 3 mmHg de media horaria, radiación ≤ 600 mSv en toda la carrera).
- **RF-11 (M)** **Señales de alarma por evento**: desmayo, pérdida de campo visual o sospecha de coágulo provocan siempre un rojo y la indicación de "avisar a médicos".
- **RF-12 (S)** Cruzar indicadores: CO₂ con cefalea, sueño con estado cognitivo, radiación con síntomas.
- **RF-13 (M)** Cada alerta explica **por qué** salta (qué valor, qué línea base, qué regla) y **cita su fuente**.

### Orientación y escalado
- **RF-14 (M)** Una recomendación por estado: qué hacer ahora, con pasos simples.
- **RF-15 (M)** Un botón "Preparar informe para médicos de vuelo" que genera un resumen (JSON/PDF/texto) con los datos, las alertas y las tendencias.
- **RF-16 (S)** Un chequeo privado de ánimo, con contacto confidencial.

### Historial
- **RF-17 (M)** Gráficas temporales por indicador, con la banda de la línea base dibujada.
- **RF-18 (S)** Vista por fase de misión.

### Conectividad
- **RF-19 (M)** Sin conexión por defecto: todo se guarda en local (SQLite).
- **RF-20 (S)** Cola de sincronización con latencia simulada (Tierra, Luna o Marte) y un estado visible: "enviado", "en tránsito" o "recibido".

### Extras
- **RF-21 (C)** Inventario de medicamentos con alertas de caducidad.
- **RF-22 (C)** Gamificación: rachas de registro e insignias por constancia, nunca por "estar sano".
- **RF-23 (S)** Selector de entorno (nave, estación, rover, Luna, Marte) que cambia la latencia y los riesgos visibles.

## 5. Otros requisitos no funcionales

- **RNF-07** Se puede usar con una mano y con objetivos táctiles grandes (≥ 56 dp), pensando en guantes y en microgravedad.
- **RNF-08** Un registro rápido lleva como mucho 3 toques y 15 s.
- **RNF-09** Accesibilidad: el color nunca es la única señal; cada estado lleva además un icono y un texto.
- **RNF-10** Los datos se guardan solo en el dispositivo y se exportan únicamente cuando el usuario lo decide.
- **RNF-11** Todo valor o regla médica sale de una fuente de la NASA o revisada, contrastada por Sol y Hydrogen. La app muestra el aviso "herramienta de apoyo, no diagnóstico".
- **RNF-12** Datos de ejemplo realistas, con un astronauta ficticio y 30 días de misión, para la demo.

## 6. Arquitectura propuesta

```
nasa-spaceapps-app/
├─ crates/
│  ├─ core/      # dominio, unidades, línea base, motor de reglas (puro, sin E/S)
│  ├─ store/     # SQLite (rusqlite) + cola de sincronización
│  └─ seed/      # generador de datos demo
├─ app/          # capa de interfaz (según la opción elegida)
└─ docs/
```

Opciones para la capa de interfaz. Está por decidir:

| Opción | Cuánto Rust | Móvil | Figura 3D | Riesgo en la hackathon |
|---|---|---|---|---|
| **A. Tauri 2** (Rust + WebView con HTML/TS) | Núcleo y backend | Android/iOS estables | Fácil (Three.js) | Bajo |
| **B. Dioxus** (interfaz en Rust con RSX) | Todo | Funciona, menos maduro | Difícil (SVG para 2D sí) | Medio |
| **C. Núcleo Rust + UniFFI + Kotlin/Compose** | Solo el núcleo | Nativo | Medio | Alto (dos lenguajes, más plomería) |
| **D. Slint 1.18** (lenguaje `.slint` + Rust) | Todo | Android/iOS estables | Sin 3D; 2D con `Path` | Bajo-medio |
| **E. Bevy / egui / Makepad** | Todo | Variable | Bevy, muy bueno | Alto: los formularios son incómodos |
| **F. Rune** (anomalyco/rune) | Todo | **No hay Android** (en su plan va al final, M7) | GPU propia, efectos | Muy alto hoy |

**Rune (revisado el 14-nov-2026):** tiene 3 semanas, no ha publicado ninguna versión y hay que fijar una rev de `dev`. Funciona en macOS, Windows y Linux, y en la web con WebGPU. El orden de su plan es escritorio → web → iOS (M6, en curso) → Android (M7, sin empezar), y en `platform/` no hay ningún shell para Android. Tampoco tiene accesibilidad móvil. Lo interesante es que cada app se puede renderizar sin pantalla (PNG y árbol de elementos), lo que encajaría con nuestro flujo de agentes. Para Android, hoy no sirve. La única forma de probarlo sería la build web abierta en Chrome de Android, que no está verificada en móvil.

Las opciones razonables para Android son **A**, **B** y **D**:
- **A (Tauri 2)** es la más segura para llegar a la demo. Rust se usa de verdad en todo lo que importa (núcleo, almacenamiento, sincronización) y la figura 3D sale fácil.
- **B (Dioxus 0.7)** tiene la interfaz en Rust y soporte "first-class" para Android (`dx serve --platform android`). La figura queda en 2D/SVG.
- **D (Slint)** es nativo, ligero y estable en Android. Su lenguaje de interfaz es propio.

En ambos casos, `core` y `store` son idénticos, así que se puede cambiar de capa de interfaz sin perder trabajo.

## 7. Fuera de alcance

- Conexión con sensores reales o wearables (se simula).
- Diagnóstico médico automático.
- Un backend real en Tierra (se simula la recepción).
- Cuentas de usuario y autenticación.

## 8. Preguntas abiertas

1. ¿Qué capa de interfaz usamos: A (Tauri 2) o B (Dioxus)?
2. ¿La plataforma de la demo es Android, iOS o ambas? ¿O basta con la build web/escritorio para el video?
3. ¿Cuánta gente del equipo programa y cuánta sabe Rust?
4. ¿Qué ideas del Google Doc del equipo hay que añadir aquí?
5. ¿Cuál es la fecha y hora límite de entrega?
