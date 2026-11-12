# OUTBOX_ADMIN_5_17.md

## Servicio administrativo de revisión y recuperación manual Outbox

### 1. Objetivo

Implementar un servicio administrativo interno para consultar discrepancias de auditoría Outbox, inspeccionar eventos y ejecutar acciones explícitas sobre casos que requieren intervención humana.

No se implementó interfaz visual ni se modificó Flutter o PHP.

### 2. Arquitectura

Módulo: `opaline_rust_backend/src/routes/outbox_admin.rs`

Rutas registradas en `/api/v1/admin/outbox`:

- `GET /discrepancies`
- `GET /events/:id`
- `GET /events/:id/history`
- `POST /events/:id/actions`

El servicio reutiliza:

- `OutboxRecovery`.
- `MongoWallAuditConsumer`.
- `require_admin`.
- Rate limiting existente.
- Tabla `outbox_admin_actions`.

### 3. Autorización

- Solo usuarios con rol `admin` pueden consultar o ejecutar acciones.
- La identidad administrativa se obtiene del Bearer token validado por Rust.
- No se acepta `admin_user_id` desde el cliente.
- Los usuarios normales reciben `403`.
- Las solicitudes sin autenticación reciben `401`.

### 4. Consulta administrativa

`GET /discrepancies` devuelve únicamente discrepancias clasificadas, con metadatos mínimos:

- ID.
- Tipo de evento.
- Tipo y ID de agregado.
- Estado.
- Intentos.
- Fechas.
- Último error.
- Correlación.
- Existencia de documento de auditoría.
- Clasificación.

También devuelve `audit_without_outbox`.

La consulta es paginada y limitada a 50 resultados.

### 5. Historial persistente

Tabla `outbox_admin_actions`:

- `id`
- `event_id`
- `admin_user_id`
- `action_type`
- `reason`
- `previous_status`
- `resulting_status`
- `result`
- `idempotency_key`
- `correlation_id`
- `created_at`
- `completed_at`

Cada intervención queda registrada con el administrador autenticado y su resultado.

### 6. Acciones permitidas

- `recover`: ejecuta recuperación controlada solo si el evento es elegible.
- `mark_manual_review`: registra una decisión sin modificar el evento.
- `record_investigation`: registra una investigación sin modificar el evento.

Acciones rechazadas:

- `failed` sin documento MongoDB.
- `processed` sin documento MongoDB.
- Documento MongoDB sin evento Outbox.
- Evento con claim activo.
- Evento no elegible para recuperación.

### 7. Idempotencia y concurrencia

- Toda acción mutativa exige `idempotency_key`.
- Una repetición con la misma clave y mismos parámetros devuelve el resultado anterior.
- La misma clave con parámetros diferentes devuelve `409`.
- El estado se reevalúa antes de ejecutar.
- Si el estado cambió, la acción se rechaza.
- Se reutiliza el fencing mediante `claim_token`.

### 8. Seguridad de datos

- No se exponen payloads completos.
- No se registran tokens, contraseñas ni credenciales.
- Los motivos administrativos se validan y limitan a 500 caracteres.
- Las consultas son parametrizadas.
- Los errores internos no se exponen al cliente.

### 9. Pruebas

Se añadió `tests/outbox_admin_integration.rs`.

Casos verificados:

- Acceso sin autenticación: 401.
- Acceso con usuario normal: 403.
- Consulta administrativa: passed.
- Inspección de evento: passed.
- Recuperación exitosa: passed.
- Idempotencia: passed.
- Rechazo de evento no elegible: passed.
- Registro de revisión manual: passed.
- Historial persistente: passed.
- Validación de motivo e idempotency key: passed.
- Reutilización de idempotency key con parámetros diferentes: 409.

Resultados:

- `cargo test --lib`: 16 passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 8 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 10. Riesgos residuales

- No hay interfaz visual para el servicio administrativo.
- No hay paginación avanzada ni filtros complejos.
- La recuperación automática sigue limitada a casos elegibles.
- No se implementó una cola administrativa en PHP.
- La garantía sigue siendo al menos una vez, no exactamente una vez.