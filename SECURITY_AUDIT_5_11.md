# SECURITY_AUDIT_5_11.md

## Auditoría de consistencia, trazabilidad y recuperación MySQL–MongoDB

Fecha: 2026-11-08  
Modo: solo lectura. No se modificó código, esquema, datos ni servicios.  
Único archivo creado: `SECURITY_AUDIT_5_11.md`.

---

## 1. Resumen ejecutivo

Opaline AI utiliza MySQL como fuente principal para datos transaccionales y MongoDB para conversaciones y eventos de auditoría. Las operaciones relevantes son mayormente de un solo motor o combinan una escritura principal en MySQL con una escritura de auditoría en MongoDB.

No se encontró un mecanismo de atomicidad distribuida entre MySQL y MongoDB. Por tanto, una falla parcial puede dejar un registro principal persistido sin su evento de auditoría, o un evento huérfano si la escritura principal falla después del evento.

No se confirmó pérdida de datos de usuarios, pagos o publicaciones por este motivo en el código actual. El principal riesgo es la pérdida o duplicación de auditoría y la dificultad de reconciliación.

Se recomienda implementar, en una tarea futura, un patrón Transactional Outbox en MySQL para eventos de auditoría, con procesamiento asíncrono y reintentos controlados. No se implementa en esta tarea.

---

## 2. Arquitectura de persistencia actual

| Motor | Uso principal |
|---|---|
| MySQL | Usuarios, sesiones, planes, suscripciones, pagos, facturas, posts, reportes, gastos, modelos |
| MongoDB | Conversaciones, métricas de IA, eventos de autenticación, pagos y muro |
| LM Studio | Inferencia |

Fuente de verdad:
- MySQL: identidad, autorización, pagos, suscripciones, muro y moderación.
- MongoDB: conversaciones y eventos de auditoría.

No existe una transacción distribuida entre MySQL y MongoDB.

---

## 3. Inventario de operaciones

| Operación | Endpoint / función | MySQL | MongoDB | Orden observado | Reintento | Idempotencia |
|---|---|---|---|---|---|---|
| Registro | `auth::register` | INSERT user + session | No | MySQL primero | No | No |
| Login | `auth::login` | SELECT + INSERT session | No | MySQL | No | No |
| Logout | `auth::logout` | DELETE session | No | MySQL | No | No |
| Refresh | `auth::refresh` | SELECT + UPDATE session | No | MySQL | No | Condicionada |
| Cambio de contraseña | `auth::change_password` | UPDATE user + DELETE sessions | No | MySQL | No | No |
| Recuperación | `auth::forgot_password` | No | INSERT auth_event | Mongo | No | No |
| Crear orden | `payments::create_order` | INSERT payment + subscription | No | MySQL transacción | No | No |
| Chat | `chat::chat` | No | upsert conversation + metrics | Mongo | No | No (cliente envía ID) |
| Eliminar conversación | `chat::delete_conversation` | No | DELETE conversation | Mongo | No | Sí por filtro |
| Crear post | `wall::create_post` | INSERT post | INSERT wall_event | MySQL primero, luego Mongo | No | Clave por usuario |
| Editar post | `wall::update_post` | UPDATE post | INSERT wall_event | MySQL primero, luego Mongo | No | No |
| Eliminar post | `wall::delete_post` | UPDATE post | INSERT wall_event | MySQL primero, luego Mongo | No | No |
| Reportar post | `wall::report_post` | INSERT report + UPDATE post | INSERT wall_event | MySQL primero, luego Mongo | No | Deduplicación por usuario |
| Moderar post | `wall::moderate_post` | UPDATE post | INSERT wall_event | MySQL primero, luego Mongo | No | No |
| Gasto admin | `admin::create_expense` | INSERT expense | No | MySQL | No | No |
| Toggle modelo | `admin::toggle_model` | UPDATE model | No | MySQL | No | No |

---

## 4. Matriz de fallos parciales

| Escenario | Resultado actual | Riesgo |
|---|---|---|
| MySQL exitoso / MongoDB exitoso | Operación completa | Normal |
| MySQL exitoso / MongoDB fallido | La operación principal puede devolver 500, pero la escritura MySQL ya quedó persistida | Alto para auditoría |
| MySQL fallido / MongoDB exitoso | No aplica a operaciones actuales: Mongo se escribe después de MySQL | Bajo |
| Timeout durante escritura | Resultado incierto; el cliente puede reintentar | Alto |
| Reintento de publicación con misma clave | No duplica gracias a idempotency key | Controlado |
| Reintento de reporte | No duplica por índice único | Controlado |
| Reintento de chat | Puede duplicar mensajes si se repite con el mismo `conversation_id` | Medio |
| Caída del servicio durante operación | El estado final depende de qué escritura se completó | Alto |

---

## 5. Auditoría de registros de auditoría

Eventos observados en MongoDB:

- `auth_events`: recuperación de contraseña.
- `ai_metrics`: solicitudes de chat.
- `wall_events`: creación, edición, eliminación, reporte y moderación.
- `payment_events`: no se genera en el flujo actual porque la verificación de pago está deshabilitada.

Calidad de los eventos:

- Incluyen tipo de evento.
- Incluyen `user_id` o actor cuando corresponde.
- Incluyen identificador del recurso cuando corresponde.
- Incluyen timestamp.
- Incluyen motivo en reportes y moderación.
- No incluyen contraseñas ni tokens.

Debilidades:

- La escritura de auditoría se realiza después de la escritura principal.
- Si Mongo falla, la operación principal puede devolver 500 aunque MySQL ya haya cambiado.
- En `create_post`, `update_post`, `delete_post` y `report_post`, la escritura Mongo se ejecuta antes de `tx.commit()` en algunos casos, pero no existe una transacción distribuida.
- En `chat`, los errores de persistencia ahora se propagan, pero el cliente puede reintentar y duplicar mensajes.
- No hay un identificador de correlación compartido entre MySQL y MongoDB.
- No hay reconciliación periódica.

---

## 6. Idempotencia

| Mecanismo | Estado |
|---|---|
| Clave de idempotencia por usuario en publicaciones | Implementada |
| Índice único `(user_id, idempotency_key)` | Implementado, best-effort |
| Deduplicación de reportes por `(post_id, user_id)` | Implementada |
| Clave de idempotencia en pagos | No implementada; pagos pendientes deshabilitados |
| Idempotencia de chat | No implementada |
| Clave de correlación entre MySQL y Mongo | No implementada |

La idempotencia actual es parcial y se limita al muro.

---

## 7. Riesgos encontrados

### CRÍTICO
Ninguno confirmado como pérdida de datos transaccionales críticos. No se observó pérdida de usuarios, pagos o publicaciones por fallo parcial en el código actual.

### ALTO
- **A-5.11-01: Auditoría puede quedar ausente tras una escritura MySQL exitosa.**  
  Operaciones: publicaciones, edición, eliminación, reportes, moderación.  
  Impacto: pérdida de trazabilidad.  
  Recomendación: Transactional Outbox.

- **A-5.11-02: Eventos huérfanos posibles si una escritura Mongo se completa y la principal falla después.**  
  Impacto actual: bajo porque Mongo se escribe después de MySQL en la mayoría de los casos, pero posible en reintentos o cambios futuros.  
  Recomendación: outbox + procesamiento asíncrono.

- **A-5.11-03: Reintentos de chat pueden duplicar mensajes.**  
  Impacto: historial inconsistente.  
  Recomendación: clave de idempotencia por mensaje o reconstrucción desde Mongo.

### MEDIO
- **M-5.11-01: No hay reconciliación periódica entre MySQL y MongoDB.**
- **M-5.11-02: No hay identificador de correlación compartido.**
- **M-5.11-03: Los índices únicos son best-effort y pueden fallar con duplicados legacy.**
- **M-5.11-04: Los errores de auditoría se propagan, pero no existe cola de reintento.**

### BAJO
- **B-5.11-01: La auditoría de `ai_metrics` puede fallar y hacer fallar la respuesta de chat.**
- **B-5.11-02: `wall_events` se escribe dentro de transacciones SQL, pero Mongo no participa en la transacción.**

---

## 8. Estrategia de recuperación recomendada

Se recomienda implementar **Transactional Outbox** en MySQL.

### Flujo recomendado

1. La operación principal se ejecuta dentro de una transacción MySQL.
2. En la misma transacción, se inserta un evento en una tabla `outbox_events`.
3. La transacción se confirma una sola vez.
4. Un procesador asíncrono dentro del mismo backend lee eventos pendientes.
5. El procesador inserta el evento en MongoDB.
6. El evento se marca como procesado.
7. Si Mongo falla, el evento permanece pendiente y se reintenta.
8. Los eventos procesados se conservan por un periodo de auditoría.

### Ventajas

- Fuente de verdad única: MySQL.
- No se pierde auditoría por fallo de Mongo.
- Reintentos controlados.
- Posibilidad de reconciliación.
- Compatible con Axum y SQLx.
- No requiere Redis ni Kafka en esta etapa.

### Tabla propuesta

```sql
CREATE TABLE IF NOT EXISTS outbox_events (
    id VARCHAR(36) PRIMARY KEY,
    aggregate_type VARCHAR(64) NOT NULL,
    aggregate_id VARCHAR(64) NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    payload JSON NOT NULL,
    status ENUM('pending','processed','failed') NOT NULL DEFAULT 'pending',
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at DATETIME NULL,
    KEY idx_status_created (status, created_at)
);
```

Esta tabla es una propuesta, no una migración ejecutada.

### Política de reintentos

- Reintentar con backoff exponencial.
- Máximo de intentos configurable.
- Marcar como `failed` tras superar el límite.
- Registrar el último error sin exponer datos sensibles.
- No eliminar eventos procesados inmediatamente.

### Recuperación

- Al arrancar, el procesador busca eventos `pending`.
- Si un evento ya fue procesado en Mongo, se puede marcar como procesado por idempotencia.
- Los eventos `failed` requieren revisión manual o reintento administrativo.
- No se deben eliminar eventos históricos.

---

## 9. Alternativas evaluadas

| Alternativa | Evaluación |
|---|---|
| Transactional Outbox | Recomendada para la etapa actual |
| Procesamiento asíncrono directo | Insuficiente sin persistencia de intención |
| Reintentos en handler | No garantiza recuperación tras reinicio |
| Kafka/RabbitMQ | Sobreingeniería para la etapa actual |
| Transacción distribuida 2PC | Complejidad y soporte limitado |

---

## 10. Plan de implementación por etapas

### Etapa 1: diseño
- Definir tipos de eventos.
- Definir payload mínimo.
- Definir política de reintentos.

### Etapa 2: outbox en MySQL
- Crear tabla aditiva.
- Insertar eventos en la misma transacción principal.
- No modificar eventos históricos.

### Etapa 3: procesador asíncrono
- Tarea Tokio de fondo.
- Lectura de eventos pendientes.
- Inserción en MongoDB.
- Actualización de estado.

### Etapa 4: idempotencia
- Usar `event id` como identificador único.
- Evitar duplicados en Mongo.
- Añadir índice único en Mongo cuando sea seguro.

### Etapa 5: observabilidad
- Métricas de eventos pendientes.
- Logs sin datos sensibles.
- Alertas por eventos fallidos.

### Etapa 6: reconciliación
- Comparación periódica entre MySQL y Mongo.
- Reporte de eventos pendientes o fallidos.

---

## 11. Pruebas necesarias

- Fallo de Mongo después de commit MySQL.
- Reinicio del backend con eventos pendientes.
- Reintento de evento duplicado.
- Evento fallido tras máximo de intentos.
- Concurrencia de procesadores.
- Ausencia de duplicación de eventos.
- Recuperación de eventos legacy.

Estas pruebas no se implementaron en esta tarea.

---

## 12. Limitaciones de la auditoría

- No se ejecutaron pruebas de fallo real.
- No se modificó ningún dato.
- No se inspeccionó el contenido actual de MongoDB.
- No se validó la carga ni el rendimiento.
- No se implementó Transactional Outbox.
- No se evaluó la compatibilidad con múltiples instancias.

---

## 13. Conclusión

La arquitectura actual es funcional, pero la consistencia entre MySQL y MongoDB depende del orden de escritura y del manejo de errores. El Transactional Outbox es la alternativa más adecuada para la etapa actual de Opaline AI.

No se realizaron cambios en esta tarea. La implementación debe autorizarse por separado.

---

# Recomendación para la Tarea 5.12

Implementar Transactional Outbox de forma incremental, comenzando por los eventos del muro y la autenticación, con pruebas de fallo controladas y sin modificar datos existentes.