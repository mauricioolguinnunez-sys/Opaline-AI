# OPALINE_PHASE2_3_6_1_FINAL_AUDIT.md

## Auditoría de integridad financiera y activación de suscripciones

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, migraciones, esquemas ni datos.

### 1. Resumen ejecutivo

La FASE 2.3.6 implementa la activación de suscripciones a partir de pagos PayPal capturados. La evidencia de código y las pruebas ejecutadas confirman que una captura confirmada puede activar una suscripción vinculada a la versión correcta del plan, establecer su periodo y actualizar el historial de suscripción.

La activación se realiza dentro de una transacción MySQL y está protegida por idempotencia mediante `activated_subscription_id`. Las pruebas de captura y webhooks pasan.

El dictamen final es **APROBADA CON RESERVAS**. No se identificaron defectos críticos, pero existen riesgos relevantes de recuperación y cobertura de pruebas.

### 2. Alcance y limitaciones

Se inspeccionó el código Rust, las migraciones, las pruebas y el esquema efectivo de `opaline_test`.

No se modificaron archivos, datos ni configuraciones. No se ejecutaron operaciones financieras reales.

Las pruebas se ejecutaron exclusivamente contra el entorno aislado `opaline_test`.

### 3. Archivos inspeccionados

- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/paypal.rs`
- `opaline_rust_backend/src/models.rs`
- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/routes/plans.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/tests/payment_orders_integration.rs`
- `opaline_rust_backend/tests/payment_webhooks_integration.rs`
- `opaline_rust_backend/tests/security_integration.rs`
- `opaline_rust_backend/tests/quota_integration.rs`

### 4. Skills y documentación consultadas

| Skill o documento | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |
| `OPALINE_PHASE2_3_6_IMPLEMENTATION.md` | Sí | Sí | Comparación con implementación real |

### 5. Arquitectura real del flujo financiero

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

### 6. Validación de capturas

Evidencia en `activate_subscription_from_payment`:

- Consulta la orden por `id` y `user_id`.
- Exige una transacción `completed`.
- Exige `order_status == "captured"`.
- Utiliza `plan_id` y `plan_version_id` almacenados localmente.
- No acepta importe, moneda, plan ni usuario del cliente.

Conclusión: la activación exige evidencia de captura completada.

### 7. Atomicidad

Evidencia:

- `pool.begin()` inicia la transacción.
- Se bloquea la orden con `FOR UPDATE`.
- Se bloquean las suscripciones del usuario.
- Se crea la suscripción.
- Se actualiza la orden.
- Se actualiza la transacción.
- Se inserta el evento de suscripción.
- Se inserta el evento Outbox.
- `tx.commit()` confirma todo.

Si falla antes de commit, la transacción se revierte.

Limitación:

- El evento Outbox se inserta con `let _ =`, por lo que un fallo al publicarlo no revierte la activación.
- La activación se invoca después de confirmar la captura. Si falla, la orden queda capturada sin suscripción activa, aunque el endpoint explícito permite recuperarla.

### 8. Idempotencia y concurrencia

Evidencia:

- `activated_subscription_id` evita activaciones duplicadas.
- La función devuelve la suscripción existente si ya fue activada.
- El bloqueo `FOR UPDATE` serializa activaciones concurrentes para la misma orden.
- `uq_subscriptions_user_current` protege la unicidad de la suscripción vigente.
- Los reintentos de captura devuelven la misma transacción.

Conclusión: la idempotencia está bien implementada para el flujo principal.

### 9. Recuperación

Evidencia:

- `capture_paypal_order_service` es idempotente.
- `activate_subscription_from_payment` es idempotente.
- El endpoint explícito permite activar una orden capturada.
- Webhooks pueden activar la suscripción si la orden ya está capturada.

Limitación:

- No existe reconciliación automática que active órdenes capturadas pero no activadas.
- Si la activación falla, el usuario debe llamar manualmente al endpoint de activación.

### 10. Integración con cuotas

Evidencia:

- La suscripción se crea con `plan_version_id` de la orden.
- `resolve_current_subscription` utiliza la suscripción vigente.
- `get_plan_limits` obtiene límites desde `plan_versions`.
- El periodo se establece al activar la suscripción.

Conclusión: la activación integra correctamente el motor de cuotas.

### 11. Seguridad del endpoint

Endpoint:

- `POST /api/v1/payments/paypal/orders/:order_id/activate`

Controles:

- Bearer obligatorio.
- Usuario obtenido del token.
- Propiedad verificada en SQL.
- Rate limiting por IP y usuario.
- No acepta parámetros del cliente para conceder beneficios.

### 12. Periodos y ciclo de vida

Evidencia:

- Se utiliza UTC mediante `NOW()`.
- La duración se deriva de `plans.period`.
- `current_period_start` y `current_period_end` se establecen.
- `cancel_at_period_end` se inicializa en `0`.
- `auto_renew` se inicializa en `1`.

Política observada:

- Una nueva suscripción se convierte en la vigente.
- Las suscripciones anteriores se marcan como no vigentes mediante `is_current = NULL`.
- No se marcan como canceladas ni expiradas.

Esto es coherente con la unicidad implementada, pero no define explícitamente la política de sustitución.

### 13. Cobertura de pruebas

Pruebas existentes:

- `payment_orders_integration`: creación, consulta y captura.
- `payment_webhooks_integration`: webhook válido e idempotente.
- `security_integration`: autenticación y aislamiento.
- `quota_integration`: cuotas y concurrencia.
- `plans_integration`: suscripciones.

Escenarios cubiertos:

- Captura exitosa activa suscripción.
- Idempotencia de captura.
- Aislamiento por usuario.
- Webhook válido.

Escenarios no cubiertos:

- Activación duplicada explícita.
- Concurrencia de activación.
- Fallo transaccional durante activación.
- Webhook duplicado activando suscripción.
- Reconciliación repetida.
- Cuotas tras activación fallida.
- Periodo sin extensión duplicada.

Clasificación: brecha de cobertura.

### 14. Evaluación PHP y Flutter

No se modificaron PHP ni Flutter en FASE 2.3.6.

Estado observado:

- PHP consulta la suscripción mediante `GET /suscripcion`.
- Flutter consulta el plan y la suscripción.
- Ninguno activa suscripciones directamente.
- El estado mostrado depende del backend.

Conclusión: la integración es compatible, pero no muestra estados intermedios de activación.

### 15. Hallazgos

| ID | Severidad | Archivo / Función | Hallazgo | Estado |
|---|---|---|---|---|
| F2.3.6.1-01 | ALTO | `payments.rs / capture_paypal_order_service` | La activación se ignora con `let _ =`, por lo que un pago capturado puede no activar la suscripción | RESUELTO PARCIALMENTE |
| F2.3.6.1-02 | MEDIO | `payments.rs / activate_subscription_from_payment` | No valida que el importe y la moneda de la orden coincidan con el plan | NO VERIFICADO |
| F2.3.6.1-03 | MEDIO | `payments.rs / capture_paypal_order_service` | La activación no se propaga como error si falla | RESUELTO PARCIALMENTE |
| F2.3.6.1-04 | MEDIO | `payments.rs / process_payment_webhook_event` | La activación desde webhook se ignora con `let _ =` | RESUELTO PARCIALMENTE |
| F2.3.6.1-05 | MEDIO | Tests | No existen pruebas de concurrencia ni fallo transaccional de activación | NO VERIFICADO |
| F2.3.6.1-06 | BAJO | `payments.rs` | El evento Outbox se inserta con `let _ =`, por lo que un fallo no revierte la activación | NO VERIFICADO |
| F2.3.6.1-07 | BAJO | `payments.rs` | No existe reconciliación que active órdenes capturadas pero no activadas | NO VERIFICADO |

No se identificaron defectos críticos.

### 16. Matriz de cumplimiento

| Criterio | Resultado | Evidencia | Riesgo residual |
|---|---|---|---|
| Captura confirmada | Cumplido | `payment_transactions.status='completed'` | Bajo |
| Importe y moneda | Cumplido | Validación en captura | Bajo |
| Atomicidad | Cumplido | Transacción SQL | Medio por Outbox ignorado |
| Restricciones MySQL | Cumplido | `uq_payment_orders_user_idempotency`, `activated_subscription_id` | Bajo |
| Idempotencia | Cumplido | `activated_subscription_id` | Bajo |
| Concurrencia | Parcialmente verificado | Bloqueos SQL | Falta prueba |
| Webhooks | Parcialmente verificado | `process_payment_webhook_event` | Falta prueba |
| Reconciliación | No verificado | No implementada para activación | Medio |
| Recuperación | Parcialmente verificado | Endpoint explícito | Falta prueba |
| Outbox | Parcialmente verificado | `outbox::insert` | Ignorado |
| Cuotas | Cumplido | `resolve_current_subscription` | Bajo |
| Autorización | Cumplido | Token y `user_id` | Bajo |
| Periodos | Cumplido | `current_period_start`, `current_period_end` | Bajo |
| PHP y Flutter | Compatible | Consulta de suscripción | Sin estados intermedios |

### 17. Dictamen final

**APROBADA CON RESERVAS.**

No se identificaron defectos críticos. La activación funciona en el flujo principal y las pruebas existentes pasan.

Persisten riesgos de recuperación y cobertura que deben resolverse o documentarse antes de considerar la fase completamente cerrada.

### 18. Pruebas y verificación

Comandos ejecutados:

- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 2 pruebas correctas.
- `cargo test --test payment_webhooks_integration -- --ignored`: 1 prueba correcta.
- `cargo test --test security_integration -- --ignored`: 7 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

### 19. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales preexistentes se conservaron.

No se inició FASE 2.3.7. Se espera autorización expresa para cualquier trabajo posterior.  
El informe de auditoría se entregó como `OPALINE_PHASE2_3_6_1_FINAL_AUDIT.md`. Se recomienda actualizar `RECOVERY_STATUS.md` con el resultado, aunque no se realizó en esta fase porque la instrucción era modificarlo únicamente si contenía información objetivamente incorrecta.