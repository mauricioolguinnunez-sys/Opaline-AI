# OPALINE_PHASE2_1_0_IMPLEMENTATION.md

## FASE 2.1.0 — Implementación del catálogo de planes y núcleo de suscripciones

Fecha: 2026-11-08  
Modo: implementación verificada, sin commits, push ni despliegues.

### 1. Resumen de implementación

Se implementó el catálogo de planes con versionado, el núcleo de suscripciones, la trazabilidad de eventos, la restricción MySQL de una suscripción vigente por usuario y los endpoints de consulta, historial, cancelación y restauración de renovación.

Las migraciones son aditivas. No se eliminaron tablas ni columnas existentes. No se implementaron cuotas, metering, pagos reales ni renovación automática.

### 2. Archivos modificados

Backend Rust:

- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/models.rs`
- `opaline_rust_backend/src/routes/mod.rs`
- `opaline_rust_backend/src/routes/plans.rs`
- `opaline_rust_backend/src/routes/admin.rs`
- `opaline_rust_backend/src/routes/wall.rs`

Tests:

- `opaline_rust_backend/tests/plans_integration.rs`

Documentación:

- `OPALINE_PHASE2_1_0_IMPLEMENTATION.md`
- `RECOVERY_STATUS.md`

### 3. Migraciones creadas

Tablas nuevas:

- `plan_versions`
- `subscription_events`

Columnas nuevas en `subscriptions`:

- `plan_version_id`
- `current_period_start`
- `current_period_end`
- `cancel_at_period_end`
- `terminated_at`
- `activation_source`
- `auto_renew`
- `is_current`

Índices y restricciones:

- `uq_plan_version (plan_id, version)`
- `uq_subscriptions_user_current (user_id, is_current)`
- índices de soporte para eventos y consultas.

La restricción `uq_subscriptions_user_current` utiliza `NULL` para suscripciones históricas y `1` para la suscripción vigente. MySQL permite múltiples valores `NULL`, por lo que no se requiere un índice parcial.

### 4. Cambios en el modelo de datos

`plan_versions` conserva precio, moneda, límites, características, fechas de vigencia y estado. Las suscripciones conservan `plan_version_id`, por lo que una modificación posterior del plan no altera las condiciones contratadas.

`subscription_events` registra transiciones de estado, actor, motivo e identificador de correlación.

`is_current` protege la unicidad de la suscripción vigente. Las validaciones de aplicación se ejecutan dentro de transacciones con bloqueo de filas.

### 5. Endpoints implementados

Públicos:

- `GET /api/v1/planes`
- `GET /api/v1/planes/:id/versions`

Autenticados:

- `GET /api/v1/suscripcion`
- `GET /api/v1/suscripcion/:id`
- `GET /api/v1/suscripcion/history`
- `POST /api/v1/suscripcion/cancel`
- `POST /api/v1/suscripcion/cancelar-renovacion`
- `POST /api/v1/suscripcion/restore`
- `POST /api/v1/suscripcion/restaurar`

Administrativos:

- `POST /api/v1/admin/planes/:id/versions`

Compatibilidad conservada:

- `POST /api/v1/suscripcion/cancelar-renovacion`
- `POST /api/v1/suscripcion/restaurar`

### 6. Reglas de negocio

- Toda transición de estado se valida antes de ejecutarse.
- La cancelación de renovación marca `cancel_at_period_end = 1` y `auto_renew = 0`, pero conserva el acceso hasta `end_date`.
- La restauración solo es válida para suscripciones canceladas y no vencidas.
- Una suscripción vencida se marca como `expired` de forma segura al consultarse.
- Los eventos de suscripción registran estado anterior, estado nuevo, actor y motivo.
- La activación de una nueva suscripción marca las suscripciones vigentes anteriores como no vigentes.
- PHP y Flutter no contienen lógica comercial nueva; consumen los contratos de Rust.

### 7. Máquina de estados

Estados soportados:

- `pending`
- `active`
- `grace`
- `cancelled`
- `past_due`
- `suspended`
- `expired`

Transiciones implementadas:

- `active` o `grace` a `cancelled`: cancelación de renovación.
- `cancelled` a `active`: restauración de renovación.
- `active`, `grace`, `cancelled` o `past_due` a `expired`: vencimiento del periodo.

No se permite activar una suscripción vencida por una restauración local.

### 8. Restricción de suscripción vigente única

La unicidad se protege con:

- índice único `(user_id, is_current)` en MySQL;
- transacciones al activar suscripciones;
- actualización de `is_current` dentro de la misma transacción.

Las pruebas de concurrencia verificaron que solo una suscripción queda marcada como vigente.

### 9. Pruebas ejecutadas

Comandos:

```bash
cargo check
cargo test -- --ignored
php -l frontend/*.php
flutter analyze
git diff --check
```

Resultados:

- `cargo check`: correcto.
- `cargo test -- --ignored`: correcto, incluyendo 7 pruebas nuevas de planes y suscripciones.
- `php -l`: correcto.
- `flutter analyze`: 27 avisos informativos, sin errores.
- `git diff --check`: correcto.

Pruebas nuevas:

- listado de planes;
- suscripción inexistente;
- cancelación y restauración;
- vencimiento;
- concurrencia de activación;
- aislamiento por propietario;
- historial de suscripciones.

### 10. Problemas detectados

No se detectaron bloqueos funcionales. Las advertencias de Rust son preexistentes o informativas y no impiden la compilación ni las pruebas.

### 11. Riesgos pendientes

- PayPal real no está integrado.
- Las cuotas no se aplican.
- El consumo de tokens no se registra.
- No hay renovación automática mediante proveedor.
- No hay verificación de correo.
- No hay monitoreo operativo.

### 12. Compatibilidad PHP y Flutter

No se modificaron interfaces completas. Los contratos existentes se mantuvieron y se añadieron campos adicionales en la respuesta de suscripción. PHP y Flutter pueden consumir los nuevos campos de forma incremental.

### 13. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 14. Funcionalidades deliberadamente fuera de alcance

- Motor de cuotas.
- Reservas de consumo.
- Registro de tokens.
- Metering.
- Costos de inferencia.
- AI Gateway.
- Pagos reales.
- Integración PayPal.
- Webhooks de pago.
- Reembolsos.
- Renovación automática mediante proveedor.
- Sistema de correo.
- Verificación de correo.
- Nuevos sistemas de monitoreo.
- Rediseño visual completo.

### 15. Recomendación para la siguiente fase

No iniciar FASE 2.2 hasta tener autorización expresa. Antes de implementar cuotas, se recomienda validar el catálogo y el núcleo de suscripciones en un entorno controlado.


## Corrección posterior: FASE 2.1.1

La restricción inicial `UNIQUE (user_id, is_current)` se corrigió mediante una columna generada nullable `current_user_id` y la restricción `UNIQUE (current_user_id)`.

Documentación detallada: `OPALINE_PHASE2_1_1_CORRECTION.md`.

La corrección permite múltiples suscripciones históricas por usuario y mantiene la garantía de una única suscripción vigente a nivel de MySQL.
