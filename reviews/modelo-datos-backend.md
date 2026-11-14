## Veredicto

**v0.1 necesita correcciones antes de implementarse:** permite sobrescrituras y no garantiza una sincronización completa.  
Las decisiones nuevas prevalecen: MySQL para identidad/perfil; DynamoDB para datos de la app; sin referencia previa al vuelo.  
Revisión documental en solo lectura; no se modificaron archivos ni se comprobó una implementación.

## Bloqueantes

- **B1 — La condición idempotente permite sobrescribir (§2, L89).** Las tablas tienen `record_id`, `event_id`, etc., pero ningún atributo `sk`. `attribute_not_exists(sk)` será verdadero incluso si existe el elemento. Usar `attribute_not_exists(#pk)`, con `#pk = astronaut_id`: se evalúa sobre el elemento identificado por **ambas claves**. [AWS: escrituras condicionales](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Expressions.ConditionExpressions.html).
- **B2 — El cursor no tiene un algoritmo implementable (§2 y §4).** Hay cinco tablas, empates en `updated_at`, paginación y relojes móviles potencialmente atrasados. Un único máximo temporal puede saltarse cambios. Además, los GSI se actualizan de forma eventual: añadir `#id` al timestamp resuelve empates, pero **no** los elementos publicados tarde en el índice. Recomiendo el diario único de cambios descrito abajo. [AWS: consistencia de GSI](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GSI.html).
- **B3 — La doble escritura puede dejar `health_status` incorrecto (L213).** Si DynamoDB guarda y MySQL falla, el reintento se acepta como duplicado sin asegurar la reparación. Tampoco se define qué significa «más reciente» ante correcciones, fechas iguales o relojes adelantados. Recomiendo retirar esta persistencia de MySQL; si se conserva, necesita proyección reconstruible, reintentos duraderos y reconciliación.
- **B4 — Faltan permisos por operación, además del aislamiento (§6).** Comparar `astronaut_id` no impide que el móvil envíe `source: earth`, `review`, un autor médico o una confirmación clínica falsa. Faltan identidades y permisos de Tierra, y la ruta de revisión ni siquiera incluye la partición necesaria para localizar la recomendación.
- **B5 — Cambiar de móvil no recupera todo (§4, L217).** El pull omite eventos, evaluaciones y recomendaciones originales sin revisión. Tampoco contempla cambios independientes de `health_status` ni antecedentes médicos. Debe recuperar todo el historial autorizado, aunque proceda del mismo dispositivo.
- **B6 — Faltan límites y paginación.** «Todo el historial» no puede significar una respuesta ilimitada. DynamoDB limita cada elemento a **400 KB**, incluidos nombres de atributos, y cada página de `Query` a **1 MB**; hay que continuar con `LastEvaluatedKey`, incluso tras una página filtrada vacía. Faltan límites de textos, listas, lotes y respuestas. [AWS: tamaños](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Constraints.html), [paginación](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Query.Pagination.html).

## Importantes

- **Idempotencia incompleta:** UUIDv7 permite identificar reintentos, pero no basta. Mismo ID y contenido distinto debe producir conflicto, no `accepted`. Debe compararse el contenido normalizado sin campos generados por el servidor. La respuesta necesita `(entity,id)`, resultado individual y distinción entre error permanente y reintentable.
- **Índices incompletos:** `astronaut_id#indicator` no identifica claramente un atributo materializado. Definir `indicator_pk`, su construcción, SK y proyección. Los empates en claves de GSI son válidos; no sobrescriben elementos, pero exigen paginación correcta.
- **Inmutabilidad contradictoria:** «Nada se sobrescribe» choca con `review`, `read_at`, perfil y estado actual. Delimitar qué es historial inmutable y qué es proyección mutable. Las revisiones deben conservar autor e historial; `read_at` carece de endpoint.
- **Correcciones sin resolución:** faltan validación del mismo propietario/indicador, tratamiento de referencias todavía no recibidas y resolución de ramas concurrentes. Una corrección con la misma fecha debe poder reemplazar el dato anterior sin contar ambos en gráficas o evaluaciones.
- **Sin referencia previa:** `no_reference` y el significado de `change` siguen ligados al diseño anterior. `health_status` es último dato conocido, no referencia ni prueba de normalidad. Las evaluaciones por alarma necesitan también `input_event_ids`.
- **Privacidad incompatible con la decisión nueva:** `private: true` impide enviar todo a Tierra y también recuperarlo en otro móvil. Retirarlo del contrato v0.2 conforme a la instrucción recibida.
- **Tiempo incompleto:** falta misión y origen del MET. Separar `measured_at`/`occurred_at` del móvil y `received_at` del servidor; UUIDv7 no constituye orden causal ni reloj fiable.
- **Garmin:** necesita clave de deduplicación del proveedor y correspondencia de usuario Garmin↔astronauta. No deduplicar solo por hora y valor.

Los rangos de §3 **no cumplen** su promesa de rechazar únicamente imposibles:

| Indicador | Hallazgo |
|---|---|
| Pulso 20–250 | Excluye valores documentados de [15 bpm](https://pmc.ncbi.nlm.nih.gov/articles/PMC8741918/) y [300 bpm](https://pmc.ncbi.nlm.nih.gov/articles/PMC6166305/). |
| Presión 50–260 / 30–160 | Excluye mediciones documentadas de [40/20](https://pmc.ncbi.nlm.nih.gov/articles/PMC12355791/) y [300/180 mmHg](https://pmc.ncbi.nlm.nih.gov/articles/PMC8110033/). |
| Temperatura 30–45 | El mínimo excluye hipotermia real: se documentó una paciente con [14 °C](https://pubmed.ncbi.nlm.nih.gov/41767209/). |
| Glucosa 10–1000 | El máximo excluye hiperglucemia real: existe un caso de [2375 mg/dL](https://pmc.ncbi.nlm.nih.gov/articles/PMC13043460/). |
| Sueño 0–24 | Válido únicamente si representa sueño acumulado en una ventana explícita de 24 horas. Falta esa definición. |
| Ánimo, cefalea y síntoma 0–10 | Correctos como límites de una escala definida; especificar si solo admite enteros. |

Estos casos demuestran que los límites actuales son incorrectos; **no establecen nuevos límites universales**. Convertirlos en advertencias de comprobación, sin descartar automáticamente una medición extrema.

## Menores

- Las filas con `| null` y alternativas separadas por `|` rompen varias tablas Markdown. Definir obligatoriedad y nulabilidad en columnas separadas.
- La regla «nunca `0`» debe decir «no usar cero para representar ausencia»: cero es válido para sueño y puntuaciones.
- La API debe usar JSON normal; `S`, `M` y `L` describen almacenamiento DynamoDB. Fijar esquema, enums de zonas, unidades, precisión y política de campos desconocidos.
- Añadir un indicador no exige migración de tabla, pero sí actualizar validadores, catálogo y consumidores.
- `GET /me.astronaut` debe ser un DTO explícito que excluya `password_hash` y datos de sesión.

## Recorte para mañana

- **Conservar:** login, perfil verificado, ocho indicadores, registro local persistente, reintentos, historial completo, alarmas, recomendaciones v5 y revisión/análisis de Tierra.
- **Posponer:** Garmin real —usar registros sintéticos identificados—, automatización del modelo terrestre, confirmaciones de lectura y panel avanzado. Mantener publicación manual de análisis y revisiones.
- Evitar los cinco GSI `by_updated` y la doble escritura MySQL↔DynamoDB. Un diario común resuelve el pull sin sincronizar cinco cursores.
- Prueba mínima de aceptación: cortar red tras guardar pero antes de responder; reintentar; descargar varias páginas; recuperar en otro móvil; recibir una corrección terrestre; rechazar acceso cruzado y autoría médica desde el móvil.

## Cambios propuestos para v0.2

1. **Reglas comunes:** «El historial clínico es inmutable. Perfil y proyecciones son actualizables. IDs de origen móvil se generan localmente; Tierra y Garmin generan los suyos en backend. El servidor asigna `received_at`; ningún reloj cliente controla el cursor».
2. **Claves:** conservar las PK/SK declaradas y sustituir L89 por `ConditionExpression: attribute_not_exists(#pk)`, `ExpressionAttributeNames: {"#pk":"astronaut_id"}`. Mismo contenido normalizado → `duplicate`; distinto → `id_conflict`.
3. **Diario:** crear `merken_changes`: PK `astronaut_id` (`S`), SK `seq` (`N`), campos `entity`, `entity_id`, `payload`, `received_at`. Crear `merken_sync_heads`: PK `astronaut_id`, `last_seq`.
4. **Publicación atómica:** leer el contador consistentemente; transacción con escritura de entidad, entrada de diario y avance condicionado del contador esperado. Si compite otro escritor, reintentar. Así no se publican secuencias posteriores dejando huecos pendientes. Aplicar a móvil, Garmin y Tierra; no hacer escrituras externas que eludan este recorrido.
5. **Pull:** `GET /v1/sync/pull?cursor=…&limit=…` → `{changes:[{seq,entity,id,payload}],next_cursor,has_more}`. Consultar el diario con lectura fuerte, en orden ascendente. Cursor opaco firmado: `{v,astronaut_id,last_seq,through_seq}`; `through_seq` fija el final del recorrido. SQLite aplica página y cursor en una sola transacción. Sin cursor comienza en cero.
6. **Perfil y estado:** MySQL conserva identidad, sesiones y perfil verificado. Para la demo, derivar `/me.health_status` de los registros efectivos de DynamoDB, incluyendo mediciones verificadas introducidas por Tierra. Clave lógica `(indicator,body_zone)` para síntomas. Resolver sustituciones y desempatar por `(measured_at,seq)`; fechas dudosas requieren comprobación antes de desplazar el estado actual.
7. **Versión de perfil:** añadir `profile_version` al agregado MySQL; incrementarlo transaccionalmente al cambiar perfil o antecedentes. Consultar `/me` al reconectar, independientemente del cursor DynamoDB. Leer el agregado en una transacción consistente.
8. **Revisión:** crear entidad `suggestion_review` inmutable: `{review_id,suggestion_id,status,corrected_text,reviewer_id,reviewed_at,replaces}`. `corrected_text` obligatorio para `corrected`. Usar `POST /v1/astronauts/{astronaut_id}/suggestions/{suggestion_id}/reviews`; autor y fecha los fija el servidor. Retirar `read_at` por ahora.
9. **Autorización:** el servidor obtiene astronauta/sesión del token y valida cualquier ID recibido contra ellos. Permisos separados `astronaut`, `ground_reviewer`, `ground_service`; el móvil solo publica las cuatro entidades permitidas. Referencias siempre del mismo astronauta; `confirmed` exige identidad autorizada. Modelo terrestre escribe mediante API autenticada.
10. **Tokens:** definir formato, expiración y revocación. Propuesta sencilla: tokens opacos aleatorios, hashes solo en servidor, acceso 15 minutos y refresh 30 días; añadir expiraciones y sesión a MySQL. Rotar refresh transaccionalmente y detectar reutilización; `device_id` no autentica. HTTPS obligatorio, login limitado por intentos y mensajes de credenciales genéricos. [Referencia: RFC 9700 §4.14](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14).
11. **Días sin red:** conservar acceso local y bandeja de salida aunque caduquen tokens. Al reconectar, renovar antes de sincronizar; refresh expirado/revocado → pedir login para sincronizar, sin borrar datos ni bloquear funciones locales. Una revocación remota no puede hacerse efectiva sin comunicación. Proteger tokens mediante cifrado con clave de [Android Keystore](https://developer.android.com/privacy-and-security/keystore), sin añadir bloqueo de app; verificar soporte de la librería Tauri elegida.
12. **Límites propuestos, no límites AWS:** 50 operaciones y 1 MiB por push; 32 KiB de JSON UTF-8 por entidad; pull hasta 50 cambios y 512 KiB de respuesta. No truncar: responder `payload_too_large`. Cada resultado incluye `{entity,id,status,code,retryable}`; `accepted` significa persistido duraderamente, nunca revisado.
13. **Catálogo y tiempo:** sustituir rangos fisiológicos por advertencias; validar tipos, unidades, finitud y restricciones de dominio justificadas. Definir sueño con `window_start`/`window_end`. Añadir `mission_id`, `mission_epoch_utc` y versión; calcular `MET_ms = instant_utc - mission_epoch_utc`. Sustituir `no_reference` por motivos acordes al estado actual, sin convertir ausencia en `stable`.
14. **GSI opcional:** si el panel lo necesita, `by_indicator`: PK `indicator_pk = astronaut_id + "#" + indicator`, SK `measurement_key = measured_at + "#" + record_id`, proyección explícita. Fijar versión del SDK y verificar marshalling, paginación y transacciones; los límites citados corresponden a la documentación AWS consultada. La idempotencia persistente no debe depender del token transaccional AWS, cuya ventana documentada es de [10 minutos](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html).