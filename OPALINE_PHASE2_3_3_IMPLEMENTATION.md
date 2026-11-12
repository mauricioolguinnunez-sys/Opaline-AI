# OPALINE_PHASE2_3_3_IMPLEMENTATION.md

## FASE 2.3.3 — Implementación de creación y consulta de órdenes PayPal en Sandbox

Fecha: 2026-11-08  
Modo: implementación verificada, sin commits, push ni despliegues.

### 1. Resumen

Se implementó el primer flujo funcional de órdenes PayPal en Sandbox.

El backend puede crear una orden real en PayPal Sandbox, persistirla en MySQL, devolver la URL de aprobación y consultar su estado. La consulta sincroniza el estado observado del proveedor con la persistencia local sin capturar el pago ni activar suscripciones.

### 2. Archivos modificados

Backend Rust:
- `opaline_rust_backend/src/routes/payments.rs`
- `opaline_rust_backend/src/paypal.rs`

Tests:
- `opaline_rust_backend/tests/payment_orders_integration.rs`

Documentación:
- `OPALINE_PHASE2_3_3_IMPLEMENTATION.md`
- `RECOVERY_STATUS.md`

### 3. Endpoints implementados

- `POST /api/v1/payments/paypal/create`
- `GET /api/v1/payments/paypal/orders/:order_id`
- `GET /api/v1/payments/paypal/verify/:order_id` (alias de consulta)

### 4. Flujo de creación

1. Autenticar al usuario.
2. Validar el plan y su versión activa.
3. Obtener importe y moneda desde el backend.
4. Reutilizar una orden existente si la clave de idempotencia coincide.
5. Crear un registro local pendiente.
6. Llamar a PayPal con `PayPal-Request-Id`.
7. Persistir el `provider_order_id`.
8. Devolver la URL de aprobación real de PayPal.

### 5. Flujo de consulta

1. Autenticar al usuario.
2. Verificar propiedad de la orden.
3. Consultar PayPal.
4. Validar correspondencia de importe y moneda.
5. Actualizar el estado local únicamente con transiciones permitidas.
6. Devolver estado local y estado del proveedor.

`APPROVED` se mapea a `approved`, no a `captured` ni a pago liquidado.

### 6. Idempotencia

- Clave por usuario y operación.
- Reintentos no crean órdenes duplicadas.
- Reutilización incompatible devuelve `409 Conflict`.
- Se utiliza `PayPal-Request-Id` para idempotencia del proveedor.

### 7. Pruebas ejecutadas

- `cargo check`: correcto.
- `cargo test --lib`: 24 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test payment_orders_integration -- --ignored`: 1 prueba correcta.
- `git diff --check`: correcto.

La prueba utiliza un servidor HTTP local que simula PayPal. No se realizaron cobros reales.

### 8. Fuera de alcance

- Captura de pagos.
- Activación de suscripciones.
- Webhooks.
- Reembolsos.
- PayPal Live.

### 9. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 10. Estado final del repositorio

- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales previos se conservaron.

FASE 2.3.3 está implementada y verificada. No se inició FASE 2.3.4. Se espera autorización expresa.