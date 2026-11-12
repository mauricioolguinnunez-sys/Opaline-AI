<?php
declare(strict_types=1);
require_once __DIR__ . '/config.php';

header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');

$raw = file_get_contents('php://input');
$input = json_decode($raw ?: '{}', true);
if (!is_array($input)) $input = [];

$action = (string) ($input['action'] ?? $_GET['action'] ?? '');
$requestStartedAt = microtime(true);
$publicActions = ['models', 'status', 'plans', 'wall_list'];
if (!in_array($action, $publicActions, true) && !is_logged_in()) {
    http_response_code(401);
    echo json_encode(['error' => 'Inicia sesión para continuar.']);
    exit;
}
if (!in_array($action, $publicActions, true) && is_logged_in()) {
    require_csrf();
}

function respond(array $res): void {
    global $requestStartedAt, $action;
    $status = (int) ($res['status'] ?? 0);
    if ($status === 0) $status = 502;
    $durationMs = isset($requestStartedAt) ? (int) round((microtime(true) - $requestStartedAt) * 1000) : 0;
    error_log(sprintf('opaline proxy action=%s status=%d duration_ms=%d', (string) ($action ?? 'unknown'), $status, $durationMs));
    http_response_code($status);
    echo json_encode($res['data'] ?? ['error' => 'Sin respuesta'], JSON_UNESCAPED_UNICODE);
    exit;
}

function body(array $input, array $keys): array {
    $out = [];
    foreach ($keys as $key) {
        if (array_key_exists($key, $input)) $out[$key] = $input[$key];
    }
    return $out;
}

switch ($action) {
    case 'models':
        respond(api('GET', '/ai/models', null, false));

    case 'status':
        respond(api('GET', '/ai/status', null, false));

    case 'plans':
        respond(api('GET', '/planes', null, false));

    case 'conversations':
        respond(api('GET', '/ai/conversations'));

    case 'chat':
        $message = trim((string) ($input['message'] ?? ''));
        if ($message === '') {
            http_response_code(400);
            echo json_encode(['error' => 'Escribe un mensaje.']);
            exit;
        }
        $payload = [
            'message' => $message,
            'model' => (string) ($input['model'] ?? ''),
            'conversation_id' => $input['conversation_id'] ?? null,
            'history' => is_array($input['history'] ?? null) ? $input['history'] : [],
        ];
        if (isset($input['idempotency_key']) && trim((string) $input['idempotency_key']) !== '') {
            $payload['idempotency_key'] = trim((string) $input['idempotency_key']);
        }
        respond(api('POST', '/ai/chat', $payload));

    case 'delete_conversation':
        $id = (string) ($input['id'] ?? '');
        if ($id === '') { http_response_code(400); echo json_encode(['error' => 'Conversación inválida.']); exit; }
        respond(api('DELETE', '/ai/conversations/' . rawurlencode($id)));

    case 'wall_list':
        respond(api('GET', '/muro', null, false));

    case 'wall_publish':
        $content = trim((string) ($input['content'] ?? ''));
        $rating = isset($input['rating']) ? (int) $input['rating'] : null;
        if (mb_strlen($content) < 20 || mb_strlen($content) > 1000) {
            http_response_code(400);
            echo json_encode(['error' => 'La publicación debe tener entre 20 y 1000 caracteres.']);
            exit;
        }
        if ($rating !== null && ($rating < 1 || $rating > 5)) $rating = null;
        respond(api('POST', '/muro', [
            'content' => $content,
            'rating' => $rating,
            'idempotency_key' => (string) ($input['idempotency_key'] ?? ''),
        ]));

    case 'wall_report':
        $postId = (string) ($input['post_id'] ?? '');
        if ($postId === '') { http_response_code(400); echo json_encode(['error' => 'Publicación inválida.']); exit; }
        respond(api('POST', '/muro/' . rawurlencode($postId) . '/reportar', [
            'reason' => (string) ($input['reason'] ?? 'otro'),
            'detail' => (string) ($input['detail'] ?? ''),
        ]));

    case 'wall_delete':
        $postId = (string) ($input['post_id'] ?? '');
        if ($postId === '') { http_response_code(400); echo json_encode(['error' => 'Publicación inválida.']); exit; }
        respond(api('DELETE', '/muro/' . rawurlencode($postId)));

    case 'wall_moderation_list':
        respond(api('GET', '/muro/moderacion'));

    case 'wall_moderate':
        $postId = (string) ($input['post_id'] ?? '');
        $action = (string) ($input['action'] ?? '');
        $reason = trim((string) ($input['reason'] ?? ''));
        if ($postId === '' || $action === '') { http_response_code(400); echo json_encode(['error' => 'Acción o publicación inválida.']); exit; }
        $payload = ['action' => $action];
        if ($reason !== '') $payload['reason'] = $reason;
        respond(api('POST', '/muro/' . rawurlencode($postId) . '/moderar', $payload));

    case 'outbox_discrepancies':
        $limit = isset($input['limit']) ? (int) $input['limit'] : 20;
        $offset = isset($input['offset']) ? (int) $input['offset'] : 0;
        $limit = max(1, min(50, $limit));
        $offset = max(0, $offset);
        $query = ['limit' => $limit, 'offset' => $offset];
        if (!empty($input['status'])) $query['status'] = (string) $input['status'];
        if (!empty($input['event_type'])) $query['event_type'] = (string) $input['event_type'];
        respond(api('GET', '/admin/outbox/discrepancies?' . http_build_query($query)));

    case 'outbox_event':
        $id = (string) ($input['id'] ?? '');
        if (!preg_match('/^[A-Za-z0-9-]{8,64}$/', $id)) { http_response_code(400); echo json_encode(['error' => 'Evento inválido.']); exit; }
        respond(api('GET', '/admin/outbox/events/' . rawurlencode($id)));

    case 'outbox_history':
        $id = (string) ($input['id'] ?? '');
        if (!preg_match('/^[A-Za-z0-9-]{8,64}$/', $id)) { http_response_code(400); echo json_encode(['error' => 'Evento inválido.']); exit; }
        respond(api('GET', '/admin/outbox/events/' . rawurlencode($id) . '/history'));

    case 'outbox_action':
        $id = (string) ($input['event_id'] ?? $input['id'] ?? '');
        $action = (string) ($input['operation'] ?? $input['admin_action'] ?? '');
        $reason = trim((string) ($input['reason'] ?? ''));
        $operationId = trim((string) ($input['operation_id'] ?? ''));
        $key = trim((string) ($input['idempotency_key'] ?? ''));
        if (!preg_match('/^[A-Za-z0-9-]{8,64}$/', $id) || $action === '' || $reason === '') {
            http_response_code(400);
            echo json_encode(['error' => 'Acción, motivo o evento inválidos.']);
            exit;
        }
        if ($operationId !== '') {
            $storedKey = outbox_pending_key($operationId);
            if ($storedKey !== null) {
                $key = $storedKey;
            } elseif ($key === '') {
                try {
                    $key = bin2hex(random_bytes(16));
                } catch (Throwable $e) {
                    $key = hash('sha256', $operationId . microtime(true), false);
                }
            }
            if ($key === '') {
                http_response_code(400);
                echo json_encode(['error' => 'Clave de idempotencia inválida.']);
                exit;
            }
            remember_outbox_pending_key($operationId, $key);
        } elseif ($key === '') {
            http_response_code(400);
            echo json_encode(['error' => 'Clave de idempotencia inválida.']);
            exit;
        }
        $payload = [
            'action' => $action,
            'reason' => $reason,
            'idempotency_key' => $key,
        ];
        if (!empty($input['correlation_id'])) $payload['correlation_id'] = (string) $input['correlation_id'];
        $res = api('POST', '/admin/outbox/events/' . rawurlencode($id) . '/actions', $payload, true, true, 30);
        if (($res['status'] ?? 0) === 200 && $operationId !== '') {
            clear_outbox_pending_key($operationId);
        }
        respond($res);

    case 'wall_edit':
        $postId = (string) ($input['post_id'] ?? '');
        $content = trim((string) ($input['content'] ?? ''));
        if ($postId === '' || $content === '') { http_response_code(400); echo json_encode(['error' => 'Publicación o contenido inválido.']); exit; }
        respond(api('PUT', '/muro/' . rawurlencode($postId), ['content' => $content]));

    case 'subscription':
        respond(api('GET', '/suscripcion'));

    case 'payments':
        respond(api('GET', '/payments/history'));

    case 'create_order':
        $planId = (string) ($input['plan_id'] ?? '');
        if ($planId === '') { http_response_code(400); echo json_encode(['error' => 'Selecciona un plan.']); exit; }
        respond(api('POST', '/payments/paypal/create', ['plan_id' => $planId]));

    case 'verify_order':
        $orderId = (string) ($input['order_id'] ?? '');
        if ($orderId === '') { http_response_code(400); echo json_encode(['error' => 'Orden inválida.']); exit; }
        respond(api('GET', '/payments/paypal/verify/' . rawurlencode($orderId)));

    case 'cancel_renewal':
        respond(api('POST', '/suscripcion/cancelar-renovacion'));

    case 'restore_subscription':
        respond(api('POST', '/suscripcion/restaurar'));

    case 'change_password':
        $current = (string) ($input['current_password'] ?? '');
        $new = (string) ($input['new_password'] ?? '');
        if (mb_strlen($new) < 10) {
            http_response_code(400);
            echo json_encode(['error' => 'La nueva contraseña debe tener al menos 10 caracteres.']);
            exit;
        }
        respond(api('POST', '/auth/change-password', [
            'current_password' => $current,
            'new_password' => $new,
        ]));

    default:
        http_response_code(400);
        echo json_encode(['error' => 'Acción no reconocida.']);
}
