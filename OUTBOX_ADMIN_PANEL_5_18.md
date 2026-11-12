# OUTBOX_ADMIN_PANEL_5_18.md

## Panel administrativo de auditoría y recuperación Outbox

### 1. Objetivo

Implementar la interfaz web de supervisión y operación del sistema Transactional Outbox, consumiendo los endpoints administrativos existentes de Rust. PHP actúa únicamente como capa de presentación y comunicación autenticada.

### 2. Archivos nuevos

- `Opaline_AI_Web_Prototipo/frontend/moderacion_outbox.php`

### 3. Archivos modificados

- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/panel.php`
- `RECOVERY_STATUS.md`

### 4. Rutas PHP

- `frontend/moderacion_outbox.php`: panel de auditoría Outbox.
- `frontend/api_proxy.php`: acciones `outbox_discrepancies`, `outbox_event`, `outbox_history` y `outbox_action`.

### 5. Endpoints Rust utilizados

- `GET /api/v1/admin/outbox/discrepancies`
- `GET /api/v1/admin/outbox/events/:id`
- `GET /api/v1/admin/outbox/events/:id/history`
- `POST /api/v1/admin/outbox/events/:id/actions`

### 6. Modelo de autenticación

- PHP valida la sesión.
- PHP verifica que el usuario tenga rol `admin`.
- El Bearer token se transmite únicamente servidor-servidor desde PHP hacia Rust.
- Rust valida el token y el rol administrativo.
- No se expone el Bearer al navegador.

### 7. Protección CSRF

- Todas las operaciones mutables usan `X-CSRF-Token`.
- El token se obtiene de una meta etiqueta generada por PHP.
- PHP valida el token antes de ejecutar la mutación.
- Rust conserva la autoridad sobre la operación.

### 8. Funcionalidad del panel

- Dashboard con contadores de discrepancias.
- Filtros por estado y tipo de evento.
- Búsqueda por ID.
- Paginación.
- Detalle del evento.
- Historial administrativo.
- Acciones `recover`, `mark_manual_review` y `record_investigation`.
- Confirmación explícita antes de ejecutar acciones.
- Manejo de errores HTTP.

### 9. Seguridad

- PHP no duplica reglas de negocio.
- Rust conserva la autoridad sobre autenticación, autorización, elegibilidad, idempotencia y concurrencia.
- No se exponen payloads completos.
- No se exponen tokens ni secretos.
- No se modificó Flutter.

### 10. Pruebas

- `php -l`: passed en todos los archivos PHP.
- `cargo test`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.
- Verificación manual con cuenta administradora: passed.

### 11. Riesgos residuales

- La cola no tiene paginación avanzada ni filtros por antigüedad.
- No hay pruebas automatizadas PHP para la interfaz.
- El historial de auditoría se registra en MongoDB, pero no se muestra en la interfaz web.
- PayPal real sigue sin implementarse.

### 12. Skills consultadas y aplicadas

| Skill | Consultada | Aplicada | Aplicación concreta |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección de skills y prioridades |
| Auditoría previa de PHP | Sí | Sí | Reutilización del proxy y CSRF |
| Auditoría previa de Rust | Sí | Sí | Consumo de endpoints administrativos reales |

### 13. Recomendación para la siguiente etapa

Esperar autorización antes de iniciar FASE 2 o nuevas correcciones. La siguiente prioridad recomendada es añadir pruebas automatizadas PHP para el panel de auditoría o implementar paginación avanzada.