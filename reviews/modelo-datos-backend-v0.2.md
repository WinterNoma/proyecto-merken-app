## Veredicto

**Listo para backend: no, todavía.** La v0.2 corrige la base del diseño, pero necesita cerrar el cursor, la resolución de correcciones y la autenticación/reintentos de Tierra antes de implementarla.
Revisión documental de solo lectura; no se modificaron archivos ni se verificó código ejecutándose. Se mantienen las decisiones del usuario.

## Bloqueantes B1–B6

- **B1 — Sí.** §4 usa `attribute_not_exists(#pk)` sobre la entidad identificada por ambas claves; evita sobrescrituras. [AWS: condiciones](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Expressions.ConditionExpressions.html).
- **B2 — Parcial.** El diario y el contador resuelven el orden; faltan inicialización/renovación de `through_seq` y avance exacto del cursor.
- **B3 — Sí.** Se elimina la doble escritura de `health_status` en MySQL; los defectos nuevos de su cálculo se indican abajo.
- **B4 — Parcial.** Hay permisos por operación y autoría del servidor, pero §8 presupone un astronauta en tokens de Tierra y el login tiene identidades ambiguas.
- **B5 — Sí.** El diario incluye las seis entidades y permite descargar desde cero; perfil y antecedentes se recuperan mediante `/me`.
- **B6 — Parcial.** Existen límites explícitos, pero falta definir el cursor cuando la respuesta se corta por 512 KiB antes de terminar la página consultada.

## Errores nuevos

- **§4: el diseño atómico es correcto.** Tres acciones sobre elementos distintos, con la condición dentro del `Update` del contador, permiten publicar entidad, diario y avance conjuntamente. La ausencia de condición en el `Put` del diario no rompe el algoritmo bajo la prohibición de escritores externos. Esta conclusión se deriva de la atomicidad documentada por [AWS: TransactWriteItems](https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_TransactWriteItems.html).
- **§7, L343–346: cursor incompleto.** Si `through_seq` permanece congelado después de terminar, nunca llegarán cambios posteriores. Si se devuelve el final consultado después de recortar la respuesta, se saltarán cambios no entregados.
- **§6, L279–281: resolución ambigua.** Con B y C reemplazando A, y D reemplazando B, pueden quedar C y D efectivos. Además, una corrección con fecha futura puede invalidar el dato anterior y luego quedar excluida del estado actual.
- **§8 y login: contexto de identidad incompleto.** Tierra elige el astronauta mediante la ruta; no lo obtiene de su token. Además, `username` es único por tabla: dos principales distintos pueden compartir credenciales de identificación y `/login` no distingue el tipo.
- **§4 y POST de Tierra: reintentos incompletos.** Generar un ID nuevo en cada intento duplica registros, revisiones o análisis. Comparar excluyendo solo tres campos tampoco basta si se regeneran `reviewed_at`, `created_at` o atributos derivados.
- **Contradicción menor:** §1 exige UUIDv7 para todos los IDs, pero §4 prescribe UUIDv5 para Garmin. Debe aclararse antes de escribir el validador.

## Correcciones exactas

1. **§4, paso 3, sustituir el texto del contador:** «Calcular `next = n + 1` en backend. Usar `UpdateExpression: SET #last = :next`, con `#last = last_seq`; condición `#last = :expected` si existe el contador, o `attribute_not_exists(#pk)` si no existe, con `#pk = astronaut_id`. Pasar valores numéricos mediante `ExpressionAttributeValues`. No añadir un `ConditionCheck` separado sobre ese mismo contador». [AWS: expresiones](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Expressions.UpdateExpressions.html), [restricción de acciones](https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_TransactWriteItems.html).

2. **§4, paso 4, añadir:** «Distinguir condición fallida, conflicto concurrente, falta de capacidad y errores de validación. Reintentar fallos transitorios con espera creciente; ante resultado incierto, consultar consistentemente la entidad antes de decidir. Comparar exclusivamente el DTO de entrada permitido y normalizado, excluyendo todos los campos generados o derivados por el servidor». [AWS: errores transaccionales](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html).

3. **§7, pull, añadir:** «Sin cursor: `last_seq = 0` y `through_seq = last_seq` del contador leído consistentemente, o cero si no existe. Al recibir un cursor cuyo recorrido terminó (`last_seq = through_seq`), conservar `last_seq` y capturar un nuevo `through_seq`. Durante cada recorrido, mantener este último fijo».

4. **§7, pull y §9, añadir:** «Consultar la tabla base con `ConsistentRead=true`, `ScanIndexForward=true`, partición autorizada y `seq BETWEEN :first AND :through`, donde `first = last_seq + 1`; no consultar si el intervalo está vacío. Entregar un prefijo de hasta 50 cambios cuya respuesta completa no supere 512 KiB. El cursor avanza únicamente hasta el último cambio entregado; `has_more = last_seq < through_seq`. Nunca avanzar sobre elementos consultados pero no entregados». La paginación interna debe respetar [AWS: Query](https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_Query.html).

5. **§6, sustituir las tres primeras reglas:** «Toda corrección conserva astronauta, indicador y, para síntomas, `value.body_zone`; `replaces` referencia un registro existente. Para calcular el estado, excluir primero los registros con `measured_at > received_at + 10 minutos`, conservándolos en el historial con `future_timestamp`. Agrupar los restantes por el registro raíz obtenido siguiendo `replaces`; en cada grupo solo es efectivo el de mayor `seq`. Entre efectivos, elegir el mayor `(measured_at, seq)` por indicador/zona».

6. **§8 y autenticación, sustituir/añadir:** «El token identifica `principal_type`, `principal_id`, rol y sesión. Para astronautas, la partición sale de la sesión; para Tierra, de la ruta, tras comprobar el permiso de operación. Login exige `principal_type: astronaut | ground_user`; responde con ese tipo, el ID y el rol. Las referencias de revisión pertenecen también a la misma recomendación».

7. **§1 y POST de Tierra, añadir:** «Los IDs móviles son UUIDv7; se permiten UUIDv5 deterministas generados por backend para Garmin y operaciones idempotentes de Tierra. Los POST de Tierra exigen `Idempotency-Key` estable por operación; el servidor deriva el ID usando un namespace fijo, principal, astronauta, operación y clave. Mismo ID y DTO normalizado → `duplicate`; contenido diferente → `id_conflict`. Conservar los campos del servidor del primer guardado».