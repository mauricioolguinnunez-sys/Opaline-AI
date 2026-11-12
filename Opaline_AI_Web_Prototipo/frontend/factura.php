<?php
declare(strict_types=1);
require_once __DIR__ . '/config.php';
require_login();

$invoiceId = trim((string) ($_GET['id'] ?? ''));
if (!preg_match('/^[A-Za-z0-9-]{8,64}$/', $invoiceId)) {
    http_response_code(400);
    header('Content-Type: application/json; charset=utf-8');
    echo json_encode(['error' => 'Factura inválida.'], JSON_UNESCAPED_UNICODE);
    exit;
}

$result = api('GET', '/facturas/' . rawurlencode($invoiceId) . '/pdf', null, true, true, 120, false);
$status = (int) ($result['status'] ?? 0);
if ($status !== 200) {
    http_response_code($status === 0 ? 502 : $status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    echo json_encode(['error' => api_error($result, 'No se pudo obtener la factura.')], JSON_UNESCAPED_UNICODE);
    exit;
}

header('Content-Type: text/plain; charset=utf-8');
header('Content-Disposition: inline; filename="comprobante.txt"');
header('Cache-Control: private, no-store');
echo (string) ($result['data']['raw'] ?? '');
