# OUTBOX_IDEMPOTENCY_HARDENING_5_14_1.md

## Endurecimiento de idempotencia MongoDB y activación segura del consumidor

### 1. Objetivo

Reforzar la garantía de idempotencia del consumidor `MongoWallAuditConsumer`, especialmente ante concurrencia, reintentos y fallos entre la escritura de MongoDB y la confirmación del evento en MySQL.

Garantizar que el worker no procese eventos de auditoría si no puede verificar que MongoDB dispone del índice único necesario para proteger `event_id`.

### 2. Estado real del índice MongoDB

Se verificó la definición real del índice en la colección `wall_events`.

Índice requerido:

- Clave: `{ event_id: 1 }`
- Unicidad: `true`
- Filtro parcial: `{ event_id: { $exists: true } }`

El índice se crea mediante `db::init_mongo` y se verifica mediante `MongoWallAuditConsumer::verify_event_id_index()`.

Si existen duplicados históricos, la creación puede fallar. En ese caso no se eliminan documentos y el problema se comunica en logs. No se afirma idempotencia persistente si el índice no puede verificarse.

### 3. Comportamiento ante duplicados

La protección principal es el índice único de MongoDB.

El consumidor:

1. Consulta si ya existe un documento con el mismo `event_id`.
2. Si existe, devuelve éxito.
3. Si no existe, intenta insertar.
4. Si la inserción falla por clave duplicada, verifica nuevamente que exista un documento con el mismo `event_id`.
5. Solo trata el duplicado como éxito si ese documento existe.
6. Otros errores de MongoDB se tratan como fallos temporales.

No se convierte cualquier error de inserción en éxito.

### 4. Activación segura del worker

El worker permanece desactivado por defecto.

Activación explícita:

`OPALINE_WALL_AUDIT_OUTBOX_ENABLED=1`

Antes de iniciar el worker, se ejecuta `verify_event_id_index()`.

- Si el índice es válido, el worker se inicia.
- Si el índice no es válido, el worker no se inicia.
- Los eventos pendientes permanecen en MySQL.
- El arranque HTTP no se bloquea.
- Se registra un error operativo inequívoco.

### 5. Pruebas implementadas

Se amplió `tests/wall_outbox_integration.rs`.

Casos verificados:

- `wall_audit_index_is_verified`: el índice real se verifica correctamente.
- `wall_audit_index_verification_rejects_missing_index`: si el índice no existe, la verificación falla y se recrea.
- `wall_audit_duplicate_processing_is_idempotent_under_concurrency`: dos procesamientos concurrentes dejan un solo documento.
- `wall_audit_duplicate_document_is_recognized_as_success`: un duplicado correspondiente al mismo `event_id` se reconoce como éxito.
- `wall_audit_event_survives_consumer_failure_before_confirmation`: un reintento no duplica el documento.
- Pruebas previas de persistencia y recuperación del evento.

### 6. Resultados de verificación

- `cargo test --lib`: 14 passed.
- `cargo test --test wall_outbox_integration -- --ignored`: 7 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

Activación manual con `OPALINE_WALL_AUDIT_OUTBOX_ENABLED=1`:

- El backend inició correctamente.
- El log confirmó: `wall audit outbox worker activated`.
- El índice se verificó antes de iniciar el worker.

### 7. Riesgos residuales

- El índice único es best-effort y puede fallar si existen duplicados legacy.
- La idempotencia persistente depende de que el índice único esté activo.
- No hay reconciliación periódica.
- No hay consumidor para otros tipos de evento.
- La garantía es al menos una vez, no exactamente una vez.

### 8. Compatibilidad

No se modificaron Flutter, PHP, contratos HTTP, autenticación, chat, pagos ni reglas de negocio del muro.

El worker permanece desactivado por defecto. Cuando está activado, procesa únicamente eventos de auditoría del muro.