# OUTBOX_RECONCILIATION_5_15.md

## Reconciliación periódica MySQL–MongoDB para auditoría del muro

### 1. Objetivo

Implementar un reconciliador de solo lectura que detecte y clasifique inconsistencias entre los eventos Outbox de MySQL y los documentos de auditoría de MongoDB.

El reconciliador no modifica estados, no crea documentos, no incrementa intentos y no libera bloqueos.

### 2. Arquitectura

Módulo: `opaline_rust_backend/src/outbox_reconciler.rs`

Componentes:

- `ReconcilerConfig`: configuración de intervalo, lote, umbrales y límite por ciclo.
- `ReconciliationReport`: reporte de discrepancias.
- `OutboxReconciler`: ejecuta ciclos de reconciliación.
- `spawn_reconciler`: inicia el reconciliador en Tokio.

El reconciliador utiliza:

- MySQL para consultar `outbox_events`.
- MongoDB para consultar `wall_events`.
- `MongoWallAuditConsumer::verify_event_id_index()` para verificar el índice único.

### 3. Clasificación implementada

- Pendientes antiguos sin documento MongoDB.
- Pendientes antiguos con documento MongoDB.
- Procesamiento vencido sin documento MongoDB.
- Procesamiento vencido con documento MongoDB.
- Eventos fallidos sin documento MongoDB.
- Eventos fallidos con documento MongoDB.
- Eventos procesados sin documento MongoDB.
- Documentos MongoDB existentes con estado MySQL no procesado.
- Documentos MongoDB sin evento Outbox correspondiente.

Solo se examinan eventos cuyo `event_type` comienza con `wall.post.`.

### 4. Configuración

Variables opcionales:

- `OPALINE_WALL_AUDIT_RECONCILIATION_ENABLED`
- `OPALINE_RECONCILER_POLL_SECS`
- `OPALINE_RECONCILER_BATCH_SIZE`
- `OPALINE_RECONCILER_PENDING_AGE_SECS`
- `OPALINE_RECONCILER_PROCESSING_TIMEOUT_SECS`
- `OPALINE_RECONCILER_MAX_EVENTS`

Valores por defecto:

- Intervalo: 60 segundos.
- Lote: 100.
- Antigüedad pendiente: 300 segundos.
- Timeout de procesamiento: 300 segundos.
- Máximo por ciclo: 1000.

### 5. Activación

El reconciliador permanece desactivado por defecto.

Activación explícita:

`OPALINE_WALL_AUDIT_RECONCILIATION_ENABLED=1`

Antes de iniciar:

1. Verifica MySQL.
2. Verifica MongoDB.
3. Verifica el índice único de `event_id`.
4. Si alguna dependencia falla, registra el error y no procesa eventos.
5. No bloquea el arranque HTTP.

### 6. Reglas de solo lectura

El reconciliador no:

- Cambia estados de Outbox.
- Incrementa intentos.
- Libera bloqueos.
- Crea documentos de auditoría.
- Elimina registros.
- Modifica publicaciones ni reportes.

Las invariantes de solo lectura se verificaron mediante pruebas.

### 7. Observabilidad

Cada ciclo registra:

- Eventos examinados.
- Discrepancias detectadas.
- Errores de MySQL o MongoDB.
- Duración implícita por el ciclo.

Los logs incluyen `event_id` cuando corresponde y no registran payloads sensibles.

### 8. Pruebas

Se añadió `tests/outbox_reconciliation_integration.rs`.

Casos verificados:

- Pendiente antiguo sin documento.
- Pendiente antiguo con documento.
- Procesamiento vencido sin documento.
- Procesamiento vencido con documento.
- Fallido sin documento.
- Fallido con documento.
- Procesado sin documento.
- Documento MongoDB con estado MySQL no procesado.
- Documento MongoDB sin evento Outbox.
- Exclusión de eventos no relacionados con el muro.
- Invariantes de solo lectura.

Resultados:

- 5 pruebas passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 9. Riesgos residuales

- No hay recuperación automática.
- No hay reconciliación para otros tipos de evento.
- El índice único es best-effort.
- La garantía sigue siendo al menos una vez, no exactamente una vez.
- No se validó el comportamiento bajo carga real.

### 10. Recomendación para la Tarea 5.16

Esperar autorización. La siguiente prioridad recomendada es implementar recuperación controlada para discrepancias críticas, con garantías transaccionales y pruebas de fallo.