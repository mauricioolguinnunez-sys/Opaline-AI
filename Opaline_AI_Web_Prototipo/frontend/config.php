<?php
declare(strict_types=1);

if (session_status() === PHP_SESSION_NONE) {
    $secure = !empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off';
    session_set_cookie_params([
        'lifetime' => 0,
        'path' => '/',
        'httponly' => true,
        'secure' => $secure,
        'samesite' => 'Lax',
    ]);
    ini_set('session.use_strict_mode', '1');
    ini_set('session.use_only_cookies', '1');
    session_start();
}

function csrf_token(): string {
    if (empty($_SESSION['csrf_token']) || !is_string($_SESSION['csrf_token'])) {
        try {
            $_SESSION['csrf_token'] = bin2hex(random_bytes(32));
        } catch (Throwable $e) {
            $_SESSION['csrf_token'] = bin2hex(hash('sha256', session_id() . microtime(true), true));
        }
    }
    return (string) $_SESSION['csrf_token'];
}

function csrf_validate(?string $token): bool {
    if (!is_string($token) || $token === '' || empty($_SESSION['csrf_token'])) {
        return false;
    }
    return hash_equals((string) $_SESSION['csrf_token'], $token);
}

function outbox_pending_key(string $operationId): ?string {
    if ($operationId === '' || !isset($_SESSION['outbox_pending_keys']) || !is_array($_SESSION['outbox_pending_keys'])) {
        return null;
    }
    $value = $_SESSION['outbox_pending_keys'][$operationId] ?? null;
    return is_string($value) && $value !== '' ? $value : null;
}

function remember_outbox_pending_key(string $operationId, string $key): void {
    if ($operationId === '' || $key === '') return;
    if (!isset($_SESSION['outbox_pending_keys']) || !is_array($_SESSION['outbox_pending_keys'])) {
        $_SESSION['outbox_pending_keys'] = [];
    }
    $_SESSION['outbox_pending_keys'][$operationId] = $key;
}

function clear_outbox_pending_key(string $operationId): void {
    if (isset($_SESSION['outbox_pending_keys'][$operationId])) {
        unset($_SESSION['outbox_pending_keys'][$operationId]);
    }
}

function require_csrf(): void {
    $token = $_POST['csrf_token'] ?? $_SERVER['HTTP_X_CSRF_TOKEN'] ?? '';
    if (!csrf_validate(is_string($token) ? $token : null)) {
        http_response_code(419);
        header('Content-Type: application/json; charset=utf-8');
        echo json_encode(['error' => 'Token CSRF inválido o ausente. Recarga la página e inténtalo de nuevo.'], JSON_UNESCAPED_UNICODE);
        exit;
    }
}

define('API_BASE', rtrim(getenv('OPALINE_API') ?: 'http://127.0.0.1:8090/api/v1', '/'));

function api(string $method, string $path, ?array $body = null, bool $auth = true, bool $retry = true, int $timeout = 120, bool $expectJson = true): array {
    $ch = curl_init(API_BASE . $path);
    $headers = ['Accept: application/json'];
    if ($body !== null) {
        $headers[] = 'Content-Type: application/json';
        curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($body, JSON_UNESCAPED_UNICODE));
    }
    if ($auth && !empty($_SESSION['access_token'])) {
        $headers[] = 'Authorization: Bearer ' . $_SESSION['access_token'];
    }
    curl_setopt_array($ch, [
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_CUSTOMREQUEST => $method,
        CURLOPT_HTTPHEADER => $headers,
        CURLOPT_CONNECTTIMEOUT => 5,
        CURLOPT_TIMEOUT => $timeout,
    ]);
    $raw = curl_exec($ch);
    $status = (int) curl_getinfo($ch, CURLINFO_HTTP_CODE);
    $err = curl_error($ch);
    curl_close($ch);
    if ($raw === false) {
        return ['status' => 0, 'data' => ['error' => $err ?: 'API no disponible']];
    }
    $data = json_decode($raw, true);
    $contentType = (string) curl_getinfo($ch, CURLINFO_CONTENT_TYPE);
    if ($expectJson) {
        if (!is_array($data)) {
            $status = ($status >= 200 && $status < 300) ? 502 : ($status === 0 ? 502 : $status);
            $data = ['error' => 'Respuesta inválida del backend.'];
        } elseif (stripos($contentType, 'application/json') === false && $status >= 200 && $status < 300) {
            $status = 502;
            $data = ['error' => 'Respuesta inválida del backend.'];
        }
    } elseif (!is_array($data)) {
        $data = ['raw' => $raw];
    }
    // La sesion de la API dura 15 min; renovarla de forma transparente.
    if ($auth && $retry && $status === 401 && !empty($_SESSION['refresh_token'])) {
        $refresh = api('POST', '/auth/refresh', ['refresh_token' => $_SESSION['refresh_token']], false, false);
        if ($refresh['status'] === 200 && !empty($refresh['data']['access_token'])) {
            $_SESSION['access_token'] = $refresh['data']['access_token'];
            $_SESSION['refresh_token'] = $refresh['data']['refresh_token'] ?? '';
            $_SESSION['user'] = $refresh['data']['user'] ?? $_SESSION['user'];
            return api($method, $path, $body, $auth, false);
        }
        unset($_SESSION['access_token'], $_SESSION['refresh_token'], $_SESSION['user']);
    }
    return ['status' => $status, 'data' => $data];
}

function api_error(array $result, string $fallback = 'No se pudo completar la operación.'): string {
    $data = $result['data'] ?? [];
    return (string) ($data['message'] ?? $data['error'] ?? $fallback);
}

function current_user(): ?array {
    return $_SESSION['user'] ?? null;
}

function is_logged_in(): bool {
    return !empty($_SESSION['access_token']) && !empty($_SESSION['user']);
}

function require_login(): void {
    if (!is_logged_in()) {
        header('Location: login.php');
        exit;
    }
}

function e(?string $value): string {
    return htmlspecialchars((string) $value, ENT_QUOTES, 'UTF-8');
}

function flash(?string $message = null, string $type = 'info'): ?array {
    if ($message !== null) {
        $_SESSION['flash'] = ['message' => $message, 'type' => $type];
        return null;
    }
    $f = $_SESSION['flash'] ?? null;
    unset($_SESSION['flash']);
    return $f;
}

function money($amount, string $currency = 'MXN'): string {
    return '$' . number_format((float) $amount, 2) . ' ' . $currency;
}

function redirect(string $url): void {
    header('Location: ' . $url);
    exit;
}
