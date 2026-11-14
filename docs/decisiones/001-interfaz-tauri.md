# Decisión 001 — Interfaz con Tauri 2

Fecha: 14-nov-2026 · Estado: **aceptada**

## Decisión
La interfaz se hace con **Tauri 2**: TypeScript + Vite dentro de la WebView del sistema, con prioridad para Android. Toda la lógica que importa va en Rust: evaluación, almacenamiento, sincronización y SLM. La interfaz solo dibuja.

## Opciones evaluadas

| Opción | Cuánto Rust | Android | Figura 3D | Riesgo en la hackathon |
|---|---|---|---|---|
| **Tauri 2** (Rust + WebView con TS) | Núcleo y backend | Estable desde 2.0 | Fácil (Three.js) | Bajo* |
| Dioxus 0.7 (interfaz en Rust, `rsx!`) | Casi todo | "First-class", menos probado | Posible con Three.js (WebView) | Medio: los agentes escriben la API de versiones anteriores |
| Slint 1.18 (lenguaje `.slint`) | Lógica | Estable | Difícil | Bajo-medio |
| Núcleo Rust + UniFFI + Kotlin/Compose | Solo el núcleo | Nativo | Medio | Alto: dos lenguajes |
| Bevy / egui / Makepad | Todo | Variable | Bevy, muy bueno | Alto: los formularios son incómodos |
| Rune (anomalyco/rune) | Todo | **No hay** (en su plan va al final, M7) | GPU propia | Muy alto |

\* Sol: lo de "bajo" no está comprobado hasta tener un APK que llame a Rust y escriba en SQLite ([reviews/requisitos-app-movil.md](../../reviews/requisitos-app-movil.md)).

**Rune (revisado el 14-nov-2026):** tiene 3 semanas, no ha publicado ninguna versión y funciona en escritorio y en la web con WebGPU. Su plan sigue el orden escritorio → web → iOS (M6) → Android (M7, sin empezar), y no tiene accesibilidad móvil. Lo interesante es que cada app se puede renderizar sin pantalla, para que la revisen agentes.

## Consecuencias
- iOS queda fuera: Tauri necesita macOS y Xcode para compilarlo.
- Una demo web necesita un adaptador propio, porque los comandos de Tauri no existen en el navegador.
- Se usa un único generador de tipos TS: `ts-rs` o `tauri-specta`.
