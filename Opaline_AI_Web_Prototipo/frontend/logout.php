<?php
require_once __DIR__ . '/config.php';

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    http_response_code(405);
    header('Allow: POST');
    header('Content-Type: text/html; charset=utf-8');
    echo '<!DOCTYPE html><html lang="es"><head><meta charset="UTF-8"><title>Cerrar sesión</title></head><body><form method="post" action="logout.php"><input type="hidden" name="csrf_token" value="' . e(csrf_token()) . '"><button type="submit">Cerrar sesión</button></form></body></html>';
    exit;
}

require_csrf();

if (is_logged_in()) {
    api('POST', '/auth/logout');
}

$_SESSION = [];
if (ini_get('session.use_cookies')) {
    $params = session_get_cookie_params();
    setcookie(session_name(), '', time() - 42000, $params['path'], $params['domain'] ?? '', (bool) ($params['secure'] ?? false), (bool) ($params['httponly'] ?? true));
}
session_destroy();

header('Location: index.php');
exit;