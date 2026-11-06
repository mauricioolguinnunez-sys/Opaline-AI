<?php
function call_lm_studio_models(): array {
    $url = 'http://localhost:8000/api/v1/ai/models';
    $ch = curl_init($url);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
    curl_setopt($ch, CURLOPT_TIMEOUT, 5);
    $res = curl_exec($ch);
    $http = curl_getinfo($ch, CURLINFO_HTTP_CODE);
    curl_close($ch);
    if ($res === false || $http < 200 || $http >= 300) {
        return ['local-model'];
    }
    $data = json_decode($res, true);
    $models = $data['models'] ?? [];
    return $models ?: ['local-model'];
}

function call_lm_studio_chat(string $message, string $model = 'local-model', array $history = []): ?string {
    $url = 'http://localhost:8000/api/v1/ai/chat';
    $payload = json_encode(['message' => $message, 'model' => $model, 'history' => $history]);
    $ch = curl_init($url);
    curl_setopt($ch, CURLOPT_POST, true);
    curl_setopt($ch, CURLOPT_POSTFIELDS, $payload);
    curl_setopt($ch, CURLOPT_HTTPHEADER, ['Content-Type: application/json']);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
    curl_setopt($ch, CURLOPT_TIMEOUT, 30);
    $res = curl_exec($ch);
    $http = curl_getinfo($ch, CURLINFO_HTTP_CODE);
    curl_close($ch);
    if ($res === false || $http < 200 || $http >= 300) {
        return null;
    }
    $data = json_decode($res, true);
    return $data['reply'] ?? null;
}
