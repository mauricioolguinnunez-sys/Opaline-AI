# OUTBOX_WORKER_5_13.md

## Procesador controlado de eventos Transactional Outbox

### 1. Arquitectura del worker

El procesador se implementa en `opaline_rust_backend/src/outbox_worker.rs`.

Componentes:

- `OutboxWorkerConfig`: configuración de intervalo, lote, timeout, intentos y backoff.
- `OutboxWorker<C>`: worker genérico sobre un consumidor desacoplado.
- `EventConsumer`: contrato para procesar eventos.
- `ConsumerOutcome`: resultado del procesamiento.
- `spawn_worker`: función auxiliar para iniciar el worker en Tokio.
- `NoopConsumer`: consumidor de prueba que marca eventos como fallidos permanentes.

El worker no está integrado con operaciones de negocio ni con MongoDB.

### 2. Ciclo de vida

El worker ejecuta un bucle controlado:

1. Comprueba la señal de shutdown.
2. Reclama un lote de eventos pendientes.
3. Procesa cada evento mediante el consumidor.
4. Registra éxito, fallo temporal o fallo permanente.
5. Libera bloqueos expirados.
6. Espera el intervalo configurado o la señal de cancelación.

La cancelación se realiza mediante `tokio::sync::watch`. El worker termina de forma ordenada al recibir la señal.

### 3. Mecanismo de fencing

La reclamación genera un `claim_token` único por operación.

Las actualizaciones de estado incluyen la condición:

`WHERE id = ? AND claim_token = ?`

Si un worker antiguo intenta confirmar después de que otro worker ha reclamado el mismo evento, la actualización afecta cero filas y no sobrescribe el resultado.

Esto evita que un worker lento confirme una reclamación que ya pertenece a otro procesador.

### 4. Contrato de consumidores

El consumidor recibe un `Arc<OutboxEvent>` con:

- `id`
- `event_type`
- `aggregate_type`
- `aggregate_id`
- `payload`
- `correlation_id`

Debe devolver:

- `Success`
- `Temporary(error)`
- `Permanent(error)`

Los tipos desconocidos deben tratarse explícitamente. El worker no marca eventos como procesados silenciosamente.

El procesamiento asume entrega al menos una vez. Los consumidores futuros deben ser idempotentes.

### 5. Estrategia de idempotencia

El worker usa el `event_id` como identificador único.

El consumidor de prueba evita duplicar efectos comprobando si el identificador del agregado ya fue procesado.

Se demuestra que:

- Repetir un evento no duplica su efecto.
- Un fallo después del efecto pero antes de la confirmación no genera un efecto duplicado al reintentarse.

Los consumidores reales, especialmente los que interactúen con MongoDB, deberán implementar su propia idempotencia.

### 6. Política de reintentos

Se reutiliza `outbox::retry_delay`.

- Fallos temporales se programan con backoff exponencial.
- Fallos permanentes se marcan como `failed`.
- El número de intentos se incrementa al reclamar.
- El último error se conserva.
- Los eventos fallidos no se eliminan.

### 7. Manejo de errores

- Los errores de base de datos se propagan desde `process_once`.
- Los panics del consumidor se capturan y se registran como fallos permanentes.
- La cancelación del consumidor se registra como fallo temporal.
- Un error de confirmación por claim vencido no sobrescribe el estado.
- El worker no termina el proceso principal ante un error de lote.

### 8. Shutdown

El worker acepta `tokio::sync::watch::Receiver<bool>`.

Al recibir la señal:

1. Termina el bucle principal.
2. No crea nuevas reclamaciones.
3. Los eventos en procesamiento quedan con bloqueo expirado y se recuperan posteriormente.

No se liberan forzosamente bloqueos de otros workers.

### 9. Observabilidad

Se registran mediante `tracing`:

- ID del evento.
- Tipo de evento.
- Intento.
- Resultado.
- Errores.

No se registran payloads sensibles, tokens, contraseñas ni credenciales.

### 10. Configuración

Variables opcionales:

- `OPALINE_OUTBOX_POLL_MS`
- `OPALINE_OUTBOX_BATCH_SIZE`
- `OPALINE_OUTBOX_LOCK_TIMEOUT_SECS`
- `OPALINE_OUTBOX_MAX_ATTEMPTS`

Valores por defecto:

- Intervalo: 5 segundos.
- Lote: 10.
- Timeout de bloqueo: 300 segundos.
- Intentos máximos: 5.

### 11. Activación

El worker no se activa automáticamente en `main.rs`.

Puede iniciarse explícitamente mediante `OutboxWorker::new(...).run(shutdown)` o `spawn_worker`.

Esta decisión evita activar un consumidor no configurado y evita bloquear el arranque HTTP.

### 12. Pruebas

Se añadió `tests/outbox_worker_integration.rs`.

Casos verificados:

- Procesamiento normal.
- Fallo temporal y reintento.
- Fallo permanente.
- Panic del consumidor.
- Idempotencia del efecto.
- Shutdown ordenado.
- Fencing de claim vencido.

Resultados:
- 7 pruebas passed.
- `cargo test --lib`: 14 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 13. Limitaciones

- No hay worker permanente activado.
- No hay consumidor MongoDB real.
- No hay integración con operaciones de negocio.
- No hay reconciliación periódica.
- No hay interfaz administrativa para reintentos.
- La garantía es al menos una vez, no exactamente una vez.

### 14. Recomendación para la Tarea 5.14

Esperar autorización. La siguiente prioridad recomendada es implementar un consumidor controlado para un tipo de evento real del muro, con idempotencia y pruebas de fallo, sin modificar los endpoints existentes.