# OPALINE_PHASE2_3_6_3_FINAL_AUDIT.md

## Auditoría final de recuperación, reconciliación e integridad de pagos

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, migraciones, esquemas ni datos.

### 1. Resumen ejecutivo

La FASE 2.3.6.2 corrigió los hallazgos críticos de la auditoría 2.3.6.1.

La activación ya no se ignora en captura ni en webhooks. Se añadió validación de importe y moneda, reconciliación de órdenes capturadas y pruebas de concurrencia. El flujo principal garantiza que un pago capturado puede activar la suscripción correspondiente sin duplicación.

La recuperación automática está implementada mediante funciones y reconciliación, pero los workers permanecen desactivados por defecto. Por ello, la recuperación automática es una capacidad implementada, no una operación activa por defecto.

Dictamen final: **APROBADA CON RESERVAS**.

### 2. Alcance y limitaciones

Se inspeccionó el código Rust, las migraciones, las pruebas y el esquema efectivo de `opaline_test`.

No se modificaron archivos de código. No se ejecutaron operaciones financieras reales. Las pruebas se ejecutaron exclusivamente en el entorno aislado.

### 3. Archivos inspeccionados

- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/paypal.rs`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/main.rs`
- `opaline_rust_backend/src/routes/plans.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `opaline_rust_backend/tests/payment_webhooks_integration.rs`
- `opaline_rust_backend/tests/security_integration.rs`
- `opaline_rust_backend/tests/plans_integration.rs`
- `opaline_rust_backend/tests/quota_integration.rs`
- `opaline_rust_backend/tests/chat_operations_integration.rs`
- `OPALINE_PHASE2_3_6_IMPLEMENTATION.md`
- `OPALINE_PHASE2_3_6_1_FINAL_AUDIT.md`
- `OPALINE_PHASE2_3_6_2_IMPLEMENTATION.md`

### 4. Skills y documentación consultadas

| Skill o documento | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |
| `OPALINE_PHASE2_3_6_IMPLEMENTATION.md` | Sí | Sí | Comparación con implementación real |
| `OPALINE_PHASE2_3_6_1_FINAL_AUDIT.md` | Sí | Sí | Seguimiento de hallazgos |
| `OPALINE_PHASE2_3_6_2_IMPLEMENTATION.md` | Sí | Sí | Verificación de correcciones |

### 5. Arquitectura real de captura y activación

Flujo observado:

1. `create_paypal_order` crea la orden local y en PayPal.
2. `capture_paypal_order_service` valida elegibilidad.
3. La captura se ejecuta en PayPal.
4. Se persiste `payment_transactions`.
5. La orden se marca como `captured`.
6. Se publica `payment.capture.completed` en Outbox.
7. Se invoca `activate_subscription_from_payment`.
8. La activación crea la suscripción y vincula la orden.
9. El motor de cuotas utiliza la suscripción activa.

### 6. Evaluación de webhooks

Evidencia en `process_payment_webhook_event`:

- El evento se verifica antes de procesarlo.
- La captura completada inserta la transacción si no existe.
- Se actualiza el estado de la orden.
- Se invoca la activación.
- Si la activación falla, el evento no se marca como procesado.

Los eventos duplicados devuelven éxito idempotente.

### 7. Evaluación de reconciliación

Evidencia:

- `reconcile_payment_orders` compara estado, importe y moneda con PayPal.
- `reconcile_payment_activations` detecta órdenes capturadas sin suscripción.
- Ambas pueden ejecutarse en modo auditoría o aplicación.
- La reconciliación de activaciones es idempotente.

Limitación:

- No existe un worker que invoque automáticamente la reconciliación de activaciones.
- La recuperación automática depende de activación explícita mediante variables de entorno.

### 8. Estado operativo de workers

Variables relevantes:

- `OPALINE_PAYMENT_WEBHOOK_OUTBOX_ENABLED=1`
- `OPALINE_QUOTA_RECOVERY_ENABLED=1`

Comportamiento:

- Los workers permanecen desactivados por defecto.
- La función de reconciliación existe, pero no se ejecuta automáticamente.
- La recuperación de reservas existe, pero no se ejecuta automáticamente.

Conclusión: la capacidad de recuperación está implementada, pero no es operativa por defecto.

### 9. Idempotencia y concurrencia

Evidencia:

- `payment_orders` tiene `UNIQUE (user_id, idempotency_key)`.
- `payment_transactions` tiene `UNIQUE (provider_transaction_id)`.
- `payment_webhook_events` tiene `UNIQUE (provider, provider_event_id)`.
- `payment_orders` tiene `activated_subscription_id`.
- `capture_paypal_order_service` reutiliza transacciones pendientes.
- `activate_subscription_from_payment` devuelve la suscripción existente si ya fue activada.
- Los bloqueos `FOR UPDATE` serializan activaciones.

Pruebas:

- `payment_activation_is_idempotent_under_concurrency` pasó.
- `paypal_order_can_be_captured_once` pasó.
- `payment_webhooks_integration` pasó.

### 10. Atomicidad y recuperación

Evidencia:

- `activate_subscription_from_payment` inicia una transacción SQL.
- Bloquea la orden y las suscripciones del usuario.
- Crea la suscripción.
- Vincula la orden y la transacción.
- Registra el evento de suscripción.
- Inserta el evento Outbox.
- Confirma la transacción.

Si falla antes del commit, la transacción se revierte.

Limitación:

- El evento Outbox se inserta con `let _ =`, por lo que un fallo al publicarlo no revierte la activación.
- La activación se invoca después de confirmar la captura. Si falla, la orden queda capturada sin suscripción, aunque el endpoint explícito permite recuperarla.

### 11. Integridad MySQL

Evidencia:

- `payment_orders` tiene claves foráneas a `users`, `plans` y `plan_versions`.
- `payment_transactions` tiene claves foráneas a `payment_orders` y `users`.
- `payment_refunds` tiene claves foráneas a `payment_transactions` y `users`.
- `activated_subscription_id` tiene clave foránea a `subscriptions`.
- `uq_payment_orders_provider_order` protege el identificador de PayPal.
- `uq_payment_orders_user_idempotency` protege la clave de idempotencia.

La reconciliación puede buscar órdenes por `provider_order_id` y `status`.

### 12. Integración con cuotas

Evidencia:

- `activate_subscription_from_payment` crea la suscripción con `plan_version_id` de la orden.
- `resolve_current_subscription` utiliza la suscripción vigente.
- `get_plan_limits` obtiene límites desde `plan_versions`.
- El periodo se establece al activar.

Conclusión: la activación integra correctamente el motor de cuotas.

### 13. Seguridad y Sandbox

Evidencia:

- PayPal Live permanece bloqueado en `PayPalEnvironment`.
- Las credenciales se leen desde variables de entorno.
- Los endpoints requieren autenticación.
- La propiedad de la orden se verifica en SQL.
- Los webhooks requieren verificación criptográfica.

No se identificaron vulnerabilidades críticas.

### 14. PHP y Flutter

Estado observado:

- PHP consulta la suscripción mediante `GET /suscripcion`.
- Flutter consulta el plan y la suscripción.
- Ninguno activa suscripciones directamente.
- El estado mostrado depende del backend.

Limitación:

- No se muestran estados intermedios de activación.
- No se invocan endpoints de activación desde los clientes.

### 15. Cobertura de pruebas

Pruebas existentes:

- `payment_orders_integration`: 4 pruebas.
- `payment_webhooks_integration`: 1 prueba.
- `security_integration`: 7 pruebas.
- `plans_integration`: 9 pruebas.
- `quota_integration`: 4 pruebas.
- `chat_operations_integration`: 3 pruebas.

Escenarios cubiertos:

- Creación y consulta de órdenes.
- Captura idempotente.
- Activación idempotente bajo concurrencia.
- Reconciliación de órdenes capturadas.
- Webhook válido e idempotente.
- Autenticación y aislamiento.
- Cuotas y concurrencia.

Escenarios no cubiertos:

- Activación duplicada explícita.
- Fallo transaccional durante activación.
- Webhook duplicado activando suscripción.
- Reconciliación repetida.
- Recuperación tras reinicio con operación pendiente.
- Cuotas tras activación fallida.
- Periodo sin extensión duplicada.

Clasificación: brecha de cobertura.

### 16. Hallazgos

| ID | Severidad | Archivo / Función | Hallazgo | Estado |
|---|---|---|---|---|
| F2.3.6.3-01 | MEDIO | `payments.rs / capture_paypal_order_service` | Si la activación falla, la orden queda capturada sin suscripción | RESUELTO PARCIALMENTE |
| F2.3.6.3-02 | MEDIO | `payments.rs / process_payment_webhook_event` | Si la activación falla, el evento queda pendiente de reintento | RESUELTO PARCIALMENTE |
| F2.3.6.3-03 | MEDIO | `main.rs` | No existe worker que invoque automáticamente la reconciliación de activaciones | NO RESUELTO |
| F2.3.6.3-04 | MEDIO | Tests | No existen pruebas de fallo transaccional de activación | NO VERIFICADO |
| F2.3.6.3-05 | BAJO | `payments.rs` | El evento Outbox se inserta con `let _ =` | NO VERIFICADO |
| F2.3.6.3-06 | BAJO | `quota.rs` | `usage_periods` no incluye `period_end` en su restricción de unicidad | NO RESUELTO |
| F2.3.6.3-07 | BAJO | `quota.rs` | `usage_counters` permanece sin uso | NO RESUELTO |

No se identificaron defectos críticos.

### 17. Matriz de cumplimiento

| Criterio | Estado | Evidencia | Riesgo residual |
|---|---|---|---|
| Captura confirmada | Cumplido | `payment_transactions.status='completed'` | Bajo |
| Activación desde captura | Cumplido | `activate_subscription_from_payment` | Medio |
| Activación desde webhook | Cumplido | `process_payment_webhook_event` | Medio |
| Activaciones pendientes | Implementado | `reconcile_payment_activations` | Worker desactivado |
| Reconciliación operativa | Parcialmente verificado | Función disponible | Sin worker |
| Recuperación tras reinicio | Parcialmente verificado | Endpoint explícito | Sin automatización |
| Idempotencia | Cumplido | `activated_subscription_id` | Bajo |
| Concurrencia | Verificado | Prueba de concurrencia | Bajo |
| Atomicidad MySQL | Cumplido | Transacción SQL | Medio por Outbox ignorado |
| Outbox | Parcialmente verificado | `outbox::insert` | Ignorado |
| Cuotas | Cumplido | `resolve_current_subscription` | Bajo |
| Periodos | Cumplido | `current_period_start`, `current_period_end` | Bajo |
| Seguridad | Cumplido | Token y `user_id` | Bajo |
| PayPal Live bloqueado | Cumplido | `PayPalEnvironment` | Bajo |
| PHP y Flutter | Compatible | Consulta de suscripción | Sin estados intermedios |

### 18. Dictamen final

**APROBADA CON RESERVAS.**

No se identificaron defectos críticos. La activación funciona en el flujo principal y las pruebas existentes pasan.

Persisten riesgos de recuperación automática y cobertura de pruebas que deben resolverse o documentarse antes de considerar la fase completamente cerrada.

### 19. Pruebas y verificación

Comandos ejecutados:

- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 3 pruebas correctas.
- `git diff --check`: correcto.

### 20. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales preexistentes se conservaron.

No se inició FASE 2.3.7. Se espera autorización expresa para cualquier trabajo posterior.  
El informe de auditoría se entregó como `OPALINE_PHASE2_3_6_3_FINAL_AUDIT.md`. Se recomienda actualizar `RECOVERY_STATUS.md` con el resultado, aunque no se realizó en esta fase porque la instrucción era modificarlo únicamente si contenía información objetivamente incorrecta.