# OPALINE_PHASE2_2_0_IMPLEMENTATION.md

## FASE 2.2.0 — Implementación del motor de cuotas, reservas de consumo y metering

Fecha: 2026-11-08  
Modo: implementación verificada, sin commits, push ni despliegues.

### 1. Resumen

Se implementó el motor de cuotas y medición de consumo de Opaline AI.

Las solicitudes de chat ahora requieren una reserva válida antes de ejecutar inferencia. Los límites proceden de la versión del plan contratada. Las reservas son atómicas en MySQL y las solicitudes son idempotentes. El consumo real se registra con distinción entre medición del proveedor y estimación local.

No se implementaron pagos reales, PayPal, webhooks, reembolsos, renovación automática ni monitoreo operativo.

### 2. Archivos modificados

Backend Rust:

- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`
- `opaline_rust_backend/src/lib.rs`
- `opaline_rust_backend/src/quota.rs`
- `opaline_rust_backend/src/routes/chat.rs`
- `opaline_rust_backend/src/routes/mod.rs`
- `opaline_rust_backend/src/routes/usage.rs`

Tests:

- `opaline_rust_backend/tests/quota_integration.rs`

Documentación:

- `OPALINE_PHASE2_2_0_IMPLEMENTATION.md`
- `RECOVERY_STATUS.md`

### 3. Migraciones creadas

Tablas nuevas:

- `usage_periods`
- `usage_reservations`
- `usage_records`

Columnas nuevas en `plan_versions`:

- `requests_per_period`
- `input_tokens_per_period`
- `output_tokens_per_period`
- `total_tokens_per_period`

`requests_per_period` se inicializa con `messages_per_month` cuando no existe.

### 4. Arquitectura del motor de cuotas

Flujo implementado:

1. Autenticar al usuario.
2. Resolver la suscripción vigente.
3. Obtener la versión del plan contratada.
4. Obtener los límites.
5. Validar idempotencia.
6. Crear una reserva transaccional.
7. Ejecutar la inferencia.
8. Obtener el consumo real del proveedor.
9. Registrar el consumo.
10. Confirmar la reserva y actualizar contadores.
11. Devolver el resultado.

Si no existe cuota disponible, la inferencia no se ejecuta.

### 5. Reservas y concurrencia

La reserva utiliza:

- Transacción SQL.
- `SELECT ... FOR UPDATE` sobre el periodo.
- Actualización condicional de contadores.
- Restricción única por `(user_id, idempotency_key)`.
- Restricción única por `(user_id, request_id)`.

La garantía de no superar cuotas procede de MySQL, no de memoria local.

### 6. Idempotencia

Clave de idempotencia:

- Alcance por usuario.
- Reutilización con los mismos parámetros devuelve el resultado original.
- Reutilización con parámetros diferentes devuelve `409 Conflict`.
- Dos solicitudes concurrentes con la misma clave no duplican consumo.

### 7. Medición de tokens

Fuentes soportadas:

- Tokens reportados por LM Studio cuando están disponibles.
- Estimación local cuando el proveedor no reporta métricas.

Campos registrados:

- `token_source`: `provider`, `local`, `estimated` o `unavailable`.
- `measurement_status`: `measured`, `estimated`, `unavailable` o `pending`.

No se presentan estimaciones como mediciones exactas.

### 8. Tratamiento de errores

- Error antes de inferencia: reserva no creada o liberada.
- Error durante inferencia: reserva liberada.
- Timeout: reserva liberada.
- Respuesta vacía: reserva liberada.
- Error al persistir consumo: reserva liberada y respuesta de error.
- Reintento de confirmación: no incrementa contadores dos veces.

### 9. Recuperación de reservas

Se implementó `reap_expired_reservations`, que libera reservas expiradas y las marca como recuperadas.

No se activa automáticamente un worker en producción sin configuración explícita.

### 10. Endpoints implementados

- `GET /api/v1/usage`
- `GET /api/v1/usage/history`
- `GET /api/v1/usage/summary`

Todos requieren Bearer token y aíslan los datos por usuario.

### 11. Compatibilidad PHP y Flutter

No se modificaron las interfaces completas. Los contratos existentes se mantuvieron. El motor de cuotas se integra en Rust y las aplicaciones pueden consumir los nuevos endpoints de forma incremental.

### 12. Pruebas ejecutadas

Comandos:

```bash
cargo check
cargo test --lib
cargo test
cargo test -- --ignored
cargo test --test quota_integration -- --ignored
git diff --check
```

Resultados:

- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto, 0 pruebas normales de integración.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `git diff --check`: correcto.

Pruebas de cuotas:

- usuario sin suscripción;
- reserva, liberación y consulta;
- idempotencia después de confirmación;
- concurrencia respetando límite.

### 13. Riesgos pendientes

- PayPal real no está integrado.
- No hay renovación automática mediante proveedor.
- No hay verificación real de correo.
- No hay monitoreo operativo.
- LM Studio puede no reportar tokens de forma fiable.

### 14. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 15. Funcionalidades fuera de alcance

- PayPal real.
- Webhooks de pago.
- Reembolsos.
- Renovación automática mediante proveedor.
- Facturación real.
- Cobros por tokens.
- Pasarelas de pago.
- Microservicios.
- Redis.
- Kafka.
- Nuevo AI Gateway.
- Sistema completo de monitoreo.
- Rediseño visual completo.
- Nuevos proveedores de inferencia.

### 16. Recomendación para la siguiente fase

No iniciar FASE 2.3 hasta tener autorización expresa. Antes de integrar pagos reales, se recomienda validar el motor de cuotas en un entorno controlado con usuarios reales.