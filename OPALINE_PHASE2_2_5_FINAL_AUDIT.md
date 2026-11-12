# OPALINE_PHASE2_2_5_FINAL_AUDIT.md

## FASE 2.2.5 — Auditoría final de idempotencia y recuperación extremo a extremo

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, configuraciones, bases de datos ni dependencias.

### 1. Resumen ejecutivo

La implementación de FASE 2.2.4 mejora la idempotencia en los clientes Flutter y PHP. Ambos generan y conservan una clave por operación lógica durante fallos de transporte y la reenvían al backend.

El backend Rust ya disponía de un mecanismo sólido de recuperación basado en `chat_operations`, Outbox y restricciones únicas. La auditoría confirma que el flujo principal permite recuperar una operación completada sin duplicar inferencia ni consumo.

Sin embargo, la recuperación no es completamente demostrada en los clientes: las claves no sobreviven a cierres o recargas, no existe un endpoint de consulta de operación y no hay pruebas automatizadas para varios escenarios de fallo distribuido.

Dictamen final: **APROBADO CON RESERVAS**.

### 2. Estado inicial del repositorio

- Cambios locales antes de la auditoría: 118.
- Cambios locales después de la auditoría: 118.
- No se modificaron archivos de código.

### 3. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 4. Auditoría de idempotencia en Flutter

Archivo: `opaline_ai_mobile/lib/providers/chat_provider.dart`

Evidencia:

- Línea 24: `final Map<String, String> _idempotencyKeys`.
- Líneas 194-198: generación de `operationKey` y `idempotencyKey`.
- Línea 223: envío de `idempotency_key`.
- Línea 237: eliminación de la clave tras recibir respuesta.

Clasificación:

- Generación de clave: RESUELTO.
- Conservación durante fallos de transporte: RESUELTO.
- Conservación ante respuesta HTTP: PARCIALMENTE RESUELTO. La clave se elimina tras cualquier respuesta HTTP, incluidos errores temporales.
- Persistencia tras cierre de aplicación: NO RESUELTO. El mapa es solo en memoria.
- Recuperación de operación pendiente: NO RESUELTO. No existe consulta de operación ni almacenamiento local durable.
- Prevención de mensajes duplicados en reintentos: PARCIALMENTE RESUELTO. Un reintento agrega nuevamente el mensaje del usuario y la respuesta.
- Asociación con usuario autenticado: RESUELTO a nivel de backend. El cliente no valida explícitamente la propiedad.

### 5. Auditoría de idempotencia en PHP

Archivos:

- `Opaline_AI_Web_Prototipo/frontend/chat.php`
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`

Evidencia:

- `chat.php`: variables `pendingIdempotencyKey` y `pendingOperationText`.
- `chat.php`: la clave se genera con `crypto.randomUUID()` o fallback.
- `chat.php`: se envía `idempotency_key` en `apiProxy`.
- `chat.php`: la clave se elimina si el backend responde.
- `chat.php`: la clave se conserva si el error no tiene `status`.
- `api_proxy.php`: reenvía `idempotency_key` al backend.

Clasificación:

- Generación de clave: RESUELTO.
- Conservación durante fallos de transporte: RESUELTO.
- Conservación tras recarga de página: NO RESUELTO. El estado está en memoria JavaScript.
- Recuperación de operación pendiente: NO RESUELTO. No existe consulta de operación.
- Prevención de mensajes duplicados: PARCIALMENTE RESUELTO. Un reintento agrega mensajes nuevos.
- Seguridad: ADECUADA. El proxy mantiene CSRF y autorización en Rust.

### 6. Auditoría del contrato Rust

Archivo: `opaline_rust_backend/src/routes/chat.rs`

Evidencia:

- Acepta `idempotency_key` en el cuerpo o mediante `X-Idempotency-Key`.
- Llama a `chat_operations::find_by_idempotency`.
- Si la operación está `inferred`, `persisting` o `persisted`, devuelve el resultado original.
- Si está `failed`, devuelve `409 Conflict`.
- Crea `chat_operations` tras reservar cuota.
- Devuelve `idempotency_key` en la respuesta.

Clasificación:

- Generación y validación: RESUELTO.
- Asociación con usuario: RESUELTO.
- Persistencia de operación: RESUELTO.
- Recuperación de resultado: RESUELTO.
- Prevención de inferencia duplicada: RESUELTO.
- Prevención de consumo duplicado: RESUELTO.
- Endpoint de consulta de operación: NO RESUELTO. No existe ruta pública para consultar una operación por clave o identificador.

Estados HTTP observados:

- `200 OK`: operación completada o recuperada.
- `400 Bad Request`: mensaje inválido.
- `401 Unauthorized`: sin token.
- `403 Forbidden`: sin suscripción válida.
- `409 Conflict`: operación fallida o clave reutilizada con parámetros incompatibles.
- `429 Too Many Requests`: cuota agotada.
- `500 Internal Server Error`: error interno.
- `503 Service Unavailable`: proveedor no disponible.

No se implementaron `202 Accepted`, `422`, `502`.

### 7. Recuperación de respuestas perdidas

Flujo observado:

1. Cliente envía mensaje con clave.
2. Rust reserva cuota.
3. Rust crea operación durable.
4. Rust ejecuta inferencia.
5. Rust confirma consumo.
6. Rust actualiza operación.
7. Rust intenta persistir en MongoDB.
8. Si falla, encola evento Outbox.
9. Rust responde con la clave.
10. Si la respuesta se pierde, el cliente reintenta con la misma clave.
11. Rust recupera la operación y devuelve el resultado original.

Conclusión: el backend soporta recuperación de respuestas perdidas si el cliente conserva la clave.

Limitación: si el cliente pierde la clave por recarga o cierre, no puede recuperar la operación porque no existe un endpoint de consulta.

### 8. Pruebas de fallos distribuidos

Pruebas automatizadas ejecutadas:

| Prueba | Resultado |
|---|---|
| `cargo test` | Correcto |
| `cargo test --lib` | 16 correctas |
| `cargo test -- --ignored` | Correcto |
| `cargo test --test quota_integration -- --ignored` | 4 correctas |
| `cargo test --test plans_integration -- --ignored` | 9 correctas |
| `cargo test --test chat_operations_integration -- --ignored` | 2 correctas |
| `php -l` | Correcto |
| `flutter analyze` | 27 avisos informativos, sin errores |
| `git diff --check` | Correcto |

Escenarios cubiertos por pruebas:

- reserva, liberación y consulta;
- idempotencia después de confirmación;
- concurrencia respetando límite;
- persistencia idempotente de operaciones;
- reconciliación de operaciones pendientes.

Escenarios no cubiertos por pruebas automatizadas:

- MongoDB desconectado después de inferencia exitosa.
- Pérdida de confirmación del evento Outbox.
- Reinicio del backend con operación pendiente.
- Dos workers compitiendo por una operación.
- Proceso antiguo intentando confirmar después de fencing.
- Reserva ambigua que debe pasar a revisión manual.
- Contador desincronizado.
- Registro de consumo duplicado.
- Métrica desconocida.
- Clave de idempotencia repetida con parámetros diferentes.
- Respuesta HTTP perdida después de completar una operación.
- Usuario intentando recuperar una operación ajena.
- Recarga de Flutter con operación pendiente.
- Recarga de PHP con operación pendiente.

Clasificación: brecha de cobertura de pruebas.

### 9. Auditoría de workers y recuperación automática

Configuración observada:

- `OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED=1`: activa el consumidor de persistencia.
- `OPALINE_QUOTA_RECOVERY_ENABLED=1`: activa la recuperación de cuotas.
- Ambos están desactivados por defecto.

El consumidor de persistencia procesa eventos `chat.operation.persist` de forma idempotente.

El worker de recuperación ejecuta `reap_expired_reservations` y `reconcile_operations` cada 60 segundos.

Fencing:

- `recovery_owner` y `recovery_lease_expires_at` protegen reservas expiradas.
- Los claims de Outbox utilizan `claim_token`.

No se ejecutaron workers durante la auditoría.

### 10. Observabilidad

Los logs existentes permiten correlacionar:

- Acciones de proxy.
- Errores de worker.
- Eventos Outbox.
- Recuperación de reservas.

No se registran contraseñas, tokens Bearer, cookies ni contenido sensible.

No se identificó un endpoint o métrica para contar operaciones pendientes o su antigüedad.

### 11. Hallazgos

| ID | Severidad | Archivo / Función | Hallazgo | Estado |
|---|---|---|---|---|
| F2.2.5-01 | MEDIO | `chat_provider.dart` | La clave no sobrevive a cierre o recarga de la aplicación | NO RESUELTO |
| F2.2.5-02 | MEDIO | `chat.php` | La clave no sobrevive a recarga de página | NO RESUELTO |
| F2.2.5-03 | MEDIO | Backend Rust | No existe endpoint de consulta de operación | NO RESUELTO |
| F2.2.5-04 | BAJO | `chat_provider.dart` | Un reintento agrega mensajes duplicados en la interfaz | PARCIALMENTE RESUELTO |
| F2.2.5-05 | BAJO | `chat.php` | Un reintento agrega mensajes duplicados en la interfaz | PARCIALMENTE RESUELTO |
| F2.2.5-06 | BAJO | Tests | Cobertura insuficiente para fallos distribuidos | NO RESUELTO |

No se identificaron hallazgos críticos ni altos.

### 12. Matriz final

| Área | Estado | Evidencia | Riesgo |
|---|---|---|---|
| Idempotencia Flutter | Parcial | Mapa en memoria | Pérdida al cerrar app |
| Persistencia Flutter | No implementada | Sin almacenamiento local durable | Recuperación incompleta |
| Idempotencia PHP | Parcial | Variables en memoria | Pérdida al recargar página |
| Persistencia PHP | No implementada | Sin almacenamiento durable | Recuperación incompleta |
| Proxy HTTP | Adecuado | Reenvío de clave | Bajo |
| Contrato Rust | Implementado | `find_by_idempotency` | No |
| Recuperación de operaciones | Implementada | `chat_operations` | Configuración |
| Prevención de inferencias duplicadas | Implementada | Restricción única | No |
| Prevención de consumo duplicado | Implementada | Restricción única | No |
| Persistencia MongoDB | Implementada | Outbox + claims | Configuración |
| Workers | Documentados | Variables de entorno | Desactivados por defecto |
| Fencing | Implementado | Lease + claim token | No |
| Observabilidad | Parcial | Logs estructurados | Sin métricas de operaciones pendientes |
| Pruebas distribuidas | Insuficientes | Pruebas unitarias e integración | Cobertura |

### 13. Dictamen final

**APROBADO CON RESERVAS.**

El sistema está técnicamente preparado para continuar hacia pagos reales desde el punto de vista del control de consumo. La idempotencia del backend es sólida y recuperable.

Sin embargo, la recuperación end-to-end en los clientes no está completamente demostrada porque las claves no sobreviven a cierres o recargas y no existe un endpoint de consulta de operación.

### 14. Respuestas al dictamen

1. ¿El consumo se registra sin duplicaciones? Sí.
2. ¿Una inferencia completada puede recuperarse después de un fallo? Sí, en el backend.
3. ¿MongoDB puede recuperarse sin duplicar mensajes? Sí.
4. ¿Outbox puede procesar eventos repetidos de forma segura? Sí.
5. ¿Las reservas abandonadas se recuperan correctamente? Sí, cuando el worker está activo.
6. ¿La reconciliación detecta inconsistencias? Sí.
7. ¿Los contadores son verificables? Sí.
8. ¿La medición de tokens es honesta y trazable? Sí.
9. ¿PHP y Flutter conservan la idempotencia? Parcialmente.
10. ¿El sistema está preparado técnicamente para cobrar por consumo? Sí, con reservas operativas.

### 15. Recomendación

Antes de FASE 2.3:

- Añadir almacenamiento local durable de la clave en Flutter y PHP, o documentar que la recuperación depende de mantener la página o aplicación abierta.
- Añadir un endpoint de consulta de operación o documentar que la recuperación se realiza mediante reintento con la misma clave.
- Ampliar la cobertura de pruebas para fallos distribuidos.

### 16. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales preexistentes se conservaron.