# OUTBOX_WALL_INTEGRATION_5_14.md

## Integración Transactional Outbox con auditoría del muro

### 1. Arquitectura anterior

Antes de esta tarea, las operaciones del muro escribían directamente en MongoDB después de confirmar la operación principal en MySQL. Si MongoDB fallaba, la auditoría podía perderse aunque la operación principal ya hubiera quedado persistida.

### 2. Arquitectura nueva

Flujo integrado:

1. El usuario ejecuta una operación del muro.
2. Rust valida autorización, suscripción, contenido y reglas de negocio.
3. La operación principal se ejecuta dentro de una transacción MySQL.
4. En la misma transacción se inserta un evento en `outbox_events`.
5. Se confirma la transacción.
6. El worker reclama el evento.
7. `MongoWallAuditConsumer` lo procesa en MongoDB.
8. El evento se marca como procesado.

MySQL es la fuente de verdad de la operación. MongoDB es el destino de auditoría.

### 3. Fuente de verdad de cada operación

| Operación | Fuente de verdad | Motor principal | Evento Outbox |
|---|---|---|---|
| Crear publicación | MySQL `posts` | MySQL | `wall.post.created` |
| Editar publicación | MySQL `posts` | MySQL | `wall.post.updated` |
| Eliminar publicación | MySQL `posts` | MySQL | `wall.post.deleted` |
| Reportar publicación | MySQL `reports` / `posts` | MySQL | `wall.post.reported`, `wall.post.auto_hidden` |
| Moderar publicación | MySQL `posts` | MySQL | `wall.post.moderated` |
| Lectura pública | MySQL `posts` | MySQL | N/A |
| Estadísticas | MySQL `posts` | MySQL | N/A |

No se migró la fuente de verdad de ninguna operación.

### 4. Operaciones integradas

- Creación de publicaciones.
- Edición de publicaciones.
- Eliminación lógica.
- Reportes.
- Ocultamiento automático por acumulación de reportes.
- Moderación administrativa.

### 5. Operaciones no integradas

No aplican a esta tarea o no se modificaron:

- Lectura pública.
- Estadísticas.
- Paginación.
- Suscripciones y pagos.
- Autenticación.
- Chat.

### 6. Eventos generados

| Evento | Agregado | Payload mínimo |
|---|---|---|
| `wall.post.created` | `post` | `post_id`, `user_id`, `content_length`, `rating`, `idempotency_key` |
| `wall.post.updated` | `post` | `post_id`, `user_id`, `content_length` |
| `wall.post.deleted` | `post` | `post_id`, `user_id` |
| `wall.post.reported` | `post` | `post_id`, `reporter_user_id`, `reason`, `detail`, `hidden` |
| `wall.post.auto_hidden` | `post` | `post_id`, `reason` |
| `wall.post.moderated` | `post` | `post_id`, `actor_user_id`, `action`, `reason`, `previous_status`, `new_status` |

Los payloads no incluyen contraseñas, tokens ni credenciales.

### 7. Garantías de atomicidad

Para cada operación compatible:

1. Se inicia una transacción MySQL.
2. Se ejecuta la modificación principal.
3. Se inserta el evento Outbox en la misma transacción.
4. Se confirma ambas escrituras juntas.

Si la transacción falla, ni la modificación principal ni el evento persisten.

Una indisponibilidad posterior de MongoDB no convierte una operación ya confirmada en un falso error de negocio.

### 8. Consumidor MongoDB

Se implementó `MongoWallAuditConsumer` en:

`opaline_rust_backend/src/consumers/mongo_wall_audit.rs`

Responsabilidades:

- Validar el tipo de evento.
- Validar el payload requerido.
- Detectar eventos ya procesados.
- Insertar el documento de auditoría en MongoDB.
- Devolver éxito, fallo temporal o fallo permanente.

La colección utilizada es `wall_events`, compatible con los registros históricos.

### 9. Estrategia de idempotencia

- El `event_id` es el identificador único.
- Antes de insertar, el consumidor consulta por `event_id`.
- Si el documento ya existe, devuelve éxito.
- Se intenta crear un índice único parcial sobre `event_id` en `wall_events`.
- Si el índice no puede crearse por duplicados legacy, la idempotencia se mantiene a nivel de consumidor, pero no se garantiza a nivel de base de datos.

### 10. Índices

- Conversaciones: índice único best-effort sobre `(user_id, id)`.
- Auditoría del muro: índice único parcial best-effort sobre `event_id`.

No se eliminaron ni modificaron registros históricos.

### 11. Configuración del worker

El worker no se activa automáticamente.

Variable de activación:

`OPALINE_WALL_AUDIT_OUTBOX_ENABLED=1`

Variables opcionales:

- `OPALINE_OUTBOX_POLL_MS`
- `OPALINE_OUTBOX_BATCH_SIZE`
- `OPALINE_OUTBOX_LOCK_TIMEOUT_SECS`
- `OPALINE_OUTBOX_MAX_ATTEMPTS`

### 12. Recuperación

- Si MongoDB no está disponible, el evento permanece pendiente en MySQL.
- El worker reintenta con backoff.
- Los bloqueos expirados se recuperan.
- Un evento procesado parcialmente no se duplica al reintentar.
- Un worker antiguo no puede sobrescribir una reclamación nueva.

### 13. Pruebas

Se añadió `tests/wall_outbox_integration.rs`.

Casos verificados:

- El evento Outbox se persiste al crear una publicación.
- El evento permanece pendiente antes de que el consumidor lo procese.
- El consumidor crea el documento de auditoría en MongoDB.
- Repetir el mismo evento no duplica el documento.
- El evento sobrevive antes de que el consumidor lo procese.

Pruebas existentes:

- `cargo test --lib`: passed.
- `cargo test --test wall_integration -- --ignored`: passed.
- `cargo test --test outbox_integration -- --ignored`: passed.
- `cargo test --test outbox_worker_integration -- --ignored`: passed.
- `cargo test --test wall_outbox_integration -- --ignored`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 14. Riesgos residuales

- El worker no se activa por defecto.
- No hay reconciliación periódica.
- No hay consumidor para otros tipos de evento.
- El índice único de auditoría es best-effort.
- La garantía es al menos una vez, no exactamente una vez.
- No se implementó una cola administrativa en PHP.

### 15. Recomendación para la Tarea 5.15

Esperar autorización. La siguiente prioridad recomendada es implementar un consumidor para otro tipo de evento real, con idempotencia persistente y pruebas de fallo, o añadir reconciliación periódica entre MySQL y MongoDB.