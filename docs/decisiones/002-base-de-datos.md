# Decisión 002 — Base de datos

Fecha: 14-nov-2026 · Estado: **aceptada** (el equipo ya está montando el servidor MySQL)

## Decisión
- **En el móvil: SQLite** (crate `rusqlite`, con la librería incluida en la app). Es la fuente de verdad sin conexión.
- **En Tierra: MySQL** (versión 8 o superior, que tiene tipo `JSON`). Lo están montando compañeros del equipo; las credenciales llegarán pronto.
- **El móvil nunca habla con la base de datos de Tierra.** Habla con una **API de sincronización** (HTTP + JSON). Gracias a eso, se puede cambiar MySQL por MongoDB o lo que sea sin tocar la app.
- **Las credenciales de MySQL solo las tiene la API, nunca la app.** Un APK se puede descompilar, así que cualquier credencial que lleve dentro queda expuesta. En el repo tampoco se guardan: van en `.env` (ignorado por git) y en el repo solo hay un `.env.example` sin valores.

## Por qué SQL encaja aquí
1. **Los datos tienen estructura:** registros con tipo, unidad y hora, eventos, evaluaciones y análisis. Los tipos de Rust (`core`) son un esquema, y SQL lo hace cumplir.
2. **Cada tipo de dato tiene un solo escritor, así que no hay conflictos.** Los registros solo los crea el móvil. Los análisis solo los crea Tierra. Nada se sobrescribe: una corrección es una versión nueva (RF-03). La sincronización se reduce a "subir lo que me falta y bajar lo que falta".
3. **SQLite es el estándar en móviles:** un solo archivo, transacciones atómicas (si la app se cierra a medio guardar, no se corrompe) y funciones JSON.
4. **MySQL 8** tiene una columna `JSON` (validada y con funciones `JSON_EXTRACT`) para los campos flexibles, y se pueden crear índices sobre columnas generadas a partir de ese JSON. UUIDv7 se guarda como `BINARY(16)`.

## Flexibilidad "tipo NoSQL" dentro de SQL
En la tabla `registro`, las columnas por las que se busca son fijas (`tipo`, `medido_en`, `astronauta_id`). El valor va en `datos` (JSON), y Rust lo valida antes de guardarlo y al leerlo (RNF-02). Así, añadir un indicador nuevo no requiere migrar la base de datos.

## Cuándo pasaríamos a NoSQL
- Si la sincronización integrada de un producto (Couchbase Lite, Realm/Atlas Device Sync…) nos ahorrara mucho trabajo. **Pega:** no tienen buen soporte para Rust ni para Tauri, y Atlas Device Sync está deprecado.
- Si los análisis de Tierra devolvieran documentos sin una estructura común.

Ninguna de las dos cosas pasa hoy. Para que migrar sea barato:
- En Rust, `store` es un **trait de repositorio**: SQLite es solo una implementación.
- El contrato es la **API de sincronización**, no el esquema de la base de datos.

## Esquema inicial en el móvil (borrador)

| Tabla | Para qué | Campos clave |
|---|---|---|
| `astronauta` | Perfil ficticio | `id` |
| `referencia` | Referencia personal previa al vuelo (RF-08) | `indicador`, `valor`, `dispersion`, `origen`, `ventana`, `version` |
| `registro` | Mediciones y síntomas (RF-01, RF-03) | `id` (UUIDv7), `tipo`, `datos` (JSON), `medido_en`, `anotado_en`, `origen` (manual, dispositivo, slm_confirmado), `reemplaza_a`, `nota` |
| `evento_alarma` | Desmayo, campo visual, trombo (RF-02) | `id`, `tipo`, `certeza` (síntoma, sospecha, confirmado), `ocurrido_en` |
| `evaluacion` | Resultado del núcleo, para auditarlo (RF-11) | `id`, `estado`, `regla_id`, `regla_version`, `fuente`, `motivo`, `evaluado_en`, `entradas` |
| `analisis_remoto` | Lo que llega de Tierra (RF-21) | `id`, `autor_tipo` (médico, modelo), `sobre` (ids), `contenido`, `version`, `creado_en`, `recibido_en` |
| `borrador_slm` | Propuestas del SLM antes de confirmarlas (RF-14) | `id`, `entrada`, `propuesta` (JSON), `modelo_version`, `aceptado` |
| `bandeja_salida` | Cambios pendientes de subir (RF-19) | `seq`, `entidad`, `entidad_id`, `payload`, `intentos`, `ultimo_error`, `estado` |
| `sync_estado` | Cursor de descarga y fecha de la última sincronización | `clave`, `valor` |

- **IDs UUIDv7:** se generan sin conexión, no chocan entre dispositivos y vienen ordenados por tiempo.
- **Horas en UTC**, en milisegundos. El reloj lo pasa el adaptador (RNF-03).
- **Migraciones con versión** (`PRAGMA user_version`), aplicadas al arrancar.

## API de sincronización (borrador)
- `POST /v1/sync/push` → `{dispositivo_id, lote: [{id, entidad, version, payload}]}`. Devuelve los ids aceptados. Es **idempotente**: si un id ya existe, se ignora.
- `GET /v1/sync/pull?cursor=…` → análisis y cambios de Tierra posteriores al cursor, más el cursor nuevo.
- Autenticación con un token por dispositivo sobre HTTPS (RNF-10).
