<?php require __DIR__ . '/includes/header.php'; ?>
<?php
if (!empty($_SESSION['user'])) {
    header('Location: home.php');
    exit;
} else {
    header('Location: login.php');
    exit;
}
