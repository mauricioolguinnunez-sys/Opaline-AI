# OUTBOX_HTTP_RESILIENCE_5_18_2.md

## Resiliencia HTTP y tolerancia a fallos PHP–Rust

### 1. Objetivo

Fortalecer la comunicación entre el panel administrativo PHP y el backend Rust para evitar que interrupciones de red, timeouts o respuestas inválidas se interpreten como operaciones exitosas.

### 2. Cambios implementados

#### Backend Rust
No se modificó Rust. Se conservaron los contratos existentes.

#### Frontend PHP
- `config.php`: timeouts explícitos y validación de respuestas JSON.
- `api_proxy.php`: registro seguro de duración y acción, sin tokens ni secretos.
- `moderacion_outbox.php`: persistencia de la clave de idempotencia en `sessionStorage` y en la sesión PHP.
- `factura.php`: compatibilidad con respuestas no JSON.

### 3. Política de timeouts

- Conexión: 5 segundos.
- Operaciones administrativas Outbox: 30 segundos.
- Operaciones generales: 120 segundos por defecto.

### 4. Manejo de errores

- Respuesta no JSON o `Content-Type` incorrecto: 502.
- HTTP 4xx/5xx: se propaga el código real.
- Timeout o conexión fallida: estado 0 y mensaje controlado.
- JSON malformado: nunca se interpreta como éxito.

### 5. Idempotencia

- La clave se genera una sola vez por operación lógica.
- Se conserva en `sessionStorage` y en la sesión PHP.
- Un reintento reutiliza la misma clave.
- Una operación fallida no limpia la clave automáticamente.
- Un reintento no genera una segunda intervención lógica.

### 6. Observabilidad

Los logs registran:

- Acción.
- Código HTTP.
- Duración en milisegundos.

No se registran tokens, cookies, contraseñas, secretos ni payloads completos.

### 7. Pruebas

Suite:

`Opaline_AI_Web_Prototipo/tests/outbox_http_resilience_test.php`

Casos verificados:

- Conexión exitosa.
- JSON válido.
- JSON malformado.
- Respuesta vacía.
- HTML inesperado.
- HTTP 500.
- Timeout.
- Proxy malformado.
- Proxy HTML.
- Proxy HTTP 500.
- Proxy echo válido.
- Conservación de clave tras fallo.
- Reutilización de clave en reintento.

Resultados:

- 16 assertions passed.
- `php -l`: passed.
- `git diff --check`: passed.

### 8. Riesgos residuales

- No hay pruebas para backend desconectado a nivel de proxy sin mock.
- No hay pruebas para respuestas truncadas.
- La validación se realizó contra el entorno local de desarrollo.
- PayPal real sigue sin implementarse.