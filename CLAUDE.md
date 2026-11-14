# NASA Space Apps 2026 — Health Monitoring Software for Astronauts

Reto: https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/
Herramienta visual para que los astronautas registren indicadores de salud (presión, pulso, glucosa, temperatura, sueño, ánimo…), evalúen su estado y reciban orientación, incluido cuándo avisar a los médicos de vuelo en Tierra. Ideas del reto: figura humana clicable, gráficas temporales, código de colores, gamificación, recordatorios, distintos entornos (nave, estación, rover, base lunar/marciana).
Recursos: https://www.nasa.gov/hrp/hazards/ · https://www.nasa.gov/hrp/risks/ · https://www.nasa.gov/hrp/ongoing-hrp-research/

## Roles

- **Claude (Opus 5.5, high por defecto)**: orquestador. Divide el trabajo, elige modelo y esfuerzo para cada tarea, lanza los agentes por CLI, lee los informes de Sol e integra. Hace o revisa las piezas visuales clave (figura humana, 3D, animaciones). Sube a **xhigh** solo para esas piezas y para la revisión final.
- **Sol (gpt-6.1-sol)**: revisor principal y modelo para problemas complejos.
- **Terra (gpt-6.1-terra)**: implementación de código.
- **Hydrogen (gemini-4-hydrogen)**: trabajo en paralelo con la cuota de Google y contraste de datos médicos (es el que menos inventa).
- **Argon (gemini-4-argon)**: solo análisis de imágenes y automatización con herramientas.
- **Astra (gpt-6-astra)**: **solo si el usuario lo pide.**

## Reparto por tarea

| Tarea | Modelo y esfuerzo |
|---|---|
| Tareas triviales (datos de ejemplo, textos, estilos) | Terra `medium` |
| Implementar componentes y funciones | Terra `high` / `xhigh` |
| Lógica difícil, arquitectura, depuración | Sol `high` / `xhigh` |
| Contenido médico (rangos, alertas, recomendaciones) | Sol `high` / `max` + contraste con Hydrogen `xhigh` |
| Interfaz rápida en paralelo | Hydrogen `high` |
| Figura humana, 3D, animaciones | Claude (xhigh), o Terra implementa y Claude revisa |
| Analizar imágenes / automatización con herramientas | Argon `medium` |
| Revisión de cada entrega | Sol `high` |
| Revisión final antes de presentar | Sol `max` |

Justificación con datos: [docs/modelos-metricas.md](docs/modelos-metricas.md) (Artificial Analysis, 14-nov-2026).

## Ciclo de trabajo

1. Claude divide la tarea y asigna modelo + esfuerzo según la tabla.
2. Lanza los agentes. Tareas que no tocan los mismos archivos pueden ir en paralelo (Codex y agy a la vez). Nunca dos agentes escribiendo los mismos archivos.
3. Sol revisa lo entregado y escribe un informe en `reviews/<tarea>.md`.
4. Claude lee el informe, encarga correcciones e integra.

## Comandos

Codex (esfuerzos: `low` `medium` `high` `xhigh` `max` `ultra`):

```bash
codex exec -m gpt-6.1-terra -c model_reasoning_effort="high" -s workspace-write "<prompt>"
```

```bash
codex exec -m gpt-6.1-sol -c model_reasoning_effort="high" -s read-only "<prompt de revisión>"
```

Antigravity (el esfuerzo va en el id del modelo; Hydrogen `low`–`xhigh`, Argon `low`–`high`):

```bash
agy -p "<prompt>" --model gemini-4-hydrogen-high --mode auto
```

## Reglas

- Si Codex falla por cuota agotada, avisar al usuario para que cambie de cuenta con Codex Switcher.
- GPT y Opus inventan datos el 45-67 % de las veces cuando no saben: todo valor médico se contrasta con Hydrogen y con fuentes de la NASA, y se cita la fuente.
- Argon nunca para código ni para textos médicos (FrontierCode 26, transparencia 22).
- Cada prompt a un agente incluye: archivos que puede tocar, qué es "terminado" y dónde dejar su informe.
