# Exportación a JSON

Comando Tauri `export_json`: guarda **todos los datos locales** del dispositivo en un único
archivo JSON, autodescriptivo y verificable sin la app. No sube nada a ninguna parte.

- Entrada opcional: `{ include_telemetry?: boolean (por defecto true), astronaut_key?: string }`.
  `astronaut_key` filtra **solo** la telemetría (el resto es del dispositivo: un perfil).
- Salida: `{ path, file_name, bytes, exported_at, counts: { records, events, evaluations, telemetry_readings, outbox_pending } }`.
- Archivo: `<datos de la app>/exports/merken-export-AAAAMMDDTHHMMSSZ.json` (UTC). Si el nombre existe se
  usa `-2`, `-3`… (hasta 20 intentos). **Nunca sobrescribe.** Se escribe completo en un temporal exclusivo y solo entonces se publica sin reemplazar nada: nunca queda un `.json` vacío o a medias.
- Todas las lecturas (incluida la verificación de la cadena) ocurren en **una sola transacción**: el
  archivo es un snapshot coherente aunque la app siga guardando telemetría.
- Si hay más de 1 000 000 lecturas de telemetría el export pide filtrar por `astronaut_key`.

## Formato (`format: "merken-export"`, `format_version: 1`)

```json
{
  "format": "merken-export", "format_version": 1,
  "exported_at": "2026-11-15T05:00:00.000Z",
  "app_version": "0.1.0", "core_version": "...", "rules_version": "...",
  "scope": { "include_telemetry": true, "astronaut_key": null },
  "profile": null,
  "records": [ { "record_id": "...", "indicator": "heart_rate", "value": { "bpm": 72.0 }, "unit": "bpm",
                 "measured_at": "...Z", "source": "form", "replaces": null, "sync": "pending", "is_demo": false } ],
  "events": [], "evaluations": [], "outbox": [], "pulled_changes": [], "sync_state": {},
  "telemetry": {
    "readings": [ { "reading_id": 1, "astronaut_key": "1", "astronaut_name": "Gabo", "indicator": "heart_rate",
                    "value": { "bpm": 70 }, "value_json": "{\"bpm\":70}", "unit": "bpm",
                    "observed_at": "...Z", "observed_at_ms": 1794718092723, "batch_id": "...",
                    "source": "live_api", "app_version": "0.1.0", "prev_hash": "000…0", "row_hash": "…" } ],
    "heads": [ { "astronaut_key": "1", "indicator": "heart_rate", "last_reading_id": 1, "last_hash": "…", "count": 1 } ],
    "verification": { "ok": true, "checked": 1, "series_checked": 1, "problem": null }
  }
}
```

Las horas van en RFC 3339 UTC con milisegundos; los campos ausentes salen como `null`. Los datos DEMO
llevan `is_demo: true` y `sync: "local_only"`. Orden estable: `records` por orden local, `events` por
fecha, `evaluations` por fecha, `outbox` por id, `readings` por `reading_id`, `heads` por serie.

## Comprobar la cadena a mano

El hash de cada lectura es SHA-256 de `"merken-telemetry-v1"` seguido de estos campos, cada uno
precedido de su longitud en 64 bits big-endian: `astronaut_key`, `indicator`, **`value_json`** (los bytes
UTF-8 exactos guardados, no `value` re-serializado), `unit`, `observed_at_ms` (8 bytes big-endian con
signo), `batch_id`, `source`, `app_version`, `prev_hash`. `prev_hash` es el `row_hash` de la lectura anterior
**de la misma serie** `(astronaut_key, indicator)`; la primera usa 64 ceros. `heads` debe coincidir con la
última lectura de cada serie.

```python
import json, hashlib, struct
d = json.load(open("merken-export.json", encoding="utf-8"))  # utf-8 explícito (en Windows el defecto es cp1252)
rows = d["telemetry"]["readings"]
last, count, bad = {}, {}, []
prev_id = 0
for r in rows:
    if r["reading_id"] <= prev_id: bad.append(("orden", r["reading_id"]))
    prev_id = r["reading_id"]
    k = (r["astronaut_key"], r["indicator"])
    h = hashlib.sha256(b"merken-telemetry-v1")
    def feed(b): h.update(struct.pack(">Q", len(b)) + b)
    for f in (r["astronaut_key"], r["indicator"], r["value_json"], r["unit"]): feed(f.encode("utf-8"))
    feed(struct.pack(">q", r["observed_at_ms"]))
    for f in (r["batch_id"], r["source"], r["app_version"], r["prev_hash"]): feed(f.encode("utf-8"))
    if r["prev_hash"] != last.get(k, ("0" * 64, 0))[0] or h.hexdigest() != r["row_hash"]:
        bad.append(("hash", r["reading_id"]))
    last[k] = (r["row_hash"], r["reading_id"])
    count[k] = count.get(k, 0) + 1
# Las cabezas detectan que borraran el final de una serie o una serie entera.
heads = {(h["astronaut_key"], h["indicator"]): h for h in d["telemetry"]["heads"]}
if set(heads) != set(last): bad.append(("series", sorted(set(heads) ^ set(last))))
for k, h in heads.items():
    if k in last and (h["last_hash"], h["last_reading_id"], h["count"]) != (*last[k], count[k]):
        bad.append(("cabeza", k))
print("lecturas:", len(rows), "series:", len(heads), "problemas:", bad)
```

Probado contra una copia de la base real: 3100 lecturas, 62 series, 0 malas (Rust y Python coinciden).

## Qué no se copia

Es una vista de los datos, no un volcado literal de las tablas: no se incluyen `profile.id` ni la columna
`profile_version` (sigue disponible dentro de `medical_profile`) ni `records.local_order` (el orden del
arreglo `records` es ese orden). El filtro `astronaut_key` aún no está en la interfaz: el botón exporta
toda la telemetría.

## Límites

- El archivo contiene **datos de salud** de los astronautas. No está cifrado; trátalo como confidencial.
- Detecta manipulación **interna** (cambiar un valor, borrar o reordenar una lectura), pero no protege
  frente a quien regenere todo el archivo con hashes nuevos.
- No incluye credenciales, `server/.env` ni rutas del sistema.
