# OUTBOX_RECOVERY_5_16.md

## Recuperación controlada de discrepancias críticas MySQL–MongoDB

### 1. Objetivo

Implementar recuperación controlada para discrepancias críticas detectadas por el reconciliador de solo lectura.

La recuperación es explícita, limitada, idempotente y auditable. No convierte automáticamente cualquier discrepancia en éxito.

### 2. Arquitectura

Módulo: `opaline_rust_backend/src/outbox_recovery.rs`

Componentes:

- `OutboxRecovery`: ejecuta recuperaciones controladas.
- `RecoveryConfig`: configuración de límites y umbrales.
- `RecoveryReport`: resultado estructurado.
- `RecoverySkip`: motivo de no recuperación.
- `spawn_recovery`: inicio controlado en Tokio.
- `claim_for_recovery`: reclamación segura y con fencing.

### 3. Política implementada

| Discrepancia | Acción |
|---|---|
| `pending` con documento MongoDB | Recuperación controlada |
| `processing` vencido con documento | Recuperación controlada |
| `processing` vencido sin documento | Reintento controlado |
| `failed` con documento y causa recuperable | Recuperación controlada |
| `failed` sin documento | Revisión manual |
| `processed` sin documento | Revisión manual |
| Documento MongoDB sin evento Outbox | Clasificación manual |

### 4. Mecanismo de fencing

La reclamación genera un `claim_token` único.

Las transiciones validan:

- Estado actual.
- Expiración del bloqueo.
- Límite de intentos.
- Causa recuperable para eventos `failed`.

Las actualizaciones incluyen `claim_token` para evitar que un worker antiguo sobrescriba una reclamación más reciente.

### 5. Trazabilidad

Cada recuperación registra:

- `event_id`.
- Estado inicial.
- Estado final.
- Motivo.
- Resultado.
- Número de intentos.
- Error, cuando existe.

No se registran payloads completos ni información sensible.

### 6. Activación

Variable:

`OPALINE_WALL_AUDIT_RECOVERY_ENABLED=1`

El módulo permanece desactivado por defecto.

Antes de iniciar:

1. Verifica MySQL.
2. Verifica MongoDB.
3. Verifica el índice único de `event_id`.
4. Si falla una dependencia, no procesa eventos.
5. No bloquea el arranque HTTP.

### 7. Pruebas

Se añadió `tests/outbox_recovery_integration.rs`.

Casos verificados:

- `pending` con documento: recuperado.
- `processing` vencido con documento: recuperado.
- `processing` vencido sin documento: recuperado.
- bloqueo activo: rechazado.
- `failed` con causa recuperable y documento: recuperado.
- `failed` sin documento: revisión manual.
- `processed` sin documento: revisión manual.
- documento MongoDB sin evento Outbox: clasificado.
- límite por ciclo.
- concurrencia de recuperación.
- idempotencia de documentos.

Resultados:

- 10 pruebas passed.
- `cargo check`: passed.
- `git diff --check`: passed.

### 8. Riesgos residuales

- No hay recuperación automática para `failed` sin documento.
- No hay reconciliación automática de documentos huérfanos.
- No se validó el comportamiento bajo carga real.
- La garantía sigue siendo al menos una vez, no exactamente una vez.