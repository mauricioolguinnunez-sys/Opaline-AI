# SECURITY_AUDIT_5_5.md

## Auditoría integral de autorización, aislamiento multiusuario y control de acceso

Fecha de auditoría: 2026-11-07  
Alcance: solo lectura. No se modificaron archivos, bases de datos, servicios ni configuraciones.  
Único archivo creado: `SECURITY_AUDIT_5_5.md`.

---

## 1. Resumen ejecutivo

Opaline AI ha recibido endurecimiento significativo en autenticación, sesiones, pagos, conversaciones y facturas. En el estado actual, los recursos privados principales están protegidos por identidad autenticada en Rust y, en el caso de facturas, por propiedad del recurso.

Se identificaron controles correctos en Rust para historial de pagos, suscripciones, conversaciones, muro y facturas. También se identificaron riesgos residuales de nivel medio y bajo, principalmente CSRF en el proxy PHP, inicialización prematura de providers en Flutter, deduplicación de reportes del muro y rate limiting por proceso.

No se confirmó un IDOR crítico en los recursos ya protegidos. El principal IDOR histórico de facturas fue corregido en la Tarea 5.4 y verificado mediante pruebas de integración.

---

## 2. Estado general de seguridad

| Área | Estado |
|---|---|
| Autenticación Rust | Implementada y endurecida |
| Hash de tokens | Implementado |
| Revocación de sesiones | Implementada |
| CORS | Allowlist configurable |
| Rate limiting | Por proceso, por IP y cuenta |
| Pagos | Solo estados pendientes; verificación/reembolso deshabilitados sin proveedor real |
| Conversaciones | Aisladas por `user_id` |
| Pagos/facturas | Aislados por `user_id` |
| Muro | CRUD por propietario; reportes sin deduplicación |
| PHP | Sesión web + proxy; falta CSRF en mutaciones GET |
| Flutter | Auth fail-closed; almacenamiento seguro; SessionGuard sin desbloqueo ficticio |
| Pruebas | Unitarias e integración MySQL para auth, pagos y facturas |

---

## 3. Inventario de endpoints

### Públicos
- `GET /health`
- `GET /`
- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/forgot-password`
- `GET /api/v1/planes`
- `GET /api/v1/ai/models`
- `GET /api/v1/ai/status`
- `GET /api/v1/muro/`

### Autenticados
- `POST /api/v1/auth/logout`
- `POST /api/v1/auth/change-password`
- `GET /api/v1/auth/sessions`
- `GET /api/v1/suscripcion/`
- `POST /api/v1/suscripcion/cancelar-renovacion`
- `POST /api/v1/suscripcion/restaurar`
- `GET /api/v1/payments/history`
- `POST /api/v1/payments/paypal/create`
- `GET /api/v1/payments/paypal/verify/:order_id`
- `GET /api/v1/facturas/:id/pdf`
- `POST /api/v1/facturas/:id/reembolso`
- `GET /api/v1/ai/conversations`
- `POST /api/v1/ai/chat`
- `DELETE /api/v1/ai/conversations/:id`
- `POST /api/v1/muro/`
- `PUT /api/v1/muro/:id`
- `DELETE /api/v1/muro/:id`
- `POST /api/v1/muro/:id/reportar`

### Administrativos
- `GET /api/v1/admin/users`
- `GET /api/v1/admin/metrics`
- `GET /api/v1/admin/ai-stats`
- `PUT /api/v1/admin/models/:id/toggle`
- `GET /api/v1/admin/expenses`
- `POST /api/v1/admin/expenses`
- `GET /api/v1/admin/reports/financial`

### No implementados o deshabilitados de forma segura
- `GET /api/v1/payments/paypal/verify/:order_id`: devuelve `501`.
- `POST /api/v1/facturas/:id/reembolso`: devuelve `501`.
- `POST /api/v1/auth/forgot-password`: no envía correo real; responde genéricamente.

---

## 4. Matriz de autenticación y autorización

| Recurso | Autenticación | Autorización | Aislamiento |
|---|---|---|---|
| Conversaciones | Bearer | Propietario | `user_id` en todas las operaciones |
| Pagos | Bearer | Propietario | `user_id` en historial y creación |
| Facturas | Bearer | Propietario | `user_id` en consulta SQL |
| Suscripciones | Bearer | Propietario | `user_id` en consulta y mutación |
| Muro | Bearer para mutaciones | Propietario para edición/borrado | `user_id` en UPDATE/DELETE |
| Administración | Bearer | Rol admin consultado en MySQL | Global por diseño |
| Muro listado | Público | N/A | Solo posts aprobados |

---

## 5. Auditoría SQL

### Controles correctos
- `payments.history`: filtra por `user_id`.
- `suscripcion`: filtra por `user_id`.
- `wall.update_post` y `delete_post`: incluyen `user_id`.
- `chat.delete_conversation`: incluye `user_id`.
- `invoice_pdf`: ahora incluye `p.user_id = ?`.
- `create_order`: usa `uid` obtenido del token, no del cliente.

### Consultas globales legítimas
- `admin.list_users`: global por diseño, protegido por `require_admin`.
- `admin.metrics`: global por diseño.
- `admin.expenses`: global por diseño.
- `admin.financial_report`: global por diseño.

### Riesgos o deuda
- `admin.toggle_model`: actualiza por `id OR code`, sin verificar que el recurso exista; puede devolver éxito aunque no modifique filas.
- `wall.report_post`: no deduplica reportes por usuario.
- `wall.create_post`: no valida rango de `rating` en Rust.
- `admin.create_expense`: no valida importes, categorías ni monedas en Rust.
- `cancel_renewal` y `restore`: actualizan todas las filas del usuario con estados seleccionados; la semántica es frágil pero no es IDOR.

---

## 6. Auditoría MongoDB

### Controles correctos
- `list_conversations`: filtra por `user_id`.
- `chat`: `find_one`, `update_one` e `insert_one` usan filtro compuesto `{"id": ..., "user_id": ...}`.
- `delete_conversation`: filtra por `id` y `user_id`.
- `ai_metrics`: incluye `user_id`.

### Riesgos o deuda
- `find_one` seguido de `insert_one` puede crear duplicados para el mismo usuario bajo concurrencia extrema; no permite acceso cruzado porque el filtro compuesto incluye `user_id`.
- El índice único `(user_id, id)` es best-effort y puede fallar si existen duplicados legacy.
- `ai_metrics` y `auth_events` no tienen aislamiento administrativo por usuario, pero son datos de auditoría globales por diseño.

---

## 7. Auditoría PHP

### Controles correctos
- `api_proxy.php` utiliza allowlist de acciones.
- Los tokens se almacenan en sesión PHP, no en JavaScript.
- `factura.php` transmite el Bearer internamente y no lo expone.
- El historial de pagos se obtiene mediante el proxy autenticado.

### Riesgos o deuda
- No hay tokens CSRF en formularios ni validación de `Origin`/`Referer`.
- Algunas acciones mutantes del proxy pueden ejecutarse mediante GET porque `action` se acepta desde query string.
- `logout.php` usa GET y es susceptible a CSRF de logout.
- Las cookies PHP usan `SameSite=Lax`; no hay `Secure` en HTTP local.
- No hay CSP ni cabeceras de seguridad adicionales.
- No hay expiración absoluta de sesión PHP; el refresh indefinido extiende su vida útil efectiva.

---

## 8. Auditoría Flutter

### Controles correctos
- Login y registro son fail-closed.
- Los tokens se almacenan con `flutter_secure_storage`.
- `SessionGuard.unlock()` no concede acceso automáticamente.
- Logout limpia datos privados de providers.
- El chat incluye Bearer mediante `ApiClient`.

### Riesgos o deuda
- `PlanProvider` se inicializa antes de autenticar y puede mostrar estados demo si el backend responde de forma inesperada.
- `SessionGuard` restaura localmente una sesión existente sin validarla contra el backend.
- El desbloqueo biométrico no está implementado.
- No hay pruebas de integración para auth, logout y limpieza de datos.

---

## 9. Pruebas realizadas

### Backend
- `cargo test`: passed.
- `cargo test -- --ignored`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### Integración MySQL
- Autenticación: tokens almacenados como hash.
- Refresh normal.
- Concurrencia de refresh.
- Replay después de la ventana de gracia.
- Refresh expirado.
- Logout y persistencia.
- Cambio de contraseña.
- Persistencia entre conexiones.
- CORS: 4 pruebas passed.
- Seguridad de pagos: 7 pruebas passed, incluyendo aislamiento de facturas.

### PHP
- `php -l`: passed en todos los archivos PHP.
- Flujo manual de factura con sesión: passed.
- Flujo sin sesión: redirige a login.

### Flutter
- `flutter analyze`: 27 hallazgos, todos nivel `info`.
- `flutter test`: 1 test passed.
- `flutter build linux`: passed.

---

## 10. Hallazgos clasificados

### CRÍTICO
Ninguno confirmado en el estado actual para los recursos ya protegidos.

### ALTO
- **A-01: CSRF en mutaciones del proxy PHP.**  
  Archivo: `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`  
  Evidencia: acciones mutantes aceptan `action` por query string y no validan CSRF.  
  Escenario: una navegación top-level GET cross-site puede ejecutar acciones autenticadas.  
  Impacto: modificación no autorizada de suscripción o contraseña.  
  Corrección recomendada: exigir POST, token CSRF por sesión y validación de Origin.  
  Prueba necesaria: petición cross-origin sin token CSRF.

- **A-02: Facturas consultables por ID sin autenticación en Rust antes de Tarea 5.4.**  
  Estado: corregido en Tarea 5.4 y verificado.  
  Evidencia: `invoice_pdf` ahora exige Bearer y filtra por `user_id`.  
  Escenario histórico: consulta directa por UUID.  
  Impacto: IDOR de facturas.  
  Corrección aplicada: autenticación y propiedad en SQL.  
  Prueba verificada: `invoice_access_is_isolated_by_owner`.

### MEDIO
- **M-01: Inicialización prematura de PlanProvider en Flutter.**  
  Archivo: `opaline_ai_mobile/lib/main.dart`, `lib/providers/plan_provider.dart`  
  Evidencia: se inicializa antes de autenticar y puede mostrar estados demo.  
  Impacto: datos inconsistentes o demo en una sesión real.  
  Corrección recomendada: inicializar solo tras autenticación y limpiar al logout.  
  Prueba necesaria: login real tras instalación nueva.

- **M-02: SessionGuard restaura sesión local sin validar backend.**  
  Archivo: `opaline_ai_mobile/lib/core/services/session_guard.dart`  
  Evidencia: `_authenticated` se deriva de `lastAuthTime`, no del token.  
  Impacto: estado local puede no coincidir con el backend.  
  Corrección recomendada: validar la sesión en `init()` o marcarla como pendiente de validación.  
  Prueba necesaria: token inválido en almacenamiento.

- **M-03: Reportes del muro no deduplicados.**  
  Archivo: `opaline_rust_backend/src/routes/wall.rs`  
  Evidencia: cada petición inserta un reporte; no hay clave única `(post_id, user_id)`.  
  Impacto: un usuario puede ocultar publicaciones con autorreportes.  
  Corrección recomendada: deduplicar por usuario y post.  
  Prueba necesaria: tres reportes del mismo usuario.

- **M-04: Validación incompleta de rating y reportes.**  
  Archivo: `opaline_rust_backend/src/routes/wall.rs`  
  Evidencia: `rating` acepta cualquier `i32`; `reason` y `detail` no se validan.  
  Impacto: datos inválidos y posible error en PHP.  
  Corrección recomendada: validar rangos y longitudes.  
  Prueba necesaria: rating fuera de 1–5.

- **M-05: Validación administrativa incompleta.**  
  Archivo: `opaline_rust_backend/src/routes/admin.rs`  
  Evidencia: importes negativos, categorías vacías y monedas arbitrarias se aceptan.  
  Impacto: métricas financieras incorrectas.  
  Corrección recomendada: validar en Rust.  
  Prueba necesaria: gasto con importe negativo.

### BAJO
- **B-01: Rate limiter por proceso.**  
  Archivo: `opaline_rust_backend/src/security.rs`  
  Evidencia: estructura en memoria.  
  Impacto: no coordina varias instancias.  
  Corrección recomendada: documentar limitación o añadir backend compartido solo si se escala.  
  Prueba necesaria: múltiples instancias.

- **B-02: Tokens SHA-256 sin pepper.**  
  Archivo: `opaline_rust_backend/src/routes/auth.rs`  
  Evidencia: hash SHA-256 directo.  
  Impacto: si la base se filtra, los tokens de alta entropía siguen siendo resistentes, pero un pepper añadiría defensa en profundidad.  
  Corrección recomendada: evaluar pepper en configuración futura.  
  Prueba necesaria: N/A.

- **B-03: Sesiones legacy inutilizadas.**  
  Archivo: `opaline_rust_backend/src/db.rs`  
  Evidencia: tokens en texto claro no coinciden con hash.  
  Impacto: sesiones antiguas quedan inutilizables.  
  Corrección recomendada: limpieza futura o expiración natural.  
  Prueba necesaria: N/A.

### INFORMATIVO
- Las consultas SQL utilizan parámetros vinculados; no se observó concatenación SQL con entrada de usuario.
- Las mutaciones de muro incluyen `user_id` en UPDATE y DELETE.
- El historial de pagos filtra por `user_id`.
- Las conversaciones se aíslan mediante filtro compuesto.
- Las facturas se protegen por Bearer y propiedad.
- Flutter usa almacenamiento nativo seguro.

---

## 11. Riesgos residuales

1. CSRF en el proxy PHP.
2. Inicialización prematura de providers en Flutter.
3. Reportes duplicados en el muro.
4. Validación incompleta de rating, reportes y gastos.
5. Rate limiter por proceso.
6. Facturas públicas por ID antes de Tarea 5.4; corregido.
7. PayPal real no implementado.
8. Ausencia de pruebas de integración para PHP y Flutter.

---

## 12. Dependencias pendientes

- Pruebas de integración PHP.
- Pruebas de integración Flutter.
- Pruebas de rate limiting distribuido.
- Pruebas de concurrencia de reportes.
- Pruebas de validación administrativa.
- Pruebas de IDOR con dos usuarios reales para todos los recursos.

---

## 13. Priorización de correcciones

1. CSRF en PHP.
2. Deduplicación y validación del muro.
3. Validación administrativa.
4. Inicialización de providers en Flutter.
5. Rate limiting distribuido solo si se escala horizontalmente.
6. Pepper para tokens solo si se requiere defensa adicional.

---

## 14. Recomendación de arquitectura de autorización uniforme

Mantener el patrón actual:

1. Extraer el Bearer en el backend.
2. Resolver el usuario autenticado desde la base de datos.
3. Filtrar todos los recursos privados por `user_id`.
4. No confiar en `user_id` enviado por el cliente.
5. Usar 401 para falta de autenticación.
6. Usar 404 para recursos inexistentes o ajenos, evitando revelar existencia.
7. Mantener endpoints administrativos protegidos por rol consultado en base de datos.
8. Mantener PHP como proxy que transmite el Bearer internamente.
9. Mantener Flutter con almacenamiento nativo y auth fail-closed.

---

## 15. Conclusión

No se confirmó un IDOR crítico en los recursos ya protegidos. Los principales riesgos residuales son CSRF en PHP, validación incompleta del muro y administración, e inicialización prematura de providers en Flutter. La base de seguridad del backend es sólida para continuar con FASE 2, siempre que se documenten y prioricen los riesgos residuales.

No se modificó ningún archivo durante esta auditoría, salvo la creación de este informe.


## Actualización Tarea 5.8 — Pruebas PHP del muro

Se implementó `Opaline_AI_Web_Prototipo/tests/wall_proxy_test.php` con 14 assertions.

Resultados verificados:
- Lectura pública: 200.
- Publicación sin sesión: 401.
- Login con CSRF válido: 302.
- Publicación sin CSRF: 419.
- Contenido inválido: 400.
- Publicación válida: 200.
- Idempotencia con la misma clave: mismo ID.
- Acción arbitraria en proxy: 400.
- Logout sin CSRF: 419.
- Logout con CSRF válido: 302.

El proxy mantiene allowlist de acciones y transmite la clave de idempotencia al backend. No se modificó Flutter. No se cambiaron las reglas de negocio de Rust.


## Actualización Tarea 5.9 — Pruebas avanzadas PHP–Rust del muro

Se implementó `Opaline_AI_Web_Prototipo/tests/wall_advanced_test.php` con 18 assertions.

Resultados verificados:
- Edición válida: 200.
- Edición con contenido inválido: 400.
- Edición sin CSRF: 419.
- Eliminación válida: 204.
- Eliminación repetida: 403.
- Reporte válido: 200.
- Reporte duplicado: 200 con `duplicate: true`.
- Reporte sin CSRF: 419.
- Reporte inexistente: 404.
- JSON malformado: 400.
- Moderación sin motivo: 400.
- Moderación con motivo: 200.

El proxy mantiene allowlist de acciones y transmite CSRF. No se modificó Flutter. No se cambiaron las reglas de negocio de Rust.
