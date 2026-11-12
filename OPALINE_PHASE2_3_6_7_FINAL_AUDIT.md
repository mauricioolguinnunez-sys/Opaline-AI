# OPALINE_PHASE2_3_6_7_FINAL_AUDIT.md

## FASE 2.3.6.7 — Auditoría final de pagos PayPal, captura, webhooks, activación, reconciliación, workers, cuotas y seguridad

Fecha: 2026-11-08  
Modo: auditoría de solo lectura, sin implementar correcciones, sin habilitar workers, sin PayPal Live, sin commits, push ni despliegues.  
Repositorio: `/home/mauricioon/Documentos/Opaline_build`  
HEAD auditado: `cf210dd`

---

## 1. Resumen ejecutivo

El flujo feliz de FASE 2.3.6 funciona y está cubierto por pruebas:

- Creación idempotente de órdenes PayPal Sandbox con monto y moneda resueltos en servidor.
- Captura de una orden aprobada, con validación de monto/moneda contra PayPal, transacción local y activación de suscripción.
- Webhook firmado, persistido con deduplicación por `provider_event_id`, y activación en el camino exitoso.
- Activación idempotente y serializada por orden.
- Reconciliación de activaciones para órdenes que ya están locales en `captured` y tienen una transacción completada.
- Aislamiento de pruebas respecto de desarrollo/producción: únicamente se usó `opaline_test`.

Sin embargo, **la garantía de recuperación persistente y de consistencia financiera no queda aprobada**. La auditoría encontró cuatro hallazgos de severidad ALTA que afectan el objetivo central de la fase:

1. Un reintento de captura puede devolver éxito sin ejecutar la activación si la activación falló en el primer intento.
2. Varias rutas pueden dejar una orden local en `captured` sin transacción completada; la recuperación automática no repara ese estado.
3. Un webhook válido puede quedar marcado como `processed` aunque la consulta a PayPal falle o la orden no exista; además la respuesta HTTP es 200 y PayPal no reintenta.
4. Los errores de inserción en Outbox se ignoran en captura, webhook y activación, contradiciendo la afirmación de F2.3.6.6 de que los errores se propagan.

Adicionalmente, los workers están implementados y registrados, pero **no están habilitados por defecto, no tienen supervisión/graceful shutdown y la recuperación manual de Outbox solo acepta eventos `wall.post.`**, por lo que un evento de pago fallido no tiene ruta de revisión manual.

Dictamen global: **NO APROBADA PARA CIERRE DE FASE 2.3.6.7**. El camino feliz está aprobado; la recuperación ante fallos y la consistencia financiera no.

### Conteo de hallazgos

| Severidad | Cantidad |
|---|---:|
| CRÍTICO | 0 |
| ALTO | 4 |
| MEDIO | 6 |
| BAJO | 3 |
| **Total** | **13** |

No se encontró captura no autorizada, habilitación de PayPal Live ni movimiento de dinero real. Los hallazgos ALTO son de pérdida de activación/recuperación y de integridad de estados, no de cobro indebido.

---

## 2. Alcance y limitaciones

### 2.1 Alcance inspeccionado

- Órdenes PayPal Sandbox: creación, reutilización por idempotencia, consulta y sincronización de estado.
- Captura: validación de estado remoto, monto, moneda, idempotencia, concurrencia y transacción local.
- Webhooks: verificación de firma, persistencia, deduplicación, procesamiento síncrono, Outbox y activación.
- Activación: creación de suscripción, idempotencia, bloqueo de orden, eventos y Outbox.
- Reconciliación: órdenes, activaciones, worker automático y recuperación manual.
- Workers: registro, variables de habilitación, ciclo de vida, errores, shutdown, claims/fencing y supervisión.
- Cuotas: periodos de uso, reservas, límites y restricciones de unicidad.
- Seguridad: autenticación, autorización por propietario, rate limiting, CORS, PayPal Live bloqueado, validación de webhook y aislamiento de pruebas.
- Esquema efectivo de `opaline_test` y migraciones aditivas.

### 2.2 Limitaciones

- **No se crearon órdenes, capturas, reembolsos ni suscripciones reales en PayPal Sandbox.** Las pruebas de PayPal usan servidores HTTP locales simulados (`127.0.0.1`), por lo que la integración real con Sandbox no queda verificada de extremo a extremo.
- No se ejecutó `main` ni se habilitó ningún worker de producción. Las únicas instancias de worker ejecutadas fueron las de pruebas unitarias/integración sobre la base aislada.
- No se probó el comportamiento detrás de un proxy real, con múltiples instancias, ni con las credenciales/webhook ID reales de Sandbox.
- No se implementaron correcciones. Los hallazgos se documentan, no se reparan.
- No se inició FASE 2.3.7.

### 2.3 Aislamiento de pruebas

Se usó exclusivamente:

```bash
export OPALINE_TEST_DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline_test"
export OPALINE_TEST_MONGO_URL="mongodb://127.0.0.1:27018/opaline_test"
```

Evidencia:

- `opaline_rust_backend/tests/common/mod.rs:11-38` exige que la URL MySQL esté en `127.0.0.1` o `localhost`, que el nombre de la base contenga `test` y que no sea `opaline`.
- `opaline_rust_backend/tests/common/mod.rs:49-69` construye el estado de prueba con esa URL y con MongoDB `.database("opaline_test")`.
- Consulta directa a la base usada: `DATABASE() = opaline_test`, host local, puerto `3307`.
- La base ya contenía datos de ejecuciones anteriores (usuarios, órdenes, webhooks y suscripciones). Las pruebas no truncan las tablas compartidas; generan identificadores únicos. Esto es aceptable para el aislamiento solicitado, pero la suite no es hermética.
- Algunas pruebas de Outbox sí hacen `DELETE FROM outbox_events`/`outbox_admin_actions`, pero exclusivamente en `opaline_test`.

No se accedió a bases de desarrollo o producción.

---

## 3. Archivos y funciones inspeccionados

| Archivo | Funciones / secciones relevantes |
|---|---|
| `opaline_rust_backend/src/routes/payments.rs` | `router`, `list_payments`, `create_order`, `create_paypal_order`, `fetch_paypal_order`, `capture_paypal_order`, `capture_paypal_order_service`, `map_paypal_order_status`, `paypal_webhook`, `persist_payment_webhook_event`, `process_payment_webhook_event`, `reconcile_payment_orders`, `activate_subscription_from_payment`, `activate_paypal_subscription`, `reconcile_payment_activations`, `reap_pending_payment_activations`, `refund`, `invoice_pdf` |
| `opaline_rust_backend/src/paypal.rs` | `PayPalConfig::from_env`, `PayPalClient::new`, `access_token`, `verify_webhook_signature`, `create_order`, `capture_order`, `get_order`, pruebas de Sandbox/Live |
| `opaline_rust_backend/src/main.rs` | Registro y arranque de workers, incluido `OPALINE_PAYMENT_WEBHOOK_OUTBOX_ENABLED` y `OPALINE_PAYMENT_RECOVERY_ENABLED` |
| `opaline_rust_backend/src/outbox.rs` | `insert`, `list_pending`, `claim_pending`, `mark_processed`, `record_failure`, `recover_event_by_id`, `recover_event_by_id_with_consumer` |
| `opaline_rust_backend/src/outbox_worker.rs` | `OutboxWorkerConfig`, `OutboxWorker::run`, `process_once`, `spawn_worker`, claims, reintentos y shutdown |
| `opaline_rust_backend/src/consumers/paypal_webhook.rs` | `PayPalWebhookConsumer`, filtro de `payment.webhook.received` |
| `opaline_rust_backend/src/consumers/mongo_wall_audit.rs` | Consumidor de eventos `wall.post.` |
| `opaline_rust_backend/src/consumers/mongo_chat_persist.rs` | Consumidor de eventos de chat |
| `opaline_rust_backend/src/routes/outbox_admin.rs` | Listado, recuperación y revisión manual, restringidos a `wall.post.` |
| `opaline_rust_backend/src/outbox_reconciler.rs` | Reconciliación de Outbox |
| `opaline_rust_backend/src/outbox_recovery.rs` | Recuperación de Outbox |
| `opaline_rust_backend/src/quota.rs` | `ensure_usage_period`, `reserve_quota`, `commit_quota`, `release_quota`, `resolve_current_subscription` |
| `opaline_rust_backend/src/db.rs` | `init`, `ensure_financial_schema`, `ensure_commercial_schema`, migraciones aditivas |
| `opaline_rust_backend/migrations.sql` | `payment_orders`, `payment_transactions`, `payment_webhook_events`, `subscriptions`, `usage_periods`, `usage_counters`, `outbox_events` |
| `opaline_rust_backend/tests/common/mod.rs` | Guard de base de prueba, `setup_state`, `test_lock` |
| `opaline_rust_backend/tests/payment_orders_integration.rs` | Creación/consulta, captura única, concurrencia, reconciliación |
| `opaline_rust_backend/tests/payment_webhooks_integration.rs` | Firma, persistencia, deduplicación, activación por webhook |
| `opaline_rust_backend/tests/security_integration.rs` | Autenticación, rate limiting, aislamiento por usuario |
| `opaline_rust_backend/tests/plans_integration.rs` | Activación y suscripciones |
| `opaline_rust_backend/tests/quota_integration.rs` | Cuotas y concurrencia |
| `opaline_rust_backend/tests/outbox_worker_integration.rs` | Claims, reintentos, fencing y shutdown del worker genérico |
| `opaline_rust_backend/tests/outbox_admin_integration.rs` | Revisión manual de Outbox |
| `OPALINE_PHASE2_3_6_5_FINAL_AUDIT.md` | Línea base de hallazgos |
| `OPALINE_PHASE2_3_6_6_IMPLEMENTATION.md` | Afirmaciones de implementación contrastadas |

También se revisaron `RECOVERY_STATUS.md` y los documentos de fases anteriores para trazabilidad.

---

## 4. Arquitectura verificada del flujo de pago

### 4.1 Creación de orden

`create_order` (`payments.rs:66-89`) autentica, aplica rate limit por IP (`20/h`) y por usuario (`10/h`), resuelve la clave de idempotencia y delega en `create_paypal_order` (`payments.rs:182-318`).

- El monto y la moneda se resuelven en servidor desde `plan_versions` (`payments.rs:584-596`), no desde el cliente.
- Se reutiliza una orden existente por `(user_id, idempotency_key)`; si el plan, versión, monto o moneda cambiaron, responde `409` (`payments.rs:192-202`).
- Si ya existe `provider_order_id`, consulta PayPal y sincroniza el estado local (`payments.rs:208-226`).
- Si la fila local existe pero no hay `provider_order_id`, reutiliza `provider_request_id` para que PayPal aplique idempotencia (`payments.rs:229-238`).
- El rate limit de creación está probado en `security_integration.rs` (`payment_creation_rate_limit_returns_429`).

Observación: la ruta de reutilización `payments.rs:208-215` actualiza el estado local con `map_paypal_order_status` **sin validar monto/moneda remotos** y sin materializar una transacción. Esto contribuye al hallazgo F2.3.6.7-02.

### 4.2 Consulta/sincronización de orden

`fetch_paypal_order` (`payments.rs:320-387`) consulta PayPal, valida que el `id` remoto coincida, valida monto y moneda (`payments.rs:352-368`) y actualiza `payment_orders.status` si cambió (`payments.rs:370-375`).

Observación crítica: si PayPal devuelve `COMPLETED`, esta función marca la orden local como `captured` **sin crear una transacción completada ni activar la suscripción**. Es una ruta de lectura que puede producir un estado financiero incompleto (F2.3.6.7-02).

### 4.3 Captura

`capture_paypal_order_service` (`payments.rs:403-582`):

1. Autentica y busca la orden local por `id` y `user_id` (`payments.rs:409-410`).
2. Si ya existe una transacción completada, devuelve un éxito idempotente (`payments.rs:422-440`).
3. Consulta PayPal y exige estado remoto `APPROVED` (`payments.rs:443-452`).
4. Valida monto y moneda remotos contra los locales (`payments.rs:453-455`).
5. Usa una transacción local pendiente o crea una nueva (`payments.rs:459-508`).
6. Ejecuta la captura remota con `request_id` idempotente (`payments.rs:510-529`).
7. Actualiza la transacción local y el estado de la orden dentro de una transacción MySQL (`payments.rs:530-565`).
8. Ejecuta `activate_subscription_from_payment` después del commit (`payments.rs:567-569`).

El orden de los pasos es correcto para el camino feliz: primero se confirma la captura y la transacción local, después se activa. La activación es idempotente y bloquea la orden (`payments.rs:963-978`).

### 4.4 Webhook

`paypal_webhook` (`payments.rs:644-729`):

1. Rate limit por IP (`120/5 min`) y límite de cuerpo de 1 MiB (`payments.rs:650-656`).
2. Extrae `id`, `event_type` y `resource_id`; usa `supplementary_data.related_ids.order_id` y cae a `resource.id` (`payments.rs:658-673`).
3. Exige los headers de transmisión (`PAYPAL-TRANSMISSION-ID`, `PAYPAL-CERT-URL`, `PAYPAL-AUTH-ALGO`, `PAYPAL-TRANSMISSION-SIG`, `PAYPAL-TRANSMISSION-TIME`) (`payments.rs:675-679`).
4. Verifica la firma llamando a PayPal (`payments.rs:681-689`).
5. Persiste el evento con deduplicación (`payments.rs:691-708`).
6. Inserta un evento Outbox `payment.webhook.received` (`payments.rs:710-724`).
7. Ejecuta el procesamiento síncrono “best-effort” (`payments.rs:726-728`).

El punto 7 descarta el resultado y siempre devuelve `200 OK`; el punto 6 ignora el resultado de `outbox::insert`. Ambos hechos son centrales en los hallazgos F2.3.6.7-03 y F2.3.6.7-04.

### 4.5 Activación

`activate_subscription_from_payment` (`payments.rs:928-1020`):

- Rechaza órdenes no `captured` (`payments.rs:956-958`).
- Exige una transacción completada (`payments.rs:949-953`).
- Bloquea la orden y todas las suscripciones del usuario (`payments.rs:963-978`).
- Crea una suscripción activa, la marca como actual, vincula la orden y la transacción, inserta un evento de suscripción e inserta un evento Outbox `subscription.activated` (`payments.rs:980-1017`).
- Es idempotente: si `activated_subscription_id` ya existe, devuelve la suscripción (`payments.rs:945-947` y `969-973`).

La activación es correcta en el camino feliz, pero no valida monto/moneda contra la versión del plan (F2.3.6.7-07) y el Outbox se inserta con resultado ignorado (F2.3.6.7-04).

### 4.6 Reconciliación

- `reconcile_payment_orders` (`payments.rs:870-925`) recorre órdenes con `provider_order_id`, consulta PayPal, valida monto/moneda, actualiza estado y, si corresponde, intenta activar. **No tiene llamador en el código de producción** y descarta el error de activación con `let _ =` (`payments.rs:898-900`).
- `reconcile_payment_activations` (`payments.rs:1043-1059`) solo selecciona órdenes locales `status='captured'` (`payments.rs:1044`). Si no tienen transacción completada, `activate_subscription_from_payment` devuelve `409` y la reconciliación reporta `"failed"` sin reparar nada (`payments.rs:1051-1053`).
- El worker de `main.rs:142-155` solo llama a `reconcile_payment_activations`; nunca a `reconcile_payment_orders`.

### 4.7 Quotas

- `resolve_current_subscription` selecciona la suscripción actual/grace/cancelled vigente (`quota.rs:27-30`).
- `ensure_usage_period` busca por `(user_id, period_start, period_end)` y, si no existe, inserta (`quota.rs:70-99`).
- La restricción única real es `(user_id, period_start)` (`migrations.sql:206`, `db.rs:262`), no la terna usada por la consulta. Esto produce el hallazgo F2.3.6.7-11.
- `usage_counters` existe en el esquema (`migrations.sql:100-107`) pero no tiene uso en el código; F2.3.6.5-06 sigue sin resolverse (F2.3.6.7-12).

---

## 5. Hallazgos detallados

### F2.3.6.7-01 — ALTO — El reintento de captura devuelve éxito sin activar si la activación falló

**Evidencia**

- `payments.rs:422-440`: si existe una transacción completada, `capture_paypal_order_service` devuelve inmediatamente `status: "captured"`, `idempotent: true`, sin mirar `payment_orders.activated_subscription_id` ni llamar a `activate_subscription_from_payment`.
- `payments.rs:567-569`: la activación solo se ejecuta después del primer commit de captura.
- Si la activación falla (por ejemplo, error de base de datos, plan inexistente, bloqueo, error transitorio), la orden queda `captured` y la transacción `completed`, pero sin suscripción.
- Un reintento del cliente vuelve a entrar por `payments.rs:422-440` y recibe un éxito idempotente. No se reintenta la activación.
- El worker de recuperación podría repararlo, pero está deshabilitado por defecto (`main.rs:142-155`) y no forma parte del reintento del endpoint.
- La prueba `paypal_order_can_be_captured_once` (`tests/payment_orders_integration.rs:157-175`) solo prueba el reintento **después de una activación exitosa**; no prueba el reintento después de un fallo de activación.

**Impacto**

El usuario pagó y la API le confirma “captured”, pero no recibe suscripción. El estado es recuperable solo si se habilita el worker o si el cliente llama explícitamente a `/paypal/orders/:id/activate`; ninguna de esas condiciones está garantizada.

**Remediación requerida (no implementada en esta auditoría)**

- En la ruta idempotente de captura, si la transacción completada existe pero `activated_subscription_id` es `NULL`, ejecutar la activación antes de devolver éxito, o devolver un estado que obligue al cliente a reintentar la activación.
- Añadir una prueba de fallo de activación seguida de reintento de captura.

---

### F2.3.6.7-02 — ALTO — Órdenes `captured` sin transacción completada no son reparables automáticamente

**Evidencia**

Varias rutas pueden poner `payment_orders.status='captured'` sin crear una transacción completada:

- `fetch_paypal_order` (`payments.rs:370-375`): sincroniza el estado remoto y puede marcar `captured` si PayPal devuelve `COMPLETED`, sin insertar `payment_transactions`.
- `create_paypal_order`, rama de reutilización (`payments.rs:208-215`): actualiza el estado local según PayPal, incluido `captured`, sin transacción.
- `reconcile_payment_orders` (`payments.rs:894-896`): actualiza el estado a `captured` sin crear la transacción; además ignora el error de activación (`payments.rs:898-900`).

La recuperación automática no cubre ese estado:

- `reconcile_payment_activations` solo selecciona `status='captured'` (`payments.rs:1044`).
- `activate_subscription_from_payment` exige una transacción `completed` (`payments.rs:949-953`); si no existe, devuelve `409`.
- `main.rs:148` solo llama a `reconcile_payment_activations`; `reconcile_payment_orders` no tiene llamador.
- La prueba de reconciliación (`tests/payment_orders_integration.rs:287-301`) siembra explícitamente una orden `captured` **y** una transacción `completed`, por lo que no cubre el estado incompleto.

**Escenario realista**

1. PayPal ejecuta la captura.
2. La respuesta se pierde antes de que el backend confirme la transacción local, o el commit local falla después de la captura remota.
3. El cliente consulta `/paypal/orders/:id` o reintenta crear la orden; `fetch_paypal_order`/`create_paypal_order` marca la orden local como `captured`.
4. No hay transacción completada. El worker de activaciones selecciona la orden, pero falla con `409`; el reporte se descarta.
5. El webhook puede repararlo si llega y si se procesa correctamente, pero no hay garantía de que llegue ni de que el procesamiento no falle (ver F2.3.6.7-03).

**Impacto**

Pago capturado sin suscripción y sin reparación automática. Es el escenario de “pérdida de respuesta después del commit” que la fase debía garantizar.

**Remediación requerida**

- Toda transición a `captured` debe materializar una transacción completada a partir de datos verificados de PayPal y ejecutar la activación, o bien prohibir esas transiciones fuera de captura/webhook.
- La recuperación debe escanear también órdenes `approved` y `captured` sin transacción, consultar PayPal, hacer upsert de la transacción y activar.
- Añadir una prueba de respuesta perdida / commit fallido / orden `captured` sin transacción.

---

### F2.3.6.7-03 — ALTO — Un webhook válido se marca `processed` aunque falle la consulta a PayPal o no exista la orden

**Evidencia**

- `process_payment_webhook_event` usa `if let Ok(order) = client.get_order(&resource_id).await` (`payments.rs:794`). Cualquier error de red, timeout, 5xx o 404 se descarta.
- Si la orden no existe o la consulta falla, el flujo continúa y marca el evento como `processed` para todos los tipos “soportados” (`payments.rs:861-866`).
- `paypal_webhook` descarta el resultado de `process_payment_webhook_event` (`let proc = ...`) y siempre devuelve `Ok(StatusCode::OK)` (`payments.rs:726-728`). PayPal recibe 200 y no reintenta.
- La prueba `paypal_webhook_verifies_persists_and_deduplicates` envía `CHECKOUT.ORDER.APPROVED` con `resource.id = "NO-SUCH-ORDER"` y un mock que no tiene ruta de órdenes (`tests/payment_webhooks_integration.rs:52-55`, `25-48`). La prueba considera exitoso el webhook (`:73`, `:81`) y solo verifica deduplicación (`:83-85`); no verifica que el evento quede pendiente ni que se registre el error. De hecho, el código lo marca `processed`.
- El Outbox podría servir de reintento, pero su inserción se ignora (`payments.rs:711-723`) y el worker está deshabilitado por defecto.

**Impacto**

Un webhook de pago verificado puede perderse silenciosamente. Si la captura síncrona no creó la suscripción, el webhook es la última oportunidad y se descarta. No hay reintento de PayPal ni reparación automática.

**Remediación requerida**

- No marcar `processed` si la consulta a PayPal falla o la orden local no existe; devolver error/5xx y dejar el evento en estado reintentable.
- Insertar el Outbox de forma obligatoria antes de responder 200, o responder 5xx si no se pudo persistir.
- Registrar `last_error` y usar un estado `failed`/`retryable`; añadir prueba de fallo de `get_order` y de orden inexistente.

---

### F2.3.6.7-04 — ALTO — Los errores de inserción en Outbox se ignoran

**Evidencia**

- `outbox::insert` devuelve `Result<String, sqlx::Error>` (`outbox.rs:96-115`).
- `payments.rs:551-559`: el `Result` de `outbox::insert` en captura se descarta.
- `payments.rs:711-723`: el `Result` de `outbox::insert` en webhook se descarta.
- `payments.rs:1009-1017`: el `Result` de `outbox::insert` en activación se descarta.
- `cargo check` emite tres advertencias `unused Result that must be used` exactamente en `payments.rs:551`, `:711` y `:1009`.
- La prueba genérica `outbox_insert_is_atomic_with_business_write` (`tests/outbox_integration.rs:29-87`) demuestra la atomicidad del helper, pero **no** que los flujos de pago propaguen el error. No hay prueba de fallo de inserción en captura/webhook/activación.
- `OPALINE_PHASE2_3_6_6_IMPLEMENTATION.md` afirma “Errores de Outbox propagados” / “RESUELTO”; el código actual lo contradice.

**Impacto**

La promesa de recuperación durable no se cumple. En el webhook, si la inserción falla, no hay evento que el worker pueda reintentar; en captura/activación, el commit de negocio ocurre sin el evento, aunque esos eventos no tengan consumidor actualmente.

**Remediación requerida**

- Propagar el error con `?` y revertir la transacción local o devolver 5xx según corresponda.
- Añadir pruebas de fallo de inserción Outbox en los tres flujos.

---

### F2.3.6.7-05 — MEDIO — Outbox no enruta por tipo de evento; los eventos de pago no tienen consumidor y la revisión manual es solo para `wall.post.`

**Evidencia**

- `OutboxWorker::process_once` reclama **cualquier** evento pendiente (`outbox_worker.rs:118-119`); no hay filtro por tipo.
- `PayPalWebhookConsumer` rechaza todo lo que no sea `payment.webhook.received` (`consumers/paypal_webhook.rs:18-20`).
- `MongoWallAuditConsumer` rechaza lo que no sea `wall.post.` (`consumers/mongo_wall_audit.rs:69-70`).
- `MongoChatPersistConsumer` rechaza lo que no sea de chat (`consumers/mongo_chat_persist.rs:21-22`).
- `payment.capture.completed` (`payments.rs:553`) y `subscription.activated` (`payments.rs:1011`) se insertan, pero **no tienen ningún consumidor**.
- `outbox_admin.rs:71`, `:118` y `:189` restringen listado, acciones y recuperación a eventos que empiezan con `wall.post.`.

**Impacto**

- Si se habilitan varios workers, cualquiera puede reclamar el evento de otro y marcarlo `failed`/`permanent` al no reconocer el tipo.
- Si se habilita solo el worker de pagos, reclamará `payment.capture.completed` y `subscription.activated` y los marcará como fallidos por no tener handler.
- No hay revisión manual ni recuperación administrativa para eventos de pago. Un webhook fallido queda fuera de la consola de Outbox.

El default es workers deshabilitados, por lo que el impacto es operativo condicionado a la habilitación. Aun así, impide declarar la recuperación como lista para producción.

**Remediación requerida**

- Filtro de claim por tipo de evento o tablas/colas separadas.
- Registrar consumidores para los eventos de pago o dejar de emitirlos.
- Permitir eventos de pago en la revisión manual, con permisos y auditoría.
- Prueba de múltiples workers y de evento no soportado.

---

### F2.3.6.7-06 — MEDIO — Workers sin supervisión, shutdown ni backoff; errores descartados

**Evidencia**

- `main.rs:142-155`: el worker de recuperación se lanza con `tokio::spawn`; el `JoinHandle` se guarda, pero nunca se une, reinicia ni aborta. Los errores de `reconcile_payment_activations` se descartan con `let _ =` (`main.rs:148`).
- `main.rs:112-123`: el worker de webhook guarda `(handle, tx)`, pero no hay ningún `tx.send(true)` en el código de producción; el `watch` de shutdown nunca se activa.
- `main.rs:112-119` no valida la configuración de PayPal antes de arrancar el worker de webhook. `PayPalWebhookConsumer` devuelve `Permanent("paypal not configured")` si `PayPalClient::from_env()` falla (`consumers/paypal_webhook.rs:32-35`), por lo que un worker habilitado en un entorno mal configurado marcaría los eventos como fallidos de forma permanente.
- `main.rs` no configura `with_graceful_shutdown` para el servidor.
- El worker de recuperación no tiene backoff ni claim/lease de worker; la exclusión mutua de la activación se apoya únicamente en el lock de la orden. Recorre todas las órdenes `captured` cada 60 segundos y reintenta indefinidamente los fallos permanentes.
- `reconcile_payment_activations` solo reporta `"failed"` sin motivo (`payments.rs:1051-1053`); `main` no persiste ese reporte ni lo alerta.
- El test de shutdown del worker genérico (`tests/outbox_worker_integration.rs:189-207`) demuestra que el worker de Outbox soporta `watch`, pero no que `main` lo use para los workers de pago.
- No hay prueba de reinicio, supervisión, backoff ni recuperación tras caída del worker de pagos.

**Impacto**

Un worker que muere o se cuelga no se reinicia; un despliegue no apaga los workers de forma ordenada; los fallos permanentes generan carga y no son visibles. La recuperación automática no es operable con garantías.

**Remediación requerida**

- Supervisor con reinicio y backoff, señales de shutdown y `JoinHandle` unido.
- Persistir resultados/errores y métricas; alertar fallos.
- Claim/fencing o lease para instancias múltiples; pruebas de reinicio y caos.

---

### F2.3.6.7-07 — MEDIO — La activación no valida monto/moneda contra la versión del plan

**Evidencia**

- `activate_subscription_from_payment` lee la orden y la transacción (`payments.rs:935-953`) y el periodo del plan (`payments.rs:960-961`), pero no consulta el precio/moneda de `plan_versions` ni compara con `payment_orders.amount/currency` o `payment_transactions.amount/currency`.
- La captura y el webhook sí validan PayPal contra la orden local (`payments.rs:453-455`, `payments.rs:824-826`), pero la activación confía en lo persistido.
- `OPALINE_PHASE2_3_6_6_IMPLEMENTATION.md` afirma que se añadió “validación de importe y moneda en la activación”; el código actual no la contiene.
- Las pruebas de activación usan montos coincidentes y no prueban un monto/moneda inconsistente.

**Impacto**

Si una orden o transacción local queda inconsistente (error previo, cambio de plan, intervención manual, migración), la activación puede otorgar un plan sin verificar el precio. Es una brecha de defensa en profundidad y una contradicción documental, aunque el camino normal ya valida contra PayPal.

**Remediación requerida**

- Comparar el monto/moneda de la orden y la transacción contra `plan_versions.price/currency` antes de activar.
- Añadir prueba de monto/moneda manipulados o desactualizados.

---

### F2.3.6.7-08 — MEDIO — No se validan localmente `PAYPAL-CERT-URL`, algoritmo ni vigencia temporal

**Evidencia**

- `paypal_webhook` extrae `cert_url`, `auth_algo` y `transmission_time` de headers y los reenvía a PayPal sin allowlist ni validación de formato (`payments.rs:675-684`).
- `PayPalClient::verify_webhook_signature` construye el body con esos valores y llama al endpoint de verificación de PayPal (`paypal.rs:247-287`); no hace verificación criptográfica local ni valida el host del certificado.
- La prueba usa `PAYPAL-CERT-URL: https://example.com/cert`, `PAYPAL-AUTH-ALGO: SHA256withRSA` y `PAYPAL-TRANSMISSION-TIME: 2026-01-01T00:00:00Z` con un mock que devuelve `SUCCESS` (`tests/payment_webhooks_integration.rs:61-64`, `:162-165`, `:136-138`). La prueba demuestra que el código local no impone un host de certificado de PayPal ni una ventana de tiempo.
- No se encontró una prueba de certificado malicioso, algoritmo inesperado o replay fuera de ventana.

**Impacto**

La única barrera es la respuesta del endpoint de verificación de PayPal. No se afirma un bypass explotable; se documenta que falta defensa en profundidad y que un certificado/URL arbitrario se reenvía a PayPal. Si el entorno o el endpoint de PayPal aceptan el certificado indicado, la verificación local no aporta restricciones.

**Remediación requerida**

- Allowlist de hosts de certificados PayPal (`api.paypal.com`/Sandbox), validación de algoritmo permitido y ventana de transmisión.
- Pruebas de cert URL no permitida, algoritmo inválido y timestamp expirado.

---

### F2.3.6.7-09 — MEDIO — Procesamiento de webhook sin claim atómico ni estado `processing`/`failed`

**Evidencia**

- `process_payment_webhook_event` lee `processing_status` y solo se detiene si ya es `processed` o `ignored` (`payments.rs:775-785`). No hace un `UPDATE ... WHERE processing_status IN (...)` atómico ni adquiere un claim/lease.
- El handler síncrono y el worker de Outbox pueden procesar el mismo evento concurrentemente.
- La tabla permite `received,pending,processed,ignored,failed`, pero el código solo escribe `processed`/`ignored` (`payments.rs:861-866`); no usa `pending`, `failed`, `last_error` ni un estado de procesamiento.
- La inserción de la transacción verifica existencia y luego inserta (`payments.rs:829-840`), pero no es atómica; la restricción única evita duplicados, no la carrera de lectura/escritura.
- No hay prueba de dos procesamientos concurrentes del mismo webhook.

**Impacto**

Puede haber dobles intentos, respuestas 500 intermitentes y falta de observabilidad/reintento clasificado. La activación es idempotente, lo que reduce el daño, pero la gestión del evento no es robusta.

**Remediación requerida**

- Claim atómico con lease, estados `processing`/`failed`, `last_error` y reintentos clasificados.
- Prueba de concurrencia del mismo evento y de reintento tras fallo parcial.

---

### F2.3.6.7-10 — MEDIO — Webhooks de denegación/reembolso se marcan `processed` sin actualizar el estado financiero

**Evidencia**

- `PAYMENT.CAPTURE.DENIED` está incluido en `should_update` (`payments.rs:845`), pero `map_paypal_order_status` no tiene un mapeo específico para denegación (`payments.rs:616-623`); el estado local se deriva del estado de la orden, no de la captura denegada.
- `PAYMENT.CAPTURE.REFUNDED` y `PAYMENT.CAPTURE.REVERSED` están en la lista de eventos “soportados” (`payments.rs:861`) y se marcan `processed`, pero no hay actualización de `payment_transactions`, `payment_refunds` ni de la suscripción.
- `refund` responde `501 NOT_IMPLEMENTED` (`payments.rs:157-178`), por lo que no hay flujo local de reembolso.
- No hay pruebas de denegación, reembolso o reversión.

**Impacto**

Una denegación no queda registrada como fallo de captura; un reembolso o reversión externo no revoca la suscripción ni actualiza la transacción. El evento queda marcado como procesado, lo que oculta la inconsistencia. La fase no implementa reembolsos, pero el webhook declara soporte y no lo cumple.

**Remediación requerida**

- Handlers explícitos para denegación, reembolso y reversión, con actualización transaccional y revocación/ajuste de suscripción.
- Si no se van a implementar, excluirlos de los eventos soportados y documentarlo.
- Pruebas de cada evento.

---

### F2.3.6.7-11 — BAJO — Unicidad de `usage_periods` no coincide con la consulta

**Evidencia**

- `migrations.sql:206` define `UNIQUE KEY uq_usage_period_user_start (user_id, period_start)`.
- `db.rs:262` crea la misma restricción.
- `quota.rs:71-73` y `quota.rs:93-95` buscan por `(user_id, period_start, period_end)`.
- Si existe un periodo con el mismo `period_start` pero distinto `period_end`, el `INSERT` (`quota.rs:78-89`) falla por duplicado y la recuperación por la terna no encuentra fila, devolviendo `500` (`quota.rs:92-96`).
- No hay prueba de este caso.

**Impacto**

Error 500 en un caso de borde de cuota; no afecta el camino feliz.

**Remediación requerida**

- Alinear la restricción única y la consulta (incluir `period_end` o buscar por `user_id, period_start`).
- Añadir prueba de mismo inicio con distinto fin.

---

### F2.3.6.7-12 — BAJO — `usage_counters` sigue sin uso

**Evidencia**

- La tabla se define en `migrations.sql:100-107`.
- No hay referencias de código fuera de la migración; la cuota usa `usage_periods`.
- F2.3.6.5-06 sigue sin resolverse.

**Impacto**

Esquema muerto/confuso; no afecta el runtime actual, pero dificulta el mantenimiento y las métricas.

**Remediación requerida**

- Eliminar/documentar la tabla o implementar su uso previsto.

---

### F2.3.6.7-13 — BAJO — `auto_renew=1` se activa aunque no existe renovación automática

**Evidencia**

- `payments.rs:986` inserta la suscripción con `auto_renew=1`.
- `plans.rs:232-260` también activa con `auto_renew=1` (`plans.rs:250`).
- No existe worker/proceso de cobro o renovación automática en el código auditado.
- El cliente puede leer `auto_renew` como una promesa de renovación (`plans.rs:126`).

**Impacto**

Estado engañoso para el usuario y para la UI; no hay cobro automático. No es un fallo de seguridad.

**Remediación requerida**

- Mantener `auto_renew=0` hasta implementar renovación, o renombrar/documentar el campo como preferencia no ejecutada.

---

## 6. Matriz de trazabilidad de garantías

| Garantía / requisito | Evidencia de código | Evidencia de prueba | Resultado |
|---|---|---|---|
| PayPal Live bloqueado; Sandbox por defecto | `paypal.rs:8-25`, `:80-96`; test `blocks_live_environment` `paypal.rs:404-414` | Unit test pasa | **CUMPLE** |
| Orden Sandbox con monto/moneda de servidor | `payments.rs:182-318`, `:584-596` | `paypal_order_create_and_query` 4/4 | **CUMPLE** (mock, no Sandbox real) |
| Idempotencia de creación por usuario/clave | `payments.rs:192-238`, restricciones `migrations.sql:322-324` | Misma prueba | **CUMPLE** |
| Captura única y validación de monto/moneda | `payments.rs:443-455`, `:530-565` | `paypal_order_can_be_captured_once` | **CUMPLE** |
| Concurrencia de captura/activación | Bloqueo de orden `payments.rs:963-978` | `payment_activation_is_idempotent_under_concurrency` | **CUMPLE** (con aserción `>=1` mejorable) |
| Activación idempotente | `payments.rs:945-947`, `:969-978` | Prueba de concurrencia | **CUMPLE** en camino feliz |
| Reintento tras fallo de activación | `payments.rs:422-440` no activa | No hay prueba | **NO CUMPLE** (F2.3.6.7-01) |
| Orden `captured` siempre con transacción completada | `payments.rs:370-375`, `:208-215`, `:894-896` | No hay prueba de estado incompleto | **NO CUMPLE** (F2.3.6.7-02) |
| Recuperación automática de órdenes/activaciones | `main.rs:142-155`, `payments.rs:1043-1059` | `captured_payment_can_be_reconciled_to_subscription` solo con tx sembrada | **PARCIAL** (solo activaciones, no órdenes) |
| Firma de webhook | `payments.rs:675-689`, `paypal.rs:247-287` | `paypal_webhook_verifies...` | **PARCIAL** (sin validación local de cert/algo/tiempo) |
| Deduplicación de webhook | `payments.rs:731-768`, unique `provider_event_id` | Misma prueba, count=1 | **CUMPLE** |
| Webhook fallido no se pierde | `payments.rs:794`, `:861-866`, `:727-728` | La prueba con `NO-SUCH-ORDER` lo marca processed | **NO CUMPLE** (F2.3.6.7-03) |
| Outbox durable en pagos | `payments.rs:551`, `:711`, `:1009` | Solo prueba genérica del helper | **NO CUMPLE** (F2.3.6.7-04) |
| Worker de pagos habilitado/supervisado | `main.rs:112-155` | No hay prueba de main; worker genérico sí tiene shutdown | **NO CUMPLE** para producción (F2.3.6.7-06) |
| Revisión manual de eventos de pago | `outbox_admin.rs:71`, `:118`, `:189` solo `wall.post.` | Pruebas de admin solo wall | **NO CUMPLE** (F2.3.6.7-05) |
| Validación de monto/moneda en activación | `payments.rs:935-953` no compara plan_version | No hay prueba de mismatch | **NO CUMPLE** (F2.3.6.7-07) |
| Cuotas con periodo único consistente | `quota.rs:70-99` vs `migrations.sql:206` | 4/4 pruebas, sin caso de borde | **PARCIAL** (F2.3.6.7-11) |
| `usage_counters` usado | Solo `migrations.sql:100-107` | No hay prueba | **NO CUMPLE** (F2.3.6.7-12) |
| Seguridad de autenticación/autorización de pagos | `payments.rs:40-74`, `security_integration.rs` | 7/7 | **CUMPLE** en alcance probado |
| Rate limiting de creación/webhook/activación | `payments.rs:68`, `:74`, `:651`, `:1030`, `:1035` | `security_integration.rs` 2 pruebas de rate limit | **CUMPLE** (en memoria, por instancia) |

---

## 7. Matriz de cobertura de pruebas de fallo

| Escenario | ¿Existe prueba? | Evidencia | Resultado |
|---|---|---|---|
| Activación falla y se reintenta la captura | No | `payment_orders_integration.rs:157-175` solo éxito | Brecha F2.3.6.7-01 |
| Respuesta de captura perdida / commit local falla | No | No hay test | Brecha F2.3.6.7-02 |
| Orden local `captured` sin transacción | No | `:287-294` siembra tx | Brecha F2.3.6.7-02 |
| `get_order` falla en webhook | No | `payment_webhooks_integration.rs:52-85` usa orden inexistente y espera 200 | Brecha F2.3.6.7-03 |
| Inserción Outbox falla | No | Solo helper `outbox_integration.rs:29-87` | Brecha F2.3.6.7-04 |
| Múltiples workers reclaman el mismo evento | No | No hay prueba | Brecha F2.3.6.7-05 |
| Worker de pagos se reinicia/cae | No | Solo worker genérico `outbox_worker_integration.rs` | Brecha F2.3.6.7-06 |
| Webhook duplicado concurrente | No | Prueba secuencial | Brecha F2.3.6.7-09 |
| Cert URL/algoritmo/tiempo inválidos | No | Test usa `example.com` y hora antigua | Brecha F2.3.6.7-08 |
| Reembolso/denegación/reversión | No | No hay test | Brecha F2.3.6.7-10 |
| Cuota mismo inicio/distinto fin | No | `quota_integration.rs` no lo cubre | Brecha F2.3.6.7-11 |
| Reconciliación de órdenes sin llamador | No | `reconcile_payment_orders` sin test de worker | Brecha F2.3.6.7-02 |

La suite prueba el camino feliz y la concurrencia de activación, pero **no inyecta fallos**. Por eso los 92 tests verdes no demuestran la garantía de recuperación que la fase declara.

---

## 8. Pruebas ejecutadas y resultados

### 8.1 Entorno

```bash
export PATH="/home/mauricioon/.cache/puccinialin/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
export OPALINE_TEST_DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline_test"
export OPALINE_TEST_MONGO_URL="mongodb://127.0.0.1:27018/opaline_test"
```

Base MySQL verificada: `opaline_test`, puerto `3307`, host local.  
MongoDB de prueba: base `opaline_test` configurada por `tests/common/mod.rs`.

### 8.2 Resultados

| Comando | Resultado |
|---|---|
| `cargo check` | Sin errores. 37 warnings de lib y 2 de bin; incluye tres `unused Result` en `payments.rs:551`, `:711`, `:1009`. |
| `cargo test --lib` | 24 passed; 0 failed. |
| `cargo test` | 24 unitarias + 4 CORS = 28 passed; 92 ignored; 0 failed. |
| `cargo test -- --ignored` | 92 passed; 0 failed; 15 suites. |
| `cargo test --test payment_orders_integration -- --ignored` | 4 passed; 0 failed. |
| `cargo test --test payment_webhooks_integration -- --ignored` | 2 passed; 0 failed. |
| `cargo test --test security_integration -- --ignored` | 7 passed; 0 failed. |
| `cargo test --test plans_integration -- --ignored` | 9 passed; 0 failed. |
| `cargo test --test quota_integration -- --ignored` | 4 passed; 0 failed. |
| `cargo test --test chat_operations_integration -- --ignored` | 3 passed; 0 failed. |
| `git diff --check` | Exit 0; sin errores de whitespace. |

Distribución de las 92 pruebas ignoradas:

| Suite | Pruebas |
|---|---:|
| `auth_integration` | 8 |
| `chat_operations_integration` | 3 |
| `financial_model_integration` | 1 |
| `outbox_admin_integration` | 11 |
| `outbox_integration` | 8 |
| `outbox_reconciliation_integration` | 5 |
| `outbox_recovery_integration` | 10 |
| `outbox_worker_integration` | 7 |
| `payment_orders_integration` | 4 |
| `payment_webhooks_integration` | 2 |
| `plans_integration` | 9 |
| `quota_integration` | 4 |
| `security_integration` | 7 |
| `wall_integration` | 6 |
| `wall_outbox_integration` | 7 |
| **Total** | **92** |

### 8.3 Pruebas de PayPal

Las pruebas de PayPal usan un servidor HTTP local (`axum`) y variables de entorno de Sandbox con credenciales ficticias. No se llamó a `api-m.sandbox.paypal.com` ni a `api-m.paypal.com`. Esto valida la lógica local, no la integración real con Sandbox.

### 8.4 Workers

No se ejecutó `main` ni se activó `OPALINE_PAYMENT_RECOVERY_ENABLED`/`OPALINE_PAYMENT_WEBHOOK_OUTBOX_ENABLED`. Las pruebas de Outbox instancian workers de prueba contra `opaline_test`; no son workers de producción. Los workers de pago están **implementados y registrados**, pero **no habilitados, no supervisados y no probados de extremo a extremo**.

---

## 9. Contradicciones con las afirmaciones de F2.3.6.6

| Afirmación documental | Código actual | Resultado |
|---|---|---|
| “Validación de importe y moneda en la activación” | `activate_subscription_from_payment` no compara con `plan_versions`; solo lee orden/transacción (`payments.rs:935-953`) | **NO VERIFICADO / FALSO** |
| “Errores de Outbox propagados” | `payments.rs:551`, `:711`, `:1009` ignoran el `Result`; `cargo check` lo advierte | **FALSO** |
| “Worker automático de reconciliación” | `main.rs:148` solo reconcilia activaciones; `reconcile_payment_orders` no tiene llamador | **PARCIAL** |
| “Se garantiza que una captura confirmada permanezca vinculada a un proceso recuperable” | La activación puede fallar y el reintento de captura no la repite; órdenes `captured` sin tx no se reparan | **NO CUMPLE** |
| F2.3.6.5-01 “RESUELTO PARCIALMENTE” | Sigue el bypass de activación en el reintento idempotente | **NO RESUELTO** |
| F2.3.6.5-02 “RESUELTO” | Worker existe, pero solo activaciones y sin supervisión | **PARCIAL** |
| F2.3.6.5-05 “NO RESUELTO” | Sigue la discrepancia de unicidad de `usage_periods` | **NO RESUELTO** |
| F2.3.6.5-06 “NO RESUELTO” | `usage_counters` sigue sin uso | **NO RESUELTO** |

La documentación de F2.3.6.6 no debe usarse como evidencia de cierre sin corregir estas contradicciones.

---

## 10. Riesgos residuales

1. **Integración real con PayPal Sandbox no verificada.** Todo el flujo PayPal se probó con mocks locales. No se validó latencia, expiración de token, errores reales de PayPal, formato exacto de webhooks ni certificados reales.
2. **Workers deshabilitados por defecto.** La recuperación automática no está operativa salvo que se configure explícitamente `OPALINE_PAYMENT_RECOVERY_ENABLED=1` y `OPALINE_PAYMENT_WEBHOOK_OUTBOX_ENABLED=1`. Aun así, los hallazgos 01–06 limitan su eficacia.
3. **Revisión manual de pagos inexistente.** `outbox_admin` solo opera sobre `wall.post.`; un pago fallido no tiene bandeja de revisión.
4. **Historial de pagos legacy.** `list_payments` consulta la tabla `payments` (`payments.rs:39-58`), no `payment_orders`. Las órdenes PayPal nuevas no aparecen necesariamente en el historial ni generan facturas. Es una brecha funcional fuera del núcleo de captura, pero relevante para el usuario final.
5. **Migraciones aditivas que ignoran errores.** `db.rs:256` y las migraciones financieras usan `let _ = ...` para ALTER/CREATE. Un esquema incompleto puede no impedir el arranque y fallar en runtime. En `opaline_test` el esquema observado es consistente, pero no hay una tabla de versiones de migración ni una verificación de columnas obligatorias.
6. **Rate limiter en memoria y por instancia.** No es distribuido; con varias réplicas los límites se multiplican. No se probó detrás de proxy.
7. **Suite no hermética.** `opaline_test` acumula datos de ejecuciones previas; las pruebas usan IDs únicos, pero no truncan tablas compartidas. Es aceptable para el aislamiento solicitado, no para CI paralelo.
8. **Webhook replay/ventana temporal.** No hay validación local de vigencia de `PAYPAL-TRANSMISSION-TIME`; la deduplicación por `provider_event_id` evita duplicados exactos, pero no se probó replay fuera de ventana.
9. **Refunds y renovación automática no implementados.** El webhook declara eventos de reembolso/reversión y la suscripción se crea con `auto_renew=1`, pero no hay flujo real de reembolso ni cobro recurrente.
10. **Sin observabilidad de fallos de pago.** No hay métricas, alertas ni persistencia de `last_error`; los reportes de reconciliación se descartan.

---

## 11. Dictamen final

### 11.1 Dictamen por área

| Área | Dictamen |
|---|---|
| Creación/consulta de órdenes Sandbox | **APROBADA** (con mocks; Sandbox real no verificado) |
| Captura y validación de monto/moneda | **APROBADA** en camino feliz |
| Idempotencia/concurrencia de captura | **APROBADA** con reserva de cobertura |
| Activación en camino feliz | **APROBADA** |
| Reintento de captura tras fallo de activación | **NO APROBADA** (F2.3.6.7-01) |
| Consistencia `captured` ↔ transacción | **NO APROBADA** (F2.3.6.7-02) |
| Webhooks y recuperación ante fallos | **NO APROBADA** (F2.3.6.7-03/04) |
| Workers y recuperación operativa | **NO APROBADA** (F2.3.6.7-05/06) |
| Validación de monto/moneda en activación | **NO APROBADA** (F2.3.6.7-07) |
| Seguridad de webhook | **APROBADA CON RESERVAS** (F2.3.6.7-08) |
| Cuotas | **APROBADA CON RESERVAS** (F2.3.6.7-11/12) |

### 11.2 Dictamen global

**NO APROBADA PARA CIERRE DE FASE 2.3.6.7.**

El flujo feliz está implementado y pasa 92 pruebas de integración, pero la fase se definió como “garantía de activación financiera, recuperación persistente y validación transaccional”. Esa garantía no se sostiene ante fallos de activación, pérdida de respuesta, fallos de consulta a PayPal o fallos de Outbox. Los cuatro hallazgos ALTO deben corregirse y cubrirse con pruebas de inyección de fallos antes de declarar la fase cerrada.

No se autoriza iniciar FASE 2.3.7 con base en este estado. Tampoco se autoriza habilitar workers ni PayPal Live.

### 11.3 Condiciones mínimas para reconsiderar el dictamen

1. Corregir F2.3.6.7-01: el reintento de captura debe activar si la transacción existe pero la suscripción no.
2. Corregir F2.3.6.7-02: toda transición a `captured` debe materializar transacción completada y activación, y la recuperación debe reparar `captured` sin transacción.
3. Corregir F2.3.6.7-03: no marcar `processed` si falla PayPal o no existe la orden; devolver error y mantener el evento reintentable.
4. Corregir F2.3.6.7-04: propagar errores de Outbox en captura, webhook y activación.
5. Resolver el enrutamiento/consumidores de Outbox y habilitar revisión manual de pagos (F2.3.6.7-05).
6. Añadir supervisión, shutdown, backoff y claim/fencing al worker de pagos (F2.3.6.7-06).
7. Añadir validación de monto/moneda en activación y pruebas de fallo (F2.3.6.7-07).
8. Validar certificado/algoritmo/tiempo de webhook o documentar formalmente la dependencia de PayPal (F2.3.6.7-08).
9. Corregir la unicidad de `usage_periods` y decidir el destino de `usage_counters` (F2.3.6.7-11/12).
10. Ejecutar pruebas de inyección de fallos, reinicio de worker y webhook concurrente.

---

## 12. Cumplimiento del modo auditoría

- No se modificó código fuente, migraciones, pruebas ni archivos de entorno.
- No se implementaron correcciones.
- No se crearon órdenes, capturas, reembolsos ni suscripciones reales.
- No se habilitó PayPal Live.
- No se habilitaron workers de producción ni se ejecutó `main`.
- No se hizo commit, push, reset, clean ni despliegue.
- Las pruebas escribieron únicamente en la base aislada `opaline_test` (MySQL y MongoDB), nunca en desarrollo o producción.
- El único archivo creado por esta auditoría es este informe: `OPALINE_PHASE2_3_6_7_FINAL_AUDIT.md`.
- Los cambios preexistentes del repositorio (52 modificados, 15 eliminados, 68 no rastreados antes de este informe) se preservaron sin alteración.

**Fin de la auditoría.**
