# OPALINE_PHASE2_1_1_CORRECTION.md

## FASE 2.1.1 — Corrección de integridad, concurrencia y validación de suscripciones

Fecha: 2026-11-08  
Modo: corrección técnica acotada, sin commits, push ni despliegues.

### 1. Resumen

Se corrigió la restricción de suscripción vigente única por usuario. La restricción anterior, `UNIQUE (user_id, is_current)`, no expresaba correctamente la regla comercial y podía impedir múltiples suscripciones históricas con un mismo valor no nulo en `is_current`.

La solución aplicada utiliza una columna generada nullable `current_user_id`, cuyo valor es `user_id` únicamente cuando `is_current = 1` y `NULL` en cualquier otro caso. La unicidad se aplica sobre `current_user_id`, permitiendo múltiples filas históricas con `NULL`.

### 2. Entorno verificado

- Motor: MariaDB 13.0.2, compatible con MySQL.
- Tipo real de `subscriptions.user_id`: `VARCHAR(36)`.
- Tipo de la columna generada: `VARCHAR(36)`.
- La columna generada es `STORED`.
- La unicidad se implementa mediante `UNIQUE KEY uq_subscriptions_current_user (current_user_id)`.

### 3. Problema original

La restricción:

```sql
UNIQUE KEY uq_subscriptions_user_current (user_id, is_current)
```

no garantizaba una única suscripción vigente de forma semántica. Además, cualquier valor no nulo repetido en `is_current`, por ejemplo `0`, impedía múltiples suscripciones históricas para el mismo usuario.

### 4. Solución aplicada

Columna generada:

```sql
current_user_id VARCHAR(36)
GENERATED ALWAYS AS (
    CASE
        WHEN is_current = 1 THEN user_id
        ELSE NULL
    END
) STORED
```

Restricción:

```sql
UNIQUE KEY uq_subscriptions_current_user (current_user_id)
```

La restricción anterior se elimina de forma segura cuando existe. La nueva restricción permite:

- una suscripción vigente por usuario;
- múltiples suscripciones históricas;
- ausencia de suscripción vigente;
- suscripciones vigentes simultáneas para usuarios distintos.

### 5. Migraciones realizadas

Archivos:

- `opaline_rust_backend/migrations.sql`
- `opaline_rust_backend/src/db.rs`

Cambios:

- Se añadió `current_user_id` al esquema base de `subscriptions`.
- Se añadió la restricción única sobre `current_user_id`.
- `ensure_commercial_schema` crea la columna generada si no existe.
- `ensure_commercial_schema` elimina el índice anterior si existe.
- `ensure_commercial_schema` crea la nueva restricción si no existe.
- No se eliminaron registros.
- No se borraron tablas.

### 6. Integridad transaccional y concurrencia

Operaciones revisadas:

- Creación de suscripción pendiente.
- Activación de suscripción.
- Cancelación de renovación.
- Restauración de renovación.
- Vencimiento al consultar.
- Cambio de plan mediante versionado.

Las operaciones utilizan transacciones SQL. La activación captura el error de unicidad de MySQL y lo convierte en `409 Conflict`.

No se confía únicamente en una consulta previa. MySQL actúa como barrera final de integridad.

### 7. Pruebas ejecutadas

Comandos:

```bash
cargo check
cargo test --lib
cargo test -- --ignored
cargo test --test plans_integration -- --ignored
git diff --check
```

Resultados:

- `cargo check`: correcto.
- `cargo test --lib`: 16 pruebas correctas.
- `cargo test -- --ignored`: correcto.
- `cargo test --test plans_integration -- --ignored`: 9 pruebas correctas.
- `git diff --check`: correcto.

Pruebas específicas de suscripciones:

- listado de planes;
- suscripción inexistente;
- cancelación y restauración;
- vencimiento;
- concurrencia de activación;
- aislamiento por propietario;
- historial de suscripciones;
- múltiples suscripciones históricas coexisten;
- MySQL rechaza dos suscripciones vigentes del mismo usuario.

### 8. Revisión de archivos fuera del alcance principal

#### `src/routes/admin.rs`

Cambios atribuibles a esta fase:

- Ruta administrativa `POST /api/v1/admin/planes/:id/versions`.
- `require_admin` se hizo público para ser utilizado por el código de moderación ya presente en el árbol de trabajo.

Justificación:

- El endpoint permite administrar versiones de planes.
- La visibilidad pública de `require_admin` evita duplicar la autorización administrativa.

#### `src/routes/wall.rs`

Cambios atribuibles a esta fase:

- La elegibilidad de publicación y el plan mostrado en el muro incluyen suscripciones con estado `cancelled` mientras su periodo no haya vencido.

Justificación:

- La cancelación de renovación no debe retirar el acceso anticipadamente.
- El muro debe seguir mostrando el plan del usuario hasta `end_date`.

El resto de las diferencias en `wall.rs` corresponde a trabajo previo no introducido por esta corrección y se conservó sin modificarlo.

### 9. Riesgos pendientes

- PayPal real no está integrado.
- Las cuotas no se aplican.
- El consumo de tokens no se registra.
- No hay renovación automática mediante proveedor.
- No hay verificación real de correo.
- No hay monitoreo operativo.

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

### 12. Punto de parada

FASE 2.1.1 está implementada y verificada. No se inició FASE 2.2. Se espera autorización expresa antes de implementar cuotas, metering o pagos reales.