# OUTBOX_DESIGN_5_12.md

## Infraestructura base de Transactional Outbox para Opaline AI

### 1. Objetivo

Implementar una base persistente y comprobable para registrar eventos de auditoría en MySQL y procesarlos posteriormente, sin alterar las operaciones de negocio existentes.

Esta tarea no integra Outbox con muro, autenticación, pagos ni chat. Solo construye la infraestructura y sus pruebas.

### 2. Arquitectura implementada

Componentes:

- `opaline_rust_backend/migrations.sql`: tabla `outbox_events`.
- `opaline_rust_backend/src/outbox.rs`: repositorio y lógica base.
- `opaline_rust_backend/src/lib.rs`: exportación del módulo.
- `opaline_rust_backend/tests/outbox_integration.rs`: pruebas de integración.
- `opaline_rust_backend/tests/common/mod.rs`: bloqueo de base de datos de pruebas.

Flujo futuro previsto:

1. Una operación de negocio inicia una transacción MySQL.
2. Ejecuta su modificación principal.
3. Inserta un evento en `outbox_events` usando la misma transacción.
4. Confirma ambas escrituras con un único `commit`.
5. Un procesador asíncrono lee eventos pendientes.
6. El procesador inserta el evento en MongoDB.
7. El evento se marca como procesado.

MySQL es la fuente de verdad de la intención del evento. MongoDB es el destino de auditoría.

### 3. Esquema de la tabla

```sql
CREATE TABLE IF NOT EXISTS outbox_events (
  id VARCHAR(36) PRIMARY KEY,
  event_type VARCHAR(64) NOT NULL,
  aggregate_type VARCHAR(64) NOT NULL,
  aggregate_id VARCHAR(64) NOT NULL,
  payload JSON NOT NULL,
  status ENUM('pending','processing','processed','failed') NOT NULL DEFAULT 'pending',
  attempts INT NOT NULL DEFAULT 0,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  available_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  processed_at DATETIME NULL,
  last_error TEXT NULL,
  locked_at DATETIME NULL,
  correlation_id VARCHAR(64) NULL,
  KEY idx_outbox_status_available (status, available_at),
  KEY idx_outbox_aggregate (aggregate_type, aggregate_id)
);
```

La migración es aditiva y no elimina datos existentes.

### 4. Modelo de eventos

`NewOutboxEvent` contiene:

- `id`: opcional; si no se proporciona, se genera un UUID.
- `event_type`: tipo de evento.
- `aggregate_type`: entidad relacionada.
- `aggregate_id`: identificador de la entidad.
- `payload`: JSON serializado como texto.
- `correlation_id`: identificador de correlación opcional.
- `available_at`: fecha de disponibilidad opcional.

`OutboxEvent` representa el estado persistido.

### 5. API interna del repositorio

Funciones principales:

- `insert(executor, event)`: inserta un evento usando un executor o transacción SQLx existente.
- `list_pending(pool, limit)`: consulta eventos pendientes disponibles.
- `get_event(pool, id)`: consulta un evento por ID.
- `claim_pending(pool, limit, lock_timeout_secs)`: reclama eventos para procesamiento.
- `mark_processed(pool, id)`: marca un evento como procesado.
- `record_failure(pool, id, error, next_available_at, max_attempts)`: registra un error y programa el siguiente intento o marca como fallido.
- `release_expired_locks(pool, lock_timeout_secs)`: recupera eventos con bloqueo expirado.
- `retry_delay(attempt, base, max)`: calcula backoff exponencial.

### 6. Garantías de atomicidad

La función `insert` acepta un executor genérico de SQLx. Esto permite usar la misma transacción que la operación de negocio:

1. `pool.begin()`
2. Modificación de negocio.
3. `outbox::insert(&mut *tx, &event)`
4. `tx.commit()`

Si cualquier paso falla, el rollback evita que quede una modificación sin su evento, o un evento sin la modificación.

La atomicidad se demostró mediante pruebas de integración con commit y rollback.

### 7. Estrategia de concurrencia

`claim_pending` utiliza:

- `BEGIN`
- `SELECT ... FOR UPDATE SKIP LOCKED`
- Actualización a estado `processing`
- `COMMIT`

Esto evita que dos procesadores reclamen el mismo evento. Eventos distintos pueden procesarse concurrentemente.

La recuperación de bloqueos expirados utiliza `release_expired_locks`, que devuelve eventos `processing` con `locked_at` anterior al umbral a estado `pending`.

### 8. Política de reintentos

- Los intentos se incrementan al reclamar un evento.
- `record_failure` registra el error y programa `available_at`.
- Si los intentos alcanzan el máximo, el estado pasa a `failed`.
- El backoff exponencial se calcula con `retry_delay`.
- Los eventos fallidos se conservan para revisión o reintento administrativo futuro.

No se implementa una interfaz administrativa en esta tarea.

### 9. Idempotencia

- Cada evento tiene un `id` único.
- El consumidor futuro debe tratar la aplicación de un evento como idempotente.
- El estado `processed` no es la única garantía; un proceso puede fallar después de aplicar un efecto externo y antes de marcar el evento.
- Por eso, el consumidor debe usar el `event_id` o una clave de idempotencia derivada para detectar efectos ya aplicados.

No se implementa el consumidor MongoDB en esta tarea.

### 10. Limitaciones

- No hay worker permanente activado.
- No hay integración con endpoints existentes.
- No hay reconciliación periódica.
- No hay procesamiento MongoDB.
- No hay interfaz administrativa para reintentos.
- No se garantiza exactamente una vez; la garantía es al menos una vez.
- La unicidad del índice depende de la ausencia de duplicados legacy.

### 11. Instrucciones de pruebas

Variables de entorno:

```bash
export OPALINE_TEST_DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline_test"
```

Ejecutar:

```bash
cd opaline_rust_backend
cargo test --lib
cargo test --test outbox_integration -- --ignored
```

Las pruebas validan persistencia, atomicidad, claim, procesamiento, fallo, recuperación de bloqueo, concurrencia, idempotencia y persistencia entre conexiones.

### 12. Preparación para futuras integraciones

La infraestructura está lista para integrarse progresivamente con:

- Creación de publicaciones.
- Edición y eliminación.
- Reportes y moderación.
- Eventos de autenticación.
- Pagos, cuando PayPal real esté implementado.

La integración debe realizarse en tareas separadas, con pruebas de fallo controladas y sin modificar datos existentes.

## 13. Evolución en la Tarea 5.13

La Tarea 5.13 añadió un procesador controlado sin activarlo en producción:

- `src/outbox_worker.rs`.
- `EventConsumer` desacoplado.
- `OutboxWorker` con ciclo de vida controlado.
- `claim_token` para fencing de reclamaciones vencidas.
- Reintentos, fallos temporales y permanentes.
- Captura de panics.
- Shutdown ordenado.
- Configuración por variables de entorno.
- Pruebas de worker.

El diseño original de la tabla y el repositorio se mantiene. La nueva capa no modifica endpoints ni reglas de negocio.


## 14. Evolución en la Tarea 5.14

La Tarea 5.14 integró las operaciones compatibles del muro con Transactional Outbox:

- `wall.post.created`
- `wall.post.updated`
- `wall.post.deleted`
- `wall.post.reported`
- `wall.post.auto_hidden`
- `wall.post.moderated`

Las operaciones principales y sus eventos se confirman en la misma transacción MySQL. Un consumidor desacoplado procesa los eventos en MongoDB con idempotencia persistente. El worker se activa únicamente mediante configuración explícita.
