# RECOVERY_STATUS.md

## Fase actual
FASE 2.2.6 — Persistencia de idempotencia y recuperación de operaciones completada y verificada. En espera de autorización.

## Tareas terminadas
- Seeds de administrador y demo quedaron condicionados a `OPALINE_DEV_SEEDS=1|true|yes|on`.
- `seed_admin` y `seed_demo` ya no se ejecutan por defecto.
- `seed_demo` crea el pago/factura de ejemplo solo para un usuario demo recién creado.
- Los datos demo existentes no se reparan, sobrescriben ni eliminan al arrancar.
- Se agregó una prueba unitaria para el opt-in explícito de seeds.
- Se documentó `OPALINE_DEV_SEEDS=1` en README.

## Archivos modificados
- `opaline_rust_backend/src/db.rs`
- `README.md`
- `RECOVERY_STATUS.md` (nuevo)

## Pruebas ejecutadas
- `cargo test` en `opaline_rust_backend`: 1 test passed, 0 failed.
- `cargo check`: compilación correcta con warnings preexistentes de código no usado.
- `git diff --check`: sin conflictos de espacios.

## Decisiones arquitectónicas
- Los seeds de usuarios demo/administrador son opt-in por variable de entorno.
- Producción debe provisionar administradores mediante un proceso controlado.
- Las tablas de planes y modelos siguen sembrándose porque no contienen credenciales.
- No se eliminaron ni modificaron usuarios, contraseñas ni datos existentes.

## Riesgos pendientes
- El usuario administrador existente `admin@opaline-ai.com` y el demo existente permanecen en la base actual.
- No hay aún rate limiting, CORS allowlist ni autenticación centralizada.
- Pagos y reembolsos siguen bloqueados por la siguiente tarea de FASE 1.
- Flutter sigue con auth fail-open y almacenamiento inseguro hasta su tarea correspondiente.

## Tarea 2 completada — Pagos y reembolsos

### Hallazgos confirmados
- La verificación local anterior marcaba pagos como completados y activaba suscripciones sin evidencia de PayPal.
- El reembolso anterior era público, no distinguía `invoice_id` de `payment_id` y devolvía éxito aunque no modificara filas.
- La factura inexistente devolvía HTTP 200.
- La creación de órdenes no filtraba planes inactivos ni persistía la moneda del plan.
- La creación de pago y suscripción pendiente no era transaccional.

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `RECOVERY_STATUS.md`

### Endpoints protegidos o deshabilitados
- `POST /payments/paypal/create`: requiere Bearer válido; solo crea una orden/suscripción pendiente con datos del plan; escrituras transaccionales.
- `GET /payments/paypal/verify/:order_id`: requiere Bearer válido; devuelve `401` sin token y `501 Not Implemented` porque no hay integración real con PayPal.
- `POST /facturas/:id/reembolso`: requiere Bearer válido; devuelve `401` sin token y `501 Not Implemented`.
- `GET /facturas/:id/pdf`: devuelve `404` cuando la factura no existe.
- `GET /payments/history`: sin cambios de contrato, continúa filtrando por usuario autenticado.

### Comportamiento anterior y nuevo
- Antes, una llamada pública a verificación podía fabricar un pago completado, una factura y una suscripción activa.
- Ahora, ninguna operación mutante de pago se declara exitosa sin integración real.
- Antes, el reembolso modificaba una columna local y respondía `ok: true`.
- Ahora, el reembolso exige autenticación y queda deshabilitado de forma segura.
- Antes, una factura inexistente devolvía 200 con texto.
- Ahora, devuelve 404.

### Pruebas ejecutadas
- `cargo test` en `opaline_rust_backend`: 2 tests passed, 0 failed.
- `cargo check`: compilación correcta, con warnings preexistentes de código no usado.
- `git diff --check`: sin errores de formato.

### Riesgos restantes
- No existe integración real con PayPal; por tanto, activación y reembolso permanecen deshabilitados.
- `invoice_pdf` sigue siendo público por compatibilidad con la web actual; su IDOR completo queda para una tarea posterior con cambios coordinados en PHP.
- No hay tests de integración contra base de datos para ownership, idempotencia y transacciones.
- Flutter y PHP aún consumen flujos de pago que ahora devolverán `501`; su adaptación corresponde a tareas posteriores.

## Tarea 3 completada — Autenticación, sesiones y almacenamiento seguro en Flutter

### Vulnerabilidades confirmadas
- Login y registro convertían errores de red/servidor en una sesión demo Plus.
- `SessionGuard.unlock()` concedía acceso incondicionalmente.
- Los tokens se almacenaban con XOR + Base64 sobre SharedPreferences.
- Logout no limpiaba datos privados de PlanProvider, ChatProvider ni WallProvider.
- La restauración de sesión podía reactivar un modo demo legado.

### Archivos modificados
- `opaline_ai_mobile/pubspec.yaml`
- `opaline_ai_mobile/lib/core/services/secure_storage.dart`
- `opaline_ai_mobile/lib/providers/auth_provider.dart`
- `opaline_ai_mobile/lib/core/services/session_guard.dart`
- `opaline_ai_mobile/lib/main.dart`
- `opaline_ai_mobile/lib/providers/plan_provider.dart`
- `opaline_ai_mobile/lib/providers/chat_provider.dart`
- `opaline_ai_mobile/lib/providers/wall_provider.dart`
- `opaline_ai_mobile/test/session_guard_test.dart`
- `RECOVERY_STATUS.md`

### Mecanismo de almacenamiento nuevo
- Se usa `flutter_secure_storage`.
- Android: EncryptedSharedPreferences respaldado por Android Keystore.
- iOS/macOS: Keychain.
- Se migran una sola vez los valores legados XOR de SharedPreferences y luego se eliminan.
- No se guardan tokens en texto plano en SharedPreferences, Hive ni archivos de configuración.

### Cambios en autenticación y SessionGuard
- Login y registro ahora fallan de forma segura ante red, servidor o respuesta incompleta.
- `_onLoginSuccess` valida access token, refresh token y usuario antes de persistir la sesión.
- Se eliminó `_activateDemoSession` y la restauración de modo demo.
- `SessionGuard.unlock()` ya no concede acceso; devuelve `false`.
- La biometría sigue deshabilitada porque no hay integración real.

### Cambios en logout y providers
- Logout elimina tokens y datos de usuario del almacenamiento seguro.
- Logout invoca limpiadores registrados de PlanProvider, ChatProvider y WallProvider.
- Se evita que una nueva cuenta visualice datos privados de una sesión anterior.

### Pruebas y resultados
- `flutter analyze`: 27 hallazgos, todos nivel `info` (deprecaciones), sin errores.
- `flutter test`: 1 test passed, 0 failed.
- `flutter build linux`: compilación exitosa.

### Limitaciones por plataforma
- Android e iOS usan almacenamiento nativo seguro.
- Linux compila mediante el plugin nativo de flutter_secure_storage; en este entorno se aplicó un ajuste local de compilación del plugin para tratar como warning un literal operator deprecado.
- La biometría no está implementada; no se simula.
- El bloqueo por inactividad ya no concede acceso automáticamente.

### Riesgos pendientes
- No hay pruebas de integración de login, refresh, logout ni limpieza de datos privados.
- `PlanProvider` todavía puede inicializarse antes de autenticar y mostrar estados demo si el backend responde de forma inesperada.
- `SessionGuard` restaura localmente una sesión existente sin validarla contra el backend; la validación real ocurre en las llamadas API.
- El almacenamiento seguro en web depende del backend de flutter_secure_storage para navegador.

## Siguiente tarea recomendada
FASE 1 está completa en sus tres tareas. Esperar autorización para FASE 2 o para una tarea adicional de validación/integración.


## Tarea 4 completada — Aislamiento de datos y autorización de conversaciones MongoDB

### Vulnerabilidad confirmada
- La actualización de una conversación filtraba únicamente por `id`, permitiendo que un usuario reutilizara un identificador ajeno y provocara escrituras cruzadas.
- Los errores de persistencia de conversaciones y métricas se ignoraban y podían convertirse en respuestas exitosas.

### Operaciones corregidas
- `list_conversations`: filtra por `user_id` autenticado.
- `chat`: `find_one`, `update_one` e `insert_one` usan el filtro compuesto `{"id": ..., "user_id": ...}`.
- `delete_conversation`: ya filtraba por propietario y se mantiene así.
- Los errores de `find_one`, `update_one`, `insert_one` y métricas ahora se propagan como `500` en lugar de ignorarse.

### Archivos modificados
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/main.rs`
- `RECOVERY_STATUS.md`

### Índices añadidos o propuestos
- Se intenta crear un índice único compuesto `(user_id, id)` en la colección `conversations`.
- La creación es best-effort: si existen duplicados legados, el arranque no falla y no se eliminan conversaciones.

### Pruebas y resultados
- `cargo test`: 4 tests passed, 0 failed.
- `cargo check`: compilación correcta, con warnings preexistentes de código no usado.
- `git diff --check`: sin errores de formato.

### Riesgos pendientes
- La unicidad `(user_id, id)` no se puede garantizar si ya existen duplicados legacy.
- No hay pruebas de integración contra MongoDB para verificar aislamiento real entre dos usuarios.
- El historial se sigue recibiendo parcialmente desde el cliente; la cuota y el modelo aún no se validan en esta tarea.
- No se implementó paginación de conversaciones ni límite de tamaño por mensaje más allá del mensaje actual.

## Tarea 5 completada — Endurecimiento de autenticación, sesiones y seguridad general

### Hallazgos confirmados
- Los refresh tokens no tenían expiración efectiva.
- La rotación no era atómica y podía permitir replay/concurrencia.
- Los tokens se almacenaban en texto claro.
- CORS era totalmente permisivo.
- No existía rate limiting.
- bcrypt se ejecutaba síncronamente en handlers async.
- `change-password` aceptaba contraseñas vacías.
- Los errores de persistencia podían ocultarse en algunas rutas.

### Diseño implementado
- Access y refresh tokens se almacenan como hash SHA-256.
- Refresh token tiene expiración de 7 días.
- La rotación usa una actualización condicionada por el hash anterior; si falla, se rechaza.
- La reutilización de un token rotado elimina la sesión.
- Logout y cambio de contraseña revocan sesiones de forma efectiva.
- CORS usa allowlist configurable mediante `OPALINE_CORS_ORIGINS`, con valores locales seguros por defecto.
- Rate limiting en memoria, compartido entre clones, con límites por IP y por cuenta.
- bcrypt se ejecuta mediante `spawn_blocking`.
- Contraseñas validadas de forma consistente en registro y cambio.
- Migración aditiva y no destructiva para `previous_refresh_token` y `refresh_expires_at`.

### Archivos modificados
- `opaline_rust_backend/src/routes/auth.rs`
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/security.rs`
- `opaline_rust_backend/src/state.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/src/db.rs`
- `RECOVERY_STATUS.md`

### Migraciones propuestas o implementadas
- `ALTER TABLE sessions ADD COLUMN previous_refresh_token VARCHAR(64) NULL` si no existe.
- `ALTER TABLE sessions ADD COLUMN refresh_expires_at DATETIME NULL` si no existe.
- No se eliminan sesiones ni usuarios existentes.
- Las sesiones antiguas con tokens en texto claro dejan de funcionar porque la comparación ahora usa hash.

### Pruebas y resultados
- `cargo test`: 8 tests passed, 0 failed.
- `cargo check`: compilación correcta, con warnings preexistentes de código no usado.
- `git diff --check`: sin errores de formato.

### Riesgos pendientes
- El rate limiter es por proceso; no protege despliegues con varias instancias.
- No hay pruebas de integración de refresh replay, concurrencia ni persistencia.
- CORS por defecto incluye localhost; producción debe configurar orígenes explícitos.
- Las sesiones antiguas quedan inutilizadas, pero no se eliminan automáticamente.

## Tarea 5.1 — Verificación de seguridad y compatibilidad

### Auditoría de tokens
- Verificado por código: los access y refresh tokens se generan con UUID v4 y se almacenan como hash SHA-256.
- Verificado por código: `user_id_from_token`, `logout`, `change_password` y `refresh` comparan hashes, no tokens en texto claro.
- Verificado por unit test: el hash es determinista, sensible a cambios y tiene 64 caracteres hexadecimales.
- No se sustituye SHA-256 por bcrypt; los tokens tienen alta entropía y no requieren hashing lento.

### Rotación y replay
- Verificado por código: la rotación depende del hash actual y usa una actualización condicionada por `refresh_token = hash anterior`.
- Verificado por código: la reutilización de un token rotado elimina la sesión, salvo una ventana de 5 segundos para evitar falsos positivos por solicitudes concurrentes legítimas.
- Verificado por unit test: la validación de contraseñas es consistente.
- Pendiente de validación real: replay, concurrencia y persistencia contra MySQL.

### Revocación
- Verificado por código: logout elimina la fila de sesión.
- Verificado por código: cambio de contraseña revoca las demás sesiones.
- Verificado por código: la revocación persiste en MySQL.
- Pendiente de validación real: reinicio del backend y consultas desde una nueva conexión.

### CORS
- Verificado por código: no se usa wildcard con credenciales.
- Verificado por código: `OPALINE_CORS_ORIGINS` se parsea como lista exacta de orígenes HTTP(S).
- Verificado por unit test: orígenes válidos, inválidos y vacíos.
- Producción debe configurar orígenes explícitos; el valor por defecto es solo para desarrollo local.

### Rate limiting
- Verificado por código: login, registro, recuperación, refresh y pagos tienen límites por IP y, cuando corresponde, por cuenta.
- Verificado por unit test: límite, separación de claves, compartición entre clones y concurrencia.
- Limitación conocida: el limiter es por proceso y no coordina varias instancias.
- La IP proviene de `ConnectInfo`, no de `X-Forwarded-For`.

### bcrypt y contraseñas
- Verificado por código: `hash` y `verify` se ejecutan con `spawn_blocking`.
- Verificado por código: registro y cambio de contraseña usan la misma validación.
- Verificado por unit test: contraseñas vacías e inválidas se rechazan.
- No se registran contraseñas en logs.

### Sesiones heredadas
- Verificado por código: las sesiones antiguas con tokens en texto claro ya no coinciden con los hashes SHA-256 y quedan inutilizadas.
- No se eliminan automáticamente ni se migran a texto claro.
- Recomendación: mantenerlas inutilizadas y limpiarlas mediante una política futura de expiración o una tarea de mantenimiento, sin migración destructiva.

### Tests unitarios
- `cargo test`: 12 tests passed, 0 failed.
- `cargo check`: compilación correcta, con warnings preexistentes de código no usado.
- `git diff --check`: sin errores de formato.
- `cargo fmt -- --check`: no disponible porque rustfmt no está instalado en el toolchain.

### Tests de integración pendientes
- MySQL aislado no estaba disponible ni se usó en esta tarea.
- Pendientes: refresh normal, replay, concurrencia, expiración, persistencia y revocación real.
- No se afirma que esos casos se hayan validado contra MySQL.

### Riesgos residuales
- El rate limiter no es distribuido.
- La detección de replay usa una ventana de 5 segundos para evitar falsos positivos por concurrencia.
- Las sesiones legacy quedan inutilizadas.
- No hay pruebas de integración reales contra MySQL.
- No se ha validado el comportamiento completo con Flutter/PHP en esta tarea.

### Compatibilidad con Flutter y PHP
- Los endpoints mantienen sus rutas y respuestas principales.
- Los clientes deben enviar el refresh token original; el backend lo hashea internamente.
- No se modificaron Flutter ni PHP en esta tarea.


## Tarea 5.2 — Infraestructura de pruebas de integración MySQL

### Arquitectura del entorno de testing
- Base MySQL aislada: `opaline_test`.
- Pool SQLx independiente mediante `OPALINE_TEST_DATABASE_URL`.
- MongoDB de prueba opcional mediante `OPALINE_TEST_MONGO_URL`.
- Migraciones ejecutadas mediante `db::init`.
- Limpieza limitada a `sessions` y `users` en la base de prueba.
- Pruebas en `tests/auth_integration.rs` con `#[ignore]`.

### Protección contra bases incorrectas
- `OPALINE_TEST_DATABASE_URL` es obligatoria.
- La URL debe apuntar a localhost o 127.0.0.1.
- El nombre de la base debe contener `test`.
- Se rechaza explícitamente la base `opaline`.

### Casos implementados
- Almacenamiento de tokens como hash.
- Refresh normal.
- Concurrencia de refresh.
- Replay después de la ventana de gracia.
- Rechazo de refresh expirado.
- Logout y persistencia de revocación.
- Cambio de contraseña y revocación de otras sesiones.
- Persistencia entre conexiones.

### Casos ejecutados y resultados
- `cargo test`: 12 unit tests passed, 0 failed.
- `cargo test -- --ignored`: 8 integration tests passed, 0 failed.
- Ejecución en paralelo: 8/8 passed.
- `cargo check`: correcto.
- `git diff --check`: correcto.
- `cargo fmt -- --check`: no disponible porque rustfmt no está instalado.

### Incidencias encontradas y corregidas
- `db::init` ejecutado concurrentemente provocaba columnas duplicadas; se cambió a `ADD COLUMN IF NOT EXISTS`.
- La limpieza con `TRUNCATE` causaba bloqueos; se cambió a `DELETE`.
- Una prueba de concurrencia movía una variable dos veces; se corrigió clonando el token.

### Riesgos pendientes
- Las pruebas de integración dependen de un MySQL local disponible.
- El rate limiter sigue siendo por proceso.
- No hay pruebas de integración para CORS, rate limiting ni pagos.
- `rustfmt` no está instalado en el toolchain actual.

### Procedimiento de ejecución
```bash
export OPALINE_TEST_DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline_test"
cd opaline_rust_backend
cargo test -- --ignored
```


## Tarea 5.3 — Validación de CORS, rate limiting y pagos

### Configuración CORS
- Allowlist exacta mediante `OPALINE_CORS_ORIGINS`.
- Sin wildcard con credenciales.
- Orígenes locales por defecto para desarrollo.
- Producción debe configurar orígenes explícitos.

### Pruebas OPTIONS
- Preflight validado mediante `CorsLayer` real.
- Origen autorizado recibe `Access-Control-Allow-Origin`.
- Métodos permitidos incluyen GET, POST, PUT, DELETE y OPTIONS.

### Endpoints con rate limiting
- Login: 20 por IP y 5 por cuenta cada 15 minutos.
- Registro: 10 por IP por hora.
- Recuperación: 10 por IP y 5 por cuenta cada 15 minutos.
- Refresh: 60 por IP cada 15 minutos.
- Cambio de contraseña: 10 por usuario por hora.
- Creación de pagos: 20 por IP por hora y 10 por usuario por hora.
- Verificación y reembolso: límites por IP y usuario.

### Resultados de límites por IP y cuenta
- Login devuelve 429 en la sexta solicitud fallida.
- Creación de pagos devuelve 429 en la undécima solicitud del mismo usuario.
- Verificado por integración real contra MySQL.

### Auditoría de pagos
- `POST /payments/paypal/create` requiere autenticación.
- Crea únicamente estados pendientes.
- No marca pagos como completados sin proveedor real.
- `GET /payments/paypal/verify/:order_id` devuelve 501 sin mutar estado.
- `POST /facturas/:id/reembolso` devuelve 501 sin mutar estado.
- Los errores de persistencia no se convierten en éxito.

### Auditoría de facturas e IDOR
- `GET /facturas/:id/pdf` devuelve 404 para facturas inexistentes.
- El endpoint sigue siendo público por compatibilidad con la web actual.
- Riesgo residual: una factura puede consultarse por ID sin autenticación. No se corrigió en esta tarea porque cambiarlo podría romper el frontend PHP. Recomendación: implementar acceso autenticado o firmado en una tarea coordinada.

### Estados HTTP
- 401 para falta o token inválido.
- 404 para factura inexistente.
- 429 para exceso de solicitudes.
- 501 para verificación y reembolso no implementados.
- 200 para creación e historial exitosos.

### Rustfmt y toolchain
- `rustfmt` no está instalado en el toolchain activo.
- No se modificó el toolchain global.
- Se documenta la instalación segura mediante `rustup component add rustfmt`.

### Pruebas ejecutadas
- `cargo test`: unit tests y CORS tests passed.
- `cargo test -- --ignored`: 6 security integration tests passed.
- `cargo check`: correcto.
- `git diff --check`: correcto.
- `cargo fmt -- --check`: no disponible.

### Pruebas pendientes
- Integración real de CORS a nivel de middleware con Origin y preflight ya cubierta por tests de router.
- Pruebas de rate limiting para refresh, recuperación y reembolso mediante integración.
- Pruebas de concurrencia de rate limiting.
- Pruebas de IDOR con dos usuarios reales para facturas.

### Riesgos residuales
- Rate limiter por proceso.
- Facturas públicas por ID.
- PayPal real no implementado.
- `rustfmt` no disponible.


## Tarea 5.4 — Protección de facturas y autorización Rust–PHP

### Vulnerabilidad original
- `GET /facturas/:id/pdf` era público.
- Una persona podía intentar consultar facturas ajenas mediante identificadores.

### Flujo anterior
- El navegador solicitaba directamente `API_BASE/facturas/:id/pdf`.
- Rust devolvía el comprobante sin autenticación ni verificación de propiedad.

### Flujo nuevo
- El navegador solicita `factura.php?id=...`.
- PHP valida la sesión.
- PHP transmite el access token mediante `Authorization: Bearer`.
- Rust valida el token y obtiene el usuario autenticado.
- Rust consulta la factura únicamente si pertenece a ese usuario.
- PHP devuelve el comprobante o un error coherente.

### Cambios en Rust
- `invoice_pdf` ahora exige Bearer token.
- La consulta SQL incluye `p.user_id = ?`.
- Sin token devuelve 401.
- Token inválido o expirado devuelve 401.
- Factura inexistente o ajena devuelve 404.

### Cambios en PHP
- Se creó `frontend/factura.php`.
- `cuenta.php` ahora enlaza a `factura.php?id=...`.
- PHP transmite el Bearer desde la sesión.
- PHP procesa 401 y 404 sin exponer el token.

### Seguridad de sesiones y tokens
- El token no se expone en HTML, JavaScript ni URLs.
- PHP mantiene la sesión web y renueva el access token mediante el flujo existente.
- No se acepta `user_id` arbitrario desde el navegador.

### Protección contra IDOR
- Usuario A puede consultar su factura.
- Usuario A no puede consultar la factura de B.
- Usuario B no puede consultar la factura de A.
- Sin autenticación se devuelve 401.
- Factura inexistente devuelve 404.

### Pruebas de integración MySQL
- `invoice_requires_authentication`: passed.
- `invoice_access_is_isolated_by_owner`: passed.
- Ambas usan la base aislada `opaline_test`.

### Pruebas PHP
- `php -l` passed en todos los archivos PHP.
- Flujo manual con sesión demo verificado: devuelve comprobante.
- Flujo sin sesión devuelve 302 a login.

### Resultados de regresión
- `cargo test`: passed.
- `cargo test -- --ignored`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.
- `php -l`: passed.

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/frontend/factura.php`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/tests/security_integration.rs`
- `Opaline_AI_Web_Prototipo/frontend/cuenta.php`
- `RECOVERY_STATUS.md`

### Riesgos residuales
- Facturas siguen siendo identificables por UUID; la autorización ahora depende del backend.
- PHP transmite el Bearer internamente, pero el navegador no lo recibe.
- Flutter no se modificó y no consume facturas en esta tarea.
- PayPal real sigue sin implementarse.

### Compatibilidad con Flutter
- Flutter permanece intacto.
- No se modificaron contratos de Flutter.
- Si Flutter consume facturas en el futuro, deberá usar autenticación Bearer como PHP.

### Estado de RECOVERY_STATUS.md
Se agregó la sección `TAREA 5.4 — Protección de facturas y autorización Rust–PHP`, incluyendo vulnerabilidad original, flujo anterior, flujo nuevo, cambios, pruebas, compatibilidad y riesgos residuales.


## Tarea 5.6 — Protección CSRF en el frontend PHP

### Rutas protegidas
- `login.php`: formularios de login y acceso demo.
- `register.php`: formulario de registro.
- `checkout.php`: confirmación de compra.
- `panel.php`: registro de gastos administrativos.
- `logout.php`: cierre de sesión mediante POST.
- `api_proxy.php`: todas las acciones autenticadas no públicas.

### Mecanismo implementado
- Token CSRF generado con `random_bytes(32)`.
- Token asociado a la sesión PHP.
- Validación mediante `hash_equals()`.
- Token transmitido en formularios ocultos y mediante `X-CSRF-Token` en AJAX.
- Sesión PHP endurecida con `use_strict_mode`, `use_only_cookies`, `HttpOnly`, `Secure` condicional y `SameSite=Lax`.
- Regeneración de ID de sesión tras login y registro.

### Integración AJAX
- `chat.php`, `muro.php` y `cuenta.php` envían `X-CSRF-Token`.
- El token se obtiene de una meta etiqueta generada por PHP.
- No se almacena en `localStorage`.

### Pruebas
- `php -l`: passed en todos los archivos PHP.
- Login con CSRF válido: passed.
- Login con CSRF inválido: rechazado con 419.
- `api_proxy` sin CSRF: 419.
- `api_proxy` con CSRF válido: 200.
- Logout con CSRF válido: 302.
- Logout con CSRF inválido: 419.
- Gasto administrativo con CSRF: 302.
- Gasto administrativo sin CSRF: 419.

### Limitaciones
- No hay suite automatizada de pruebas PHP.
- La validación se realizó mediante solicitudes HTTP manuales.
- No se modificó Rust ni Flutter.

### Riesgos residuales
- Las mutaciones GET legacy ya no se usan para estado; logout ahora exige POST.
- El token CSRF se regenera con la sesión.
- La protección depende de la integridad de la sesión PHP.


## Tarea 5.7 — Integridad y seguridad del muro

### Hallazgos corregidos
- Validación de contenido por caracteres Unicode, no bytes.
- Rechazo de HTML y caracteres de control.
- Validación de rating de 1 a 5.
- Elegibilidad basada en suscripción activa o grace no vencida.
- Deduplicación de publicaciones mediante clave de idempotencia por usuario.
- Edición limitada a 24 horas y solo por propietario.
- Eliminación lógica por propietario.
- Reportes deduplicados por usuario y publicación.
- Ocultamiento automático tras 3 reportes de usuarios distintos.
- Endpoints de moderación administrativa.
- Paginación con límite máximo de 10.
- Ordenamiento validado.
- Consulta de estadísticas del muro.
- Auditoría en MongoDB para creación, edición, eliminación, reporte y moderación.

### Archivos modificados
- `opaline_rust_backend/src/routes/wall.rs`
- `opaline_rust_backend/src/routes/admin.rs`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/tests/wall_integration.rs`
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/muro.php`
- `RECOVERY_STATUS.md`

### Migraciones
- Columna `posts.idempotency_key VARCHAR(128) NULL`.
- Índice único `uq_posts_user_idempotency (user_id, idempotency_key)`.
- Índice único `uq_reports_post_user (post_id, user_id)`.
- Migraciones aditivas y best-effort; no eliminan datos existentes.

### Pruebas
- `cargo test`: passed.
- `cargo test -- --ignored`: 6 wall integration tests passed.
- `php -l`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- La unicidad de reportes puede no aplicarse si existen duplicados legacy.
- No hay pruebas de integración PHP para el muro.
- No se implementó una cola administrativa en la interfaz web.
- La paginación se expone mediante headers y un endpoint de estadísticas; no se cambió el contrato base de lista para no romper PHP.


## Tarea 5.8 — Pruebas de integración PHP–Rust del muro

### Infraestructura de pruebas
- Suite ligera en PHP sin dependencias externas:
  `Opaline_AI_Web_Prototipo/tests/wall_proxy_test.php`.
- Ejecución mediante PHP CLI contra el servidor PHP local y el backend Rust.
- Variables de entorno: `OPALINE_WEB_BASE`.
- Backend aislado no se usa directamente desde PHP; las pruebas ejercitan el proxy PHP y el backend Rust real local.

### Casos automatizados
- Lectura pública del muro.
- Publicación sin sesión.
- Login con CSRF.
- Publicación sin CSRF rechazada.
- Contenido inválido rechazado.
- Publicación válida.
- Idempotencia con la misma clave.
- Rechazo de acción arbitraria en el proxy.
- Logout sin CSRF rechazado.
- Logout con CSRF válido.

### Resultados
- `php -l`: passed.
- `wall_proxy_test.php`: 14 assertions passed, 0 failed.
- `cargo test`: passed.
- `cargo test -- --ignored`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Pruebas de seguridad
- CSRF ausente: 419.
- CSRF inválido en logout: 419.
- Acción arbitraria en proxy: 400.
- Publicación sin sesión: 401.
- Publicación con CSRF válido: 200.

### Compatibilidad PHP–Rust
- PHP transmite la clave de idempotencia al backend.
- PHP conserva y presenta el resultado real del backend.
- No se modificó Flutter.
- No se cambiaron las reglas de negocio de Rust.

### Riesgos residuales
- No hay suite automatizada para edición, eliminación, reportes ni moderación en PHP.
- No hay pruebas PHP para respuestas malformadas o timeouts.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.9 — Pruebas avanzadas PHP–Rust del muro

### Casos implementados
- Edición válida.
- Edición con contenido inválido.
- Edición sin CSRF.
- Eliminación válida.
- Eliminación repetida.
- Reporte válido.
- Reporte duplicado.
- Reporte sin CSRF.
- Reporte de publicación inexistente.
- JSON malformado.
- Moderación como usuario normal rechazada indirectamente mediante Rust.
- Moderación como administrador.
- Moderación sin motivo.
- Moderación con motivo.

### Resultados
- `wall_advanced_test.php`: 18 assertions passed, 0 failed.
- `wall_proxy_test.php`: 14 assertions passed, 0 failed.
- `php -l`: passed.
- `cargo test`: passed.
- `cargo test -- --ignored`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay pruebas PHP para timeouts o backend desconectado.
- No hay pruebas PHP para respuestas HTML inesperadas.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.10 — Cola administrativa y panel de moderación del muro

### Páginas implementadas
- `Opaline_AI_Web_Prototipo/frontend/moderacion.php`: cola administrativa del muro.

### Endpoints utilizados
- `GET /muro/moderacion` mediante `wall_moderation_list`.
- `POST /muro/:id/moderar` mediante `wall_moderate`.

### Acciones disponibles
- Aprobar publicaciones.
- Ocultar publicaciones con motivo obligatorio.
- Eliminar publicaciones con motivo obligatorio.
- Actualizar la cola después de cada acción.

### Seguridad y permisos
- PHP verifica sesión y rol administrativo antes de mostrar la página.
- Rust valida el rol administrativo en cada operación.
- CSRF se transmite en todas las acciones AJAX.
- Un usuario normal no puede acceder a la cola ni ejecutar moderación.
- No se modificó Flutter.

### Pruebas
- `php -l`: passed.
- `cargo test`: passed.
- `cargo test -- --ignored`: passed.
- `git diff --check`: passed.
- Verificación manual de la página y del proxy con cuenta administradora: passed.

### Riesgos residuales
- La cola administrativa no tiene paginación ni filtros avanzados.
- No hay pruebas automatizadas PHP para la interfaz de moderación.
- El historial de auditoría se registra en MongoDB, pero no se muestra en la interfaz web.

## Tarea 5.11 — Auditoría de consistencia, trazabilidad y recuperación MySQL–MongoDB

### Alcance
- Auditoría de solo lectura.
- No se modificó código, esquema ni datos.
- Se creó `SECURITY_AUDIT_5_11.md`.

### Hallazgos principales
- No existe atomicidad distribuida entre MySQL y MongoDB.
- Los eventos de auditoría pueden quedar ausentes si Mongo falla después de MySQL.
- Los reintentos de chat pueden duplicar mensajes.
- No hay reconciliación periódica ni identificador de correlación compartido.
- Transactional Outbox se recomienda como estrategia futura.

### Pruebas
- `cargo check`: passed.
- `cargo test --lib`: 12 passed.
- `cargo test -- --ignored`: 21 passed en total con base aislada.
- `git diff --check`: passed.

### Riesgos residuales
- No se implementó Transactional Outbox.
- No se validó el comportamiento bajo fallos reales.
- No se inspeccionó el contenido actual de MongoDB.
- No se evaluó rendimiento ni múltiples instancias.

### Recomendación
Esperar autorización para implementar Transactional Outbox de forma incremental en la Tarea 5.12.

## Tarea 5.12 — Infraestructura base de Transactional Outbox

### Archivos nuevos
- `opaline_rust_backend/src/outbox.rs`
- `opaline_rust_backend/tests/outbox_integration.rs`
- `OUTBOX_DESIGN_5_12.md`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/tests/common/mod.rs`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Tabla `outbox_events` con estados, intentos, bloqueo y correlación.
- Repositorio de inserción compatible con transacciones SQLx.
- Consulta, reclamo, procesamiento, fallo, recuperación de bloqueos y reintentos.
- Pruebas unitarias y de integración.

### Garantías verificadas
- Inserción atómica en transacción.
- Rollback sin evento persistido.
- Reclamo concurrente sin duplicados.
- Recuperación de bloqueos expirados.
- Idempotencia por identificador único.
- Persistencia entre conexiones.

### Riesgos residuales
- No hay worker permanente.
- No hay integración con operaciones de negocio.
- No hay reconciliación periódica.
- No hay procesamiento MongoDB.
- Los índices únicos son best-effort.

### Pruebas
- `cargo test --lib`: 14 passed.
- `cargo test --test outbox_integration -- --ignored`: 8 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

## Tarea 5.13 — Procesador controlado de eventos Transactional Outbox

### Archivos nuevos
- `opaline_rust_backend/src/outbox_worker.rs`
- `opaline_rust_backend/tests/outbox_worker_integration.rs`
- `OUTBOX_WORKER_5_13.md`

### Archivos modificados
- `opaline_rust_backend/src/outbox.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/tests/outbox_integration.rs`
- `OUTBOX_DESIGN_5_12.md`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Worker controlado con Tokio.
- Reclamación concurrente y segura.
- Fencing mediante `claim_token`.
- Contrato `EventConsumer` desacoplado.
- Reintentos, fallos temporales y permanentes.
- Captura de panics.
- Shutdown ordenado.
- Configuración por variables de entorno.
- Pruebas unitarias e integración.

### Pruebas
- `cargo test --lib`: 14 passed.
- `cargo test --test outbox_integration -- --ignored`: 8 passed.
- `cargo test --test outbox_worker_integration -- --ignored`: 7 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- Worker no activado en producción.
- No hay consumidor MongoDB real.
- No hay integración con operaciones de negocio.
- No hay reconciliación periódica.
- La garantía es al menos una vez, no exactamente una vez.

## Tarea 5.14 — Integración Transactional Outbox con auditoría del muro y consumidor MongoDB

### Archivos nuevos
- `opaline_rust_backend/src/consumers/mod.rs`
- `opaline_rust_backend/src/consumers/mongo_wall_audit.rs`
- `opaline_rust_backend/tests/wall_outbox_integration.rs`
- `OUTBOX_WALL_INTEGRATION_5_14.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/wall.rs`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/main.rs`
- `OUTBOX_DESIGN_5_12.md`
- `RECOVERY_STATUS.md`

### Operaciones integradas
- Creación de publicaciones.
- Edición de publicaciones.
- Eliminación lógica.
- Reportes.
- Ocultamiento automático.
- Moderación administrativa.

### Eventos generados
- `wall.post.created`
- `wall.post.updated`
- `wall.post.deleted`
- `wall.post.reported`
- `wall.post.auto_hidden`
- `wall.post.moderated`

### Pruebas
- `cargo test --lib`: passed.
- `cargo test --test wall_integration -- --ignored`: passed.
- `cargo test --test outbox_integration -- --ignored`: passed.
- `cargo test --test outbox_worker_integration -- --ignored`: passed.
- `cargo test --test wall_outbox_integration -- --ignored`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- Worker no activado por defecto.
- No hay reconciliación periódica.
- No hay consumidor para otros tipos de evento.
- Índice único de auditoría best-effort.
- La garantía es al menos una vez, no exactamente una vez.


## Tarea 5.14.1 — Endurecimiento de idempotencia MongoDB y activación segura del consumidor

### Archivos nuevos
- `OUTBOX_IDEMPOTENCY_HARDENING_5_14_1.md`

### Archivos modificados
- `opaline_rust_backend/src/consumers/mongo_wall_audit.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/tests/wall_outbox_integration.rs`
- `RECOVERY_STATUS.md`

### Índice real de MongoDB
- Colección: `wall_events`.
- Clave: `{ event_id: 1 }`.
- Unicidad: `true`.
- Filtro parcial: `{ event_id: { $exists: true } }`.

### Comportamiento ante duplicados
- Se consulta primero por `event_id`.
- Si existe, se devuelve éxito.
- Si la inserción falla por clave duplicada, se verifica nuevamente el documento correspondiente.
- Solo se trata como éxito si el documento con el mismo `event_id` existe.
- Otros errores de MongoDB se tratan como fallos temporales.

### Activación segura
- El worker permanece desactivado por defecto.
- Activación mediante `OPALINE_WALL_AUDIT_OUTBOX_ENABLED=1`.
- Antes de iniciar, se ejecuta `verify_event_id_index()`.
- Si el índice no es válido, el worker no se inicia y los eventos permanecen en MySQL.
- El arranque HTTP no se bloquea.

### Pruebas
- `wall_audit_index_is_verified`: passed.
- `wall_audit_index_verification_rejects_missing_index`: passed.
- `wall_audit_duplicate_processing_is_idempotent_under_concurrency`: passed.
- `wall_audit_duplicate_document_is_recognized_as_success`: passed.
- `wall_audit_event_survives_consumer_failure_before_confirmation`: passed.
- `cargo test --lib`: 14 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- El índice único es best-effort.
- No hay reconciliación periódica.
- No hay consumidor para otros tipos de evento.
- La garantía sigue siendo al menos una vez, no exactamente una vez.

## Tarea 5.15 — Reconciliación periódica MySQL–MongoDB para auditoría del muro

### Archivos nuevos
- `opaline_rust_backend/src/outbox_reconciler.rs`
- `opaline_rust_backend/tests/outbox_reconciliation_integration.rs`
- `OUTBOX_RECONCILIATION_5_15.md`

### Archivos modificados
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/src/outbox.rs`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Reconciliador periódico de solo lectura.
- Clasificación de discrepancias entre MySQL y MongoDB.
- Consultas paginadas.
- Activación explícita mediante variable de entorno.
- Verificación de MySQL, MongoDB e índice único.
- Logs estructurados.
- Shutdown ordenado.

### Pruebas
- `cargo test --lib`: 14 passed.
- `cargo test --test outbox_reconciliation_integration -- --ignored`: 5 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay recuperación automática.
- No hay reconciliación para otros tipos de evento.
- No se validó el comportamiento bajo carga real.
- No hay consumidor para otros tipos de evento.

## Tarea 5.16 — Recuperación controlada de discrepancias críticas MySQL–MongoDB

### Archivos nuevos
- `opaline_rust_backend/src/outbox_recovery.rs`
- `opaline_rust_backend/tests/outbox_recovery_integration.rs`
- `OUTBOX_RECOVERY_5_16.md`

### Archivos modificados
- `opaline_rust_backend/src/outbox.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/main.rs`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Recuperación controlada para discrepancias críticas.
- Clasificación de elegibilidad.
- Fencing mediante `claim_token`.
- Reintentos y recuperación de bloqueos.
- Límite por ciclo.
- Activación explícita mediante variable de entorno.
- Verificación de MySQL, MongoDB e índice único.
- Logs estructurados.

### Pruebas
- `cargo test --lib`: 15 passed.
- `cargo test --test outbox_recovery_integration -- --ignored`: 10 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay recuperación automática para `failed` sin documento.
- No hay reconciliación automática de documentos huérfanos.
- No se validó el comportamiento bajo carga real.
- La garantía sigue siendo al menos una vez, no exactamente una vez.

## Tarea 5.17 — Servicio administrativo de revisión y recuperación manual Outbox

### Archivos nuevos
- `opaline_rust_backend/src/routes/outbox_admin.rs`
- `opaline_rust_backend/tests/outbox_admin_integration.rs`
- `OUTBOX_ADMIN_5_17.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/mod.rs`
- `opaline_rust_backend/src/outbox_recovery.rs`
- `opaline_rust_backend/migrations.sql`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Consulta administrativa de discrepancias.
- Inspección de eventos.
- Historial persistente de intervenciones.
- Acciones explícitas y autorizadas.
- Idempotencia por administrador y clave.
- Protección contra concurrencia.
- Reutilización de Outbox y fencing.

### Pruebas
- `cargo test --lib`: 16 passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 8 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay interfaz visual para el servicio administrativo.
- No hay paginación avanzada ni filtros complejos.
- La recuperación automática sigue limitada a casos elegibles.
- No se implementó una cola administrativa en PHP.
- La garantía sigue siendo al menos una vez, no exactamente una vez.

## Tarea 5.18 — Panel administrativo de auditoría y recuperación Outbox

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/frontend/moderacion_outbox.php`
- `OUTBOX_ADMIN_PANEL_5_18.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/panel.php`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Panel de auditoría Outbox en PHP.
- Dashboard de discrepancias.
- Filtros y paginación.
- Detalle del evento.
- Historial administrativo.
- Acciones con confirmación y CSRF.
- Integración con endpoints Rust reales.

### Pruebas
- `php -l`: passed.
- `cargo test`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.
- Verificación manual con cuenta administradora: passed.

### Riesgos residuales
- No hay pruebas automatizadas PHP para el panel.
- No hay paginación avanzada ni filtros por antigüedad.
- El historial de auditoría se registra en MongoDB, pero no se muestra en la interfaz web.
- PayPal real sigue sin implementarse.

## Tarea 5.17.1 — Auditoría de aceptación y cierre de brechas del servicio administrativo Outbox

### Archivos nuevos
- `OUTBOX_ADMIN_ACCEPTANCE_5_17_1.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/outbox_admin.rs`
- `opaline_rust_backend/tests/outbox_admin_integration.rs`
- `RECOVERY_STATUS.md`

### Brechas corregidas
- `failed` sin documento MongoDB ahora es recuperable cuando la causa es recuperable.
- `audit_without_outbox` se calcula correctamente.
- Rate limiting utiliza la IP del peer.
- Se añadieron pruebas de aceptación faltantes.

### Pruebas
- `cargo test --lib`: 16 passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 11 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay interfaz visual para el servicio administrativo.
- No hay paginación avanzada ni filtros complejos.
- La recuperación automática sigue limitada a casos elegibles.
- No se implementó una cola administrativa en PHP.
- La garantía sigue siendo al menos una vez, no exactamente una vez.

## Tarea 5.18 — Panel administrativo de auditoría y recuperación Outbox

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/frontend/moderacion_outbox.php`
- `OUTBOX_ADMIN_PANEL_5_18.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/panel.php`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Panel de auditoría Outbox en PHP.
- Dashboard de discrepancias.
- Filtros y paginación.
- Detalle del evento.
- Historial administrativo.
- Acciones con confirmación y CSRF.
- Integración con endpoints Rust reales.

### Pruebas
- `php -l`: passed.
- `cargo test`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.
- Verificación manual con cuenta administradora: passed.

### Riesgos residuales
- No hay pruebas automatizadas PHP para el panel.
- No hay paginación avanzada ni filtros por antigüedad.
- El historial de auditoría se registra en MongoDB, pero no se muestra en la interfaz web.
- PayPal real sigue sin implementarse.

## Tarea 5.18.1 — Pruebas automatizadas PHP y cierre de trazabilidad administrativa Outbox

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/tests/outbox_admin_panel_test.php`
- `OUTBOX_ADMIN_TESTING_5_18_1.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `opaline_rust_backend/src/routes/outbox_admin.rs`
- `RECOVERY_STATUS.md`

### Pruebas
- `outbox_admin_panel_test.php`: 22 assertions passed.
- `php -l`: passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 11 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Hallazgos corregidos
- El endpoint de acciones ahora devuelve 400 para acciones desconocidas.
- La suite PHP cubre autenticación, autorización, CSRF, errores y acciones administrativas.

### Riesgos residuales
- No hay suite automatizada para edición, eliminación, reportes ni moderación en PHP.
- No hay pruebas PHP para timeouts o backend desconectado.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.18.2 — Resiliencia HTTP y tolerancia a fallos PHP–Rust

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/tests/outbox_http_resilience_test.php`
- `OUTBOX_HTTP_RESILIENCE_5_18_2.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/frontend/config.php`
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/factura.php`
- `Opaline_AI_Web_Prototipo/frontend/moderacion_outbox.php`
- `RECOVERY_STATUS.md`

### Funcionalidad implementada
- Timeouts explícitos.
- Validación de respuestas JSON.
- Persistencia de clave de idempotencia.
- Logs seguros.
- Manejo de respuestas inválidas.

### Pruebas
- `outbox_http_resilience_test.php`: 16 assertions passed.
- `php -l`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay pruebas para backend desconectado a nivel de proxy sin mock.
- No hay pruebas para respuestas truncadas.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.18.3 — Validación de fallos reales y recuperación de operaciones inciertas

### Archivos nuevos
- `OUTBOX_HTTP_FAILURE_VALIDATION_5_18_3.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/tests/outbox_http_resilience_test.php`
- `RECOVERY_STATUS.md`

### Pruebas
- `outbox_http_resilience_test.php`: 16 assertions passed.
- `php -l`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay pruebas para backend desconectado a nivel de proxy sin mock.
- No hay pruebas para respuestas truncadas.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.18.4 — Cierre de brechas de resiliencia y validación integral

### Archivos nuevos
- `Opaline_AI_Web_Prototipo/tests/outbox_http_closure_test.php`
- `OUTBOX_HTTP_CLOSURE_5_18_4.md`

### Archivos modificados
- `Opaline_AI_Web_Prototipo/frontend/config.php`
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/factura.php`
- `Opaline_AI_Web_Prototipo/frontend/moderacion_outbox.php`
- `RECOVERY_STATUS.md`

### Pruebas
- `outbox_http_closure_test.php`: 5 assertions passed.
- `outbox_http_resilience_test.php`: 16 assertions passed.
- `php -l`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Riesgos residuales
- No hay pruebas para backend desconectado a nivel de proxy sin mock.
- No hay pruebas para respuestas truncadas a nivel de proxy sin mock.
- La validación se realizó contra el entorno local de desarrollo.

## Tarea 5.19 — Auditoría de preparación técnica y comercial para FASE 2

### Archivos nuevos
- `OPALINE_PHASE2_READINESS_AUDIT_5_19.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Hallazgos principales
- La base backend es sólida para desarrollo y demo.
- Faltan cuotas, consumo medido y pagos reales.
- Las suscripciones no tienen renovación ni vencimiento automático.
- No existe verificación real de correo.
- No existe monitoreo operativo suficiente.
- La experiencia comercial está parcialmente duplicada entre PHP y Flutter.

### Dictamen
- Preparación técnica: lista con condiciones.
- Preparación de seguridad: lista con condiciones.
- Preparación comercial: no lista.
- Preparación operativa: no lista.

### Bloqueadores reales
1. No hay integración real de pagos.
2. No hay cuotas ni consumo medido.
3. No hay renovación ni vencimiento de suscripciones.
4. No hay verificación de correo.
5. No hay monitoreo operativo.

### Recomendación
Opaline AI puede iniciar FASE 2.0 y FASE 2.1 con la base actual, pero no puede ofrecer un servicio comercial real hasta implementar FASE 2.2 y FASE 2.3. No se debe iniciar FASE 2.4 ni FASE 2.5 hasta que las fases anteriores estén verificadas.

## Tarea 5.19 — Auditoría de preparación técnica y comercial para FASE 2

### Archivos nuevos
- `OPALINE_PHASE2_READINESS_AUDIT_5_19.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Hallazgos principales
- La base backend es sólida para desarrollo y demo.
- Faltan cuotas, consumo medido y pagos reales.
- Las suscripciones no tienen renovación ni vencimiento automático.
- No existe verificación real de correo.
- No existe monitoreo operativo suficiente.
- La experiencia comercial está parcialmente duplicada entre PHP y Flutter.

### Dictamen
- Preparación técnica: lista con condiciones.
- Preparación de seguridad: lista con condiciones.
- Preparación comercial: no lista.
- Preparación operativa: no lista.

### Bloqueadores reales
1. No hay integración real de pagos.
2. No hay cuotas ni consumo medido.
3. No hay renovación ni vencimiento de suscripciones.
4. No hay verificación de correo.
5. No hay monitoreo operativo.

### Recomendación
Opaline AI puede iniciar FASE 2.0 y FASE 2.1 con la base actual, pero no puede ofrecer un servicio comercial real hasta implementar FASE 2.2 y FASE 2.3. No se debe iniciar FASE 2.4 ni FASE 2.5 hasta que las fases anteriores estén verificadas.

## FASE 2.0.1 — Diseño arquitectónico y especificación del núcleo comercial

### Archivos nuevos
- `OPALINE_PHASE2_ARCHITECTURE_2_0_1.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se elaboró la especificación arquitectónica del núcleo comercial de Opaline AI. No se implementó ninguna funcionalidad.

### Hallazgos confirmados
- Las tablas `plans`, `subscriptions`, `payments`, `invoices` y `usage_counters` existen.
- `usage_counters` no se utiliza.
- No existe AI Gateway.
- No existe integración real de pagos.
- No hay renovación ni vencimiento automático.
- No hay verificación real de correo.
- No hay monitoreo operativo.

### Decisiones arquitectónicas
- MySQL sigue siendo la fuente de verdad comercial.
- Rust conserva la autoridad comercial.
- El Outbox se utiliza para eventos de auditoría y consistencia eventual.
- No se proponen microservicios.
- Se proponen entidades aditivas para versionado de planes, eventos de suscripción, reservas de consumo y registros de consumo.

### Riesgos pendientes
- PayPal real no está integrado.
- Las cuotas no se aplican.
- El consumo de tokens no se registra.
- No hay renovación ni vencimiento automático.
- No hay verificación de correo.
- No hay monitoreo operativo.
- La experiencia comercial está duplicada.

### Recomendación
Esperar autorización antes de implementar FASE 2.1. No iniciar FASE 2.2 ni posteriores hasta que FASE 2.1 esté verificada.

## FASE 2.0.2 — Refinamiento técnico y cierre de especificaciones comerciales

### Archivos nuevos
- `OPALINE_PHASE2_REFINEMENT_2_0_2.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se refinó la especificación de la FASE 2.0.1 para corregir ambigüedades en versionado de planes, suscripción única, motor de cuotas, idempotencia, pagos, renovación y contratos API. No se implementó ninguna funcionalidad.

### Correcciones realizadas
- Se definió `plan_versions` y `plan_version_id` en suscripciones.
- Se propuso `is_current` para garantizar una suscripción vigente por usuario en MySQL.
- Se diseñó un motor transaccional de cuotas con reservas.
- Se definió idempotencia de consumo por usuario y clave.
- Se separó cancelación de renovación de pérdida de acceso.
- Se definió flujo transaccional de pagos y webhooks.
- Se incluyeron reembolsos parciales y completos.
- Se definió renovación y expiración automática.
- Se completaron contratos API.
- Se añadió estrategia Outbox para pagos.
- Se añadió matriz de pruebas.

### Riesgos pendientes
- PayPal real no está integrado.
- Las cuotas no se aplican.
- El consumo de tokens no se registra.
- No hay renovación ni vencimiento automático.
- No hay verificación real de correo.
- No hay monitoreo operativo.
- La experiencia comercial está duplicada.

### Recomendación
Esperar autorización antes de implementar FASE 2.1. No iniciar FASE 2.2 ni posteriores hasta que FASE 2.1 esté verificada.


## FASE 2.1.0 — Implementación del catálogo de planes y núcleo de suscripciones

### Archivos nuevos
- `OPALINE_PHASE2_1_0_IMPLEMENTATION.md`
- `opaline_rust_backend/tests/plans_integration.rs`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/models.rs`
- `opaline_rust_backend/src/routes/mod.rs`
- `opaline_rust_backend/src/routes/plans.rs`
- `opaline_rust_backend/src/routes/admin.rs`
- `opaline_rust_backend/src/routes/wall.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó el catálogo de planes con versionado, el núcleo de suscripciones, eventos de suscripción, restricción MySQL de una suscripción vigente por usuario y endpoints de consulta, historial, cancelación y restauración. Las migraciones son aditivas. No se implementaron cuotas, metering, pagos reales ni renovación automática.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test -- --ignored`: correcto.
- `php -l`: correcto.
- `flutter analyze`: 27 avisos informativos, sin errores.
- `git diff --check`: correcto.

### Riesgos pendientes
- PayPal real no está integrado.
- Las cuotas no se aplican.
- El consumo de tokens no se registra.
- No hay renovación automática mediante proveedor.
- No hay verificación real de correo.
- No hay monitoreo operativo.

### Recomendación
No iniciar FASE 2.2 hasta tener autorización expresa.


## FASE 2.1.1 — Corrección de integridad, concurrencia y validación de suscripciones

### Archivos nuevos
- `OPALINE_PHASE2_1_1_CORRECTION.md`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/routes/plans.rs`
- `opaline_rust_backend/tests/plans_integration.rs`
- `OPALINE_PHASE2_1_0_IMPLEMENTATION.md`
- `RECOVERY_STATUS.md`

### Resumen
Se corrigió la restricción de suscripción vigente única mediante una columna generada nullable `current_user_id`. Se eliminó la restricción anterior y se añadió `UNIQUE (current_user_id)`. Se capturan conflictos de unicidad como `409 Conflict`. Se añadieron pruebas de historial múltiple y restricción a nivel de base de datos.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test -- --ignored`: correcto.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.2 hasta tener autorización expresa.


## FASE 2.2.0 — Implementación del motor de cuotas, reservas de consumo y metering

### Archivos nuevos
- `OPALINE_PHASE2_2_0_IMPLEMENTATION.md`
- `opaline_rust_backend/src/quota.rs`
- `opaline_rust_backend/src/routes/usage.rs`
- `opaline_rust_backend/tests/quota_integration.rs`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/src/routes/mod.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó el motor de cuotas y medición de consumo. Las solicitudes de chat requieren una reserva válida. Los límites proceden de la versión del plan. Las reservas son atómicas en MySQL y las solicitudes son idempotentes. El consumo real se registra con distinción entre medición del proveedor y estimación local. Se añadieron endpoints de uso.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### Riesgos pendientes
- PayPal real no está integrado.
- No hay renovación automática mediante proveedor.
- No hay verificación real de correo.
- No hay monitoreo operativo.
- LM Studio puede no reportar tokens de forma fiable.

### Recomendación
No iniciar FASE 2.3 hasta tener autorización expresa.


## FASE 2.2.1 — Auditoría de integridad, concurrencia y recuperación del motor de cuotas

### Archivos nuevos
- `OPALINE_PHASE2_2_1_AUDIT.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se realizó una auditoría de solo lectura del motor de cuotas. Se verificó concurrencia, idempotencia, reservas, medición de tokens, errores de inferencia, contadores y seguridad. El dictamen es APROBADO CON RESERVAS.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test -- --ignored`: correcto.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### Hallazgos principales
- ALTO: fallo de persistencia en MongoDB después de confirmar cuota.
- MEDIO: recuperación automática ausente.
- MEDIO: reconciliación no implementada.
- MEDIO: estimaciones pueden registrarse como medición real.

### Recomendación
Corregir los hallazgos ALTO y MEDIO antes de iniciar FASE 2.3.


## FASE 2.2.2 — Corrección de integridad transaccional, recuperación y reconciliación del consumo

### Archivos nuevos
- `OPALINE_PHASE2_2_2_IMPLEMENTATION.md`
- `opaline_rust_backend/src/chat_operations.rs`
- `opaline_rust_backend/src/consumers/mongo_chat_persist.rs`
- `opaline_rust_backend/tests/chat_operations_integration.rs`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/src/quota.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/tests/quota_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó un registro durable de operaciones de chat, persistencia recuperable mediante Outbox, recuperación de reservas con fencing, reconciliación de contadores y operaciones, corrección de clasificación de tokens y retorno de la clave de idempotencia.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 2 pruebas correctas.
- `git diff --check`: correcto.

### Riesgos residuales
- Recuperación automática desactivada por defecto.
- Persistencia depende de MongoDB.
- No hay pagos reales integrados.

### Recomendación
No iniciar FASE 2.2.3 ni FASE 2.3 sin autorización expresa.


## FASE 2.2.3 — Auditoría final de integridad del ciclo de consumo, chat y persistencia

### Archivos nuevos
- `OPALINE_PHASE2_2_3_FINAL_AUDIT.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se realizó una auditoría independiente de solo lectura del ciclo completo de consumo. Se verificó persistencia durable, recuperación MongoDB, Outbox, reservas, reconciliación, contadores, medición de tokens, idempotencia, seguridad y esquema. El dictamen es APROBADO CON RESERVAS.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 2 pruebas correctas.
- `git diff --check`: correcto.

### Riesgos residuales
- Recuperación automática desactivada por defecto.
- Clientes no preservan automáticamente la idempotencia.
- Cobertura insuficiente para fallos distribuidos.

### Recomendación
Resolver o documentar las reservas operativas antes de iniciar FASE 2.3.


## FASE 2.2.4 — Cierre operativo, idempotencia extremo a extremo y resiliencia de clientes

### Archivos nuevos
- `OPALINE_PHASE2_2_4_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_ai_mobile/lib/providers/chat_provider.dart`
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/chat.php`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó la preservación de la clave de idempotencia en Flutter y PHP. Ambos clientes generan una clave por operación lógica, la envían al backend Rust y la conservan durante fallos de transporte. Se documentó la configuración operativa de los workers de recuperación.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 2 pruebas correctas.
- `php -l`: correcto.
- `flutter analyze`: 27 avisos informativos, sin errores.
- `git diff --check`: correcto.

### Riesgos residuales
- Workers desactivados por defecto.
- Clientes dependen de la clave devuelta por el backend para recuperar operaciones.

### Recomendación
No iniciar FASE 2.2.5 ni FASE 2.3 sin autorización expresa.


## FASE 2.2.5 — Auditoría final de idempotencia y recuperación extremo a extremo

### Archivos nuevos
- `OPALINE_PHASE2_2_5_FINAL_AUDIT.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se realizó una auditoría de solo lectura de la idempotencia extremo a extremo. El backend es sólido y recuperable. Los clientes preservan la clave durante fallos de transporte, pero no la persisten tras cierre o recarga. No existe endpoint de consulta de operación. El dictamen es APROBADO CON RESERVAS.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 2 pruebas correctas.
- `php -l`: correcto.
- `flutter analyze`: 27 avisos informativos, sin errores.
- `git diff --check`: correcto.

### Riesgos residuales
- Claves no persisten en clientes tras cierre o recarga.
- No existe endpoint de consulta de operación.
- Cobertura insuficiente para fallos distribuidos.

### Recomendación
Resolver las reservas operativas antes de iniciar FASE 2.3.


## FASE 2.2.6 — Persistencia de idempotencia y recuperación de operaciones

### Archivos nuevos
- `OPALINE_PHASE2_2_6_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/src/chat_operations.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_ai_mobile/lib/providers/chat_provider.dart`
- `Opaline_AI_Web_Prototipo/frontend/chat.php`
- `opaline_rust_backend/tests/chat_operations_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó la persistencia duradera de claves de idempotencia en Flutter, la conservación en PHP mediante localStorage y endpoints de consulta de operaciones en Rust. Se añadieron pruebas de consulta por identificador y clave.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test chat_operations_integration -- --ignored`: 3 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `php -l`: correcto.
- `flutter analyze`: 27 avisos informativos, sin errores.
- `git diff --check`: correcto.

### Riesgos residuales
- Workers desactivados por defecto.
- Recuperación depende de la clave conservada por el cliente.

### Recomendación
No iniciar FASE 2.2.7 ni FASE 2.3 sin autorización expresa.


## FASE 2.3.0 — Diseño arquitectónico de integración de pagos reales

### Archivos nuevos
- `OPALINE_PHASE2_3_0_PAYMENT_ARCHITECTURE.md`

### Archivos modificados
- `RECOVERY_STATUS.md`

### Resumen
Se elaboró el diseño arquitectónico para integrar pagos reales con PayPal. Se propone utilizar PayPal Orders API v2 con cobros puntuales y renovación manual para el MVP. No se implementó ninguna funcionalidad.

### Recomendación
No iniciar FASE 2.3.1 sin autorización expresa.


## FASE 2.3.1 — Implementación del modelo de datos y contratos financieros

### Archivos nuevos
- `OPALINE_PHASE2_3_1_IMPLEMENTATION.md`
- `opaline_rust_backend/tests/financial_model_integration.rs`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/models.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó el modelo de datos financiero con tablas para órdenes, transacciones, eventos de webhook y reembolsos. Se añadieron modelos Rust y pruebas de integridad. No se implementó PayPal ni endpoints de cobro.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test financial_model_integration -- --ignored`: 1 prueba correcta.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.2 sin autorización expresa.


## FASE 2.3.2 — Infraestructura de integración PayPal Sandbox

### Archivos nuevos
- `OPALINE_PHASE2_3_2_IMPLEMENTATION.md`
- `opaline_rust_backend/src/paypal.rs`

### Archivos modificados
- `opaline_rust_backend/src/lib.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó un cliente HTTP reutilizable para PayPal Sandbox, con OAuth 2.0, caché de tokens y manejo de errores. Live permanece bloqueado. No se implementaron órdenes, capturas, webhooks ni reembolsos.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.3 sin autorización expresa.


## FASE 2.3.3 — Implementación de creación y consulta de órdenes PayPal en Sandbox

### Archivos nuevos
- `OPALINE_PHASE2_3_3_IMPLEMENTATION.md`
- `opaline_rust_backend/tests/payment_orders_integration.rs`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó la creación y consulta de órdenes PayPal en Sandbox. Se añadió idempotencia, sincronización de estado y pruebas con un servidor HTTP simulado. No se implementaron capturas, activación de suscripciones ni webhooks.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 1 prueba correcta.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.4 sin autorización expresa.


## FASE 2.3.4 — Webhooks PayPal, validación criptográfica y reconciliación financiera

### Archivos nuevos
- `OPALINE_PHASE2_3_4_IMPLEMENTATION.md`
- `opaline_rust_backend/src/consumers/paypal_webhook.rs`
- `opaline_rust_backend/tests/payment_webhooks_integration.rs`

### Archivos modificados
- `opaline_rust_backend/src/paypal.rs`
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/consumers/mod.rs`
- `opaline_rust_backend/src/main.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó el receptor de webhooks PayPal con verificación criptográfica oficial, persistencia durable, idempotencia, integración con Outbox, procesamiento controlado y reconciliación de órdenes. No se implementaron capturas, reembolsos ni activación de suscripciones.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.5 sin autorización expresa.


## FASE 2.3.5 — Captura de pagos PayPal en Sandbox y confirmación financiera

### Archivos nuevos
- `OPALINE_PHASE2_3_5_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/paypal.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó la captura de órdenes PayPal aprobadas, con validación de elegibilidad, idempotencia, persistencia de transacciones y publicación de eventos Outbox. No se implementaron reembolsos ni activación de suscripciones.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 2 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.6 sin autorización expresa.


## FASE 2.3.6 — Activación de suscripciones mediante pagos confirmados

### Archivos nuevos
- `OPALINE_PHASE2_3_6_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se implementó la activación de suscripciones a partir de pagos PayPal capturados. La activación es transaccional, idempotente y recuperable. Se añadieron columnas para vincular la orden con la suscripción.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 2 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.7 sin autorización expresa.


## FASE 2.3.6.2 — Integridad y recuperación automática de activaciones de suscripciones

### Archivos nuevos
- `OPALINE_PHASE2_3_6_2_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/paypal.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se corrigieron los hallazgos de la auditoría 2.3.6.1. La activación ya no se ignora en captura ni webhooks. Se añadió validación de importe y moneda, reconciliación de activaciones y pruebas de concurrencia.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.7 sin autorización expresa.


## FASE 2.3.6.4 — Recuperación automática y consistencia transaccional de suscripciones

### Archivos nuevos
- `OPALINE_PHASE2_3_6_4_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se corrigieron los hallazgos de la auditoría 2.3.6.3. Se propagaron errores de activación, se eliminaron patrones que descartaban errores de Outbox, se implementó un worker automático de reconciliación y se añadieron pruebas de concurrencia y recuperación.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.7 sin autorización expresa.


## FASE 2.3.6.6 — Garantía de activación financiera, recuperación persistente y validación transaccional

### Archivos nuevos
- `OPALINE_PHASE2_3_6_6_IMPLEMENTATION.md`

### Archivos modificados
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `opaline_rust_backend/tests/payment_webhooks_integration.rs`
- `RECOVERY_STATUS.md`

### Resumen
Se corrigieron los riesgos pendientes de la auditoría 2.3.6.5. Se garantizó que una captura confirmada permanezca vinculada a un proceso de activación recuperable. Se añadió validación de importe y moneda, reconciliación de activaciones y un worker automático controlado.

### Pruebas ejecutadas
- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 2 pruebas correctas.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### Recomendación
No iniciar FASE 2.3.7 sin autorización expresa.
