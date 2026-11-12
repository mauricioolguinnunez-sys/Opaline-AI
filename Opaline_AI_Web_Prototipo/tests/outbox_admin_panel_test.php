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
    curl_close($ch);
    $data = null;
    if (is_string($raw)) {
        $decoded = json_decode($raw, true);
        if (is_array($decoded)) $data = $decoded;
    }
    return ['status' => $status, 'raw' => $raw, 'data' => $data];
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
    if (preg_match('/name="csrf_token"\s+value="([^"]+)"/', $html, $m)) return $m[1];
    if (preg_match('/name="csrf-token"\s+content="([^"]+)"/', $html, $m)) return $m[1];
    return '';
}

$adminJar = tempnam(sys_get_temp_dir(), 'opaline_admin_panel_');
$normalJar = tempnam(sys_get_temp_dir(), 'opaline_normal_panel_');

// Admin login.
$loginPage = request('GET', $base . '/frontend/login.php', ['cookie_jar' => $adminJar]);
$csrf = csrf_from_html((string) $loginPage['raw']);
request('POST', $base . '/frontend/login.php', [
    'cookie_jar' => $adminJar,
    'body' => http_build_query([
        'email' => 'admin@opaline-ai.com',
        'password' => 'admin123',
        'csrf_token' => $csrf,
    ]),
    'headers' => ['Content-Type: application/x-www-form-urlencoded'],
]);

// Normal user login.
$normalPage = request('GET', $base . '/frontend/login.php', ['cookie_jar' => $normalJar]);
$normalCsrf = csrf_from_html((string) $normalPage['raw']);
request('POST', $base . '/frontend/login.php', [
    'cookie_jar' => $normalJar,
    'body' => http_build_query([
        'email' => 'demo@opaline-ai.com',
        'password' => 'demo1234',
        'csrf_token' => $normalCsrf,
    ]),
    'headers' => ['Content-Type: application/x-www-form-urlencoded'],
]);

$adminCsrf = csrf_from_html((string) request('GET', $base . '/frontend/moderacion_outbox.php', ['cookie_jar' => $adminJar])['raw']);

// 1. Unauthenticated access.
$unauth = request('GET', $base . '/frontend/moderacion_outbox.php');
assert_status('unauthenticated panel access', $unauth['status'], 302);

// 2. Normal user access.
$normalPageStatus = request('GET', $base . '/frontend/moderacion_outbox.php', ['cookie_jar' => $normalJar]);
assert_status('normal user panel access', $normalPageStatus['status'], 302);

// 3. Admin access.
$adminPage = request('GET', $base . '/frontend/moderacion_outbox.php', ['cookie_jar' => $adminJar]);
assert_status('admin panel access', $adminPage['status'], 200);
assert_true('CSRF meta present', strpos((string) $adminPage['raw'], 'name="csrf-token"') !== false);

// 4. Discrepancies as admin.
$discrepancies = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'json' => ['action' => 'outbox_discrepancies', 'limit' => 5, 'offset' => 0],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('admin discrepancies', $discrepancies['status'], 200);
assert_true('discrepancies has items', is_array($discrepancies['data']['items'] ?? null));
assert_true('discrepancies has audit_without_outbox', is_array($discrepancies['data']['audit_without_outbox'] ?? null));

// 5. Discrepancies without session.
$noSession = request('POST', $base . '/frontend/api_proxy.php', [
    'json' => ['action' => 'outbox_discrepancies', 'limit' => 5],
]);
assert_status('discrepancies without session', $noSession['status'], 401);

// 6. Discrepancies without CSRF.
$noCsrf = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'json' => ['action' => 'outbox_discrepancies', 'limit' => 5],
]);
assert_status('discrepancies without CSRF', $noCsrf['status'], 419);

// 7. Discrepancies as normal user.
$normalDiscrepancies = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $normalJar,
    'json' => ['action' => 'outbox_discrepancies', 'limit' => 5],
    'headers' => ['X-CSRF-Token: ' . $normalCsrf],
]);
assert_status('discrepancies as normal user', $normalDiscrepancies['status'], 403);

// 8. Event detail.
$items = $discrepancies['data']['items'] ?? [];
$eventId = '';
if (is_array($items) && $items) {
    $eventId = (string) ($items[0]['id'] ?? '');
}
if ($eventId !== '') {
    $detail = request('POST', $base . '/frontend/api_proxy.php', [
        'cookie_jar' => $adminJar,
        'json' => ['action' => 'outbox_event', 'id' => $eventId],
        'headers' => ['X-CSRF-Token: ' . $adminCsrf],
    ]);
    assert_status('event detail', $detail['status'], 200);
    assert_true('event detail has id', ($detail['data']['id'] ?? '') === $eventId);

    $history = request('POST', $base . '/frontend/api_proxy.php', [
        'cookie_jar' => $adminJar,
        'json' => ['action' => 'outbox_history', 'id' => $eventId],
        'headers' => ['X-CSRF-Token: ' . $adminCsrf],
    ]);
    assert_status('event history', $history['status'], 200);
    assert_true('event history is array', is_array($history['data']));
}

// 9. Invalid event id.
$invalidDetail = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'json' => ['action' => 'outbox_event', 'id' => 'invalid'],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('invalid event id', $invalidDetail['status'], 400);

// 10. Invalid action.
$invalidAction = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'json' => ['action' => 'outbox_action', 'event_id' => $eventId ?: '00000000-0000-0000-0000-000000000000', 'operation' => 'invalid', 'reason' => 'motivo', 'idempotency_key' => bin2hex(random_bytes(8))],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('invalid admin action', $invalidAction['status'], 400);

// 11. Malformed JSON.
$malformed = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'body' => '{invalid json',
    'headers' => ['Content-Type: application/json', 'X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('malformed JSON', $malformed['status'], 400);

// 12. Arbitrary action.
$arbitrary = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $adminJar,
    'json' => ['action' => 'http://example.com/evil'],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('arbitrary action rejected', $arbitrary['status'], 400);

@unlink($adminJar);
@unlink($normalJar);

if ($failures > 0) {
    fwrite(STDERR, "\n$failures test(s) failed.\n");
    exit(1);
}
fwrite(STDOUT, "\nAll Outbox admin panel tests passed.\n");
exit(0);