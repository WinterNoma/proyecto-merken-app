Encontré **3 hallazgos de severidad Media**. Revisión solo lectura; no modifiqué archivos e ignoré los dos cambios staged indicados.

1. **Media — Un 401/403 durante Sync no elimina la sesión anterior.**  
   [src-tauri/src/recommendations.rs:138](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/src-tauri/src/recommendations.rs:138), [recommendations.rs:153](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/src-tauri/src/recommendations.rs:153) y [ui/src/ui/recommendations.ts:220](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:220).  
   Tras un Sync correcto, otro intento puede recibir 401/403 del login o de la descarga. El primero devuelve un error genérico; el segundo propaga «session expired». **Ninguno borra la cookie anterior**: ese borrado existe únicamente en `recommendations_refresh_backend`. Además, el catch de Sync mantiene `sessionActive` y el temporizador, incluso ante expiración.  
   **Arreglo:** centralizar el tratamiento de 401/403 para ambos comandos, borrar la cookie y devolver un error estructurado de sesión caducada. La UI debe desactivar la sesión y detener el temporizador también cuando ese error procede de Sync.

2. **Media — El refresco cambia silenciosamente la selección «On-device profile».**  
   [ui/src/ui/recommendations.ts:170](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:170), en combinación con `roster`, líneas 99–108.  
   `acceptSnapshot()` llama a `roster(next)` con `selectSnapshot=true`. Si el usuario había seleccionado el perfil local (`""`), `preferred` termina siendo el primer astronauta con Garmin y lo selecciona. Se reproduce en el siguiente refresco, sin intervención del usuario. También ocurre si desaparece el astronauta seleccionado: se elige otro automáticamente.  
   **Arreglo:** distinguir la selección inicial de la actualización periódica. Durante el refresco, conservar explícitamente `""` o el ID existente; si desaparece, avisar o volver al perfil local sin seleccionar otra persona automáticamente.

3. **Media — El refresco automático puede hacer fallar un Sync manual y después ocultar su error.**  
   [ui/src/ui/recommendations.ts:178](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:178) y [recommendations.ts:204](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:204).  
   El refresco no participa en `task`, por lo que Connect continúa habilitado mientras descarga. Si el usuario inicia Sync entonces, el guard del backend lo rechaza con «A ground snapshot sync is already running» y la contraseña ya queda vaciada. Cuando termina el refresco anterior, `acceptSnapshot()` sustituye ese error por «Connected», aunque el intento manual falló.  
   **Arreglo:** coordinar ambas operaciones en la UI. Al solicitar Sync, pausar el temporizador, esperar al refresco pendiente y ejecutar después el Sync. Evitar que una respuesta anterior sobrescriba el estado de esa operación manual.

Las comprobaciones en memoria, con la UI transpilada y DOM, reloj e IPC simulados, reprodujeron los problemas de selección, interferencia con Sync y continuación del temporizador tras un error de expiración procedente de Sync. También comprobaron un único temporizador tras varios Sync/`visibilitychange`, su eliminación al ocultar la página y en `pagehide`, y cero llamadas con `available=false`. Las pruebas nuevas no cubren los tres hallazgos.

Por inspección, el guard RAII libera `snapshot_busy` ante errores y retornos anticipados; el mutex de sesión no permanece adquirido durante la red. No encontré fugas de cookie o contraseña en disco, logs o errores, ni nuevos `unwrap`/`expect`/`panic` o indexados inseguros. La publicación mediante temporal y `rename` evita exponer un JSON parcialmente escrito; los fallos de descarga ocurren antes de reemplazar la copia.

No ejecuté Cargo ni la suite oficial por el bloqueo de procesos del entorno. No hubo validación en APK ni contra el backend real.
## Verificación de las correcciones

**Dos hallazgos resueltos y uno parcialmente resuelto. Queda una regresión Media.**

1. **401/403 — Parcialmente resuelto.** Rust invalida la cookie en login y descargas; la UI detiene el temporizador ante expiración directa de Sync. Persiste el caso concurrente descrito abajo.
2. **Selección del perfil — Resuelto.** El refresco conserva el perfil local o el astronauta elegido; si desaparece, vuelve al local y avisa. [recommendations.ts:108](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:108).
3. **Interferencia refresco/Sync — Resuelto.** Sync pausa el temporizador, espera la petición pendiente y descarta su respuesta anterior. [recommendations.ts:223](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:223).

**Media nueva — Se descarta también la invalidación de sesión del refresco pendiente.** En [recommendations.ts:198](/C:/Users/Salva/Desktop/git/nasa.spaceapps/.claude/worktrees/slm-e3e74e/ui/src/ui/recommendations.ts:198), el cambio de generación omite `handleSessionError`. Si ese refresco recibe 401/403 mientras Sync espera y después el login falla por red, la cookie queda borrada, pero `sessionActive` sigue activo y el `finally` reactiva el temporizador. Continúan llamadas cada 15 segundos que devuelven `null`. Debe procesarse la invalidación aunque se descarte la respuesta visual anterior.

Reproduje esta secuencia y comprobé las tres rutas originales con UI transpilada e IPC/reloj simulados. Las pruebas añadidas cubren los casos directos, pero no esta combinación. No detecté otros hallazgos Alta/Media/Baja. Sin cambios en archivos; no repetí las suites ya reportadas como correctas.