<?php require __DIR__ . '/includes/header.php'; ?>
<?php
// Usuarios mock (sin DB)
$USERS = [
    'admin'   => ['password' => 'admin',   'role' => 'Administrador', 'name' => 'Administrador'],
    'cliente' => ['password' => 'cliente', 'role' => 'Cliente',       'name' => 'Cliente'],
];
$error = '';
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    $u = trim($_POST['username'] ?? '');
    $p = trim($_POST['password'] ?? '');
    if (isset($USERS[$u]) && $USERS[$u]['password'] === $p) {
        $_SESSION['user'] = $u;
        $_SESSION['name'] = $USERS[$u]['name'];
        $_SESSION['role'] = $USERS[$u]['role'];
        header('Location: home.php');
        exit;
    } else {
        $error = 'Credenciales invalidas';
    }
}
?>
<h1>Login</h1>
<?php if ($error): ?><p><?php echo htmlspecialchars($error); ?></p><?php endif; ?>
<form method="post">
    <label>Usuario: <input type="text" name="username" required></label><br>
    <label>Contrasena: <input type="password" name="password" required></label><br>
    <button type="submit">Entrar</button>
</form>
<p>Usuarios de prueba: admin/admin, cliente/cliente</p>
<?php require __DIR__ . '/includes/footer.php'; ?>
