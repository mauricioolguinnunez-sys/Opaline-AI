<?php
require_once __DIR__ . '/config.php';

if (is_logged_in()) {
    redirect('cuenta.php');
}

$error = '';
$email = '';

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    require_csrf();
    if (isset($_POST['demo'])) {
        $email = 'demo@opaline-ai.com';
        $password = 'demo1234';
    } else {
        $email = trim((string) ($_POST['email'] ?? ''));
        $password = (string) ($_POST['password'] ?? '');
    }

    if ($email === '' || $password === '') {
        $error = 'Escribe tu correo y contraseña.';
    } else {
        $res = api('POST', '/auth/login', ['email' => $email, 'password' => $password], false);
        if ($res['status'] === 200 && !empty($res['data']['access_token'])) {
            session_regenerate_id(true);
            $_SESSION['access_token'] = $res['data']['access_token'];
            $_SESSION['refresh_token'] = $res['data']['refresh_token'] ?? '';
            $_SESSION['user'] = $res['data']['user'] ?? ['name' => 'Usuario', 'role' => 'client'];
            flash('¡Bienvenido de nuevo, ' . ($_SESSION['user']['name'] ?? 'Usuario') . '!', 'success');
            redirect('chat.php');
        } elseif ($res['status'] === 401) {
            $error = 'Correo o contraseña incorrectos. Intenta de nuevo.';
        } else {
            $error = api_error($res, 'No se pudo iniciar sesión. ¿Está corriendo el backend en 127.0.0.1:8090?');
        }
    }
}
$pageTitle = 'Iniciar sesión';
require __DIR__ . '/includes/header.php';
?>

<div class="auth-wrap">
  <div class="grid grid-2" style="width:min(940px,100%);align-items:stretch">
    <div class="card auth-card">
      <h1>Hola de nuevo</h1>
      <p class="sub">Entra para continuar tus conversaciones y administrar tu plan.</p>

      <?php if ($error): ?>
        <div class="alert alert-error" style="margin-bottom:16px"><?= e($error) ?></div>
      <?php endif; ?>

      <form method="post" autocomplete="on">
        <div class="field">
          <label for="email">Correo electrónico</label>
          <input class="input" type="email" id="email" name="email" value="<?= e($email) ?>" placeholder="tu@correo.com" required autofocus>
        </div>
        <div class="field">
          <label for="password">Contraseña</label>
          <input class="input" type="password" id="password" name="password" placeholder="Tu contraseña" required>
          <span class="hint">Mínimo 10 caracteres al registrarte.</span>
        </div>
        <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
        <button class="btn btn-primary btn-block" type="submit">Entrar a Opaline</button>
      </form>

      <div style="display:flex;align-items:center;gap:10px;margin:18px 0;color:var(--muted);font-size:12px">
        <span style="flex:1;height:1px;background:var(--border)"></span> o <span style="flex:1;height:1px;background:var(--border)"></span>
      </div>

      <form method="post">
        <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
        <button class="btn btn-soft btn-block" type="submit" name="demo" value="1">Probar con cuenta demo →</button>
      </form>
      <p class="hint" style="text-align:center;margin:12px 0 0">Demo: demo@opaline-ai.com · demo1234</p>
      <p style="text-align:center;margin:18px 0 0;font-size:14px">¿No tienes cuenta? <a href="register.php">Regístrate gratis</a></p>
    </div>

    <aside class="auth-aside">
      <div>
        <span class="pill" style="background:rgba(255,255,255,.14);padding:6px 10px;border-radius:999px;font-size:12px">✦ Opaline AI</span>
        <h3 style="margin-top:18px;font-size:26px">Tu IA privada, en todas tus pantallas.</h3>
        <p style="color:#ddd2ff;font-size:14px">La misma cuenta y suscripción en la web y en la app móvil.</p>
      </div>
      <ul style="margin-top:26px">
        <li>Chat con modelos open source locales.</li>
        <li>Historial sincronizado y seguro.</li>
        <li>Muro de experiencias de la comunidad.</li>
        <li>Panel de suscripción y pagos.</li>
      </ul>
    </aside>
  </div>
</div>

<?php require __DIR__ . '/includes/footer.php'; ?>
