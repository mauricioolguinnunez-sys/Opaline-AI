<?php require __DIR__ . '/includes/header.php'; ?>
<?php
if (empty($_SESSION['user'])) {
    header('Location: login.php');
    exit;
}
?>
<h1>Home</h1>
<p>Bienvenido, <?php echo htmlspecialchars($_SESSION['name']); ?> (<?php echo htmlspecialchars($_SESSION['role']); ?>).</p>
<ul>
    <li><a href="chat.php">Chat</a></li>
    <li><a href="planes.php">Planes</a></li>
    <li><a href="muro.php">Muro</a></li>
    <li><a href="cuenta.php">Cuenta</a></li>
    <li><a href="panel.php">Panel</a></li>
    <li><a href="logout.php">Salir</a></li>
</ul>
<?php require __DIR__ . '/includes/footer.php'; ?>
