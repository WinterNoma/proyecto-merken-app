#!/usr/bin/env python3
"""Prueba técnica: copia la telemetría local (SQLite) a DynamoDB, SIN pasar por la API.

Qué hace
  - Lee `telemetry_readings` en SOLO LECTURA (la app puede seguir abierta).
  - Comprueba antes la cadena de hashes; si está rota, no envía nada.
  - Convierte cada lectura a un `record` del contrato (docs/api/modelo-datos-backend.md)
    y lo publica con la transacción de 3 escrituras del §4: entidad en la tabla de
    registros + fila en el diario `sync_changes` + contador `sync_heads`.
  - Cada astronauta va a su propia partición `<prefijo><astronaut_key>` (por defecto
    `merken-tech-test-`): NO toca las particiones de los astronautas reales.
  - Es idempotente: el `record_id` sale del astronauta y el `reading_id`; reenviar una
    lectura da "duplicado" y no consume número de secuencia.
  - Guarda hasta dónde llegó cada astronauta en `backups/dynamo-telemetry-state.json`.

Uso (las credenciales se leen de server/.env y nunca se imprimen)
  python tools/sync_telemetry_dynamo.py --dry-run          # solo cuenta lo pendiente
  python tools/sync_telemetry_dynamo.py --send --limit 20  # envía como máximo 20 por astronauta
  python tools/sync_telemetry_dynamo.py --send             # envía todo lo pendiente
  python tools/sync_telemetry_dynamo.py --purge            # borra lo escrito en las particiones de prueba
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
    return os.path.join(os.environ.get("APPDATA", ""), "com.merken.app", "merken.sqlite3")


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
    record_id = str(uuid.uuid5(uuid.NAMESPACE_URL, f"merken-telemetry:{r['astronaut_key']}:{r['reading_id']}"))
    note = f"TECH TEST - safe to delete. SQLite reading_id={r['reading_id']}, row_hash={r['row_hash']}"
    item = {
        "astronaut_id": S(partition), "record_id": S(record_id), "indicator": S(r["indicator"]),
        "value": to_attr(json.loads(r["value_json"])), "unit": S(r["unit"]),
        "measured_at": S(iso(r["observed_at"])), "logged_at": S(iso(r["observed_at"])),
        "source": S("garmin"), "external_id": S(f"garmin:merken-telemetry-{r['astronaut_key']}-{r['reading_id']}"),
        "note": S(note), "received_at": S(now),
        "created_by": to_attr({"type": "ground_service", "id": "merken-tech-test"}),
        "schema_version": N(1), "app_version": S(r["app_version"]),
    }
    return record_id, item


# ---------- Publicación (§4 del contrato) ----------
def publish(partition, item, record_id):
    """Devuelve 'sent' | 'duplicate' | lanza RuntimeError con el motivo."""
    for attempt in range(6):
        s, r = ddb("GetItem", {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "ConsistentRead": True})
        if s != 200:
            if retryable(s, r) and attempt < 5:
                time.sleep(0.4 * 2 ** attempt); continue
            raise RuntimeError(f"read heads: {err_text(s, r)}")
        n = int(r["Item"]["last_seq"]["N"]) if r.get("Item") else 0
        entity = dict(item, seq=N(n + 1))
        change = {"astronaut_id": S(partition), "seq": N(n + 1), "entity": S("record"), "entity_id": S(record_id),
                  "payload": {"M": entity}, "received_at": item["received_at"]}
        if n == 0:
            head = {"Update": {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "UpdateExpression": "SET #l = :next",
                    "ConditionExpression": "attribute_not_exists(#pk)",
                    "ExpressionAttributeNames": {"#l": "last_seq", "#pk": "astronaut_id"},
                    "ExpressionAttributeValues": {":next": N(1)}}}
        else:
            head = {"Update": {"TableName": T_HD, "Key": {"astronaut_id": S(partition)}, "UpdateExpression": "SET #l = :next",
                    "ConditionExpression": "#l = :exp", "ExpressionAttributeNames": {"#l": "last_seq"},
                    "ExpressionAttributeValues": {":next": N(n + 1), ":exp": N(n)}}}
        s, r = ddb("TransactWriteItems", {"TransactItems": [
            {"Put": {"TableName": T_REC, "Item": entity, "ConditionExpression": "attribute_not_exists(#pk)",
                     "ExpressionAttributeNames": {"#pk": "astronaut_id"}, "ReturnValuesOnConditionCheckFailure": "ALL_OLD"}},
            {"Put": {"TableName": T_CH, "Item": change}},
            head]})
        if s == 200:
            return "sent"
        if r.get("__type", "").endswith("TransactionCanceledException"):
            reasons = [c.get("Code") for c in r.get("CancellationReasons", [])]
            if reasons and reasons[0] == "ConditionalCheckFailed":
                return "duplicate"          # ya existía ese record_id
            if "TransactionConflict" in reasons or (len(reasons) > 2 and reasons[2] == "ConditionalCheckFailed"):
                time.sleep(0.2 * (attempt + 1)); continue  # otro escritor: repetir desde la lectura del contador
        if retryable(s, r) and attempt < 5:
            time.sleep(0.4 * 2 ** attempt); continue
        raise RuntimeError(f"transaction: {err_text(s, r)}")
    raise RuntimeError("transaction: too many retries")


def retryable(s, r):
    t = r.get("__type", "") if isinstance(r, dict) else ""
    return s == 0 or s >= 500 or "Throttl" in t or "ProvisionedThroughput" in t or "InternalServer" in t


def err_text(s, r):
    m = (r.get("message") or r.get("Message") or "") if isinstance(r, dict) else str(r)
    a = re.search(r"perform: (\S+)", m)
    return f"HTTP {s} {r.get('__type', '') if isinstance(r, dict) else ''} {('denied ' + a.group(1)) if a else m[:160]}".strip()


# ---------- Estado, envío y limpieza ----------
STATE = os.path.join(ROOT, "backups", "dynamo-telemetry-state.json")


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


def send_astronaut(partition, key, rows, limit, state, lock_state):
    done = dup = 0
    now = iso(int(time.time() * 1000))
    for r in rows[:limit] if limit else rows:
        record_id, item = to_record(r, partition, now)
        try:
            res = publish(partition, item, record_id)
        except RuntimeError as e:
            return key, done, dup, f"reading {r['reading_id']}: {e}"
        done += res == "sent"
        dup += res == "duplicate"
        with lock_state:
            state[partition] = {"last_reading_id": r["reading_id"]}
            if r["reading_id"] % 10 == 0:  # no hace falta escribir el archivo en cada lectura
                save_state(state)
    with lock_state:
        save_state(state)
    return key, done, dup, None


def purge(prefix):
    state = load_state()
    parts = [p for p in state if p.startswith(prefix)]
    for p in parts:
        for table, sk in ((T_REC, "record_id"), (T_CH, "seq")):
            lek = None
            while True:
                q = {"TableName": table, "KeyConditionExpression": "astronaut_id = :a",
                     "ExpressionAttributeValues": {":a": S(p)}, "ProjectionExpression": f"astronaut_id, {sk}"}
                if lek: q["ExclusiveStartKey"] = lek
                s, r = ddb("Query", q)
                if s != 200: print("query failed:", err_text(s, r)); return
                for it in r["Items"]:
                    ddb("DeleteItem", {"TableName": table, "Key": {"astronaut_id": it["astronaut_id"], sk: it[sk]}})
                lek = r.get("LastEvaluatedKey")
                if not lek: break
        ddb("DeleteItem", {"TableName": T_HD, "Key": {"astronaut_id": S(p)}})
        state.pop(p, None)
        print("purged partition:", p)
    save_state(state)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--db", default=default_db(), help="ruta de merken.sqlite3 (por defecto la de la app)")
    ap.add_argument("--prefix", default="merken-tech-test-", help="prefijo de las particiones de prueba")
    ap.add_argument("--limit", type=int, default=0, help="máximo de lecturas por astronauta en esta ejecución")
    ap.add_argument("--astronaut", help="enviar solo este astronaut_key")
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--dry-run", action="store_true")
    g.add_argument("--send", action="store_true")
    g.add_argument("--purge", action="store_true")
    a = ap.parse_args()
    if a.purge:
        return purge(a.prefix)

    rows = read_readings(a.db)
    bad = verify_chain(rows)
    if bad is not None:
        sys.exit(f"CHAIN BROKEN at reading_id {bad}: nothing was sent")
    print(f"chain ok: {len(rows)} readings")

    state = load_state()
    by_astro = {}
    for r in rows:
        if a.astronaut and r["astronaut_key"] != a.astronaut:
            continue
        part = a.prefix + r["astronaut_key"]
        if r["reading_id"] > state.get(part, {}).get("last_reading_id", 0):
            by_astro.setdefault(r["astronaut_key"], []).append(r)
    for k, v in sorted(by_astro.items()):
        print(f"  astronaut {k}: {len(v)} pending -> partition {a.prefix}{k}")
    if a.dry_run or not by_astro:
        return print("nothing sent" if a.dry_run else "nothing pending")

    lock = threading.Lock()
    t0 = time.time()
    with ThreadPoolExecutor(max_workers=min(6, len(by_astro))) as ex:
        futs = [ex.submit(send_astronaut, a.prefix + k, k, v, a.limit, state, lock) for k, v in by_astro.items()]
        failed = False
        for f in futs:
            k, sent, dup, err = f.result()
            print(f"  astronaut {k}: sent {sent}, duplicates {dup}" + (f", STOPPED: {err}" if err else ""))
            failed |= bool(err)
    print(f"done in {time.time() - t0:.1f}s" + (" with errors (rerun to resume)" if failed else ""))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
