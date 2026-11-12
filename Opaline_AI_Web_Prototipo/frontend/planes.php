<?php
$pageTitle = 'Planes';
require __DIR__ . '/includes/header.php';

$res = api('GET', '/planes', null, false);
$plans = is_array($res['data']) ? $res['data'] : [];
$user = current_user();
?>

<div class="section-head" style="margin-top:8px">
  <div>
    <span class="eyebrow">Suscripciones</span>
    <h1 style="font-size:clamp(32px,4vw,48px)">Un plan para cada forma de trabajar</h1>
    <p>Precios en pesos mexicanos. Sin permanencia forzosa: cancela cuando quieras desde tu cuenta.</p>
  </div>
</div>

<?php if (!$plans): ?>
  <div class="alert alert-error">No se pudieron cargar los planes. Asegúrate de que el backend Rust esté activo en <strong>127.0.0.1:8090</strong>.</div>
<?php else: ?>
  <div class="grid grid-3">
    <?php foreach ($plans as $i => $plan): ?>
      <?php $featured = $i === 1; ?>
      <article class="card plan-card <?= $featured ? 'featured' : '' ?>">
        <?php if ($featured): ?><span class="plan-badge">Recomendado</span><?php endif; ?>
        <span class="plan-name"><?= e($plan['name'] ?? 'Plan') ?></span>
        <div class="price"><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?><small> /<?= e($plan['period'] ?? 'mes') ?></small></div>
        <p class="hint" style="margin:0"><?= (int) ($plan['messages_per_month'] ?? 0) ?> mensajes de IA al mes</p>
        <ul class="feature-list">
          <?php foreach (($plan['features'] ?? []) as $feature): ?>
            <li><?= e($feature) ?></li>
          <?php endforeach; ?>
        </ul>
        <a class="btn <?= $featured ? 'btn-primary' : 'btn-ghost' ?>" style="margin-top:auto" href="<?= $user ? 'checkout.php?plan=' . urlencode((string) ($plan['id'] ?? '')) : 'login.php' ?>">
          <?= $user ? 'Elegir ' . e($plan['name'] ?? '') : 'Entrar para contratar' ?>
        </a>
      </article>
    <?php endforeach; ?>
  </div>

  <div class="card" style="margin-top:28px">
    <h2 style="font-size:24px">Comparación rápida</h2>
    <div style="overflow-x:auto">
      <table class="table">
        <thead>
          <tr>
            <th>Característica</th>
            <?php foreach ($plans as $plan): ?><th><?= e($plan['name'] ?? '') ?></th><?php endforeach; ?>
          </tr>
        </thead>
        <tbody>
          <tr><td><strong>Precio mensual</strong></td><?php foreach ($plans as $plan): ?><td><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?></td><?php endforeach; ?></tr>
          <tr><td><strong>Mensajes / mes</strong></td><?php foreach ($plans as $plan): ?><td><?= (int) ($plan['messages_per_month'] ?? 0) ?></td><?php endforeach; ?></tr>
          <tr><td><strong>Facturación</strong></td><?php foreach ($plans as $plan): ?><td>Mensual</td><?php endforeach; ?></tr>
          <tr><td><strong>Acceso web + móvil</strong></td><?php foreach ($plans as $plan): ?><td>✓</td><?php endforeach; ?></tr>
        </tbody>
      </table>
    </div>
  </div>
<?php endif; ?>

<div class="grid grid-3" style="margin-top:28px">
  <div class="card"><h3>Pagos seguros</h3><p class="hint">El flujo de pago está integrado con PayPal. En este entorno local se ejecuta en modo demostración y activa tu suscripción al instante.</p></div>
  <div class="card"><h3>Sin sorpresas</h3><p class="hint">Puedes cancelar la renovación desde tu cuenta. Mantienes el acceso hasta el final del periodo pagado.</p></div>
  <div class="card"><h3>¿Necesitas ayuda?</h3><p class="hint">Escríbenos desde la sección de soporte de tu cuenta o consulta las preguntas frecuentes del proyecto.</p></div>
</div>

<?php require __DIR__ . '/includes/footer.php'; ?>
