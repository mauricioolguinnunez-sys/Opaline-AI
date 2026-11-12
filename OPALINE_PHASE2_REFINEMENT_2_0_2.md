# OPALINE_PHASE2_REFINEMENT_2_0_2.md

## Refinamiento técnico y cierre de especificaciones comerciales de Opaline AI

Fecha: 2026-11-08  
Modo: análisis y documentación únicamente. No se modificó código, migraciones, bases de datos, dependencias ni configuraciones.

---

## 1. Resumen ejecutivo

La FASE 2.0.1 definió el núcleo comercial, pero dejó ambigüedades en cuatro áreas críticas:

1. Restricciones de MySQL para una suscripción vigente única.
2. Relación entre versión de plan y suscripción.
3. Idempotencia y concurrencia de reservas de consumo.
4. Separación entre cancelación de renovación y pérdida de acceso.
5. Garantías transaccionales de pagos y Outbox.
6. Distinción entre medición real y estimación.
7. Proceso de renovación y expiración.
8. Contratos API completos.

Esta FASE 2.0.2 corrige y completa esas especificaciones para permitir una implementación controlada en FASE 2.1.

La arquitectura recomendada sigue siendo modular dentro del backend Rust actual. MySQL sigue siendo la fuente de verdad comercial. El Outbox sigue siendo el mecanismo de consistencia eventual con MongoDB.

---

## 2. Hallazgos contrastados con el repositorio

### 2.1 Estructuras actuales

| Estructura | Estado real |
|---|---|
| `plans` | Existe, sin versionado |
| `subscriptions` | Existe, sin restricción de una activa por usuario |
| `payments` | Existe, sin relación robusta con factura |
| `invoices` | Existe, texto plano en PHP |
| `usage_counters` | Existe, pero no se utiliza |
| `outbox_events` | Existe y se utiliza para auditoría del muro |
| `outbox_admin_actions` | Existe |
| `sessions` | Endurecida con hash y expiración |
| `ai_models` | Existe, pero el chat no valida modelos habilitados |

### 2.2 Limitaciones confirmadas

- No existe AI Gateway independiente.
- No existe integración real de pagos.
- No hay cuotas aplicadas.
- No se registran tokens.
- No hay renovación automática.
- No hay verificación de correo.
- No hay monitoreo operativo.

---

## 3. Restricciones MySQL y modelo de datos

### 3.1 Versionado de planes

Las suscripciones deben conservar las condiciones comerciales bajo las que fueron contratadas.

Se propone añadir `plan_version_id` a `subscriptions`, manteniendo `plan_id` por compatibilidad.

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS plan_versions (
    id VARCHAR(36) PRIMARY KEY,
    plan_id VARCHAR(36) NOT NULL,
    version INT NOT NULL,
    price DECIMAL(10,2) NOT NULL,
    currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
    messages_per_month INT NOT NULL,
    features JSON,
    effective_from DATETIME NOT NULL,
    effective_to DATETIME NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_plan_version (plan_id, version),
    FOREIGN KEY (plan_id) REFERENCES plans(id)
);
```

Reglas:

- Un plan nuevo crea una nueva versión.
- Las suscripciones existentes conservan su versión.
- El precio y los límites no se modifican retroactivamente.
- El catálogo puede mostrar la versión vigente.
- Las suscripciones activas continúan con su versión contratada.

### 3.2 Suscripción única por usuario

MySQL no admite índices únicos parciales de forma portable en esta etapa.

Se propone añadir a `subscriptions`:

```sql
is_current TINYINT NULL
```

Con índice único:

```sql
UNIQUE KEY uq_user_current_subscription (user_id, is_current)
```

Interpretación:

- `is_current = 1`: suscripción comercial vigente.
- `is_current = NULL`: suscripción histórica.

MySQL permite múltiples valores `NULL` en un índice único.

Activación de una nueva suscripción:

1. Iniciar transacción.
2. Bloquear las filas del usuario.
3. Establecer `is_current = NULL` en las suscripciones vigentes anteriores.
4. Insertar o activar la nueva suscripción con `is_current = 1`.
5. Confirmar.

Esto evita múltiples suscripciones vigentes sin depender de índices parciales.

### 3.3 Reservas y registros de consumo

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS usage_reservations (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    subscription_id VARCHAR(36) NOT NULL,
    usage_period_id VARCHAR(36) NOT NULL,
    request_id VARCHAR(120) NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    model VARCHAR(120) NOT NULL,
    status ENUM('reserved','confirmed','released','expired') NOT NULL DEFAULT 'reserved',
    reserved_requests INT NOT NULL DEFAULT 1,
    reserved_input_tokens INT NOT NULL DEFAULT 0,
    reserved_output_tokens INT NOT NULL DEFAULT 0,
    actual_input_tokens INT NULL,
    actual_output_tokens INT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    confirmed_at DATETIME NULL,
    released_at DATETIME NULL,
    UNIQUE KEY uq_user_idempotency (user_id, idempotency_key),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
    FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id)
);
```

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS usage_periods (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    subscription_id VARCHAR(36) NOT NULL,
    plan_version_id VARCHAR(36) NOT NULL,
    period_start DATETIME NOT NULL,
    period_end DATETIME NOT NULL,
    request_limit INT NOT NULL,
    input_token_limit INT NOT NULL,
    output_token_limit INT NOT NULL,
    requests_used INT NOT NULL DEFAULT 0,
    input_tokens_used INT NOT NULL DEFAULT 0,
    output_tokens_used INT NOT NULL DEFAULT 0,
    reserved_requests INT NOT NULL DEFAULT 0,
    reserved_input_tokens INT NOT NULL DEFAULT 0,
    reserved_output_tokens INT NOT NULL DEFAULT 0,
    status ENUM('open','closed') NOT NULL DEFAULT 'open',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_user_period_start (user_id, period_start),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
    FOREIGN KEY (plan_version_id) REFERENCES plan_versions(id)
);
```

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS usage_records (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) NOT NULL,
    subscription_id VARCHAR(36) NOT NULL,
    usage_period_id VARCHAR(36) NOT NULL,
    request_id VARCHAR(120) NOT NULL,
    model VARCHAR(120) NOT NULL,
    provider VARCHAR(64) NOT NULL,
    input_tokens INT NOT NULL,
    output_tokens INT NOT NULL,
    total_tokens INT NOT NULL,
    token_source ENUM('provider','local','estimated','unavailable') NOT NULL,
    measurement_status ENUM('measured','estimated','unavailable') NOT NULL,
    inference_duration_ms INT NOT NULL,
    cost_estimate DECIMAL(12,6) NULL,
    cost_actual DECIMAL(12,6) NULL,
    currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
    status ENUM('confirmed','failed') NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_user_request (user_id, request_id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id),
    FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id)
);
```

### 3.4 Eventos de pago

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS payment_events (
    id VARCHAR(36) PRIMARY KEY,
    payment_id VARCHAR(36) NULL,
    provider VARCHAR(32) NOT NULL,
    provider_event_id VARCHAR(120) NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    status ENUM('received','processed','ignored','failed') NOT NULL DEFAULT 'received',
    payload JSON NOT NULL,
    received_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at DATETIME NULL,
    last_error TEXT NULL,
    UNIQUE KEY uq_provider_event (provider, provider_event_id),
    FOREIGN KEY (payment_id) REFERENCES payments(id)
);
```

`payment_id` puede ser `NULL` cuando el evento externo llega antes de existir la orden interna.

### 3.5 Reembolsos

Tabla propuesta:

```sql
CREATE TABLE IF NOT EXISTS refunds (
    id VARCHAR(36) PRIMARY KEY,
    payment_id VARCHAR(36) NOT NULL,
    amount DECIMAL(10,2) NOT NULL,
    currency VARCHAR(8) NOT NULL DEFAULT 'MXN',
    reason VARCHAR(240) NOT NULL,
    status ENUM('pending','completed','failed') NOT NULL DEFAULT 'pending',
    provider_refund_id VARCHAR(120) NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME NULL,
    FOREIGN KEY (payment_id) REFERENCES payments(id)
);
```

Regla de negocio:

- La suma de reembolsos `completed` no puede superar el importe del pago.
- La validación debe ejecutarse en Rust dentro de una transacción.
- MySQL no necesita un `CHECK` complejo si la validación transaccional es estricta.

### 3.6 Índices adicionales

```sql
CREATE UNIQUE INDEX uq_payments_paypal_order_id ON payments(paypal_order_id);
CREATE INDEX idx_subscriptions_user_current ON subscriptions(user_id, is_current);
CREATE INDEX idx_usage_periods_user_period ON usage_periods(user_id, period_start);
CREATE INDEX idx_usage_records_user_created ON usage_records(user_id, created_at);
```

---

## 4. Máquina de estados de suscripciones

### 4.1 Estados

- `pending`: orden creada, pago no confirmado.
- `trialing`: prueba temporal.
- `active`: suscripción vigente.
- `past_due`: pago vencido, acceso temporal.
- `suspended`: acceso suspendido.
- `cancelled`: cancelada, acceso hasta fin de periodo.
- `expired`: periodo finalizado.

### 4.2 Separación de conceptos

Se separan:

- `status`: estado comercial actual.
- `auto_renew`: si la suscripción intentará renovarse.
- `current_period_start`.
- `current_period_end`.
- `cancel_at_period_end`.

La cancelación de renovación no retira inmediatamente el acceso si el periodo ya fue pagado.

### 4.3 Transiciones

| Origen | Destino | Evento | Actor | Acceso |
|---|---|---|---|---|
| `pending` | `active` | Pago confirmado | Sistema | Se habilita |
| `pending` | `expired` | Orden abandonada | Sistema | No se habilita |
| `trialing` | `active` | Conversión a pago | Sistema | Se mantiene |
| `trialing` | `expired` | Fin de prueba | Sistema | Se retira |
| `active` | `cancelled` | Cancelación de renovación | Usuario/Admin | Se mantiene hasta fin de periodo |
| `active` | `past_due` | Fallo de pago | Sistema | Temporal |
| `past_due` | `active` | Pago recuperado | Sistema | Se mantiene |
| `past_due` | `suspended` | Vencimiento de gracia | Sistema | Se retira |
| `cancelled` | `expired` | Fin de periodo | Sistema | Se retira |
| `suspended` | `active` | Reactivación válida | Admin/Sistema | Se habilita |
| `expired` | `active` | Nueva contratación válida | Sistema | Se habilita |

No se permite activar una suscripción por solicitud del cliente ni por pago simulado.

---

## 5. Motor transaccional de cuotas

### 5.1 Flujo obligatorio

1. Autenticar al usuario.
2. Resolver su suscripción vigente.
3. Resolver la versión del plan contratado.
4. Obtener los límites aplicables.
5. Validar que la solicitud sea idempotente.
6. Crear una reserva de consumo.
7. Validar y reservar las dimensiones de cuota.
8. Confirmar la transacción.
9. Ejecutar inferencia.
10. Obtener las métricas disponibles.
11. Registrar el consumo final.
12. Ajustar la reserva.
13. Liberar cualquier excedente reservado.
14. Registrar fallos y permitir recuperación.

### 5.2 Protección frente a concurrencia

Estrategia recomendada:

```sql
START TRANSACTION;

SELECT id, requests_used, reserved_requests, request_limit
FROM usage_periods
WHERE user_id = ? AND period_start = ?
FOR UPDATE;

-- Validar en aplicación que:
-- used + reserved + requested <= limit

UPDATE usage_periods
SET reserved_requests = reserved_requests + ?,
    reserved_input_tokens = reserved_input_tokens + ?,
    reserved_output_tokens = reserved_output_tokens + ?
WHERE id = ?;

INSERT INTO usage_reservations (...);

COMMIT;
```

Si la validación falla, se hace `ROLLBACK` y se devuelve `429`.

### 5.3 Reservas expiradas

Una tarea periódica debe liberar reservas expiradas.

Reglas:

- No liberar una reserva si la inferencia pudo haberse ejecutado.
- Marcar la reserva como `expired`.
- Ajustar los contadores.
- Registrar el evento para conciliación.

### 5.4 Tokens no disponibles

LM Studio puede no devolver métricas fiables.

Estrategia:

- Si el proveedor informa tokens, usarlos.
- Si no, calcular localmente.
- Si no es posible, registrar como estimación.
- No presentar estimaciones como mediciones exactas.
- Registrar el método de medición.

---

## 6. Idempotencia de consumo

- `request_id` identifica la solicitud lógica.
- `idempotency_key` identifica la operación del cliente.
- `usage_reservations` tiene unicidad por `(user_id, idempotency_key)`.
- `usage_records` tiene unicidad por `(user_id, request_id)`.
- Un reintento con la misma clave devuelve el resultado original o el estado final.
- Una clave reutilizada con parámetros distintos devuelve conflicto.

---

## 7. Medición y costos

### 7.1 Fuentes de medición

| Fuente | Confianza | Uso |
|---|---|---|
| Tokens informados por proveedor | Alta | Preferida |
| Tokens calculados localmente | Media | Respaldo |
| Estimación | Baja | Último recurso |
| No disponible | Nula | Registrar como no disponible |

### 7.2 Costos

Variables configurables:

- `cost_per_input_token`
- `cost_per_output_token`
- `infrastructure_cost_per_request`
- `provider_fee`

Costos variables externos:

- Inferencia remota.
- Servicios externos.
- Comisiones.

Costos internos:

- Electricidad.
- Infraestructura.
- Conectividad.
- Depreciación.
- Mantenimiento.

No se establecen precios definitivos. Se requiere medición real.

---

## 8. Integración futura de pagos

### 8.1 Arquitectura desacoplada

La lógica comercial no debe depender directamente de PayPal.

Componentes:

- `PaymentProvider` trait.
- `PayPalProvider`.
- `PaymentService`.
- `SubscriptionService`.

### 8.2 Flujo de pago

1. Crear orden.
2. Usuario aprueba en proveedor.
3. Rust recibe confirmación o webhook.
4. Rust valida identidad, importe, moneda y destinatario.
5. Rust crea pago y factura.
6. Rust activa suscripción.
7. Evento Outbox registra la operación.

### 8.3 Validaciones

- Importe exacto.
- Moneda esperada.
- Identidad de la orden.
- Usuario propietario.
- Producto o plan contratado.
- Estado del pago en proveedor.
- Reutilización de órdenes.
- Pagos duplicados.
- Eventos fuera de orden.

### 8.4 Webhooks

Define:

- Recepción.
- Autenticación.
- Persistencia.
- Duplicados.
- Reintentos.
- Eventos fuera de orden.
- Errores temporales.
- Errores permanentes.
- Conciliación.

Si un webhook llega antes de que exista una asociación interna completa, se almacena en `payment_events` con `payment_id = NULL`.

### 8.5 Reembolsos

- Reembolsos completos.
- Reembolsos parciales.
- Varios reembolsos por pago.
- Límites acumulados.
- Idempotencia.
- Confirmación del proveedor.
- Relación con factura y suscripción.

---

## 9. Contratos de API

### 9.1 Planes

`GET /api/v1/planes`

Response:

```json
[
  {
    "id": "plan-starter",
    "code": "starter",
    "name": "Opaline Starter",
    "price": 49.00,
    "currency": "MXN",
    "period": "mensual",
    "messages_per_month": 100,
    "features": ["100 mensajes/mes"],
    "active": true
  }
]
```

### 9.2 Suscripción actual

`GET /api/v1/suscripcion`

Response:

```json
{
  "id": "uuid",
  "status": "active",
  "plan_name": "Opaline Starter",
  "plan_code": "starter",
  "price": 49.00,
  "start_date": "2026-11-08 00:00:00",
  "end_date": "2026-12-08 00:00:00"
}
```

### 9.3 Consumo

`GET /api/v1/usage/current`

Response:

```json
{
  "period": "2026-11",
  "requests_used": 12,
  "requests_limit": 100,
  "input_tokens_used": 4500,
  "output_tokens_used": 2100,
  "reservation_status": "none"
}
```

### 9.4 Crear orden

`POST /api/v1/payments/paypal/create`

Request:

```json
{
  "plan_id": "plan-starter"
}
```

Response:

```json
{
  "order_id": "uuid",
  "approval_url": "https://..."
}
```

### 9.5 Verificar orden

`POST /api/v1/payments/paypal/verify/:order_id`

Response:

```json
{
  "status": "completed",
  "invoice_id": "uuid"
}
```

### 9.6 Historial de pagos

`GET /api/v1/payments/history`

Response:

```json
[
  {
    "id": "uuid",
    "amount": 49.00,
    "currency": "MXN",
    "status": "completed",
    "created_at": "2026-11-08 00:00:00",
    "plan_name": "Opaline Starter",
    "invoice_id": "uuid",
    "folio": "OP-12345678"
  }
]
```

### 9.7 Factura

`GET /api/v1/facturas/:id/pdf`

Response:

- `200`: documento.
- `401`: no autenticado.
- `404`: no encontrada o no pertenece al usuario.

### 9.8 Administración

`GET /api/v1/admin/metrics`

Response:

```json
{
  "users": 10,
  "active_subscriptions": 3,
  "revenue": "147.00"
}
```

### 9.9 Formato de errores

```json
{
  "error": {
    "code": "quota_exceeded",
    "message": "Has agotado tu cuota mensual."
  }
}
```

Códigos:

- `401`: no autenticado.
- `403`: sin autorización.
- `404`: no encontrado.
- `409`: conflicto.
- `429`: cuota agotada o rate limit.
- `501`: no implementado.
- `503`: proveedor no disponible.

---

## 10. Integración PHP y Flutter

### PHP

- Sesión autenticada.
- Protección CSRF.
- Envío seguro del Bearer.
- Manejo de errores.
- Persistencia de claves de idempotencia.
- No confiar en límites del cliente.

### Flutter

- Gestión de sesión.
- Consulta de plan y consumo.
- Estados de carga y error.
- Manejo de expiración.
- Sincronización de información.
- No confiar en límites calculados exclusivamente en el cliente.

### Contrato único

Rust mantiene la autoridad comercial. PHP y Flutter consumen los mismos endpoints y no duplican reglas de negocio.

---

## 11. Seguridad y consistencia

Controles:

- Auth fail-closed.
- Tokens hasheados.
- Rate limiting.
- CORS allowlist.
- CSRF en PHP.
- Autorización en Rust.
- Aislamiento por usuario.
- Outbox para auditoría.

Amenazas:

- Manipulación de planes.
- Alteración de cuotas.
- Doble consumo.
- Doble pago.
- Activación sin pago confirmado.
- Repetición de webhooks.
- Reintentos.
- Escalamiento de privilegios.
- Acceso a registros ajenos.
- Manipulación de métricas.
- Eventos Outbox duplicados.
- Fallos parciales entre MySQL y MongoDB.

El Outbox garantiza consistencia eventual, no atomicidad distribuida.

---

## 12. Observabilidad y operación

Requisitos mínimos:

- Logs estructurados.
- Correlation IDs.
- Métricas de consumo.
- Errores de inferencia.
- Fallos de pagos.
- Retrasos del Outbox.
- Alertas.
- Salud de proveedores.
- Monitoreo de cuotas.
- Auditoría administrativa.

Distingue lo imprescindible para un piloto comercial de las mejoras posteriores.

No se incorpora infraestructura innecesaria en esta etapa.

---

## 13. Estrategia de pruebas

### Integridad comercial
- Activación sin pago confirmado.
- Pago duplicado.
- Orden reutilizada.
- Cambio de plan.
- Suscripción vencida.
- Cancelación al final del periodo.

### Cuotas
- Límite exacto.
- Una unidad por encima del límite.
- Solicitudes simultáneas.
- Reserva duplicada.
- Reserva expirada.
- Inferencia fallida.
- Reinicio del backend.

### Pagos
- Webhook válido.
- Webhook inválido.
- Webhook duplicado.
- Webhook fuera de orden.
- Respuesta del proveedor perdida.
- Reintento.
- Reembolso parcial.
- Reembolso completo.

### Consistencia
- Fallo al crear factura.
- Fallo al registrar Outbox.
- Fallo después de confirmar la transacción.
- Recuperación del worker.
- Evento procesado más de una vez.

### Seguridad
- Acceso a recursos ajenos.
- Manipulación de importe.
- Manipulación de plan.
- Manipulación de cuota.
- Reutilización de identificadores.

Define resultados esperados y criterios de aprobación.

---

## 14. Roadmap

### FASE 2.0 — Preparación y diseño
Objetivo: consolidar contratos y modelo de datos.

Archivos o módulos:
- `migrations.sql`
- `src/routes/plans.rs`
- `src/routes/auth.rs`
- `src/state.rs`

Dependencias: ninguna.

Riesgos: bajo.

Pruebas: auditoría de contratos.

Criterio: documentación y contratos aprobados.

### FASE 2.1 — Catálogo de planes y modelo de suscripciones
Objetivo: versionar planes y definir una suscripción activa única por usuario.

Archivos o módulos:
- `src/db.rs`
- `src/routes/plans.rs`
- `frontend/planes.php`
- `lib/providers/plan_provider.dart`

Dependencias: FASE 2.0.

Riesgos: medio.

Pruebas: creación, activación, vencimiento y cancelación.

Criterio: suscripción única y vigencia correcta.

### FASE 2.2 — Medición y cuotas
Objetivo: registrar consumo y aplicar límites.

Archivos o módulos:
- `src/routes/chat.rs`
- `src/db.rs`
- `src/state.rs`
- `lib/providers/chat_provider.dart`

Dependencias: FASE 2.1.

Riesgos: alto.

Pruebas: cuota exacta, concurrencia, liberación.

Criterio: consumo registrado y límites aplicados.

### FASE 2.3 — Integración de pagos reales
Objetivo: integrar proveedor real con idempotencia y webhooks.

Archivos o módulos:
- `src/routes/payments.rs`
- `src/db.rs`
- `frontend/checkout.php`
- `lib/features/plans/screens/checkout_screen.dart`

Dependencias: FASE 2.1.

Riesgos: alto.

Pruebas: pago aprobado, rechazado, duplicado, webhook inválido.

Criterio: suscripción activada solo por confirmación real.

### FASE 2.4 — Paneles PHP y Flutter
Objetivo: consolidar experiencia comercial.

Archivos o módulos:
- `frontend/cuenta.php`
- `frontend/planes.php`
- `lib/features/plans/screens/*`
- `lib/features/chat/screens/chat_screen.dart`

Dependencias: FASE 2.1, 2.2, 2.3.

Riesgos: medio.

Pruebas: UI, estados, errores.

Criterio: experiencia coherente.

### FASE 2.5 — Pruebas integrales y piloto comercial
Objetivo: validar flujo completo con usuarios reales.

Archivos o módulos:
- Tests Rust.
- Tests PHP.
- Tests Flutter.
- Documentación.

Dependencias: FASE 2.1 a 2.4.

Riesgos: alto.

Pruebas: flujo completo end-to-end.

Criterio: piloto controlado.

---

## 15. Riesgos y decisiones pendientes

1. PayPal real no está integrado.
2. Las cuotas no se aplican.
3. El consumo de tokens no se registra.
4. No hay renovación ni vencimiento automático.
5. No hay verificación de correo.
6. No hay monitoreo operativo.
7. La experiencia comercial está duplicada.
8. Los costos de infraestructura no están medidos.

Decisiones pendientes:

- Proveedor de pagos final.
- Estrategia de precios.
- Límites por plan.
- Política de reembolsos.
- Política de suspensión por abuso.

---

## 16. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia concreta |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección de skills y prioridades |
| `impeccable` | Sí | Sí | Auditoría de interfaces y diseño |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Identificación de riesgos |

---

## 17. Confirmación de restricciones

No se modificó código fuente.  
No se crearon ni ejecutaron migraciones.  
No se alteraron bases de datos.  
No se insertaron, modificaron ni eliminaron registros.  
No se conectó a PayPal real.  
No se ejecutaron pagos o reembolsos.  
No se cambiaron configuraciones de producción.  
No se modificaron dependencias.  
No se eliminaron archivos.  
No se sobrescribió documentación sin preservar su contenido.  
No se realizaron commits.  
No se hizo push.  
No se desplegó.

---

## 18. Recomendación de la siguiente fase

Iniciar FASE 2.0 con la documentación y contratos aprobados. No implementar FASE 2.1 hasta tener aprobación explícita. No iniciar FASE 2.2 ni posteriores hasta que FASE 2.1 esté verificada. No iniciar FASE 2.4 ni FASE 2.5 hasta que FASE 2.2 y FASE 2.3 estén verificadas. No declarar el producto listo para producción hasta que se cumplan los criterios de aceptación de seguridad, pagos, cuotas y monitoreo.