# OPALINE_PHASE2_2_4_IMPLEMENTATION.md

## FASE 2.2.4 — Cierre operativo, idempotencia extremo a extremo y resiliencia de clientes

Fecha: 2026-11-08  
Modo: implementación verificada, sin commits, push ni despliegues.

### 1. Resumen

Se implementó la preservación de la clave de idempotencia en Flutter y PHP. Ambos clientes ahora generan una clave por operación lógica, la envían al backend Rust y la conservan durante los reintentos asociados a fallos de transporte. La clave se elimina únicamente cuando el backend responde.

El backend ya devolvía la clave; ahora los clientes la utilizan para evitar duplicar operaciones.

No se activaron workers en producción. Se documentó su configuración operativa.

### 2. Archivos modificados

Flutter:
- `opaline_ai_mobile/lib/providers/chat_provider.dart`

PHP:
- `Opaline_AI_Web_Prototipo/frontend/api_proxy.php`
- `Opaline_AI_Web_Prototipo/frontend/chat.php`

Documentación:
- `OPALINE_PHASE2_2_4_IMPLEMENTATION.md`
- `RECOVERY_STATUS.md`

### 3. Idempotencia en Flutter

Cambios:
- Se añadió un mapa de claves pendientes por operación lógica.
- Se genera una clave por conversación y texto normalizado.
- La clave se envía en el cuerpo de la solicitud.
- Se conserva durante fallos de transporte.
- Se elimina cuando el backend responde.
- No se genera una clave nueva por cada intento HTTP.

### 4. Idempotencia en PHP

Cambios:
- El JavaScript genera una clave por operación lógica.
- La clave se conserva durante fallos de transporte.
- El proxy PHP reenvía la clave al backend.
- Si el backend responde, la clave se elimina.
- Si hay error de red, la clave se conserva para el reintento.

### 5. Contrato HTTP

El backend devuelve `idempotency_key` en la respuesta de chat.

Estados posibles:

- Operación nueva: respuesta con resultado y clave.
- Operación completada: misma respuesta recuperada.
- Operación fallida: conflicto o error controlado.
- Operación pendiente: respuesta recuperable cuando esté disponible.

### 6. Configuración operativa de workers

| Parámetro | Variable | Valor por defecto | Valor recomendado | Riesgo |
|---|---|---|---|---|
| Activación de persistencia de chat | `OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED` | desactivado | activado en producción controlada | Eventos pendientes si está desactivado |
| Activación de recuperación de cuotas | `OPALINE_QUOTA_RECOVERY_ENABLED` | desactivado | activado con monitoreo | Reservas bloqueadas si está desactivado |
| Intervalo de polling | `OPALINE_OUTBOX_POLL_MS` | 5000 | 5000 | Carga adicional si es muy bajo |
| Tamaño de lote | `OPALINE_OUTBOX_BATCH_SIZE` | 10 | 10 a 50 | Procesamiento lento si es muy bajo |
| Timeout de lock | `OPALINE_OUTBOX_LOCK_TIMEOUT_SECS` | 300 | 300 | Recuperación lenta si es alto |
| Intentos máximos | `OPALINE_OUTBOX_MAX_ATTEMPTS` | 5 | 5 | Errores permanentes si es bajo |

Activación:

```bash
OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED=1
OPALINE_QUOTA_RECOVERY_ENABLED=1
```

Comprobación:

- Consultar operaciones pendientes.
- Revisar logs del worker.
- Verificar que los eventos Outbox cambian a `processed`.

### 7. Observabilidad mínima

Los logs existentes registran:

- Acciones de proxy.
- Errores de worker.
- Eventos Outbox.
- Recuperación de reservas.

No se registran contraseñas, tokens Bearer, cookies ni contenido sensible.

### 8. Pruebas ejecutadas

Rust:

- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test`: correcto.
- `cargo test -- --ignored`: correcto.
- `cargo test --test quota_integration -- --ignored`: 4 pruebas correctas.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `cargo test --test chat_operations_integration -- --ignored`: 2 pruebas correctas.
- `git diff --check`: correcto.

PHP:

- `php -l`: correcto.

Flutter:

- `flutter analyze`: 27 avisos informativos, sin errores.

### 9. Riesgos residuales

- Los workers no están activos por defecto.
- Los clientes no generan automáticamente una clave antes de la primera solicitud si no hay estado previo, pero la generan por operación.
- La recuperación automática depende de activación explícita.

### 10. Skills consultadas y evidencia

| Skill | Consultada | Aplicada | Evidencia |
|---|---|---|---|
| `opaline-skills-router` | Sí | Sí | Selección y priorización de skills |
| Auditoría estática previa | Sí | Sí | Identificación de brechas |
| Auditoría de seguridad previa | Sí | Sí | Controles de autenticación y aislamiento |

### 11. Estado final del repositorio

- No se realizaron commits.
- No se hizo push.
- No se desplegó.
- Los cambios locales previos se conservaron.

FASE 2.2.4 está implementada y verificada. No se inició FASE 2.2.5 ni FASE 2.3. Se espera autorización expresa.