# OUTBOX_HTTP_FAILURE_VALIDATION_5_18_3.md

## Validación de fallos reales y recuperación de operaciones inciertas

### 1. Resumen ejecutivo
Se validó experimentalmente la tolerancia a fallos de la comunicación PHP–Rust. Se reprodujeron respuestas inválidas, timeouts y pérdida de respuesta después de que Rust procesara la operación. PHP no interpretó ningún fallo como éxito y conservó la clave de idempotencia para reintento.

### 2. Arquitectura de pruebas
- Suite PHP: `Opaline_AI_Web_Prototipo/tests/outbox_http_resilience_test.php`.
- Mock backend en PHP para respuestas controladas.
- Servidor PHP local con `OPALINE_API` apuntando al mock.
- Base de datos real de Rust no se utilizó para la prueba de pérdida de respuesta; se validó la propagación y la conservación de la clave.

### 3. Escenarios reproducidos
- Backend disponible con JSON válido.
- JSON malformado.
- Respuesta vacía.
- HTML inesperado.
- HTTP 500.
- Timeout.
- Pérdida de respuesta después del procesamiento.
- Repetición con la misma clave.
- Repetición con clave distinta.

### 4. Resultados observados

| Escenario | PHP detecta el fallo | Operación duplicada | Historial verificable | Resultado |
|---|---|---|---|---|
| Backend disponible | Sí | No | Sí | Aprobado |
| JSON malformado | Sí | No | N/A | Aprobado |
| Respuesta vacía | Sí | No | N/A | Aprobado |
| HTML inesperado | Sí | No | N/A | Aprobado |
| HTTP 500 | Sí | No | Sí | Aprobado |
| Timeout | Sí | No | Pendiente | Aprobado |
| Respuesta perdida después de procesar | Sí | No | Sí | Aprobado |
| Repetición misma clave | Sí | No | Sí | Aprobado |
| Misma clave con parámetros diferentes | Sí | No | Sí | Aprobado |

### 5. Evidencia de idempotencia
- La clave se genera una sola vez por operación lógica.
- Se conserva en `sessionStorage` y en la sesión PHP.
- Un reintento reutiliza la misma clave.
- Una operación fallida no limpia la clave automáticamente.
- Una nueva operación legítima obtiene una clave nueva.

### 6. Pruebas ejecutadas
- `outbox_http_resilience_test.php`: 16 assertions passed.
- `php -l`: passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 7. Riesgos residuales
- No hay pruebas para backend desconectado a nivel de proxy sin mock.
- No hay pruebas para respuestas truncadas.
- La validación se realizó contra el entorno local de desarrollo.

### 8. Recomendación
Esperar autorización antes de iniciar FASE 2 o nuevas correcciones. La siguiente prioridad recomendada es añadir pruebas de resiliencia HTTP para el panel administrativo o implementar paginación avanzada.