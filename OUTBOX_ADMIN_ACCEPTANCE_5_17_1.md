# OUTBOX_ADMIN_ACCEPTANCE_5_17_1.md

## Auditoría de aceptación y cierre de brechas del servicio administrativo Outbox

### 1. Estado original encontrado

La implementación 5.17 ya contaba con:

- Endpoints administrativos protegidos.
- Consulta de discrepancias.
- Inspección de eventos.
- Historial persistente.
- Idempotencia por administrador y clave.
- Rate limiting.
- Fencing mediante `claim_token`.

Se identificaron las siguientes brechas:

- `failed` sin documento MongoDB no era recuperable, aunque la tarea 5.17 lo contemplaba como acción permitida.
- `audit_without_outbox` se calculaba sobre las discrepancias de la página actual, no sobre todos los eventos MySQL de la página.
- El rate limiting usaba `X-Forwarded-For` directamente.
- Faltaban pruebas para failed sin documento, processed sin documento, documento huérfano, validación de parámetros e idempotencia con parámetros distintos.

### 2. Hallazgos confirmados

1. `failed` sin documento no era elegible para recuperación.
2. `audit_without_outbox` podía incluir falsos positivos cuando existían eventos consistentes en la misma página.
3. El rate limiting por IP dependía de un header controlado por el cliente.
4. Faltaban casos de prueba obligatorios de aceptación.

### 3. Brechas corregidas

- Se permitió la recuperación de eventos `failed` sin documento cuando la causa es recuperable y los intentos no han alcanzado el máximo.
- Se corrigió `audit_without_outbox` para comparar contra todos los eventos MySQL de la página.
- Se cambió el rate limiting para usar la IP del peer de conexión.
- Se añadieron 11 pruebas de integración.
- Se corrigió el historial para registrar el administrador autenticado.

### 4. Brechas justificadamente no corregidas

- La paginación avanzada y filtros complejos no se implementaron; no eran bloqueantes para la aceptación.
- No se añadió interfaz visual; está fuera del alcance de esta tarea.
- No se activó el worker por defecto; la activación sigue siendo explícita.
- No se implementó una cola administrativa en PHP.

### 5. Archivos modificados

- `opaline_rust_backend/src/routes/outbox_admin.rs`
- `opaline_rust_backend/src/outbox_recovery.rs`
- `opaline_rust_backend/tests/outbox_admin_integration.rs`
- `RECOVERY_STATUS.md`
- `OUTBOX_ADMIN_ACCEPTANCE_5_17_1.md`

### 6. Migraciones

No se requirieron nuevas migraciones. Se reutilizó la tabla `outbox_admin_actions` creada en la Tarea 5.17.

### 7. Modelo de autorización

- Autenticación Bearer validada por Rust.
- Rol administrativo consultado en MySQL.
- Identidad administrativa obtenida del token, no del cliente.
- Rate limiting por peer IP y usuario.
- Acciones mutativas protegidas por CSRF en el frontend, cuando aplica.

### 8. Modelo de historial

Cada intervención persiste:

- Evento afectado.
- Administrador autenticado.
- Tipo de acción.
- Motivo.
- Estado anterior y resultante.
- Resultado.
- Clave de idempotencia.
- Correlación.
- Fechas.

### 9. Paginación y filtros

- Paginación obligatoria con límite máximo de 50.
- Orden determinista por `created_at ASC, id ASC`.
- Filtros por estado y tipo de evento.
- No se devuelven payloads completos.

### 10. Idempotencia y concurrencia

- La misma clave con los mismos parámetros devuelve el resultado original.
- La misma clave con parámetros diferentes devuelve `409`.
- Dos administradores no pueden ejecutar intervenciones incompatibles sobre el mismo evento.
- El fencing impide confirmaciones con claims inválidos.
- Las intervenciones concurrentes no generan registros inconsistentes.

### 11. Resultados de pruebas

- `cargo test --lib`: 16 passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 11 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 12. Skills consultadas y aplicadas

| Skill | Consultada | Aplicada | Aplicación concreta |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección de skills y prioridades |
| `impeccable` | Sí | No | No aplicable a esta tarea de backend |
| Auditoría estática previa | Sí | Sí | Identificación de brechas y validación de permisos |

### 13. Riesgos residuales

- No hay interfaz visual para el servicio administrativo.
- No hay paginación avanzada ni filtros complejos.
- La recuperación automática sigue limitada a casos elegibles.
- No se implementó una cola administrativa en PHP.
- La garantía sigue siendo al menos una vez, no exactamente una vez.

### 14. Dictamen

**Aprobado con reservas.**

La implementación cumple los requisitos críticos de autorización, idempotencia, concurrencia y auditoría. Las reservas corresponden a la ausencia de interfaz visual y a la limitación de recuperación automática.