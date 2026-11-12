<?php
require_once __DIR__ . '/config.php';

if (is_logged_in()) {
    redirect('cuenta.php');
}

$error = '';
$name = '';
$email = '';

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    require_csrf();
    $name = trim((string) ($_POST['name'] ?? ''));
    $email = trim((string) ($_POST['email'] ?? ''));
    $password = (string) ($_POST['password'] ?? '');
    $confirm = (string) ($_POST['confirm'] ?? '');

    if ($name === '' || $email === '' || $password === '') {
        $error = 'Completa todos los campos.';
    } elseif (!filter_var($email, FILTER_VALIDATE_EMAIL)) {
        $error = 'Escribe un correo válido.';
    } elseif (mb_strlen($password) < 10) {
        $error = 'La contraseña debe tener al menos 10 caracteres.';
    } elseif ($password !== $confirm) {
        $error = 'Las contraseñas no coinciden.';
    } else {
        $res = api('POST', '/auth/register', [
            'name' => $name,
            'email' => $email,
            'password' => $password,
        ], false);

        if ($res['status'] === 200 || $res['status'] === 201) {
            // Iniciar sesión automáticamente después del registro.
            $login = api('POST', '/auth/login', ['email' => $email, 'password' => $password], false);
            if ($login['status'] === 200 && !empty($login['data']['access_token'])) {
                session_regenerate_id(true);
                $_SESSION['access_token'] = $login['data']['access_token'];
                $_SESSION['refresh_token'] = $login['data']['refresh_token'] ?? '';
                $_SESSION['user'] = $login['data']['user'] ?? ['name' => $name, 'role' => 'client'];
                flash('Cuenta creada. ¡Bienvenido a Opaline, ' . $name . '!', 'success');
                redirect('chat.php');
            }
            flash('Cuenta creada. Ahora inicia sesión.', 'success');
            redirect('login.php');
        } elseif ($res['status'] === 409) {
            $error = 'Ese correo ya tiene una cuenta. ¿Quieres iniciar sesión?';
        } else {
            $error = api_error($res, 'No se pudo crear la cuenta. Revisa los datos e inténtalo de nuevo.');
        }
    }
}
$pageTitle = 'Crear cuenta';
require __DIR__ . '/includes/header.php';
?>

<div class="auth-wrap">
  <div class="grid grid-2" style="width:min(940px,100%);align-items:stretch">
    <div class="card auth-card">
      <h1>Crea tu cuenta</h1>
      <p class="sub">Empieza gratis y mejora tu plan cuando lo necesites.</p>

      <?php if ($error): ?>
        <div class="alert alert-error" style="margin-bottom:16px"><?= e($error) ?></div>
      <?php endif; ?>

      <form method="post" autocomplete="on">
        <div class="field">
          <label for="name">Nombre</label>
          <input class="input" type="text" id="name" name="name" value="<?= e($name) ?>" placeholder="Tu nombre" required autofocus>
        </div>
        <div class="field">
          <label for="email">Correo electrónico</label>
          <input class="input" type="email" id="email" name="email" value="<?= e($email) ?>" placeholder="tu@correo.com" required>
        </div>
        <div class="field">
          <label for="password">Contraseña</label>
          <input class="input" type="password" id="password" name="password" placeholder="Mínimo 10 caracteres" minlength="10" required>
        </div>
        <div class="field">
          <label for="confirm">Confirmar contraseña</label>
          <input class="input" type="password" id="confirm" name="confirm" placeholder="Repite tu contraseña" minlength="10" required>
        </div>
        <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
        <button class="btn btn-primary btn-block" type="submit">Crear mi cuenta gratis</button>
      </form>

      <p class="hint" style="margin:14px 0 0">Al crear una cuenta aceptas el uso responsable de la IA y nuestras políticas de privacidad.</p>
      <p style="text-align:center;margin:18px 0 0;font-size:14px">¿Ya tienes cuenta? <a href="login.php">Inicia sesión</a></p>
    </div>

    <aside class="auth-aside">
      <div>
        <span class="pill" style="background:rgba(255,255,255,.14);padding:6px 10px;border-radius:999px;font-size:12px">Gratis para empezar</span>
        <h3 style="margin-top:18px;font-size:26px">Todo Opaline en una sola cuenta.</h3>
        <p style="color:#ddd2ff;font-size:14px">Regístrate una vez y accede desde la web o la app móvil.</p>
      </div>
      <ul style="margin-top:26px">
        <li>Chat con LM Studio local.</li>
        <li>Historial y preferencias sincronizadas.</li>
        <li>Muro de la comunidad.</li>
        <li>Facturas y pagos en un solo lugar.</li>
      </ul>
    </aside>
  </div>
</div>

<?php require __DIR__ . '/includes/footer.php'; ?>
