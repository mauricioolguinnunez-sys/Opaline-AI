# Opaline AI — Web PHP

Frontend web de Opaline AI. Consume la API Rust en `http://127.0.0.1:8090/api/v1`
y comparte cuenta, suscripción, chat y muro con la app Flutter.

## Ejecutar

Desde esta carpeta:

```bash
php -S localhost:8000
```

Abre **http://localhost:8000/**. La raíz redirige a `frontend/index.php`.

> En este entorno el servidor PHP escucha en IPv6 (`::1`). Usa `localhost` en
> lugar de `127.0.0.1`.

El backend Rust debe estar activo. Variables opcionales:

```bash
OPALINE_API="http://127.0.0.1:8090/api/v1" php -S localhost:8000
```

## Páginas

| Archivo | Descripción |
| --- | --- |
| `frontend/index.php` | Landing, planes y llamadas a la acción |
| `frontend/planes.php` | Catálogo y comparación de planes |
| `frontend/login.php` | Inicio de sesión y acceso demo |
| `frontend/register.php` | Registro con auto-login |
| `frontend/chat.php` | Chat con LM Studio, historial y modelos |
| `frontend/muro.php` | Muro comunitario: publicar, calificar, reportar |
| `frontend/cuenta.php` | Suscripción, pagos, facturas y contraseña |
| `frontend/checkout.php` | Pago demo de PayPal |
| `frontend/panel.php` | Panel de administración |
| `frontend/logout.php` | Cierre de sesión |
| `frontend/api_proxy.php` | Proxy JSON autenticado hacia la API Rust |

## Sesión y seguridad

- `frontend/config.php` inicia la sesión PHP y centraliza las llamadas cURL.
- El access token dura 15 minutos; al recibir un `401` se renueva con el refresh
  token y se reintenta la petición automáticamente.
- Las acciones se procesan antes de incluir el encabezado HTML para evitar
  errores de “headers already sent”.
- El proxy solo acepta acciones conocidas y exige sesión salvo en las lecturas
  públicas.

## Cuentas

- Administrador: `admin@opaline-ai.com` / `admin123`
- Demo: `demo@opaline-ai.com` / `demo1234`

El flujo de PayPal es local/demostrativo: crea la orden, la verifica y activa la
suscripción sin cargos reales.
