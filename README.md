# Opaline AI

Plataforma de chat con IA open source que corre **100 % en local** con LM Studio.
La misma cuenta, suscripción, conversaciones y muro se comparten entre la web
(PHP) y la app móvil (Flutter), respaldadas por un backend en Rust.

## Arquitectura

```
┌────────────────────┐      ┌────────────────────┐
│  Web PHP           │      │  App Flutter       │
│  localhost:8000    │      │  Linux / Android   │
└─────────┬──────────┘      └─────────┬──────────┘
          │  cURL / HTTP              │  HTTP + tokens
          ▼                           ▼
┌─────────────────────────────────────────────────┐
│  Backend Rust + Axum · 127.0.0.1:8090/api/v1    │
│  auth · planes · suscripción · pagos · chat     │
│  muro · administración · métricas               │
└───────┬──────────────────────┬──────────────────┘
        │                      │
        ▼                      ▼
┌──────────────┐       ┌──────────────────┐
│ MariaDB 3307 │       │ MongoDB 27018    │
│ usuarios,    │       │ conversaciones,  │
│ planes,      │       │ métricas IA,     │
│ pagos        │       │ auditoría        │
└──────────────┘       └──────────────────┘
        │
        ▼
┌──────────────────────────┐
│ LM Studio · 127.0.0.1:1234│
│ Ministral 3B (por defecto)│
└──────────────────────────┘
```

## Estructura

- `opaline_rust_backend/` — API REST en Rust (Axum + SQLx + MongoDB).
- `Opaline_AI_Web_Prototipo/` — frontend web en PHP con sesiones y proxy a la API.
- `opaline_ai_mobile/` — app Flutter para Linux, Android y web.

## Requisitos

- Rust estable y `cargo`.
- PHP 8.1 o superior con la extensión `curl`.
- Flutter 3.4x o superior.
- MariaDB/MySQL escuchando en `127.0.0.1:3307` con la base `opaline`.
- MongoDB escuchando en `127.0.0.1:27018`.
- LM Studio con el servidor local activo en `http://127.0.0.1:1234`.
- Al menos un modelo descargado en LM Studio. El proyecto usa por defecto
  `mistralai/ministral-3-3b` por su buena relación velocidad/calidad.

## 1. Servicios locales de datos

En este equipo los datos viven bajo `~/.opaline/`. Ejemplo de arranque:

```bash
# MariaDB (puerto 3307)
mysqld --datadir="$HOME/.opaline/mysql" \
       --port=3307 \
       --socket="$HOME/.opaline/mysql.sock" \
       --bind-address=127.0.0.1 \
       --user="$(id -un)" &

# MongoDB (puerto 27018)
mongod --dbpath "$HOME/.opaline/mongodb" \
       --port 27018 --bind_ip 127.0.0.1 &
```

La base y el usuario se crean una sola vez:

```bash
mysql --socket="$HOME/.opaline/mysql.sock" -e "
  CREATE DATABASE IF NOT EXISTS opaline CHARACTER SET utf8mb4;
  CREATE USER IF NOT EXISTS 'opaline'@'127.0.0.1' IDENTIFIED BY 'opaline';
  GRANT ALL PRIVILEGES ON opaline.* TO 'opaline'@'127.0.0.1';
  FLUSH PRIVILEGES;"
```

Las tablas, planes, modelos, administrador y usuario demo se crean/actualizan
automáticamente al arrancar el backend (`src/db.rs`).

## 2. LM Studio

1. Abre LM Studio y descarga `mistralai/ministral-3-3b`, `qwen/qwen3.5-9b` o
   `google/gemma-4-e2b`.
2. Inicia el servidor local en el puerto **1234**.
3. Comprueba: `curl http://127.0.0.1:1234/v1/models`.

El backend expone el estado en `GET /api/v1/ai/status` y la lista de modelos
habilitados en `GET /api/v1/ai/models`.

## 3. Backend Rust

```bash
cd opaline_rust_backend
OPALINE_DEV_SEEDS=1 \
DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline" \
MONGO_URL="mongodb://127.0.0.1:27018" \
LMSTUDIO_URL="http://127.0.0.1:1234" \
cargo run

> `OPALINE_DEV_SEEDS=1` habilita únicamente cuentas demo y el usuario
> administrador de desarrollo. No lo uses en producción.
```

- API: `http://127.0.0.1:8090/api/v1`
- Salud: `http://127.0.0.1:8090/health`
- Raíz: redirige a la web PHP en `http://localhost:8000/frontend/index.php`.

## 4. Web PHP

El servidor PHP debe arrancarse desde la raíz de `Opaline_AI_Web_Prototipo`:

```bash
cd Opaline_AI_Web_Prototipo
php -S localhost:8000
```

Abre **http://localhost:8000/** (redirige a `frontend/index.php`).

> El servidor integrado de PHP en este entorno escucha en IPv6 (`::1`). Usa
> `localhost`, no `127.0.0.1`, si el puerto 8000 no responde.

La web guarda los tokens en la sesión PHP, renueva el access token de forma
transparente y llama al backend Rust a través de `frontend/api_proxy.php`.

## 5. App Flutter

```bash
cd opaline_ai_mobile
flutter pub get
flutter run -d linux        # escritorio Linux
flutter run -d android      # emulador Android (usa 10.0.2.2)
```

En modo debug la app apunta a `http://127.0.0.1:8090/api/v1` (Linux) o
`http://10.0.2.2:8090/api/v1` (emulador Android). En release usa
`https://www.opaline-ai.com/api/v1`.

Compilar:

```bash
flutter build linux
flutter build web
flutter build apk --debug
```

## Cuentas de prueba

| Rol | Correo | Contraseña |
| --- | --- | --- |
| Administrador | `admin@opaline-ai.com` | `admin123` |
| Cliente demo (Starter activo) | `demo@opaline-ai.com` | `demo1234` |

También puedes crear una cuenta desde `register.php` o desde la pantalla de
registro de la app. El registro inicia sesión automáticamente.

## Funcionalidad principal

- **Autenticación** con access/refresh tokens, renovación automática y sesiones.
- **Planes y suscripciones** compartidos entre web y móvil.
- **Pagos demo** con PayPal local: crear orden + verificar activa el plan y
  genera factura descargable.
- **Chat con LM Studio**: historial, selector de modelo, títulos automáticos y
  persistencia en MongoDB.
- **Muro comunitario**: publicar, calificar, reportar y eliminar publicaciones
  propias.
- **Administración**: métricas, usuarios, uso de IA, gastos y reporte financiero.
- **Diseño Opaline**: Poppins, lavanda `#F1EAFC`, púrpura `#9369E5`, oscuro
  `#211354` y casi negro `#0B091F`.

## Pruebas de integración (backend)

Las pruebas de integración de autenticación requieren una base MySQL aislada:

```bash
export OPALINE_TEST_DATABASE_URL="mysql://opaline:opaline@127.0.0.1:3307/opaline_test"
# Opcional:
# export OPALINE_TEST_MONGO_URL="mongodb://127.0.0.1:27018/opaline_test"

cd opaline_rust_backend
cargo test -- --ignored
```

La URL de prueba debe contener `test` en el nombre de la base y apuntar a localhost. No uses la base de desarrollo ni producción.

## Comprobaciones rápidas

```bash
# Backend vivo
curl http://127.0.0.1:8090/health

# Login demo
curl -X POST http://127.0.0.1:8090/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"demo@opaline-ai.com","password":"demo1234"}'

# Web
curl -I http://localhost:8000/

# Calidad Flutter
cd opaline_ai_mobile && flutter analyze && flutter build linux
```

## Notas

- El modelo por defecto es **Ministral 3B**; Qwen 3.5 9B también funciona pero
  puede tardar ~1–2 minutos por respuesta en este equipo.
- El chat exige mensajes de 1 a 2000 caracteres; el muro, de 20 a 1000.
- El flujo de PayPal es una demostración local: no se realiza ningún cargo real.
- `flutter analyze` puede mostrar avisos de deprecaciones (`withOpacity`) que no
  impiden compilar.
