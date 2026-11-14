# Métricas de modelos (Artificial Analysis, 14-nov-2026)

Fuente: datos `initialModels` de las páginas `artificialanalysis.ai/models/<modelo>`. Puntuaciones en %, salvo Elo.
Coste = precio API por tarea (para nosotros: proxy de cuánta cuota gasta). Tiempo = minutos por tarea del Intelligence Index.

## Resumen general

| Modelo (esfuerzo) | Intel. | min/tarea | $/tarea | tok/s | Alucinación ↓ |
|---|---|---|---|---|---|
| Terra low | 29 | 1.2 | 0.07 | 62 | 42% |
| Terra medium | 38 | 2.9 | 0.17 | 62 | 40% |
| Terra high | 42 | 4.4 | 0.26 | 64 | 37% |
| Terra xhigh | 45 | 6.6 | 0.40 | 66 | 37% |
| Terra max | 51 | 14.6 | 0.96 | 69 | 35% |
| Sol low | 35 | 2.0 | 0.16 | 48 | 50% |
| Sol medium | 46 | 4.6 | 0.36 | 49 | 48% |
| Sol high | 50 | 7.9 | 0.59 | 48 | 49% |
| Sol xhigh | 52 | 10.6 | 0.76 | 47 | 48% |
| Sol max | 55 | 23.1 | 1.27 | 49 | 52% |
| Hydrogen low | 27 | 1.4 | 0.29 | 171 | 19% |
| Hydrogen medium | 41 | 3.8 | 0.80 | 172 | 13% |
| Hydrogen high | 51 | 8.4 | 1.87 | 168 | **9%** |
| Hydrogen xhigh | 54 | 14.8 | 3.16 | 174 | **9%** |
| Argon medium | 48 | 28.9 | 4.31 | 58 | 15% |
| Argon high | 50 | 29.4 | 4.97 | 66 | **4%** |
| Astra medium | 52 | 7.7 | 2.73 | 39 | 45% |
| Astra high | 55 | 9.5 | 3.24 | 41 | 43% |
| Astra xhigh | 55 | 11.9 | 3.66 | 38 | 45% |
| Astra max | 57 | 23.8 | 6.24 | 36 | 50% |
| Opus 5.5 low | 35 | 3.0 | 0.98 | 105 | 66% |
| Opus 5.5 medium | 48 | 6.9 | 2.45 | 105 | 67% |
| Opus 5.5 high | 52 | 9.5 | 3.47 | 106 | 64% |
| Opus 5.5 xhigh | 57 | 15.7 | 6.05 | 110 | 65% |
| Opus 5.5 max | 59 | 25.4 | 10.04 | 114 | 50% |

Alucinación = AA-Omniscience: cuando no sabe, ¿inventa (alto) o se abstiene (bajo)?

## Programación (agentes en repos y terminal)

| Modelo | FrontierCode 2.0 (repos reales) | Terminal-Bench 4.0 | SciCode |
|---|---|---|---|
| Opus max / xhigh / high | **46** / 44 / 36 | **60** / 60 / 57 | **67** / 65 / 60 |
| Astra max / xhigh / high | 42 / 41 / 40 | 59 / 60 / 54 | 57 / 56 / 55 |
| Terra max / xhigh / high / med | **42** / 37 / 38 / 35 | 56 / 47 / 42 / 32 | 55 / 53 / 52 / 52 |
| Sol max / xhigh / high / med | 41 / 39 / 37 / 33 | 56 / 54 / 52 / 48 | 54 / 56 / 56 / 53 |
| Hydrogen xhigh / high / med | 39 / 36 / 32 | 54 / 54 / 29 | — |
| Argon high / med | **26 / 25** | 57 / 55 | 62 / 59 |

## Razonamiento, matemáticas y ciencia

| Modelo | HLE-Diamond (razonamiento) | CritPt Hard (física) | GPQA | Omniscience (precisión de conocimiento) |
|---|---|---|---|---|
| Opus max / xhigh / high | **68** / 64 / 56 | 68 / 68 / 60 | — | **66** / 66 / 65 |
| Astra max / high / med | 64 / 59 / 53 | 71 / **72** / **72** | 96 / 95 / 94 | 62 / 61 / 60 |
| Sol max / xhigh / high | 59 / 56 / 52 | 70 / 70 / 69 | — | 62 / 61 / 60 |
| Hydrogen xhigh / high / med | 59 / 55 / 48 | 66 / 62 / 61 | — | 51 / 48 / 48 |
| Argon high / med | 57 / 43 | 53 / 54 | — | 48 / 49 |
| Terra max / xhigh / high | 47 / 42 / 35 | 68 / 61 / 62 | — | 48 / 48 / 47 |

## Salud y medicina (lo importante para nuestro reto)

| Modelo | Índice Healthcare | HealthBench | HealthBench Professional | MLCR (contexto médico largo) |
|---|---|---|---|---|
| Opus max / xhigh / high | **58** / 56 / 52 | 57 | 60 | 28 / 25 / 15 |
| Astra max | 54 | 58 | **66** | **42** |
| Sol max / xhigh / high / med | 52 / 51 / 49 / 46 | **59** | 65 | 38 / 36 / 32 / 32 |
| Hydrogen xhigh | 53 | — | — | 21 |
| Argon high | 51 | 49 | 55 | 14 |
| Terra max | 45 | — | — | 31 |

## Agentes, trabajo de conocimiento y visual

| Modelo | AutomationBench (flujos con herramientas) | Briefcase Elo (trabajo agéntico) | GDPval Elo | AA-3D Elo (experiencias 3D interactivas) | Code2Video Elo (animación en código) | MMMU-Pro (visión) |
|---|---|---|---|---|---|---|
| Opus max | 70 | **1801** | **1866** | **1202** | **1536** | 88 |
| Astra max | 69 | 1569 | 1587 | 1168 | 1427 | 87 |
| Sol max | 65 | 1555 | 1607 | 1121 | 1369 | 86 |
| Hydrogen xhigh / high | 76 / 74 | 1568 / 1497 | — | — | — | — |
| Argon high / med | **78** / 76 | 1485 / 1467 | 1634 / 1578 | 1147 | 1259 | ? / **88** |
| Terra max / high | 60 / 59 | 1401 / 1265 | 1476 / 1353 | — | — | 85 / 84 |

AA-3D, Code2Video, HealthBench y PersonalityBench solo están medidos para la variante máxima (Opus max, Astra max, Sol max, Argon high).

## Personalidad / calidad de texto (solo variantes máximas)

| | Transparencia (admite incertidumbre) | Organización | Slop (relleno; menos es mejor) |
|---|---|---|---|
| Opus max | 90 | 79 | 20 |
| Astra max | **94** | 70 | 16 |
| Sol max | 92 | **81** | **14** |
| Argon high | **22** | 56 | **68** |
