## 1. Dictamen

**Mantendría abierta la etapa 1.** Cuatro de los cinco hallazgos están cerrados y el caso positivo de zonas está cubierto. La raíz se resuelve correctamente cuando ya llegó toda la cadena, pero confirmar mientras falta un antecesor todavía puede dejar dos pulsos efectivos. El formato obligatorio también sigue fallando.

| Hallazgo | Estado | Verificación |
|---|---|---|
| 1. Identidad del perfil | **Cerrado** | Un perfil existente exige un `astronaut_id` válido. Cambiar de A a B expira la acción con cero escrituras. Actualizar el nombre de B conserva su acción. La prueba de v5 verifica que el perfil sin ID se conserva hasta repararlo. [profile.rs:34](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/profile.rs:34) · [confirmation.rs:19](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_tests/confirmation.rs:19) |
| 2. Recibo con reloj fallido | **Cerrado** | La consulta de la clave ocurre dentro de `BEGIN IMMEDIATE`, antes de invocar el reloj. Tras reabrir, la prueba recupera el recibo y detecta el conflicto con **cero lecturas del reloj**. [pulse.rs:214](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:214) · [pulse_flow.rs:191](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/api/tests/pulse_flow.rs:191) |
| 3. Raíz de corrección | **Abierto, importante** | El caso original A–C–B queda resuelto. La prueba prepara la corrección mientras falta B, pero espera a que B llegue antes de intentar confirmarla. El caso restante se detalla abajo. [history.rs:57](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_tests/history.rs:57) |
| 4. Aprobación restaurada | **Cerrado** | Rust recalcula avisos y consecuencia DEMO. Restaurar `NotRequired` cuando se necesita aprobación devuelve `NeedsConfirmation` y conserva el perfil y las filas. [pulse.rs:114](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:114) · [confirmation.rs:101](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_tests/confirmation.rs:101) |
| 5. Límite del historial | **Cerrado** | El recorrido se detiene al reunir los ganadores pedidos. La prueba con 2.004 pulsos independientes y límite uno registra una llamada al validador SQL. Verifiqué los índices de fecha y de `replaces`. [pulse_history.rs:36](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_history.rs:36) · [history.rs:116](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_tests/history.rs:116) |

**Zonas — cerrado.** Tras reabrir la base, la prueba compara el `ZoneSnapshot` completo, incluidos el brazo izquierdo, los IDs de evidencia en orden, las demás zonas y la evaluación independiente del pulso. [zones.rs:119](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/api/tests/zones.rs:119)

## 2. Archivos tocados

Ninguno. Revisión en solo lectura.

## 3. Comandos y resultados

- `git diff` y `EXPLAIN QUERY PLAN` en SQLite en memoria. Confirmé el uso de `records_indicator_history` y `records_replaces`.
- `pulsys_store-c38963de354cf160.exe pulse_tests --skip a_v5_profile --skip schema_v5 --skip future_schema` → **10/10** en el binario existente.
- `cargo fmt --all -- --check` → **salida 1**, con diferencias en `api` y `store`.
- **NO EJECUTADO** por mí el conjunto completo ni Clippy. Claude reportó core **86/86**, store **69/69**, api **9/9 + 1/1** y Clippy sin avisos en api y store.

## 4. Pendiente para cerrar

**Corrección huérfana.** Reproduje con el SQL actual que, si C reemplaza a B ausente y se guarda D para corregir C, el recibo asigna a D su propio ID como raíz. El historial devuelve **D/D y C/C**. D quedó guardado y encolado, pero C sigue contando como efectivo. **Arreglo:** rechazar la corrección cuando `replaces` está presente y su raíz es irresoluble. Añadir una prueba que confirme antes de recibir B y compruebe cero escrituras nuevas. Después, aplicar el formato y verificar salida 0. El DTO durable de FRB continúa en E2. [target.rs:109](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse/target.rs:109) · [receipt.rs:85](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse/receipt.rs:85) · [pulse_history.rs:39](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_history.rs:39)