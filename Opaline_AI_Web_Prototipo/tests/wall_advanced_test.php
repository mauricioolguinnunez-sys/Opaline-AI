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

function login(string $email, string $password, string $jar): string {
    global $base;
    $page = request('GET', $base . '/frontend/login.php', ['cookie_jar' => $jar]);
    $csrf = csrf_from_html((string) $page['raw']);
    request('POST', $base . '/frontend/login.php', [
        'cookie_jar' => $jar,
        'body' => http_build_query([
            'email' => $email,
            'password' => $password,
            'csrf_token' => $csrf,
        ]),
        'headers' => ['Content-Type: application/x-www-form-urlencoded'],
    ]);
    $account = request('GET', $base . '/frontend/cuenta.php', ['cookie_jar' => $jar]);
    return csrf_from_html((string) $account['raw']);
}

$demoJar = tempnam(sys_get_temp_dir(), 'opaline_demo_');
$adminJar = tempnam(sys_get_temp_dir(), 'opaline_admin_');

$demoCsrf = login('demo@opaline-ai.com', 'demo1234', $demoJar);
$adminCsrf = login('admin@opaline-ai.com', 'admin123', $adminJar);

// Create a post for edit/delete/report tests.
$content = 'Publicacion avanzada PHP con longitud suficiente para pruebas.';
$publish = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_publish', 'content' => $content, 'idempotency_key' => bin2hex(random_bytes(8))],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('advanced publish', $publish['status'], 200);
$postId = (string) ($publish['data']['id'] ?? '');

// Edit valid.
$edited = 'Publicacion editada desde PHP con longitud suficiente para la prueba.';
$edit = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_edit', 'post_id' => $postId, 'content' => $edited],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('edit valid', $edit['status'], 200);

// Edit invalid content.
$editInvalid = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_edit', 'post_id' => $postId, 'content' => 'corto'],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('edit invalid content', $editInvalid['status'], 400);

// Edit without CSRF.
$editNoCsrf = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_edit', 'post_id' => $postId, 'content' => $edited],
]);
assert_status('edit without CSRF', $editNoCsrf['status'], 419);

// Delete valid.
$delete = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_delete', 'post_id' => $postId],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('delete valid', $delete['status'], 204);

// Delete repeated.
$deleteAgain = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_delete', 'post_id' => $postId],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('delete repeated', $deleteAgain['status'], 403);

// Report valid on a fresh post.
$reportPost = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_publish', 'content' => 'Publicacion para reporte con longitud suficiente para la prueba.', 'idempotency_key' => bin2hex(random_bytes(8))],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
$reportId = (string) ($reportPost['data']['id'] ?? '');
$report = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_report', 'post_id' => $reportId, 'reason' => 'spam', 'detail' => 'detalle'],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('report valid', $report['status'], 200);

// Duplicate report.
$duplicateReport = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_report', 'post_id' => $reportId, 'reason' => 'spam'],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('report duplicate', $duplicateReport['status'], 200);
assert_true('report duplicate flag', ($duplicateReport['data']['duplicate'] ?? false) === true);

// Report without CSRF.
$reportNoCsrf = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_report', 'post_id' => $reportId, 'reason' => 'spam'],
]);
assert_status('report without CSRF', $reportNoCsrf['status'], 419);

// Report nonexistent post.
$reportMissing = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'json' => ['action' => 'wall_report', 'post_id' => '00000000-0000-0000-0000-000000000000', 'reason' => 'spam'],
    'headers' => ['X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('report missing post', $reportMissing['status'], 404);

// Malformed JSON to proxy.
$malformed = request('POST', $base . '/frontend/api_proxy.php', [
    'cookie_jar' => $demoJar,
    'body' => '{invalid json',
    'headers' => ['Content-Type: application/json', 'X-CSRF-Token: ' . $demoCsrf],
]);
assert_status('malformed JSON to proxy', $malformed['status'], 400);

// Direct PHP-Rust moderation tests using the existing API helper.
require_once __DIR__ . '/../frontend/config.php';

$demoLogin = api('POST', '/auth/login', [
    'email' => 'demo@opaline-ai.com',
    'password' => 'demo1234',
], false);
assert_true('demo login for moderation test', ($demoLogin['status'] ?? 0) === 200 && !empty($demoLogin['data']['access_token']));
$_SESSION['access_token'] = $demoLogin['data']['access_token'] ?? '';
$_SESSION['refresh_token'] = $demoLogin['data']['refresh_token'] ?? '';
$_SESSION['user'] = $demoLogin['data']['user'] ?? [];

$moderationPost = api('POST', '/muro', [
    'content' => 'Publicacion para moderacion PHP con longitud suficiente.',
]);
assert_true('moderation post created', ($moderationPost['status'] ?? 0) === 200 && !empty($moderationPost['data']['id']));
$moderationPostId = (string) ($moderationPost['data']['id'] ?? '');

$adminLogin = api('POST', '/auth/login', [
    'email' => 'admin@opaline-ai.com',
    'password' => 'admin123',
], false);
assert_true('admin login for moderation test', ($adminLogin['status'] ?? 0) === 200 && !empty($adminLogin['data']['access_token']));
$_SESSION['access_token'] = $adminLogin['data']['access_token'] ?? '';
$_SESSION['refresh_token'] = $adminLogin['data']['refresh_token'] ?? '';
$_SESSION['user'] = $adminLogin['data']['user'] ?? [];

$moderationList = api('GET', '/muro/moderacion');
assert_status('moderation list as admin', $moderationList['status'], 200);
assert_true('moderation list is array', is_array($moderationList['data']));

$moderateWithoutReason = api('POST', '/muro/' . rawurlencode($moderationPostId) . '/moderar', [
    'action' => 'hide',
]);
assert_status('moderate without reason', $moderateWithoutReason['status'], 400);

$moderateWithReason = api('POST', '/muro/' . rawurlencode($moderationPostId) . '/moderar', [
    'action' => 'hide',
    'reason' => 'motivo de moderacion desde PHP',
]);
assert_status('moderate with reason', $moderateWithReason['status'], 200);

// Restore normal user session for subsequent proxy tests is not needed.

@unlink($demoJar);
@unlink($adminJar);

if ($failures > 0) {
    fwrite(STDERR, "\n$failures test(s) failed.\n");
    exit(1);
}
fwrite(STDOUT, "\nAll advanced PHP wall tests passed.\n");
exit(0);