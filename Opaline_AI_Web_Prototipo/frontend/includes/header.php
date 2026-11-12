<?php
require_once __DIR__ . '/../config.php';
$user = current_user();
$current = basename($_SERVER['PHP_SELF'] ?? 'index.php');
?>
<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title><?= isset($pageTitle) ? e($pageTitle) . ' · ' : '' ?>Opaline AI</title>
<meta name="csrf-token" content="<?= e(csrf_token()) ?>">
<link rel="stylesheet" href="assets/style.css">
</head>
<body>
<div class="bg-orbs" aria-hidden="true"><span></span><span></span><span></span></div>
<header class="site-header">
  <a class="brand" href="index.php">
    <span class="brand-mark">O</span>
    <span class="brand-text">Opaline <strong>AI</strong></span>
  </a>
  <nav class="main-nav">
    <a href="index.php" class="<?= $current === 'index.php' ? 'active' : '' ?>">Inicio</a>
    <a href="planes.php" class="<?= $current === 'planes.php' ? 'active' : '' ?>">Planes</a>
    <a href="chat.php" class="<?= $current === 'chat.php' ? 'active' : '' ?>">Chat IA</a>
    <a href="muro.php" class="<?= $current === 'muro.php' ? 'active' : '' ?>">Muro</a>
    <?php if ($user): ?>
      <a href="cuenta.php" class="<?= $current === 'cuenta.php' ? 'active' : '' ?>">Mi cuenta</a>
      <?php if (($user['role'] ?? '') === 'admin'): ?>
        <a href="panel.php" class="<?= $current === 'panel.php' ? 'active' : '' ?>">Panel</a>
      <?php endif; ?>
    <?php endif; ?>
  </nav>
  <div class="header-actions">
    <?php if ($user): ?>
      <span class="user-chip"><span class="avatar"><?= e(strtoupper(substr($user['name'] ?? 'U', 0, 1))) ?></span><?= e($user['name'] ?? 'Usuario') ?></span>
      <form method="post" action="logout.php" style="display:inline;margin:0">
        <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
        <button class="btn btn-ghost" type="submit">Salir</button>
      </form>
    <?php else: ?>
      <a class="btn btn-ghost" href="login.php">Entrar</a>
      <a class="btn btn-primary btn-sm" href="register.php">Crear cuenta</a>
    <?php endif; ?>
  </div>
</header>
<?php if ($f = flash()): ?>
  <div class="flash flash-<?= e($f['type']) ?>" role="status"><?= e($f['message']) ?></div>
<?php endif; ?>
<main class="page">
