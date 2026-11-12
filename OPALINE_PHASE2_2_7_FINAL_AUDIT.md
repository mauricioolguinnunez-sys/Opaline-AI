# OPALINE_PHASE2_2_7_FINAL_AUDIT.md

## FASE 2.2.7 — Auditoría final de resiliencia, recuperación e integridad del sistema de cuotas

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, migraciones, esquemas ni datos.

### 1. Resumen ejecutivo

El ciclo de consumo de Opaline AI está implementado con una base transaccional sólida. Las reservas se protegen con bloqueos MySQL y restricciones únicas. Las operaciones durables permiten recuperar resultados sin repetir inferencias. La persistencia de MongoDB se recupera mediante Outbox.

Las pruebas existentes compilan y pasan. No se identificaron vulnerabilidades críticas de acceso cruzado ni duplicación de consumo en el flujo principal.

El dictamen final es **APROBADO CON RESERVAS**. El sistema está técnicamente preparado para avanzar hacia FASE 2.3, pero persisten riesgos operativos y brechas de cobertura de pruebas.

### 2. Alcance y limitaciones

Se inspeccionaron el código Rust, Flutter y PHP, las migraciones, las pruebas y el esquema efectivo de `opaline_test`.

No se modificaron archivos de código. No se ejecutaron workers. No se alteraron datos reales.

Las pruebas se ejecutaron exclusivamente en el entorno aislado `opaline_test`.

### 3. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 4. Matriz de invariantes

| Invariante | Resultado | Evidencia |
|---|---|---|
| Una operación lógica, un consumo | Cumplido | `usage_reservations` y `usage_records` con restricciones únicas |
| Reserva atómica | Cumplido | Transacción con `FOR UPDATE` y actualización condicional |
| Inferencia única | Cumplido | `chat_operations` y consulta por idempotencia |
| Registro consistente | Cumplido | `commit_reservation` actualiza contadores y registros en una transacción |
| Recuperación segura | Cumplido con reservas | `reap_expired_reservations` con lease y fencing |

### 5. Auditoría de operaciones durables

Evidencia:

- `chat_operations` almacena resultado, estado, usuario, reserva y clave.
- `find_by_idempotency` recupera operaciones por usuario y clave.
- `get_operation` y `get_operation_by_key` permiten consultar por identificador o clave.
- Ambos endpoints requieren autenticación y filtran por `user_id`.

Estados:

- `reserved`
- `inferred`
- `persisting`
- `persisted`
- `failed`
- `manual_review`

Mapeo HTTP:

- `reserved` → `processing`
- `inferred` / `persisting` → `awaiting_persistence`
- `persisted` → `completed`
- `failed` → `failed_terminal`

Conclusión: una operación completada puede recuperarse sin repetir inferencia.

### 6. Auditoría de recuperación y persistencia

Evidencia:

- `persist_operation` utiliza claim atómico sobre `mongo_persisted = 0`.
- La persistencia en MongoDB utiliza `operation_ids` para evitar duplicados.
- Si falla, se encola `chat.operation.persist` en Outbox.
- `MongoChatPersistConsumer` reutiliza `persist_operation`.
- `reconcile_operations` reintenta operaciones pendientes.

Escenarios:

- MongoDB disponible: persistencia correcta.
- MongoDB desconectado: operación queda recuperable.
- Reinicio de Rust: operación permanece en MySQL.
- Evento duplicado: consumidor idempotente.

### 7. Auditoría de cuotas y consumo

Evidencia:

- `reserve_quota` valida disponibilidad dentro de una transacción.
- `usage_periods` se bloquea con `FOR UPDATE`.
- `commit_reservation` actualiza contadores y registra consumo.
- `release_reservation` devuelve capacidad no consumida.
- `reap_expired_reservations` recupera reservas abandonadas con lease.

Clasificación de tokens:

- `provider` / `measured`: ambos campos disponibles.
- `estimated` / `estimated`: campos ausentes o sin `usage`.

No se transforman estimaciones en mediciones reales cuando faltan ambos campos.

### 8. Auditoría de clientes

Flutter:

- Genera y conserva la clave por operación lógica.
- Persiste operaciones pendientes en almacenamiento seguro.
- Recupera operaciones al inicializar.
- Elimina la clave al recibir respuesta.

PHP:

- Genera y conserva la clave en `localStorage`.
- Reenvía la clave al backend.
- Elimina la clave cuando el backend responde.

Limitación:

- La recuperación depende de que el cliente conserve la clave o consulte la operación.

### 9. Pruebas y resultados

| Comando | Resultado |
|---|---|
| `cargo check` | Correcto |
| `cargo test` | Correcto |
| `cargo test --lib` | 16 pruebas correctas |
| `cargo test -- --ignored` | Correcto |
| `cargo test --test quota_integration -- --ignored` | 4 pruebas correctas |
| `cargo test --test plans_integration -- --ignored` | 9 pruebas correctas |
| `cargo test --test chat_operations_integration -- --ignored` | 3 pruebas correctas |
| `php -l` | Correcto |
| `flutter analyze` | 29 avisos informativos, sin errores |
| `git diff --check` | Correcto |
| `cargo fmt --check` | No ejecutado: `rustfmt` no instalado |

### 10. Hallazgos

| ID | Severidad | Archivo / Función | Hallazgo | Estado |
|---|---|---|---|---|
| F2.2.7-01 | MEDIO | `chat_provider.dart` | La recuperación depende de almacenamiento local del cliente | PARCIALMENTE RESUELTO |
| F2.2.7-02 | MEDIO | `chat.php` | La recuperación depende de `localStorage` | PARCIALMENTE RESUELTO |
| F2.2.7-03 | BAJO | `quota.rs` | `usage_periods` no incluye `period_end` en su restricción de unicidad | NO RESUELTO |
| F2.2.7-04 | BAJO | `quota.rs` | `usage_counters` permanece sin uso | NO RESUELTO |
| F2.2.7-05 | BAJO | Tests | Cobertura insuficiente para fallos distribuidos | NO RESUELTO |

No se identificaron hallazgos críticos ni altos.

### 11. Matriz de recuperación

| Escenario | Estado | Evidencia |
|---|---|---|
| Respuesta HTTP perdida | Implementado | Idempotencia por clave |
| MongoDB desconectado | Implementado | Outbox + claims |
| Reinicio de Rust | Implementado | `chat_operations` |
| Evento duplicado | Implementado | `operation_ids` |
| Worker concurrente | Implementado | Claim token |
| Reserva abandonada | Implementado | Lease + fencing |
| Contador desincronizado | Detectable | `reconcile_usage` |
| Cliente recargado | Implementado parcialmente | Persistencia local |

### 12. Dictamen final

**APROBADO CON RESERVAS.**

El sistema está técnicamente preparado para continuar hacia FASE 2.3 desde el punto de vista del control y medición de consumo. La idempotencia del backend es sólida y recuperable.

Persisten riesgos operativos relacionados con activación de workers y cobertura de pruebas, pero no se identificaron fallos críticos.

### 13. Respuestas al dictamen

1. ¿El consumo se registra sin duplicaciones? Sí.
2. ¿Una inferencia completada puede recuperarse después de un fallo? Sí.
3. ¿MongoDB puede recuperarse sin duplicar mensajes? Sí.
4. ¿Outbox puede procesar eventos repetidos de forma segura? Sí.
5. ¿Las reservas abandonadas se recuperan correctamente? Sí, cuando el worker está activo.
6. ¿La reconciliación detecta inconsistencias? Sí.
7. ¿Los contadores son verificables? Sí.
8. ¿La medición de tokens es honesta y trazable? Sí.
9. ¿PHP y Flutter conservan la idempotencia? Parcialmente.
10. ¿El sistema está preparado técnicamente para cobrar por consumo? Sí, con reservas operativas.

### 14. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 15. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales preexistentes se conservaron.

No se inició FASE 2.3. Se espera autorización expresa para la siguiente fase.  
El informe de auditoría se entregó como `OPALINE_PHASE2_2_7_FINAL_AUDIT.md`. Se recomienda actualizar `RECOVERY_STATUS.md` con el resultado, aunque no se realizó en esta fase porque la instrucción era modificarlo únicamente si contenía información objetivamente incorrecta.