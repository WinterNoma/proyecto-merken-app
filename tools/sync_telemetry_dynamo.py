#!/usr/bin/env python3
"""Prueba técnica: copia la telemetría local (SQLite) a DynamoDB, SIN pasar por la API.

Qué hace
  - Lee `telemetry_readings` en SOLO LECTURA (la app puede seguir abierta).
  - Comprueba antes la cadena de hashes; si está rota, no envía nada.
  - Convierte cada lectura a un `record` del contrato (docs/api/modelo-datos-backend.md)
    y lo publica con la transacción de 3 escrituras del §4: entidad en la tabla de
    registros + fila en el diario `sync_changes` + contador `sync_heads`.
  - Cada astronauta va a su propia partición `<prefijo><astronaut_key>` (por defecto
    `pulsys-tech-test-`): NO toca las particiones de los astronautas reales.
  - Es idempotente: el `record_id` sale del astronauta y el `reading_id`; reenviar una
    lectura da "duplicado" y no consume número de secuencia.
  - Guarda hasta dónde llegó cada astronauta en `backups/dynamo-telemetry-state.json`.

Envío en tandas
  - Una transacción lleva hasta 49 lecturas (98 escrituras + el contador = 99 de las 100 que
    permite DynamoDB). Cada lectura conserva su propio número `seq` consecutivo, como pide el
    contrato; solo se junta el viaje de red. Con 62 lecturas/min eso son 1-2 solicitudes por minuto
    y astronauta en vez de ~10.
  - `--watch` repite el envío cada `--interval` segundos (30-60, por defecto 45).
  - Carril urgente (con `--watch`): cada pocos segundos mira la bandeja de salida de la app y
    manda de inmediato los eventos con prioridad `urgent` y las evaluaciones con escalado
    `urgent`, sin esperar la tanda. Van a las tablas de eventos/evaluaciones (partición
    `<prefijo>device`). Si el usuario de AWS no tiene permiso sobre esa tabla, lo avisa y reintenta
    cada minuto; nada se da por enviado.
  - Los valores clínicos nunca se interpretan aquí: "urgente" es lo que ya decidió la app.

Uso (las credenciales se leen de server/.env y nunca se imprimen)
  python tools/sync_telemetry_dynamo.py --dry-run            # solo cuenta lo pendiente
  python tools/sync_telemetry_dynamo.py --send --limit 20    # envía como máximo 20 por astronauta
  python tools/sync_telemetry_dynamo.py --send               # envía todo lo pendiente y termina
  python tools/sync_telemetry_dynamo.py --watch --interval 45  # tandas continuas + carril urgente
  python tools/sync_telemetry_dynamo.py --purge              # borra lo escrito en las particiones de prueba
"""
import argparse, threading, datetime, hashlib, hmac, json, os, re, sqlite3, struct, sys, time, uuid
import urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GENESIS = "0" * 64


def load_env():
    env = {}
    with open(os.path.join(ROOT, "server", ".env"), encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                env[k.strip()] = v.strip()
    return env


ENV = load_env()
AK, SK = ENV["AWS_ACCESS_KEY_ID"], ENV["AWS_SECRET_ACCESS_KEY"]
REGION = ENV.get("AWS_REGION", "us-east-1")
TOKEN = ENV.get("AWS_SESSION_TOKEN") or None
T_REC = ENV["DYNAMODB_TABLE_RECORDS"]
T_CH = ENV["DYNAMODB_TABLE_CHANGES"]
T_HD = ENV["DYNAMODB_TABLE_HEADS"]
T_EVT = ENV.get("DYNAMODB_TABLE_EVENTS", "health_events")
T_EVAL = ENV.get("DYNAMODB_TABLE_EVALUATIONS", "health_evaluations")
MAX_BATCH = 49  # 2 escrituras por lectura + 1 del contador <= 100 por transacción


# ---------- AWS SigV4 mínimo (sin dependencias) ----------
def _hmac(k, m):
    return hmac.new(k, m.encode(), hashlib.sha256).digest()


def ddb(target, payload):
    host = f"dynamodb.{REGION}.amazonaws.com"
    body = json.dumps(payload)
    t = datetime.datetime.now(datetime.timezone.utc)
    amz, ds = t.strftime("%Y%m%dT%H%M%SZ"), t.strftime("%Y%m%d")
    hdrs = {"content-type": "application/x-amz-json-1.0", "host": host, "x-amz-date": amz,
            "x-amz-target": f"DynamoDB_20120810.{target}"}
    if TOKEN:
        hdrs["x-amz-security-token"] = TOKEN
    names = sorted(hdrs)
    canon_h = "".join(f"{n}:{hdrs[n]}\n" for n in names)
    signed = ";".join(names)
    canon = f"POST\n/\n\n{canon_h}\n{signed}\n{hashlib.sha256(body.encode()).hexdigest()}"
    scope = f"{ds}/{REGION}/dynamodb/aws4_request"
    sts = f"AWS4-HMAC-SHA256\n{amz}\n{scope}\n{hashlib.sha256(canon.encode()).hexdigest()}"
    key = _hmac(_hmac(_hmac(_hmac(("AWS4" + SK).encode(), ds), REGION), "dynamodb"), "aws4_request")
    hdrs["authorization"] = (f"AWS4-HMAC-SHA256 Credential={AK}/{scope}, SignedHeaders={signed}, "
                             f"Signature={hmac.new(key, sts.encode(), hashlib.sha256).hexdigest()}")
    req = urllib.request.Request(f"https://{host}/", data=body.encode(), method="POST",
                                 headers={k: v for k, v in hdrs.items() if k != "host"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            status, text = r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        status, text = e.code, e.read().decode()
    except Exception as e:  # red caída, timeout…
        return 0, {"__type": type(e).__name__, "message": str(e)}
    try:
        return status, json.loads(text)
    except ValueError:
        return status, {"message": text[:200]}


def S(x): return {"S": x}
def N(x): return {"N": str(x)}


def to_attr(v):
    if isinstance(v, bool):
        return {"BOOL": v}
    if isinstance(v, (int, float)):
        return N(v)
    if isinstance(v, str):
        return S(v)
    if v is None:
        return {"NULL": True}
    if isinstance(v, list):
        return {"L": [to_attr(x) for x in v]}
    return {"M": {k: to_attr(x) for k, x in v.items()}}


def iso(ms):
    return datetime.datetime.fromtimestamp(ms / 1000, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


# ---------- SQLite ----------
def default_db():
    return os.path.join(os.environ.get("APPDATA", ""), "com.pulsys.app", "pulsys.sqlite3")


def row_hash(r):
    h = hashlib.sha256(b"merken-telemetry-v1")
    def feed(b): h.update(struct.pack(">Q", len(b)) + b)
    for f in (r["astronaut_key"], r["indicator"], r["value_json"], r["unit"]):
        feed(f.encode("utf-8"))
    feed(struct.pack(">q", r["observed_at"]))
    for f in (r["batch_id"], r["source"], r["app_version"], r["prev_hash"]):
        feed(f.encode("utf-8"))
    return h.hexdigest()


def read_readings(db_path):
    con = sqlite3.connect("file:" + db_path.replace("\\", "/") + "?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    con.execute("BEGIN")  # un solo snapshot de lectura
    rows = [dict(r) for r in con.execute("SELECT * FROM telemetry_readings ORDER BY reading_id")]
    con.close()
    return rows


def verify_chain(rows):
    last = {}
    for r in rows:
        k = (r["astronaut_key"], r["indicator"])
        if r["prev_hash"] != last.get(k, GENESIS) or row_hash(r) != r["row_hash"]:
            return r["reading_id"]
        last[k] = r["row_hash"]
    return None


def to_record(r, partition, now):
    record_id = str(uuid.uuid5(uuid.NAMESPACE_URL, f"pulsys-telemetry:{r['astronaut_key']}:{r['reading_id']}"))
    note = f"TECH TEST - safe to delete. SQLite reading_id={r['reading_id']}, row_hash={r['row_hash']}"
    item = {
        "astronaut_id": S(partition), "record_id": S(record_id), "indicator": S(r["indicator"]),
        "value": to_attr(json.loads(r["value_json"])), "unit": S(r["unit"]),
        "measured_at": S(iso(r["observed_at"])), "logged_at": S(iso(r["observed_at"])),
        "source": S("garmin"), "external_id": S(f"garmin:pulsys-telemetry-{r['astronaut_key']}-{r['reading_id']}"),
        "note": S(note), "received_at": S(now),
        "created_by": to_attr({"type": "ground_service", "id": "pulsys-tech-test"}),
        "schema_version": N(1), "app_version": S(r["app_version"]),
    }
    return record_id, item


# ---------- Publicación (§4 del contrato), en tandas ----------
def retryable(s, r):
    t = r.get("__type", "") if isinstance(r, dict) else ""
    return s == 0 or s >= 500 or "Throttl" in t or "ProvisionedThroughput" in t or "InternalServer" in t


def err_text(s, r):
    m = (r.get("message") or r.get("Message") or "") if isinstance(r, dict) else str(r)
    a = re.search(r"perform: (\S+)", m)
    kind = (r.get("__type", "") if isinstance(r, dict) else "").split("#")[-1]
    return f"HTTP {s} {kind} {('denied ' + a.group(1)) if a else m[:160]}".strip()


def publish_batch(table, partition, entity, items):
    """Publica 1..49 entidades en UNA transacción: entidades + diario + contador.

    `items` es una lista de (entity_id, item). Cada una recibe su propio seq consecutivo.
    Devuelve "sent", "duplicate" (una sola y ya existía) o "has_duplicates" (tanda con repetidas:
    el llamador debe reenviarlas de una en una). Lanza RuntimeError con el motivo si falla.
    """
    k = len(items)
    assert 1 <= k <= MAX_BATCH
    for attempt in range(6):
        s, r = ddb("GetItem", {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "ConsistentRead": True})
        if s != 200:
            if retryable(s, r) and attempt < 5:
                time.sleep(0.4 * 2 ** attempt); continue
            raise RuntimeError(f"read heads: {err_text(s, r)}")
        n = int(r["Item"]["last_seq"]["N"]) if r.get("Item") else 0
        actions = []
        for i, (eid, item) in enumerate(items):
            entity_item = dict(item, seq=N(n + 1 + i))
            actions.append({"Put": {"TableName": table, "Item": entity_item,
                                    "ConditionExpression": "attribute_not_exists(#pk)",
                                    "ExpressionAttributeNames": {"#pk": "astronaut_id"}}})
            actions.append({"Put": {"TableName": T_CH, "Item": {
                "astronaut_id": S(partition), "seq": N(n + 1 + i), "entity": S(entity), "entity_id": S(eid),
                "payload": {"M": entity_item}, "received_at": item["received_at"]}}})
        if n == 0:
            head = {"Update": {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "UpdateExpression": "SET #l = :next",
                    "ConditionExpression": "attribute_not_exists(#pk)",
                    "ExpressionAttributeNames": {"#l": "last_seq", "#pk": "astronaut_id"},
                    "ExpressionAttributeValues": {":next": N(k)}}}
        else:
            head = {"Update": {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "UpdateExpression": "SET #l = :next",
                    "ConditionExpression": "#l = :exp", "ExpressionAttributeNames": {"#l": "last_seq"},
                    "ExpressionAttributeValues": {":next": N(n + k), ":exp": N(n)}}}
        actions.append(head)
        s, r = ddb("TransactWriteItems", {"TransactItems": actions})
        if s == 200:
            return "sent"
        if r.get("__type", "").endswith("TransactionCanceledException"):
            reasons = [c.get("Code") for c in r.get("CancellationReasons", [])]
            if any(reasons[2 * i] == "ConditionalCheckFailed" for i in range(k) if 2 * i < len(reasons)):
                return "duplicate" if k == 1 else "has_duplicates"
            if "TransactionConflict" in reasons or (reasons and reasons[-1] == "ConditionalCheckFailed"):
                time.sleep(0.2 * (attempt + 1)); continue  # otro escritor: repetir desde la lectura del contador
        if retryable(s, r) and attempt < 5:
            time.sleep(0.4 * 2 ** attempt); continue
        raise RuntimeError(f"transaction: {err_text(s, r)}")
    raise RuntimeError("transaction: too many retries")


def publish_many(table, partition, entity, items):
    """Envía en tandas; si una tanda trae repetidas, reenvía esa tanda una por una."""
    sent = dup = 0
    for i in range(0, len(items), MAX_BATCH):
        chunk = items[i:i + MAX_BATCH]
        res = publish_batch(table, partition, entity, chunk)
        if res == "sent":
            sent += len(chunk)
        elif res == "duplicate":
            dup += 1
        else:
            for one in chunk:
                r1 = publish_batch(table, partition, entity, [one])
                sent += r1 == "sent"
                dup += r1 == "duplicate"
    return sent, dup


# ---------- Estado, envío y limpieza ----------
STATE = os.path.join(ROOT, "backups", "dynamo-telemetry-state.json")
STATE_LOCK = threading.Lock()


def load_state():
    try:
        with open(STATE, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_state(state):
    os.makedirs(os.path.dirname(STATE), exist_ok=True)
    tmp = STATE + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=2)
    for attempt in range(10):  # en Windows el antivirus puede bloquear el archivo un instante
        try:
            os.replace(tmp, STATE)
            return
        except PermissionError:
            time.sleep(0.1 * (attempt + 1))
    # Si no se pudo anotar el avance no pasa nada grave: al repetir, esas lecturas salen como duplicadas.


def send_astronaut(partition, key, rows, limit, state):
    done = dup = 0
    now = iso(int(time.time() * 1000))
    rows = rows[:limit] if limit else rows
    for i in range(0, len(rows), MAX_BATCH):
        chunk = rows[i:i + MAX_BATCH]
        items = [to_record(r, partition, now) for r in chunk]
        try:
            sent, d = publish_many(T_REC, partition, "record", items)
        except RuntimeError as e:
            return key, done, dup, f"readings {chunk[0]['reading_id']}-{chunk[-1]['reading_id']}: {e}"
        done += sent
        dup += d
        with STATE_LOCK:
            state[partition] = {"last_reading_id": chunk[-1]["reading_id"]}
            save_state(state)
    return key, done, dup, None


def pending_by_astronaut(rows, state, prefix, only=None):
    out = {}
    for r in rows:
        if only and r["astronaut_key"] != only:
            continue
        if r["reading_id"] > state.get(prefix + r["astronaut_key"], {}).get("last_reading_id", 0):
            out.setdefault(r["astronaut_key"], []).append(r)
    return out


def run_once(a, state):
    rows = read_readings(a.db)
    bad = verify_chain(rows)
    if bad is not None:
        raise SystemExit(f"CHAIN BROKEN at reading_id {bad}: nothing was sent")
    by_astro = pending_by_astronaut(rows, state, a.prefix, a.astronaut)
    total = sum(len(v) for v in by_astro.values())
    if not by_astro:
        return len(rows), 0, 0, False
    with ThreadPoolExecutor(max_workers=min(6, len(by_astro))) as ex:
        futs = [ex.submit(send_astronaut, a.prefix + k, k, v, a.limit, state) for k, v in by_astro.items()]
        sent = dup = 0
        failed = False
        for f in futs:
            k, s_, d_, err = f.result()
            sent += s_; dup += d_
            if err:
                failed = True
                print(f"  astronaut {k}: STOPPED: {err}")
    return len(rows), sent, dup, failed


# ---------- Carril urgente ----------
def urgent_candidates(db_path, state):
    """Eventos con prioridad urgent y evaluaciones con escalado urgent aún no enviados."""
    con = sqlite3.connect("file:" + db_path.replace("\\", "/") + "?mode=ro", uri=True)
    sent = set(state.get("_urgent_sent", []))
    out = []
    for oid, entity, entity_id, payload in con.execute(
            "SELECT outbox_id, entity, entity_id, payload_json FROM outbox "
            "WHERE state = 'pending' AND entity IN ('event', 'evaluation') ORDER BY outbox_id"):
        if entity_id in sent:
            continue
        p = json.loads(payload)
        if (entity == "event" and p.get("priority") == "urgent") or (entity == "evaluation" and p.get("escalation") == "urgent"):
            out.append((entity, entity_id, p))
    con.close()
    return out


def send_urgent(a, state, warned):
    part = a.prefix + "device"
    now = iso(int(time.time() * 1000))
    for entity, eid, p in urgent_candidates(a.db, state):
        item = {k: to_attr(v) for k, v in p.items() if v is not None}
        item.update({"astronaut_id": S(part), "received_at": S(now),
                     "created_by": to_attr({"type": "ground_service", "id": "pulsys-tech-test"})})
        table = T_EVT if entity == "event" else T_EVAL
        try:
            publish_batch(table, part, entity, [(eid, item)])
        except RuntimeError as e:
            if (table, "x") not in warned or time.time() - warned[(table, "x")] > 60:
                print(f"[urgent] {entity} {eid} NOT sent, will retry: {e}")
                warned[(table, "x")] = time.time()
            continue
        with STATE_LOCK:
            state.setdefault("_urgent_sent", []).append(eid)
            save_state(state)
        print(f"[urgent] {entity} {eid} sent immediately")


def urgent_loop(a, state, stop):
    warned = {}
    while not stop.wait(3):
        try:
            send_urgent(a, state, warned)
        except Exception as e:  # el carril nunca debe morir por un fallo puntual
            print(f"[urgent] error: {type(e).__name__}: {e}")


def purge(prefix):
    state = load_state()
    parts = [p for p in state if p.startswith(prefix)] + ([prefix + "device"] if state.get("_urgent_sent") else [])
    for p in dict.fromkeys(parts):
        for table, sk in ((T_REC, "record_id"), (T_CH, "seq"), (T_EVT, "event_id"), (T_EVAL, "evaluation_id")):
            lek = None
            while True:
                q = {"TableName": table, "KeyConditionExpression": "astronaut_id = :a",
                     "ExpressionAttributeValues": {":a": S(p)}, "ProjectionExpression": f"astronaut_id, {sk}"}
                if lek: q["ExclusiveStartKey"] = lek
                s, r = ddb("Query", q)
                if s != 200:
                    if table in (T_EVT, T_EVAL): break  # sin permiso: nunca escribimos ahí
                    print("query failed:", err_text(s, r)); return
                for it in r["Items"]:
                    ddb("DeleteItem", {"TableName": table, "Key": {"astronaut_id": it["astronaut_id"], sk: it[sk]}})
                lek = r.get("LastEvaluatedKey")
                if not lek: break
        ddb("DeleteItem", {"TableName": T_HD, "Key": {"astronaut_id": S(p)}})
        state.pop(p, None)
        print("purged partition:", p)
    state.pop("_urgent_sent", None)
    save_state(state)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--db", default=default_db(), help="ruta de pulsys.sqlite3 (por defecto la de la app)")
    ap.add_argument("--prefix", default="pulsys-tech-test-", help="prefijo de las particiones de prueba")
    ap.add_argument("--limit", type=int, default=0, help="máximo de lecturas por astronauta en esta ejecución")
    ap.add_argument("--astronaut", help="enviar solo este astronaut_key")
    ap.add_argument("--interval", type=int, default=45, help="segundos entre tandas con --watch (30-60)")
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--dry-run", action="store_true")
    g.add_argument("--send", action="store_true")
    g.add_argument("--watch", action="store_true")
    g.add_argument("--purge", action="store_true")
    a = ap.parse_args()
    if a.purge:
        return purge(a.prefix)
    a.interval = max(30, min(60, a.interval))

    state = load_state()
    if a.dry_run:
        rows = read_readings(a.db)
        bad = verify_chain(rows)
        if bad is not None:
            sys.exit(f"CHAIN BROKEN at reading_id {bad}")
        print(f"chain ok: {len(rows)} readings")
        by = pending_by_astronaut(rows, state, a.prefix, a.astronaut)
        for k, v in sorted(by.items()):
            print(f"  astronaut {k}: {len(v)} pending -> partition {a.prefix}{k} ({-(-len(v) // MAX_BATCH)} transactions)")
        print("urgent pending:", [(e, i) for e, i, _ in urgent_candidates(a.db, state)] or "none")
        return print("nothing sent")

    if a.send:
        t0 = time.time()
        total, sent, dup, failed = run_once(a, state)
        print(f"chain ok: {total} readings | sent {sent}, duplicates {dup} in {time.time() - t0:.1f}s"
              + (" | with errors (rerun to resume)" if failed else ""))
        sys.exit(1 if failed else 0)

    stop = threading.Event()
    threading.Thread(target=urgent_loop, args=(a, state, stop), daemon=True).start()
    print(f"watching: batch every {a.interval}s, urgent lane every 3s (Ctrl+C to stop)")
    try:
        while True:
            t0 = time.time()
            try:
                total, sent, dup, failed = run_once(a, state)
                print(time.strftime("%H:%M:%S"), f"sent {sent}, duplicates {dup}"
                      + (" | errors, will retry next batch" if failed else ""), flush=True)
            except SystemExit as e:
                print("STOPPED:", e); break
            except Exception as e:
                print(time.strftime("%H:%M:%S"), f"batch failed, will retry: {type(e).__name__}: {e}", flush=True)
            time.sleep(max(1, a.interval - (time.time() - t0)))
    except KeyboardInterrupt:
        pass
    finally:
        stop.set()


if __name__ == "__main__":
    main()
