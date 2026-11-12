# OPALINE_PHASE2_2_1_AUDIT.md

## FASE 2.2.1 — Auditoría de integridad, concurrencia y recuperación del motor de cuotas

Fecha: 2026-11-08  
Modo: auditoría de solo lectura. No se modificó código, migraciones, esquemas ni datos.

### 1. Resumen ejecutivo

El motor de cuotas implementado en FASE 2.2.0 tiene una base transaccional sólida para controlar consumo concurrente e idempotente. Las reservas se realizan con bloqueos de MySQL y las restricciones únicas evitan duplicados. El consumo real y los contadores se actualizan dentro de transacciones.

Sin embargo, se identificaron debilidades relevantes en recuperación automática, precisión de medición de tokens, reintentos fallidos y reconciliación. Por ello, el dictamen es **APROBADO CON RESERVAS**, no completamente aprobado para facturación exacta.

No se recomienda iniciar FASE 2.3 sin corregir o documentar explícitamente las reservas identificadas.

### 2. Arquitectura real

Componentes relevantes:

- `src/quota.rs`: motor de cuotas, reservas, confirmación, liberación, expiración y consultas.
- `src/routes/chat.rs`: integración del motor en el flujo de inferencia.
- `src/routes/usage.rs`: endpoints de consulta de consumo.
- `migrations.sql` y `src/db.rs`: tablas `usage_periods`, `usage_reservations`, `usage_records`.
- `tests/quota_integration.rs`: pruebas de cuotas.

La fuente de verdad del consumo es MySQL. MongoDB se utiliza únicamente para conversaciones y métricas auxiliares.

### 3. Auditoría de reservas

Estados implementados:

- `reserved`
- `committed`
- `released`
- `expired`
- `failed`

Transiciones observadas:

- `reserved -> committed`: confirmación exitosa.
- `reserved -> released`: liberación normal.
- `reserved -> failed`: error de inferencia o persistencia.
- `reserved -> expired`: recuperación de reservas expiradas.

No existe transición de `committed` a `released`, lo cual es correcto.

Hallazgos:

- `F2.2.1-01` — MEDIO: `reap_expired_reservations` no se invoca automáticamente. Las reservas abandonadas permanecen hasta una invocación manual.
- `F2.2.1-02` — MEDIO: una reserva expirada puede liberarse mientras la inferencia todavía esté en ejecución si supera los 300 segundos. El timeout de LM Studio es de 180 segundos, por lo que el riesgo es bajo en condiciones normales, pero no está formalmente protegido por fencing.
- `F2.2.1-03` — MEDIO: no existe un mecanismo de reconciliación implementado para reconstruir contadores desde `usage_records`.

### 4. Auditoría de concurrencia

Evidencia de código:

- `reserve_quota` inicia transacción SQL.
- Bloquea `usage_periods` con `FOR UPDATE`.
- Valida límites antes de actualizar contadores.
- Actualiza contadores con SQL atómico.
- Inserta la reserva con restricciones únicas.

Escenario de última unidad de cuota:

- Dos solicitudes concurrentes se serializan por el bloqueo de fila.
- La primera reserva incrementa `reserved_requests`.
- La segunda observa la cuota agotada y devuelve `429 Too Many Requests`.

Conclusión: la concurrencia está protegida por MySQL, no por memoria local.

Pruebas:

- `quota_concurrency_respects_limit` pasó.
- La prueba demuestra que solo una de dos solicitudes concurrentes obtiene la reserva.

### 5. Auditoría de idempotencia

Evidencia de código:

- `usage_reservations` tiene `UNIQUE (user_id, idempotency_key)`.
- `usage_reservations` tiene `UNIQUE (user_id, request_id)`.
- `usage_records` tiene `UNIQUE (user_id, request_id)`.
- `commit_reservation` es idempotente si la reserva ya está `committed`.
- Reintentos de confirmación no incrementan contadores dos veces.

Hallazgos:

- `F2.2.1-04` — MEDIO: si una reserva está en estado `reserved`, `failed` o `expired`, una nueva solicitud con la misma clave devuelve `409 Conflict`. No se permite reintentar automáticamente la misma operación fallida.
- `F2.2.1-05` — MEDIO: la clave de idempotencia no se devuelve en la respuesta HTTP. Si la respuesta se pierde, el cliente no puede reintentar con la misma clave, salvo que la haya enviado explícitamente.
- `F2.2.1-06` — BAJO: dos solicitudes concurrentes con la misma clave pueden provocar que una obtenga `409 Conflict` en lugar del resultado original si ambas llegan antes de que la primera se confirme.

### 6. Auditoría de tokens

Evidencia de código:

- LM Studio se consulta mediante `/v1/chat/completions`.
- Si la respuesta incluye `usage`, se utilizan `prompt_tokens` y `completion_tokens`.
- Si no incluye `usage`, se estiman tokens mediante caracteres divididos por cuatro.
- `token_source` y `measurement_status` se registran.

Hallazgos:

- `F2.2.1-07` — MEDIO: si el objeto `usage` existe pero no contiene `prompt_tokens` o `completion_tokens`, el sistema utiliza estimaciones pero registra `token_source = "provider"` y `measurement_status = "measured"`. Esto puede presentar una estimación como medición real.
- `F2.2.1-08` — BAJO: el modelo registrado es el modelo solicitado, no necesariamente el modelo reportado por el proveedor en la respuesta.
- `F2.2.1-09` — BAJO: `total_tokens` se calcula como suma de entrada y salida, no se utiliza un campo `total_tokens` del proveedor si existiera.

### 7. Auditoría de errores de inferencia

Comportamiento observado:

- Error de red: reserva liberada.
- Respuesta HTTP no exitosa: reserva liberada.
- Error al parsear JSON: reserva liberada.
- Respuesta vacía: reserva liberada.
- Error al confirmar consumo: reserva liberada.
- Fallo al almacenar conversación en MongoDB: se devuelve error HTTP 500, aunque la cuota ya fue confirmada.

Hallazgos:

- `F2.2.1-10` — ALTO: existe una ventana en la que la inferencia se completa y la cuota se confirma, pero el almacenamiento de la conversación falla. El usuario recibe error HTTP 500 y, si no reutiliza la misma clave de idempotencia, puede consumir nuevamente.
- `F2.2.1-11` — MEDIO: no se registra consumo parcial cuando el proveedor devuelve una respuesta parcial o se interrumpe la generación.

### 8. Auditoría de contadores y reconciliación

Estructuras:

- `usage_periods`: contadores agregados por periodo.
- `usage_reservations`: reservas activas e históricas.
- `usage_records`: consumo confirmado.
- `usage_counters`: tabla existente, no utilizada por el nuevo motor.

Hallazgos:

- `F2.2.1-12` — MEDIO: no existe reconciliación implementada entre `usage_periods` y `usage_records`.
- `F2.2.1-13` — BAJO: `usage_counters` permanece sin uso, lo que puede generar confusión operativa.
- `F2.2.1-14` — BAJO: la restricción de `usage_periods` es `UNIQUE (user_id, period_start)`, pero no incluye `period_end`. Esto permitiría periodos superpuestos con el mismo inicio.

### 9. Auditoría de recuperación

Evidencia:

- `reap_expired_reservations` existe.
- No se invoca automáticamente en `main.rs`.
- No existe worker activo por defecto.
- No existe fencing adicional más allá del estado de la reserva.

Hallazgos:

- `F2.2.1-15` — MEDIO: las reservas abandonadas no se recuperan automáticamente.
- `F2.2.1-16` — MEDIO: no existe un mecanismo de reconciliación para operaciones cuya inferencia pudo completarse pero cuyo consumo no quedó registrado.

### 10. Auditoría de seguridad

Controles observados:

- Todos los endpoints de chat y uso requieren Bearer token.
- Los datos se filtran por `user_id`.
- Los límites proceden del plan contratado.
- El cliente no puede modificar límites.
- Las claves de idempotencia se asocian al usuario.

Controles adecuados:

- Aislamiento por usuario.
- Autorización en endpoints de uso.
- Validación de suscripción.
- Restricciones únicas en MySQL.

No se identificaron vulnerabilidades críticas de acceso cruzado.

### 11. Pruebas ejecutadas

| Comando | Resultado |
|---|---|
| `cargo check` | Correcto |
| `cargo test` | Correcto |
| `cargo test --lib` | 16 pruebas correctas |
| `cargo test -- --ignored` | Correcto |
| `cargo test --test plans_integration -- --ignored` | 9 pruebas correctas |
| `cargo test --test quota_integration -- --ignored` | 4 pruebas correctas |
| `git diff --check` | Correcto |
| `cargo fmt --check` | No ejecutado: `rustfmt` no instalado |

Pruebas de cuotas existentes:

- usuario sin suscripción;
- reserva, liberación y consulta;
- idempotencia después de confirmación;
- concurrencia respetando límite.

No se agregaron pruebas durante la auditoría.

### 12. Hallazgos clasificados

| ID | Severidad | Archivo / Función | Hallazgo |
|---|---|---|---|
| F2.2.1-01 | MEDIO | `quota.rs / reap_expired_reservations` | No se invoca automáticamente |
| F2.2.1-02 | MEDIO | `quota.rs / reap_expired_reservations` | Posible liberación de reserva aún en proceso si supera 300 segundos |
| F2.2.1-03 | MEDIO | `quota.rs` | No existe reconciliación implementada |
| F2.2.1-04 | MEDIO | `quota.rs / reserve_quota` | Reintentos fallidos con misma clave devuelven 409 |
| F2.2.1-05 | MEDIO | `chat.rs / chat` | La clave de idempotencia no se devuelve al cliente |
| F2.2.1-06 | BAJO | `quota.rs / reserve_quota` | Concurrencia con misma clave puede producir 409 |
| F2.2.1-07 | MEDIO | `chat.rs / chat` | Estimaciones pueden registrarse como medición real |
| F2.2.1-08 | BAJO | `chat.rs / chat` | Se registra el modelo solicitado, no necesariamente el ejecutado |
| F2.2.1-09 | BAJO | `chat.rs / chat` | `total_tokens` se calcula, no se usa campo del proveedor |
| F2.2.1-10 | ALTO | `chat.rs / chat` | Fallo de persistencia en MongoDB después de confirmar cuota |
| F2.2.1-11 | MEDIO | `chat.rs / chat` | No se registra consumo parcial |
| F2.2.1-12 | MEDIO | `quota.rs` | No existe reconciliación entre periodos y registros |
| F2.2.1-13 | BAJO | `db.rs` | `usage_counters` permanece sin uso |
| F2.2.1-14 | BAJO | `migrations.sql` | `usage_periods` no incluye `period_end` en unicidad |
| F2.2.1-15 | MEDIO | `main.rs` | No hay recuperación automática |
| F2.2.1-16 | MEDIO | `quota.rs` | No hay reconciliación de inferencias completadas sin consumo |

### 13. Riesgos no resueltos

- La exactitud de tokens depende del proveedor y de estimaciones.
- No existe recuperación automática de reservas.
- No existe reconciliación de contadores.
- Un fallo de MongoDB después de confirmar cuota puede generar consumo sin respuesta visible.
- No se registra consumo parcial.

### 14. Recomendaciones

Corregir antes de FASE 2.3:

1. Devolver la clave de idempotencia al cliente.
2. Corregir el registro de `token_source` y `measurement_status` cuando faltan campos de `usage`.
3. Evitar que un fallo de MongoDB después de confirmar cuota produzca una experiencia inconsistente.
4. Implementar recuperación automática controlada o documentar su ausencia operativa.
5. Implementar reconciliación entre `usage_periods` y `usage_records`.

Mejoras posteriores:

- Eliminar o documentar `usage_counters`.
- Incluir `period_end` en la restricción de `usage_periods`.
- Registrar consumo parcial cuando sea posible.

### 15. Dictamen

**APROBADO CON RESERVAS.**

El motor de cuotas es técnicamente suficiente para controlar consumo concurrente e idempotente en el entorno actual. Sin embargo, no está completamente preparado para facturación exacta debido a las limitaciones de recuperación, reconciliación y precisión de medición.

La integración de pagos reales puede planificarse, pero debe considerar que el consumo medido puede requerir ajustes posteriores.

### 16. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 17. Estado final del repositorio

- No se modificó código.
- No se modificaron migraciones.
- No se modificaron datos.
- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales previos se conservaron.