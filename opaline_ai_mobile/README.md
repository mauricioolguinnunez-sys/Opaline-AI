# Opaline AI · App Flutter

Aplicación móvil/escritorio de Opaline AI. Comparte la misma cuenta, suscripción,
chat y muro que la web PHP mediante la API Rust.

## Requisitos

- Flutter 3.4x o superior.
- Backend Rust activo en `http://127.0.0.1:8090/api/v1`.
- LM Studio activo en `http://127.0.0.1:1234`.
- En Android, el emulador accede al host con `10.0.2.2` (ya configurado en
  `lib/core/api/api_client.dart`).

## Ejecutar

```bash
flutter pub get
flutter run -d linux
```

Otras plataformas:

```bash
flutter run -d android
flutter build linux
flutter build web
flutter build apk --debug
```

## Configuración de API

En `lib/core/api/api_client.dart`:

- Debug Linux/escritorio: `http://127.0.0.1:8090/api/v1`
- Debug Android: `http://10.0.2.2:8090/api/v1`
- Release: `https://www.opaline-ai.com/api/v1`

El cliente HTTP:

- Inyecta el access token en cada petición.
- Renueva la sesión automáticamente al recibir `401` y reintenta una vez.
- Usa timeout de 12 s para negocio y 180 s para IA.
- Mantiene un circuit breaker breve si el backend no responde.

## Pantallas principales

- `welcome` / `login` / `register`: autenticación, con botón de cuenta demo.
- `inicio`: dashboard con saludo, accesos rápidos y planes.
- `chat`: modelos LM Studio, historial y conversaciones persistidas.
- `muro`: publicaciones, calificación, reporte y creación de posts.
- `cuenta`: perfil, suscripción, facturas y cambio de contraseña.
- `checkout` / `suscripción`: flujo de pago demo y gestión del plan.

## Cuentas de prueba

- Administrador: `admin@opaline-ai.com` / `admin123`
- Demo con Starter activo: `demo@opaline-ai.com` / `demo1234`

## Verificación

```bash
flutter analyze
flutter build linux
flutter build web
```
