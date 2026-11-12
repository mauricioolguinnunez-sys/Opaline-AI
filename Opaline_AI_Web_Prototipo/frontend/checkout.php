<?php
require_once __DIR__ . '/config.php';

$planId = (string) ($_GET['plan'] ?? $_POST['plan_id'] ?? '');
$plansRes = api('GET', '/planes', null, false);
$plans = is_array($plansRes['data']) ? $plansRes['data'] : [];
$plan = null;
foreach ($plans as $p) {
    if (($p['id'] ?? '') === $planId || ($p['code'] ?? '') === $planId) { $plan = $p; break; }
}
if (!$plan && $plans) $plan = $plans[0];
$user = current_user();
$error = '';

if ($_SERVER['REQUEST_METHOD'] === 'POST' && $user && $plan) {
    require_csrf();
    $order = api('POST', '/payments/paypal/create', ['plan_id' => $plan['id']]);
    if ($order['status'] === 200 && !empty($order['data']['order_id'])) {
        $verify = api('GET', '/payments/paypal/verify/' . rawurlencode($order['data']['order_id']));
        if ($verify['status'] === 200) {
            flash('¡Pago completado! Tu plan ' . ($plan['name'] ?? '') . ' ya está activo.', 'success');
            redirect('cuenta.php');
        }
        $error = 'El pago se creó pero no se pudo confirmar. Revisa tu cuenta o intenta de nuevo.';
    } else {
        $error = api_error($order, 'No se pudo iniciar el pago. ¿Está activo el backend?');
    }
}
$pageTitle = 'Checkout';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Checkout</span>
    <h1 style="font-size:clamp(30px,4vw,42px);margin-top:12px">Confirma tu plan</h1>
    <p>Revisa los detalles antes de activar tu suscripción. En este entorno el pago se procesa en modo demostración.</p>
  </div>
</div>

<?php if (!$user): ?>
  <div class="card" style="max-width:560px;margin:0 auto;text-align:center">
    <div class="brand-mark" style="margin:0 auto 14px">✦</div>
    <h2>Inicia sesión para continuar</h2>
    <p class="hint">Necesitas una cuenta para asociar tu suscripción y acceder al chat.</p>
    <a class="btn btn-primary" href="login.php">Iniciar sesión</a>
    <a class="btn btn-ghost" href="register.php">Crear cuenta</a>
  </div>
<?php elseif (!$plan): ?>
  <div class="alert alert-error">No se encontró el plan solicitado. <a href="planes.php">Ver planes disponibles</a>.</div>
<?php else: ?>
  <div class="grid grid-2" style="max-width:900px;margin:0 auto;align-items:start">
    <div class="card">
      <span class="plan-name"><?= e($plan['name'] ?? 'Plan') ?></span>
      <div class="price" style="margin:8px 0"><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?><small> /<?= e($plan['period'] ?? 'mes') ?></small></div>
      <p class="hint"><?= (int) ($plan['messages_per_month'] ?? 0) ?> mensajes de IA al mes.</p>
      <ul class="feature-list" style="margin-top:16px">
        <?php foreach (($plan['features'] ?? []) as $feature): ?><li><?= e($feature) ?></li><?php endforeach; ?>
      </ul>
    </div>

    <div class="card">
      <h2 style="font-size:22px">Resumen de compra</h2>
      <?php if ($error): ?><div class="alert alert-error" style="margin-bottom:14px"><?= e($error) ?></div><?php endif; ?>
      <div class="kv" style="color:var(--ink-soft)">
        <div style="border-bottom:1px solid var(--border)"><span>Plan</span><strong><?= e($plan['name'] ?? '') ?></strong></div>
        <div style="border-bottom:1px solid var(--border)"><span>Periodo</span><strong>Mensual</strong></div>
        <div style="border-bottom:1px solid var(--border)"><span>Subtotal</span><strong><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?></strong></div>
        <div style="border-bottom:1px solid var(--border)"><span>IVA</span><strong>Incluido</strong></div>
        <div style="border-bottom:1px solid var(--border);font-size:16px"><span>Total hoy</span><strong style="color:var(--primary-deep)"><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?></strong></div>
      </div>
      <form method="post" style="margin-top:18px">
        <input type="hidden" name="plan_id" value="<?= e($plan['id'] ?? '') ?>">
        <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
        <button class="btn btn-primary btn-block" type="submit">Pagar y activar plan</button>
      </form>
      <p class="hint" style="margin:14px 0 0;text-align:center">🔒 Pago demostrativo local. No se realizará ningún cargo real.</p>
      <a href="planes.php" style="display:block;text-align:center;margin-top:10px;font-size:13px">Volver a planes</a>
    </div>
  </div>
<?php endif; ?>

<?php require __DIR__ . '/includes/footer.php'; ?>
