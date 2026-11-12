# OUTBOX_ADMIN_TESTING_5_18_1.md

## Auditoría y pruebas automatizadas del panel administrativo Outbox

### 1. Origen real del historial

El historial administrativo se obtiene de MySQL, tabla `outbox_admin_actions`, mediante:

`GET /api/v1/admin/outbox/events/:id/history`

No se confunde con la auditoría técnica de MongoDB en `wall_events`.

### 2. Infraestructura de pruebas

- Suite PHP: `Opaline_AI_Web_Prototipo/tests/outbox_admin_panel_test.php`.
- Herramientas: PHP CLI, cURL, servidor PHP local y backend Rust local.
- No se introdujeron dependencias externas.

### 3. Casos automatizados

- Acceso sin sesión.
- Acceso como usuario normal.
- Acceso como administrador.
- CSRF ausente.
- CSRF inválido.
- CSRF válido.
- Discrepancias como administrador.
- Discrepancias sin sesión.
- Discrepancias sin CSRF.
- Discrepancias como usuario normal.
- Detalle de evento.
- Historial de evento.
- ID de evento inválido.
- Acción administrativa inválida.
- JSON malformado.
- Acción arbitraria en el proxy.

### 4. Resultados

- `php -l`: passed.
- `outbox_admin_panel_test.php`: 22 assertions passed.
- `cargo test --test outbox_admin_integration -- --ignored`: 11 passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 5. Seguridad verificada

- El Bearer no se expone al navegador.
- CSRF protege las mutaciones.
- PHP no acepta roles arbitrarios.
- El proxy mantiene allowlist de acciones.
- Rust conserva la autoridad administrativa.

### 6. Riesgos residuales

- No hay pruebas PHP para timeouts o backend desconectado.
- No hay pruebas PHP para respuestas HTML inesperadas.
- La validación se realizó contra el entorno local de desarrollo.