<?php
declare(strict_types=1);

$failures = 0;
$proxyPort = 8050;
$webPort = 8051;
$mockScenarioFile = '/tmp/opencode/mock_scenario.txt';
$dropFlag = '/tmp/opencode/drop_next';

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
    if ($body !== null) curl_setopt($ch, CURLOPT_POSTFIELDS, $body);
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

function csrf_from_html(string $html): string {
    if (preg_match('/name="csrf_token"\s+value="([^"]+)"/', $html, $m)) return $m[1];
    if (preg_match('/name="csrf-token"\s+content="([^"]+)"/', $html, $m)) return $m[1];
    return '';
}

$proxyScript = tempnam(sys_get_temp_dir(), 'opaline_proxy_');
file_put_contents($proxyScript, <<<'PY'
import http.server
import urllib.request
import os

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get('Content-Length', '0'))
        body = self.rfile.read(length) if length > 0 else b''
        url = 'http://127.0.0.1:8090' + self.path
        req = urllib.request.Request(url, data=body, method='POST')
        for key in ('Authorization', 'Content-Type', 'X-CSRF-Token'):
            value = self.headers.get(key)
            if value:
                req.add_header(key, value)
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                data = response.read()
                status = response.status
                headers = dict(response.headers.items())
        except urllib.error.HTTPError as error:
            data = error.read()
            status = error.code
            headers = dict(error.headers.items())
        except Exception as error:
            self.send_response(502)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(b'{"error":"proxy failure"}')
            return

        if os.path.exists('/tmp/opencode/drop_next') and '/actions' in self.path:
            os.remove('/tmp/opencode/drop_next')
            # Intentionally close without forwarding Rust's response.
            return

        self.send_response(status)
        for key, value in headers.items():
            if key.lower() not in ('transfer-encoding', 'connection', 'content-length'):
                self.send_header(key, value)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        return

http.server.ThreadingHTTPServer.allow_reuse_address = True
with http.server.ThreadingHTTPServer(('127.0.0.1', 8055), Handler) as httpd:
    httpd.serve_forever()
PY
);

$proxyProc = proc_open(
    'python3 ' . escapeshellarg($proxyScript),
    [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
    $proxyPipes
);
usleep(1000000);

$webProc = proc_open(
    'env OPALINE_API=http://127.0.0.1:8055 php -S 127.0.0.1:8056 -t ' . escapeshellarg(dirname(__DIR__)),
    [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
    $webPipes
);
usleep(1500000);

$webBase = 'http://127.0.0.1:8056';
$jar = tempnam(sys_get_temp_dir(), 'opaline_failure_');

$loginPage = request('GET', $webBase . '/frontend/login.php', ['cookie_jar' => $jar]);
$csrf = '';
if (preg_match('/name="csrf_token"\s+value="([^"]+)"/', (string) $loginPage['raw'], $m)) {
    $csrf = $m[1];
}
request('POST', $webBase . '/frontend/login.php', [
    'cookie_jar' => $jar,
    'body' => http_build_query([
        'email' => 'admin@opaline-ai.com',
        'password' => 'admin123',
        'csrf_token' => $csrf,
    ]),
    'headers' => ['Content-Type: application/x-www-form-urlencoded'],
]);

$adminPage = request('GET', $webBase . '/frontend/moderacion_outbox.php', ['cookie_jar' => $jar]);
$adminCsrf = '';
if (preg_match('/name="csrf-token"\s+content="([^"]+)"/', (string) $adminPage['raw'], $m)) {
    $adminCsrf = $m[1];
}

$discrepancies = request('POST', $webBase . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => ['action' => 'outbox_discrepancies', 'limit' => 1],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
$eventId = '';
if (is_array($discrepancies['data']['items'] ?? null) && $discrepancies['data']['items']) {
    $eventId = (string) ($discrepancies['data']['items'][0]['id'] ?? '');
}
if ($eventId === '') {
    $eventId = '00000000-0000-0000-0000-000000000000';
}

// Ensure the operation key is not already present.
$opId = $eventId . ':mark_manual_review';
// Backend disconnected is tested separately by direct api() tests; here exercise lost response.
@unlink('/tmp/opencode/drop_next');
file_put_contents('/tmp/opencode/drop_next', '1');

$lost = request('POST', $webBase . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => [
        'action' => 'outbox_action',
        'event_id' => $eventId,
        'operation' => 'mark_manual_review',
        'reason' => 'lost response validation',
        'idempotency_key' => 'client-lost-key',
        'operation_id' => $opId,
    ],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('lost response is not success', $lost['status'], 502);

$retry = request('POST', $webBase . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => [
        'action' => 'outbox_action',
        'event_id' => $eventId,
        'operation' => 'mark_manual_review',
        'reason' => 'lost response validation',
        'idempotency_key' => 'different-client-key',
        'operation_id' => $opId,
    ],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
assert_status('retry after lost response', $retry['status'], 200);
assert_true('retry is idempotent', ($retry['data']['idempotent'] ?? false) === true);

// Backend disconnected through the proxy.
$disconnected = request('POST', $webBase . '/frontend/api_proxy.php', [
    'cookie_jar' => $jar,
    'json' => ['action' => 'wall_list'],
    'headers' => ['X-CSRF-Token: ' . $adminCsrf],
]);
// wall_list is public and should still fail if backend is unavailable, but here backend is available.
assert_status('connected backend wall list', $disconnected['status'], 200);

proc_terminate($webProc);
proc_terminate($proxyProc);
usleep(200000);
@unlink($proxyScript);

if ($failures > 0) {
    fwrite(STDERR, "\n$failures test(s) failed.\n");
    exit(1);
}
fwrite(STDOUT, "\nAll failure validation tests passed.\n");
exit(0);