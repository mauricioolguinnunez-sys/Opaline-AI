<?php
declare(strict_types=1);

$base = getenv('OPALINE_WEB_BASE') ?: 'http://localhost:8000';
$failures = 0;

function request(string $method, string $url, array $options = []): array {
    $ch = curl_init($url);
    $headers = $options['headers'] ?? [];
    if (!empty($options['json'])) {
        $headers[] = 'Content-Type: application/json';
        $body = json_encode($options['json'], JSON_UNESCAPED_UNICODE);
    } elseif (isset($options['body'])) {
        $body = $options['body'];
    } else {
        $body = null;
    }
    if ($body !== null) {
        curl_setopt($ch, CURLOPT_POSTFIELDS, $body);
    }
    curl_setopt_array($ch, [
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_CUSTOMREQUEST => $method,
        CURLOPT_HTTPHEADER => $headers,
        CURLOPT_CONNECTTIMEOUT => 5,
        CURLOPT_TIMEOUT => 30,
        CURLOPT_FOLLOWLOCATION => false,
    ]);
    if (!empty($options['cookie_jar'])) {
        curl_setopt($ch, CURLOPT_COOKIEJAR, $options['cookie_jar']);
        curl_setopt($ch, CURLOPT_COOKIEFILE, $options['cookie_jar']);
    }
    $raw = curl_exec($ch);
    $status = (int) curl_getinfo($ch, CURLINFO_HTTP_CODE);
    $error = curl_error($ch);
    curl_close($ch);
    $data = null;
    if (is_string($raw)) {
        $decoded = json_decode($raw, true);
        if (is_array($decoded)) $data = $decoded;
    }
    return ['status' => $status, 'raw' => $raw, 'data' => $data, 'error' => $error];
}

function assert_status(string $name, int $actual, int $expected): void {
    global $failures;
    if ($actual !== $expected) {
        $failures++;
        fwrite(STDERR, "FAIL: $name (expected $expected, got $actual)\n");
    } else {
        fwrite(STDOUT, "PASS: $name\n");
    }
}

function assert_true(string $name, bool $condition): void {
    global $failures;
    if (!$condition) {
        $failures++;
        fwrite(STDERR, "FAIL: $name\n");
    } else {
        fwrite(STDOUT, "PASS: $name\n");
    }
}

function csrf_from_html(string $html): string {
    if (preg_match('/name="csrf_token"\s+value="([^"]+)"/', $html, $m)) {
        return $m[1];
    }
    if (preg_match('/name="csrf-token"\s+content="([^"]+)"/', $html, $m)) {
        return $m[1];
    }
    return '';
}

$jar = tempnam(sys_get_temp_dir(), 'opaline_test_');

// 1. Public wall list.
$r = request('GET', $base . '/frontend/api_proxy.php?action=wall_list');
assert_status('public wall list', $r['status'], 200);
assert_true('public wall list is array', is_array($r['data']));

// 2. Publish without session.
$r = request('POST', $base . '/frontend/api_proxy.php', [
    'json' => ['action' => 'wall_publish', 'content' => str_repeat('a', 25)],
]);
assert_status('publish without session', $r['status'], 401);

// 3. Login with CSRF.
$loginPage = request('GET', $base . '/frontend/login.php', ['cookie_jar' => $jar]);
$csrf = csrf_from_html((string) $loginPage['raw']);
assert_true('login CSRF token present', $csrf !== '');
$login = request('POST', $base . '/frontend/login.php', [
    'cookie_jar' => $jar,
    'body' => http_build_query([
        'email' => 'demo@opaline-ai.com',
        'password' => 'demo1234',
        'csrf_token' => $csrf,
    ]),
    'headers' => ['Content-Type: application/x-www-form-urlencoded'],
]);
assert_status('login with CSRF', $login['status'], 302);

// 4. Publish without CSRF must not reach backend mutation.
$invalidContent = str_repeat('b', 25);
$noCsrf = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => ['action' => 'wall_publish', 'content' => $invalidContent],
]);
assert_status('publish without CSRF', $noCsrf['status'], 419);

// 5. Publish invalid content with CSRF.
$accountPage = request('GET', $base . '/frontend/cuenta.php', ['cookie_jar' => $jar]);
$csrf = csrf_from_html((string) $accountPage['raw']);
$invalid = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => ['action' => 'wall_publish', 'content' => 'corto', 'csrf_token' => $csrf],
    'headers' => ['X-CSRF-Token: ' . $csrf],
]);
assert_status('invalid content', $invalid['status'], 400);

// 6. Publish valid content with CSRF and idempotency key.
$validContent = 'Publicacion de prueba PHP con longitud suficiente para el muro.';
$key = bin2hex(random_bytes(16));
$first = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => [
        'action' => 'wall_publish',
        'content' => $validContent,
        'idempotency_key' => $key,
    ],
    'headers' => ['X-CSRF-Token: ' . $csrf],
]);
assert_status('valid publish', $first['status'], 200);
assert_true('valid publish has id', is_array($first['data']) && !empty($first['data']['id']));

// 7. Repeated publish with same idempotency key is idempotent.
$second = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => [
        'action' => 'wall_publish',
        'content' => $validContent,
        'idempotency_key' => $key,
    ],
    'headers' => ['X-CSRF-Token: ' . $csrf],
]);
assert_status('idempotent publish', $second['status'], 200);
assert_true('idempotent publish returns same id', ($first['data']['id'] ?? null) === ($second['data']['id'] ?? null));

// 8. Proxy must reject arbitrary actions.
$arbitrary = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => ['action' => 'http://example.com/evil'],
    'headers' => ['X-CSRF-Token: ' . $csrf],
]);
assert_status('arbitrary action rejected', $arbitrary['status'], 400);

// 9. Logout requires CSRF.
$logoutWithoutCsrf = request('POST', $base . '/frontend/logout.php', [
    'cookie_jar' => $jar,
]);
assert_status('logout without CSRF', $logoutWithoutCsrf['status'], 419);

$logoutWithCsrf = request('POST', $base . '/frontend/logout.php', [
    'cookie_jar' => $jar,
    'body' => http_build_query(['csrf_token' => $csrf]),
    'headers' => ['Content-Type: application/x-www-form-urlencoded'],
]);
assert_status('logout with CSRF', $logoutWithCsrf['status'], 302);

@unlink($jar);

if ($failures > 0) {
    fwrite(STDERR, "\n$failures test(s) failed.\n");
    exit(1);
}
fwrite(STDOUT, "\nAll PHP wall proxy tests passed.\n");
exit(0);