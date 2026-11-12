# OPALINE_PHASE2_2_3_FINAL_AUDIT.md

## FASE 2.2.3 — Auditoría final de integridad del ciclo de consumo, chat y persistencia

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, migraciones, esquemas ni datos.

### 1. Resumen ejecutivo

El ciclo de consumo implementado en FASE 2.2.2 es sustancialmente más íntegro que el estado auditado en FASE 2.2.1. La inferencia completada queda registrada de forma durable en `chat_operations`, la persistencia de MongoDB se recupera mediante Outbox y la reconciliación detecta inconsistencias entre periodos, reservas y registros de consumo.

Las pruebas existentes compilan y pasan. No se identificaron vulnerabilidades críticas de acceso cruzado ni duplicación de consumo en las rutas principales.

El dictamen final es **APROBADO CON RESERVAS**. El sistema está preparado técnicamente para continuar hacia pagos reales, pero todavía requiere activación operativa de workers y cobertura adicional de recuperación.

### 2. Dictamen general

**APROBADO CON RESERVAS.**

No se encontró una pérdida irrecuperable de resultados en el flujo principal cuando la operación durable se crea correctamente. Sin embargo, existen brechas de cobertura y algunas rutas de recuperación que dependen de activación manual o configuración explícita.

### 3. Diagrama del ciclo de chat

Flujo real observado:

1. `chat()` autentica al usuario mediante Bearer token.
2. Valida el mensaje.
3. Resuelve la suscripción vigente y la versión del plan.
4. Calcula una estimación conservadora de tokens.
5. `reserve_quota()` crea o reutiliza una reserva en MySQL.
6. `create_operation()` crea un registro durable en `chat_operations`.
7. Se ejecuta la inferencia contra LM Studio.
8. Se obtiene la respuesta y las métricas de uso.
9. `commit_reservation()` confirma el consumo y actualiza contadores.
10. `update_inferred()` almacena el resultado durable.
11. `persist_operation()` intenta persistir la conversación en MongoDB.
12. Si falla, se encola un evento Outbox `chat.operation.persist`.
13. Se devuelve la respuesta HTTP con la clave de idempotencia.

### 4. Auditoría de operaciones durables

`chat_operations` contiene:

- Identificador único.
- Usuario.
- Suscripción.
- Periodo de uso.
- Reserva.
- Clave de idempotencia.
- Estado.
- Mensaje del usuario.
- Respuesta del asistente.
- `response_json`.
- `conversation_id`.
- Estado de persistencia MongoDB.
- Tokens y duración.

Restricciones:

- `UNIQUE (user_id, idempotency_key)`
- `UNIQUE (reservation_id)`

Esto permite recuperar una operación completada sin volver a ejecutar inferencia.

Hallazgo:

- `F2.2.3-01` — MEDIO: si una operación queda en estado `reserved` por un fallo antes de inferencia, el flujo posterior puede devolver `409 Conflict` y no reintentar automáticamente con la misma clave.

### 5. Persistencia MongoDB

`persist_operation()` utiliza un claim atómico:

- Solo persiste si `mongo_persisted = 0`.
- Marca la operación como `persisting`.
- Realiza upsert idempotente usando `operation_ids`.
- Marca `persisted` tras éxito.
- Marca `inferred` y registra el error tras fallo.

El consumidor Outbox `MongoChatPersistConsumer` reutiliza la misma función, por lo que los reintentos son idempotentes.

Escenarios verificados por código:

- MongoDB disponible: persistencia correcta.
- MongoDB desconectado: operación queda recuperable.
- Reintento de evento: no duplica mensajes gracias a `operation_ids`.
- Backend reiniciado: operación permanece en MySQL.

### 6. Outbox

La infraestructura Outbox existente se reutiliza para la persistencia de chat.

Características observadas:

- Creación durable del evento.
- Claim con token.
- Reintentos con backoff.
- Manejo de errores temporales y permanentes.
- Confirmación mediante `mark_processed`.
- Recuperación de locks expirados.

Semántica: at-least-once. El consumidor es idempotente.

No se afirma exactly-once entre MySQL y MongoDB.

### 7. Recuperación de reservas

`reap_expired_reservations()`:

- Selecciona reservas `reserved` con `expires_at < NOW()`.
- Utiliza `recovery_owner` y `recovery_lease_expires_at` como lease.
- Libera la reserva solo si el claim tiene éxito.
- Marca la reserva como `expired`.

Workers:

- `OPALINE_QUOTA_RECOVERY_ENABLED=1` activa un worker periódico.
- `OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED=1` activa el consumidor de persistencia.

Por defecto ambos están desactivados.

Hallazgo:

- `F2.2.3-02` — MEDIO: la recuperación automática depende de activación explícita mediante variables de entorno.

### 8. Reconciliación

`reconcile_usage()`:

- Compara `requests_used`, tokens usados y reservas contra `usage_records` y reservas activas.
- Puede aplicar correcciones transaccionales a los contadores.
- Reporta `committed_without_record`.

`reconcile_operations()`:

- Detecta operaciones de chat no persistidas.
- Puede reintentar su persistencia.

La modalidad de solo lectura no modifica datos. La modalidad de recuperación aplica únicamente correcciones controladas.

### 9. Contadores

- El consumo confirmado se contabiliza una sola vez.
- Las reservas pendientes afectan la disponibilidad.
- La liberación devuelve capacidad no consumida.
- Los contadores no pueden quedar negativos por el uso de `GREATEST`.
- Los límites proceden de `plan_versions`.
- Los periodos se identifican por `period_start`.

La reconstrucción desde `usage_records` es posible para auditoría, aunque no existe un proceso automático de reconciliación programado por defecto.

### 10. Medición de tokens

Clasificación implementada:

- `provider` / `measured`: si existen `prompt_tokens` y `completion_tokens`.
- `estimated` / `estimated`: si faltan campos o no existe `usage`.

Se registra:

- Modelo solicitado.
- Modelo reportado por el proveedor cuando está disponible.
- Tokens de entrada, salida y total.
- Fuente y estado de medición.
- Duración.

Las estimaciones no se presentan como mediciones reales cuando ambos campos del proveedor están ausentes. Si solo falta uno, la clasificación es estimada, lo cual es correcto.

### 11. Idempotencia

- El backend acepta `idempotency_key` en el cuerpo o mediante `X-Idempotency-Key`.
- La clave se asocia al usuario autenticado.
- Se devuelve en la respuesta HTTP.
- Una operación completada puede recuperarse sin reejecutar inferencia.
- Reutilizar una clave con parámetros diferentes produce `409 Conflict`.

Los clientes PHP y Flutter no se modificaron en esta fase. El backend genera una clave si el cliente no envía una, pero la preservación end-to-end depende de que el cliente reutilice la clave devuelta.

Hallazgo:

- `F2.2.3-03` — MEDIO: PHP y Flutter no generan ni conservan automáticamente la clave de idempotencia entre reintentos.

### 12. Seguridad

- Los endpoints de chat y uso requieren autenticación.
- Los datos se filtran por `user_id`.
- Las operaciones durables se filtran por propietario.
- Las acciones administrativas usan `require_admin`.
- No se expone consumo de otros usuarios.

No se identificaron vulnerabilidades críticas de acceso cruzado.

### 13. Esquema MySQL y MongoDB

MySQL:

- `chat_operations` tiene restricciones únicas por usuario/idempotencia y por reserva.
- `usage_reservations` tiene restricciones únicas por usuario/idempotencia y por usuario/request.
- `usage_records` tiene restricción única por usuario/request.
- `usage_reservations` incluye lease de recuperación.

MongoDB:

- `conversations` tiene índice único `(user_id, id)`.
- La persistencia de chat utiliza `operation_ids` para deduplicación.

### 14. Pruebas y resultados

| Comando | Resultado |
|---|---|
| `cargo check` | Correcto |
| `cargo test` | Correcto |
| `cargo test --lib` | 16 pruebas correctas |
| `cargo test -- --ignored` | Correcto |
| `cargo test --test quota_integration -- --ignored` | 4 pruebas correctas |
| `cargo test --test plans_integration -- --ignored` | 9 pruebas correctas |
| `cargo test --test chat_operations_integration -- --ignored` | 2 pruebas correctas |
| `git diff --check` | Correcto |
| `cargo fmt --check` | No ejecutado: `rustfmt` no instalado |

Escenarios cubiertos:

- reserva, liberación y consulta;
- idempotencia después de confirmación;
- concurrencia respetando límite;
- persistencia idempotente de operaciones;
- reconciliación de operaciones pendientes.

Escenarios no cubiertos por pruebas automatizadas:

- MongoDB desconectado después de inferencia exitosa.
- Pérdida de confirmación del evento Outbox.
- Reinicio del backend con operación pendiente.
- Dos workers compitiendo por una operación.
- Proceso antiguo intentando confirmar después de fencing.
- Reserva ambigua que debe pasar a revisión manual.
- Contador desincronizado.
- Registro de consumo duplicado.
- Métrica desconocida.
- Clave de idempotencia repetida con parámetros diferentes.
- Respuesta HTTP perdida después de completar una operación.
- Usuario intentando recuperar una operación ajena.

Clasificación: brecha de cobertura de pruebas.

### 15. Hallazgos

| ID | Severidad | Archivo / Función | Hallazgo | Estado |
|---|---|---|---|---|
| F2.2.3-01 | MEDIO | `chat.rs / chat` | Operación en estado `reserved` no es recuperable automáticamente | PARCIALMENTE RESUELTO |
| F2.2.3-02 | MEDIO | `main.rs` | Workers de recuperación desactivados por defecto | NO RESUELTO |
| F2.2.3-03 | MEDIO | Clientes PHP/Flutter | No generan ni conservan automáticamente la clave de idempotencia | NO RESUELTO |
| F2.2.3-04 | BAJO | `quota.rs` | `usage_periods` no incluye `period_end` en su restricción de unicidad | NO RESUELTO |
| F2.2.3-05 | BAJO | `quota.rs` | `usage_counters` permanece sin uso | NO RESUELTO |

No se identificaron hallazgos críticos ni altos.

### 16. Riesgos residuales

- La recuperación automática no está activa por defecto.
- Los clientes no preservan automáticamente la idempotencia.
- La cobertura de pruebas no incluye todos los escenarios de fallo distribuido.
- La precisión de tokens depende del proveedor.

### 17. Configuración operativa

Variables relevantes:

- `OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED=1`: activa el consumidor de persistencia de chat.
- `OPALINE_QUOTA_RECOVERY_ENABLED=1`: activa el worker de recuperación de cuotas.
- `OPALINE_OUTBOX_POLL_MS`, `OPALINE_OUTBOX_BATCH_SIZE`, `OPALINE_OUTBOX_LOCK_TIMEOUT_SECS`, `OPALINE_OUTBOX_MAX_ATTEMPTS`: configuran el worker.

### 18. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 19. Matriz final

| Criterio | Resultado | Evidencia | Bloqueo |
|---|---|---|---|
| Persistencia durable | Implementada | `chat_operations` | No |
| Recuperación MongoDB | Implementada | Outbox + `persist_operation` | Configuración |
| Idempotencia Outbox | Implementada | `operation_ids` + claims | No |
| Recuperación de reservas | Implementada | `reap_expired_reservations` | Configuración |
| Fencing | Implementada | `recovery_owner`, lease | No |
| Reconciliación | Implementada | `reconcile_usage`, `reconcile_operations` | No |
| Contadores | Verificables | Pruebas + consultas | No |
| Medición de tokens | Implementada | `token_source`, `measurement_status` | Precisión del proveedor |
| Clientes | Parcial | Backend genera clave, clientes no la preservan | PHP/Flutter |
| Seguridad | Adecuada | Autenticación y aislamiento | No |

### 20. Respuestas al dictamen

1. ¿El consumo se registra sin duplicaciones? Sí, mediante restricciones únicas y confirmación idempotente.
2. ¿Una inferencia completada puede recuperarse después de un fallo? Sí, mediante `chat_operations` y Outbox.
3. ¿MongoDB puede recuperarse sin duplicar mensajes? Sí, mediante `operation_ids` y claims.
4. ¿Outbox puede procesar eventos repetidos de forma segura? Sí, el consumidor es idempotente.
5. ¿Las reservas abandonadas se recuperan correctamente? Sí, cuando el worker está activo.
6. ¿La reconciliación detecta inconsistencias? Sí.
7. ¿Los contadores son verificables? Sí.
8. ¿La medición de tokens es honesta y trazable? Sí, con clasificación explícita.
9. ¿PHP y Flutter conservan la idempotencia? No automáticamente; dependen de enviar la clave devuelta.
10. ¿El sistema está preparado técnicamente para cobrar por consumo? Sí, con reservas operativas.

### 21. Dictamen final

**APROBADO CON RESERVAS.**

El sistema está técnicamente preparado para continuar hacia pagos reales desde el punto de vista del control y medición de consumo. Antes de facturar, deben resolverse o documentarse las reservas de recuperación automática y la preservación de idempotencia en los clientes.

### 22. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales preexistentes se conservaron.