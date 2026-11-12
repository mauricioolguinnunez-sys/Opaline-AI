# OPALINE_PHASE2_ARCHITECTURE_2_0_1.md

## Diseño arquitectónico y especificación del núcleo comercial de Opaline AI

Fecha: 2026-11-08  
Modo: análisis y documentación únicamente. No se modificó código, bases de datos, migraciones, dependencias ni configuraciones.

---

## 1. Resumen ejecutivo

Opaline AI tiene una base backend sólida para iniciar FASE 2.0 y FASE 2.1, pero no está preparada para un servicio comercial real hasta implementar cuotas, medición de consumo, pagos reales y renovación automática de suscripciones.

La arquitectura recomendada preserva el stack actual:

- Flutter y PHP consumen Rust.
- Rust conserva la autoridad comercial.
- MySQL es la fuente de verdad de planes, suscripciones, pagos, facturas y cuotas.
- MongoDB conserva conversaciones y auditoría.
- El Outbox existente se utiliza para eventos de auditoría y consistencia eventual.
- LM Studio se mantiene como proveedor inicial de inferencia.

No se propone microservicios ni infraestructura distribuida adicional.

---

## 2. Estado actual verificado

### 2.1 Funcionalidades reales

| Área | Estado actual |
|---|---|
| Autenticación | Real: registro, login, refresh, logout, cambio de contraseña, sesiones |
| Autorización | Real por token y rol consultado en MySQL |
| Chat | Real contra LM Studio; sin cuotas ni streaming |
| Conversaciones | Real en MongoDB, aisladas por `user_id` |
| Muro | Real con validación, edición, eliminación lógica y reportes |
| Moderación | Real en backend; interfaz web básica |
| Planes | Real en MySQL |
| Suscripción actual | Real en MySQL |
| Pagos | Demo: orden local y verificación deshabilitada |
| Facturas | Texto plano, no PDF |
| Outbox | Real como infraestructura |
| Seguridad | Endurecida: CSRF, CORS, rate limiting, tokens hasheados |

### 2.2 Funcionalidades parciales o simuladas

| Área | Limitación |
|---|---|
| Suscripciones | No hay renovación, vencimiento ni expiración automática |
| Cancelación | Cambia estado local; no coordina con proveedor |
| Cuotas | `messages_per_month` existe, pero no se aplica |
| Consumo | No se registran tokens ni solicitudes |
| Modelos por plan | No se valida contra suscripción |
| Métricas | Best-effort y contaminadas por datos demo |
| Paginación | No uniforme entre Rust y PHP |
| Pagos | PayPal no integrado realmente |
| Facturas | No son PDF |

### 2.3 Estructuras existentes relevantes

MySQL:

- `users`
- `sessions`
- `plans`
- `subscriptions`
- `payments`
- `invoices`
- `posts`
- `reports`
- `expenses`
- `ai_models`
- `usage_counters`
- `outbox_events`
- `outbox_admin_actions`

MongoDB:

- `conversations`
- `ai_metrics`
- `auth_events`
- `payment_events`
- `wall_events`

---

## 3. Hallazgos y brechas

### 3.1 Brechas críticas para comercialización

1. No existe integración real con PayPal.
2. No existen cuotas ni límites aplicados.
3. No se registra consumo de tokens.
4. No existe renovación ni vencimiento automático de suscripciones.
5. No existe verificación real de correo.
6. No existe monitoreo operativo suficiente.

### 3.2 Brechas de consistencia

1. `usage_counters` existe pero no se utiliza.
2. `plans.messages_per_month` no se aplica.
3. No hay relación robusta entre orden, pago, factura y suscripción.
4. Cancelar cambia inmediatamente a `cancelled`.
5. Restaurar puede activar suscripciones vencidas.
6. No hay restricción de múltiples suscripciones activas.

### 3.3 Brechas de experiencia

1. PHP y Flutter modelan planes y suscripciones de forma distinta.
2. Cuotas se hardcodean en Flutter.
3. El estado de suscripción no se sincroniza correctamente tras login.
4. La experiencia comercial está dividida.

---

## 4. Arquitectura propuesta

### 4.1 Principios

- Rust conserva la autoridad comercial.
- MySQL es la fuente de verdad.
- MongoDB conserva auditoría y conversaciones.
- El Outbox garantiza consistencia eventual.
- No hay microservicios.
- No hay Redis, Kafka ni colas externas en esta etapa.

### 4.2 Dominios

1. Catálogo de planes.
2. Suscripciones.
3. Cuotas y consumo.
4. Pagos.
5. Medición.
6. Experiencia comercial.

---

## 5. Modelo de dominio y entidades

### 5.1 Tablas existentes que se conservan

| Tabla | Uso actual |
|---|---|
| `users` | Identidad y rol |
| `sessions` | Tokens de acceso y refresh |
| `plans` | Catálogo de planes |
| `subscriptions` | Estado de suscripción |
| `payments` | Pagos |
| `invoices` | Comprobantes |
| `usage_counters` | Contadores por periodo |
| `outbox_events` | Eventos de auditoría |
| `outbox_admin_actions` | Historial administrativo |

### 5.2 Entidades propuestas

No se crean en esta tarea. Se proponen para implementación futura.

#### `plan_versions`

Propósito: versionar planes y sus límites.

Campos:

- `id`
- `plan_id`
- `version`
- `price`
- `currency`
- `messages_per_month`
- `features`
- `effective_from`
- `effective_to`
- `created_at`

Clave primaria: `id`.  
Clave foránea: `plan_id → plans.id`.  
Índice único: `(plan_id, version)`.

#### `subscription_events`

Propósito: registrar cambios de estado de suscripciones.

Campos:

- `id`
- `subscription_id`
- `event_type`
- `previous_status`
- `new_status`
- `actor_type`
- `actor_id`
- `reason`
- `created_at`

Clave primaria: `id`.  
Clave foránea: `subscription_id → subscriptions.id`.

#### `usage_reservations`

Propósito: reservar capacidad antes de ejecutar inferencia.

Campos:

- `id`
- `user_id`
- `subscription_id`
- `period`
- `request_units`
- `input_tokens`
- `output_tokens`
- `status`
- `created_at`
- `expires_at`
- `confirmed_at`
- `released_at`

Clave primaria: `id`.  
Claves foráneas: `user_id → users.id`, `subscription_id → subscriptions.id`.  
Índice: `(user_id, period, status)`.

#### `usage_records`

Propósito: registrar consumo real.

Campos:

- `id`
- `user_id`
- `subscription_id`
- `request_id`
- `model`
- `provider`
- `input_tokens`
- `output_tokens`
- `total_tokens`
- `cost_estimate`
- `cost_actual`
- `status`
- `created_at`

Clave primaria: `id`.  
Claves foráneas: `user_id → users.id`, `subscription_id → subscriptions.id`.  
Índice único: `(user_id, request_id)`.

#### `payment_events`

Propósito: registrar eventos de pago y webhook.

Campos:

- `id`
- `payment_id`
- `event_type`
- `provider`
- `provider_event_id`
- `status`
- `payload`
- `created_at`

Clave primaria: `id`.  
Clave foránea: `payment_id → payments.id`.  
Índice único: `(provider, provider_event_id)`.

#### `refunds`

Propósito: registrar reembolsos.

Campos:

- `id`
- `payment_id`
- `amount`
- `currency`
- `reason`
- `status`
- `provider_refund_id`
- `created_at`

Clave primaria: `id`.  
Clave foránea: `payment_id → payments.id`.

### 5.3 Entidades que no se crean todavía

- `invoices` ya existe.
- `payments` ya existe.
- No se crea una entidad separada de créditos.
- No se crea una entidad de wallet.

---

## 6. Máquinas de estados

### 6.1 Suscripciones

Estados propuestos:

- `pending`: orden creada, pago no confirmado.
- `trialing`: prueba temporal, si se implementa.
- `active`: suscripción vigente.
- `past_due`: pago vencido, acceso temporal.
- `suspended`: acceso suspendido.
- `cancelled`: cancelada, acceso hasta fin de periodo.
- `expired`: periodo finalizado.

Transiciones:

| Origen | Destino | Evento |
|---|---|---|
| `pending` | `active` | Pago confirmado |
| `pending` | `expired` | Orden abandonada |
| `active` | `cancelled` | Cancelación de renovación |
| `active` | `past_due` | Fallo de pago |
| `past_due` | `active` | Pago recuperado |
| `past_due` | `suspended` | Vencimiento de gracia |
| `cancelled` | `expired` | Fin de periodo |
| `trialing` | `active` | Conversión a pago |
| `trialing` | `expired` | Fin de prueba |

No se permite activar una suscripción por solicitud del cliente ni por pago simulado.

### 6.2 Pagos

Estados:

- `pending`
- `completed`
- `failed`
- `refunded`
- `cancelled`

Transiciones:

| Origen | Destino | Evento |
|---|---|---|
| `pending` | `completed` | Confirmación del proveedor |
| `pending` | `failed` | Rechazo del proveedor |
| `pending` | `cancelled` | Cancelación de la orden |
| `completed` | `refunded` | Reembolso confirmado |
| `completed` | `cancelled` | Cancelación administrativa, si aplica |

### 6.3 Reservas de consumo

Estados:

- `reserved`
- `confirmed`
- `released`
- `expired`

Transiciones:

| Origen | Destino | Evento |
|---|---|---|
| `reserved` | `confirmed` | Inferencia exitosa |
| `reserved` | `released` | Fallo antes de inferencia |
| `reserved` | `expired` | Timeout de reserva |

---

## 7. Diseño de cuotas

### 7.1 Tipos de consumo

- Solicitudes.
- Tokens de entrada.
- Tokens de salida.
- Tokens totales.
- Concurrencia.
- Modelos permitidos.

### 7.2 Flujo de reserva

1. Validar sesión.
2. Resolver suscripción vigente.
3. Consultar límites del plan.
4. Reservar capacidad en MySQL.
5. Ejecutar inferencia.
6. Registrar consumo real.
7. Confirmar o liberar la reserva.

### 7.3 Protección frente a concurrencia

Usar transacción MySQL con bloqueo de fila o restricción atómica.

Ejemplo conceptual:

```sql
UPDATE usage_counters
SET count = count + 1
WHERE user_id = ? AND period = ? AND count < ?;
```

Si no se afecta ninguna fila, rechazar por cuota agotada.

### 7.4 Tratamiento de tokens no disponibles

LM Studio puede no devolver métricas fiables.

Estrategia:

- Si el proveedor informa tokens, usarlos.
- Si no, calcular localmente.
- Si no es posible, registrar como estimación.
- No presentar estimaciones como mediciones exactas.
- Registrar el método de medición.

---

## 8. Diseño de metering

### 8.1 Estructura propuesta

Tabla `usage_records`.

Campos:

- `id`
- `user_id`
- `subscription_id`
- `request_id`
- `model`
- `provider`
- `input_tokens`
- `output_tokens`
- `total_tokens`
- `token_source`
- `cost_estimate`
- `cost_actual`
- `status`
- `created_at`

### 8.2 Cálculos posteriores

- Consumo por usuario.
- Consumo por plan.
- Consumo por modelo.
- Consumo por periodo.
- Coste operativo.
- Margen estimado.

### 8.3 Costos

Variables:

- `cost_per_input_token`
- `cost_per_output_token`
- `infrastructure_cost_per_request`
- `provider_fee`

No se establecen precios definitivos.

---

## 9. Integración futura de pagos

### 9.1 Arquitectura desacoplada

La lógica comercial no debe depender directamente de PayPal.

Componentes:

- `PaymentProvider` trait.
- `PayPalProvider`.
- `PaymentService`.
- `SubscriptionService`.

### 9.2 Flujo de pago

1. Crear orden.
2. Usuario aprueba en proveedor.
3. Rust recibe confirmación o webhook.
4. Rust valida identidad, importe, moneda y receptor.
5. Rust crea pago y factura.
6. Rust activa suscripción.
7. Evento Outbox registra la operación.

### 9.3 Idempotencia

- `provider_order_id` debe ser único.
- Webhooks duplicados no deben duplicar pagos.
- Reintentos deben ser seguros.

### 9.4 Estados

- `pending`
- `completed`
- `failed`
- `refunded`
- `cancelled`

---

## 10. Contratos de API

### 10.1 Planes

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

### 10.2 Suscripción actual

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

### 10.3 Consumo

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

### 10.4 Crear orden

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

### 10.5 Verificar orden

`POST /api/v1/payments/paypal/verify/:order_id`

Response:

```json
{
  "status": "completed",
  "invoice_id": "uuid"
}
```

### 10.6 Historial de pagos

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

### 10.7 Factura

`GET /api/v1/facturas/:id/pdf`

Response:

- `200`: documento.
- `401`: no autenticado.
- `404`: no encontrada o no pertenece al usuario.

### 10.8 Administración

`GET /api/v1/admin/metrics`

Response:

```json
{
  "users": 10,
  "active_subscriptions": 3,
  "revenue": "147.00"
}
```

---

## 11. Integración PHP y Flutter

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
- No confiar en límites calculados en el cliente.

### Contrato único

Rust mantiene la autoridad comercial. PHP y Flutter consumen los mismos endpoints y no duplican reglas de negocio.

---

## 12. Seguridad y consistencia

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
- Eventos duplicados.
- Fallos parciales.

El Outbox garantiza consistencia eventual, no atomicidad distribuida.

---

## 13. Observabilidad y operación

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

No se incorpora infraestructura innecesaria en esta etapa.

---

## 14. Estrategia de pruebas

### Unitarias
- Validación de planes.
- Validación de suscripciones.
- Validación de cuotas.
- Validación de pagos.

### Integración MySQL
- Creación de suscripción.
- Activación por pago.
- Vencimiento.
- Cancelación.
- Consumo concurrente.

### Integración de API
- Endpoints de planes.
- Endpoints de suscripción.
- Endpoints de consumo.
- Endpoints de pagos.

### Concurrencia
- Dos solicitudes simultáneas.
- Reintentos.
- Fallos de proveedor.

### Idempotencia
- Webhooks duplicados.
- Reintentos de pago.
- Reintentos de consumo.

### PHP y Flutter
- Estados de carga.
- Estados de error.
- Sincronización.

### Outbox
- Inserción atómica.
- Procesamiento.
- Reconciliación.
- Recuperación.

---

## 15. Roadmap

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

## 16. Riesgos y decisiones pendientes

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

## 17. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia concreta |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección de skills y prioridades |
| `impeccable` | Sí | Sí | Auditoría de interfaces y diseño |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Identificación de riesgos |

---

## 18. Confirmación de restricciones

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

## 19. Recomendación de la siguiente fase

Iniciar FASE 2.0 con la documentación y contratos aprobados. No implementar FASE 2.1 hasta tener aprobación explícita. No iniciar FASE 2.4 ni FASE 2.5 hasta que FASE 2.2 y FASE 2.3 estén verificadas.