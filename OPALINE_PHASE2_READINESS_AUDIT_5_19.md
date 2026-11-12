# OPALINE_PHASE2_READINESS_AUDIT_5_19.md

## Auditoría de preparación técnica y comercial para FASE 2

Fecha de auditoría: 2026-11-08  
Modo: solo lectura. No se modificó código funcional, migraciones ni datos.  
Único archivo creado: `OPALINE_PHASE2_READINESS_AUDIT_5_19.md`.

---

## 1. Resumen ejecutivo

Opaline AI dispone de una base técnica funcional para una demo comercial controlada, pero **no está lista para iniciar FASE 2 sin condiciones**.

La plataforma ya implementa:

- Autenticación con access/refresh tokens.
- Sesiones y renovación.
- Chat real con LM Studio.
- Persistencia de conversaciones en MongoDB.
- Muro comunitario.
- Panel administrativo básico.
- Planes y suscripciones locales.
- Pagos demo y facturas de texto.
- Endurecimiento de seguridad, CSRF, autorización y aislamiento.
- Infraestructura de auditoría Outbox, worker, reconciliación y recuperación.

Los principales bloqueadores para una FASE 2 comercial son:

1. No existe integración real con PayPal.
2. No existen cuotas ni límites de consumo de IA.
3. El consumo de tokens no se registra.
4. Las suscripciones no tienen renovación, vencimiento ni expiración automática.
5. Los pagos demo contaminan métricas financieras.
6. No existe verificación real de correo.
7. No hay monitoreo operativo ni alertas.
8. La experiencia comercial está parcialmente duplicada entre PHP y Flutter.

Dictamen: **Lista con condiciones**.

---

## 2. Estado real del producto

### 2.1 Funcionalidades implementadas y verificadas

| Funcionalidad | Estado |
|---|---|
| Registro | Implementada y funcional |
| Login | Implementada y funcional |
| Refresh token | Implementada y funcional |
| Logout | Implementada y funcional |
| Cambio de contraseña | Implementada y funcional |
| Sesiones | Implementada y funcional |
| Chat con LM Studio | Implementada y funcional |
| Historial de conversaciones | Implementada y funcional |
| Muro comunitario | Implementada y funcional |
| Panel administrativo | Implementada y funcional |
| Planes | Implementada y funcional |
| Suscripción actual | Implementada y funcional |
| Pagos demo | Implementada y funcional como demo |
| Facturas de texto | Implementada y funcional como demo |
| CSRF en PHP | Implementada y funcional |
| Almacenamiento seguro en Flutter | Implementada y funcional |
| Outbox y reconciliación | Implementada y funcional como infraestructura |

### 2.2 Funcionalidades parciales

| Funcionalidad | Estado |
|---|---|
| Suscripciones | Parcial: no hay renovación, vencimiento ni expiración automática |
| Cancelación/restauración | Parcial: cambia estado local, pero no coordina con proveedor |
| Cuotas de IA | Parcial: `messages_per_month` existe, pero no se aplica |
| Consumo de tokens | Parcial: no se registra ni se limita |
| Modelos por plan | Parcial: no se valida contra suscripción |
| Métricas administrativas | Parcial: best-effort y contaminadas por demo |
| Paginación | Parcial: backend y PHP no comparten un contrato uniforme |
| Moderación | Parcial: endpoints existen, pero la cola web es básica |

### 2.3 Funcionalidades simuladas

| Funcionalidad | Estado |
|---|---|
| PayPal | Simulado |
| Activación de suscripciones por pago | Simulado |
| Reembolsos | Simulado |
| Facturas PDF | Simulado: texto plano |
| Cuotas por plan | Simulado: no se aplica |
| RAG y citas | Simulado: `citations` siempre vacío |
| Verificación de correo | Simulado: `email_verified` se asigna sin verificar |
| Recuperación de contraseña | Simulado: solo registra evento |
| Biometría | Simulado: no implementada |
| Seguridad por inactividad | Simulado: desbloqueo deshabilitado |
| Métricas financieras | Parcialmente simuladas por datos demo |

### 2.4 Solo interfaz visual o no implementadas

| Funcionalidad | Estado |
|---|---|
| API pública | No implementada |
| Streaming de respuestas | No implementado |
| Tool calling | No implementado |
| Visión | No implementado |
| Generación de imágenes | No implementado |
| Embeddings | No implementado |
| Renovación automática | No implementada |
| Webhooks de pago | No implementados |
| Notificaciones por correo | No implementadas |
| Panel de consumo en Flutter | Parcial |
| Panel de consumo en PHP | Parcial |
| Administración de usuarios | Parcial |
| Moderación avanzada | Parcial |
| Monitoreo operativo | No implementado |

---

## 3. Inventario de funcionalidades

### Backend Rust
- Auth: registro, login, refresh, logout, cambio de contraseña, sesiones.
- Planes: catálogo y suscripción actual.
- Pagos: historial, orden demo, verificación deshabilitada, reembolso deshabilitado.
- Facturas: texto plano, autorización por propietario.
- Chat: LM Studio, historial, modelos.
- Muro: publicación, edición, eliminación, reportes, moderación.
- Admin: usuarios, métricas, IA stats, gastos, reporte financiero.
- Outbox: infraestructura, worker, reconciliación, recuperación, servicio administrativo.

### Frontend PHP
- Landing, planes, login, registro, chat, muro, cuenta, checkout, panel, moderación, auditoría Outbox.
- Sesiones PHP y proxy cURL.
- CSRF y almacenamiento seguro de tokens en sesión.

### Flutter
- Welcome, login, registro, home, chat, muro, cuenta, checkout, suscripción, perfil, configuración.
- Provider y go_router.
- Almacenamiento seguro nativo.
- Sesión fail-closed.

---

## 4. Auditoría de planes

Estado actual:

- `plans` contiene Starter, Plus y Research.
- `messages_per_month` existe, pero no se aplica.
- `features` existe, pero no se valida.
- `active` existe, pero `create_order` ahora filtra planes activos.
- No hay versionado de planes.
- No hay historial de cambios de plan.

Evaluación:

El catálogo es suficiente para FASE 2.0, pero requiere:

- Versionado.
- Límites reales por plan.
- Activación/desactivación controlada.
- Auditoría de cambios.

---

## 5. Auditoría de suscripciones

Estado actual:

- `subscriptions` almacena estado, fechas y plan.
- Se crea una suscripción pendiente al crear una orden.
- Se activa al verificar, aunque la verificación esté deshabilitada.
- No hay expiración automática.
- No hay renovación.
- No hay período de gracia real.
- Cancelar cambia inmediatamente a `cancelled`.
- Restaurar puede activar suscripciones vencidas.
- No hay restricción de múltiples suscripciones activas.

Evaluación:

La base de datos soporta el modelo, pero la lógica de negocio es insuficiente para un SaaS comercial.

Bloqueador: sí.

---

## 6. Auditoría de cuotas

Estado actual:

- `plans.messages_per_month` existe.
- `usage_counters` existe, pero no se utiliza.
- El chat no consulta suscripción ni cuota.
- No se registran tokens.
- No hay límite de concurrencia.
- No hay reserva atómica de consumo.
- No hay diferenciación por modelo.

Evaluación:

Esta es una brecha crítica para FASE 2. Sin cuotas no es posible monetizar ni proteger la infraestructura.

Bloqueador: sí.

---

## 7. Auditoría de pagos

Estado actual:

- PayPal es simulado.
- `create_order` crea una orden local y una suscripción pendiente.
- `verify_order` está deshabilitado con `501`.
- `refund` está deshabilitado con `501`.
- `payments.history` funciona y está aislado por usuario.
- Las facturas son texto plano, no PDF.
- No hay webhooks.
- No hay idempotencia real de pagos.
- No hay relación robusta entre orden, pago, factura y suscripción.

Evaluación:

No es posible activar suscripciones por pago real en el estado actual.

Bloqueador: sí.

---

## 8. Auditoría de API y modelos

Estado actual:

- LM Studio se consume directamente desde Rust.
- El modelo se selecciona por el cliente, aunque el backend expone modelos habilitados.
- No existe AI Gateway independiente.
- No hay enrutamiento por capacidad.
- No hay streaming.
- No hay tool calling.
- No hay visión ni imágenes.
- No hay embeddings.
- No hay métricas de tokens.
- No hay costos estimados.

Evaluación:

La integración actual es funcional para desarrollo, pero no es suficiente para una API comercial con múltiples proveedores.

Bloqueador: parcial. No bloquea FASE 2.0, pero sí FASE 2.2 y posteriores.

---

## 9. Auditoría PHP y Flutter

### PHP

Existe:
- Landing, planes, login, registro, chat, muro, cuenta, checkout, panel, moderación, auditoría Outbox.

Falta:
- Gestión real de suscripciones.
- Panel de consumo.
- Historial de pagos avanzado.
- Moderación avanzada.
- Notificaciones.

### Flutter

Existe:
- Welcome, login, registro, home, chat, muro, cuenta, checkout, suscripción, perfil, configuración.

Falta:
- Panel de consumo.
- Gestión real de suscripciones.
- Notificaciones.
- Recuperación de contraseña real.

### Duplicaciones
- Planes y suscripciones se modelan de forma distinta.
- Cuotas se hardcodean en Flutter.
- El estado de suscripción no se sincroniza correctamente tras login.
- La experiencia comercial está dividida.

Evaluación:

La base existe, pero la experiencia comercial debe consolidarse.

Bloqueador: medio.

---

## 10. Seguridad y consistencia financiera

Controles existentes:

- Auth fail-closed.
- Refresh tokens con hash y expiración.
- Rate limiting.
- CORS allowlist.
- CSRF en PHP.
- Autorización en Rust.
- Aislamiento de conversaciones y facturas.
- Outbox, worker, reconciliación y recuperación.

Riesgos residuales:

- Pagos demo contaminan métricas.
- Suscripciones sin pago confirmado pueden activarse por verificación local si el endpoint se reactivara.
- No hay auditoría financiera completa.
- No hay protección contra consumo excesivo.
- No hay validación de webhooks.
- No hay control de concurrencia de cuotas.

Evaluación:

La seguridad base es sólida para desarrollo. La consistencia financiera aún no es apta para producción.

Bloqueador: sí.

---

## 11. Arquitectura objetivo para FASE 2

### Módulo 1 — Catálogo de planes
- Plan gratuito.
- Planes de pago.
- Límites y características.
- Versionado de planes.
- Activación/desactivación.

### Módulo 2 — Suscripciones
- Contratación.
- Activación.
- Renovación.
- Cancelación.
- Vencimiento.
- Historial.
- Restricción de una suscripción activa por usuario.

### Módulo 3 — Cuotas
- Contadores de uso.
- Reservas de consumo.
- Confirmación.
- Liberación.
- Límites concurrentes.
- Ciclos de facturación.

### Módulo 4 — Pagos
- Proveedor real.
- Órdenes.
- Confirmación.
- Webhooks.
- Reembolsos.
- Idempotencia.

### Módulo 5 — Medición
- Registro de solicitudes.
- Tokens.
- Modelos.
- Consumo por usuario.
- Costos operativos estimados.

### Módulo 6 — Experiencia comercial
- Página de planes.
- Panel de consumo.
- Gestión de suscripción.
- Historial de pagos.
- Panel administrativo.

Dependencias:

- Cuotas depende de planes y suscripciones.
- Pagos depende de planes.
- Experiencia comercial depende de planes, suscripciones, cuotas y pagos.
- Medición depende de chat y cuotas.

---

## 12. Modelo comercial

### Costos estimados por solicitud

Para cada modelo:

`costo por solicitud = tokens de entrada + tokens de salida + infraestructura + margen de seguridad`

Variables:

- `C_in`: costo de tokens de entrada.
- `C_out`: costo de tokens de salida.
- `C_infra`: costo de infraestructura por solicitud.
- `M`: margen objetivo.

Precio sugerido por plan:

`precio_mensual = (solicitudes_estimadas × costo_promedio × (1 + M)) + costo_fijo`

### Escenarios de ejemplo

| Escenario | Usuarios | Solicitudes/mes | Costo estimado | Precio sugerido |
|---|---:|---:|---:|---:|
| Estudiantes | 100 | 10 | Bajo | Bajo |
| Desarrolladores | 50 | 100 | Medio | Medio |
| PyMEs | 10 | 1000 | Alto | Alto |

No se establecen precios definitivos. Se requiere medición real de consumo.

### Estrategia freemium
- Plan gratuito con cuota baja.
- Prueba temporal de planes de pago.
- Upgrade desde la app y la web.
- Avisos antes de agotar la cuota.

### Riesgo de usuarios intensivos
- Un usuario intensivo puede consumir más recursos que el ingreso del plan.
- Se requieren límites duros por plan.
- Se requiere monitoreo de consumo.
- Se requiere posibilidad de suspender o degradar el servicio.

---

## 13. Roadmap propuesto

### FASE 2.0 — Diseño y preparación
Objetivo: consolidar contratos y modelo de datos.

Archivos probables:
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

Archivos probables:
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

Archivos probables:
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

Archivos probables:
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

Archivos probables:
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

Archivos probables:
- Tests Rust.
- Tests PHP.
- Tests Flutter.
- Documentación.

Dependencias: FASE 2.1 a 2.4.

Riesgos: alto.

Pruebas: flujo completo end-to-end.

Criterio: piloto controlado.

---

## 14. Dependencias

- FASE 2.1 depende de FASE 2.0.
- FASE 2.2 depende de FASE 2.1.
- FASE 2.3 depende de FASE 2.1.
- FASE 2.4 depende de FASE 2.1, 2.2 y 2.3.
- FASE 2.5 depende de FASE 2.1 a 2.4.

Trabajo paralelo posible:

- Documentación y contratos.
- Diseño de UI.
- Pruebas de seguridad.
- Medición de costos.

---

## 15. Riesgos

| Riesgo | Impacto | Mitigación |
|---|---|---|
| Usuarios intensivos | Alto | Cuotas duras y monitoreo |
| Pagos duplicados | Alto | Idempotencia y webhooks |
| Suscripciones sin pago confirmado | Alto | Confirmación real |
| Consumo sin registro | Alto | Medición centralizada |
| Inconsistencia entre PHP y Flutter | Medio | Contratos compartidos |
| Contaminación de métricas por demo | Medio | Separar datos demo |
| Falta de verificación de correo | Medio | Implementar verificación real |
| Falta de monitoreo | Medio | Añadir métricas operativas |

---

## 16. Skills consultadas y aplicadas

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección de skills y prioridades |
| `impeccable` | Sí | Sí | Auditoría de interfaces y diseño |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Identificación de riesgos |

---

## 17. Dictamen de preparación

### Preparación técnica
**Lista con condiciones.**

La base backend es sólida, pero faltan cuotas, consumo y pagos reales.

### Preparación de seguridad
**Lista con condiciones.**

La seguridad base es buena, pero faltan verificación de correo, monitoreo y consistencia financiera.

### Preparación comercial
**No lista.**

No es posible cobrar ni controlar consumo real en el estado actual.

### Preparación operativa
**No lista.**

No hay monitoreo, alertas ni métricas operativas suficientes.

### Bloqueadores reales
1. No hay integración real de pagos.
2. No hay cuotas ni consumo medido.
3. No hay renovación ni vencimiento de suscripciones.
4. No hay verificación real de correo.
5. No hay monitoreo operativo.

### Conclusión

Opaline AI puede iniciar FASE 2.0 y FASE 2.1 con la base actual, pero no puede ofrecer un servicio comercial real hasta implementar FASE 2.2 y FASE 2.3.

No se debe iniciar FASE 2.4 ni FASE 2.5 hasta que las fases anteriores estén verificadas.

No se debe declarar el producto listo para producción hasta que se cumplan los criterios de aceptación de seguridad, pagos, cuotas y monitoreo.