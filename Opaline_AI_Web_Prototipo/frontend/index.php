<?php
$pageTitle = 'Inicio';
require __DIR__ . '/includes/header.php';

$plansRes = api('GET', '/planes', null, false);
$plans = is_array($plansRes['data']) ? $plansRes['data'] : [];
$planCount = count($plans);
$minPrice = null;
foreach ($plans as $p) {
    $price = (float) ($p['price'] ?? 0);
    if ($minPrice === null || $price < $minPrice) $minPrice = $price;
}
$user = current_user();
?>

<section class="hero">
  <div>
    <span class="eyebrow">✦ IA open source · 100% local</span>
    <h1>Tu asistente de IA, <em>privado</em> y sin límites en la nube.</h1>
    <p>Opaline AI corre sobre LM Studio en tu propia máquina. Chatea, comparte experiencias y administra tu suscripción desde web y móvil con la misma cuenta.</p>
    <div class="hero-actions">
      <?php if ($user): ?>
        <a class="btn btn-primary" href="chat.php">Abrir el chat →</a>
        <a class="btn btn-ghost" href="muro.php">Explorar el muro</a>
      <?php else: ?>
        <a class="btn btn-primary" href="register.php">Crear cuenta gratis</a>
        <a class="btn btn-ghost" href="login.php">Ya tengo cuenta</a>
      <?php endif; ?>
    </div>
    <div class="stats">
      <div class="stat"><strong><?= $planCount ?: 3 ?></strong><span>Planes disponibles</span></div>
      <div class="stat"><strong><?= $minPrice !== null ? money($minPrice) : '$0' ?></strong><span>Desde / mes</span></div>
      <div class="stat"><strong>3</strong><span>Modelos open source</span></div>
      <div class="stat"><strong>100%</strong><span>Datos locales</span></div>
    </div>
  </div>
  <aside class="hero-card">
    <span class="pill">● LM Studio conectado</span>
    <h3 style="margin-top:14px">Opaline Chat</h3>
    <p style="color:#d9ccff;font-size:14px;margin:0">Pregunta, redacta, programa o aprende. Sin enviar tus datos a terceros.</p>
    <div class="hero-mini-chat">
      <div class="bubble">¡Hola! Soy Opaline. ¿En qué te ayudo hoy? ✨</div>
      <div class="meta"><span>qwen3.5-9b</span><span>local · privado</span></div>
    </div>
    <ul>
      <li>Modelos servidos por LM Studio en tu equipo.</li>
      <li>Historial de conversaciones sincronizado.</li>
      <li>Misma cuenta en web y app móvil.</li>
    </ul>
  </aside>
</section>

<section>
  <div class="section-head">
    <div>
      <span class="eyebrow">Planes</span>
      <h2>Elige cómo quieres usar Opaline</h2>
      <p>Precios claros en MXN. Puedes cambiar o cancelar tu suscripción cuando quieras.</p>
    </div>
    <a class="btn btn-ghost" href="planes.php">Comparar todos</a>
  </div>
  <?php if (!$plans): ?>
    <div class="alert alert-warning">No se pudieron cargar los planes. Verifica que el backend Rust esté corriendo en <strong>127.0.0.1:8090</strong>.</div>
  <?php else: ?>
    <div class="grid grid-3">
      <?php foreach (array_slice($plans, 0, 3) as $i => $plan): ?>
        <?php $featured = $i === count(array_slice($plans, 0, 3)) - 1; ?>
        <article class="card plan-card <?= $featured ? 'featured' : '' ?>">
          <?php if ($featured): ?><span class="plan-badge">Más popular</span><?php endif; ?>
          <span class="plan-name"><?= e($plan['name'] ?? 'Plan') ?></span>
          <div class="price"><?= money($plan['price'] ?? 0, $plan['currency'] ?? 'MXN') ?><small> /<?= e($plan['period'] ?? 'mes') ?></small></div>
          <p class="hint" style="margin:0"><?= (int) ($plan['messages_per_month'] ?? 0) ?> mensajes al mes</p>
          <ul class="feature-list">
            <?php foreach (($plan['features'] ?? []) as $feature): ?>
              <li><?= e($feature) ?></li>
            <?php endforeach; ?>
          </ul>
          <a class="btn <?= $featured ? 'btn-primary' : 'btn-ghost' ?>" style="margin-top:auto" href="checkout.php?plan=<?= e($plan['id'] ?? '') ?>">Elegir <?= e($plan['name'] ?? '') ?></a>
        </article>
      <?php endforeach; ?>
    </div>
  <?php endif; ?>
</section>

<section>
  <div class="section-head">
    <div>
      <span class="eyebrow">Por qué Opaline</span>
      <h2>Todo lo que necesitas, sin fricción</h2>
    </div>
  </div>
  <div class="grid grid-3">
    <article class="card">
      <div class="brand-mark" style="margin-bottom:14px">✦</div>
      <h3>Privacidad real</h3>
      <p class="hint">Tus conversaciones se guardan en tu base de datos local. Nada se envía a OpenAI, Google ni otros proveedores.</p>
    </article>
    <article class="card">
      <div class="brand-mark" style="margin-bottom:14px">⚡</div>
      <h3>Rápido y sin cuotas ocultas</h3>
      <p class="hint">Backend en Rust y modelos optimizados con LM Studio para respuestas ágiles en tu propio hardware.</p>
    </article>
    <article class="card">
      <div class="brand-mark" style="margin-bottom:14px">♡</div>
      <h3>Comunidad Opaline</h3>
      <p class="hint">Comparte tu experiencia en el muro, descubre casos de uso y aprende de otras personas.</p>
    </article>
  </div>
</section>

<section class="card" style="margin-top:46px;background:linear-gradient(145deg,#2a1668,#4e2ea2 60%,#9369e5);color:#fff">
  <div style="max-width:680px">
    <span class="pill" style="background:rgba(255,255,255,.16);padding:6px 10px;border-radius:999px;font-size:12px">Empieza hoy</span>
    <h2 style="color:#fff;margin-top:14px">Tu IA privada te está esperando.</h2>
    <p style="color:#ddd2ff">Crea tu cuenta, elige un plan y empieza a chatear con modelos open source en minutos.</p>
    <a class="btn" style="background:#fff;color:#4e2ea2" href="<?= $user ? 'chat.php' : 'register.php' ?>"><?= $user ? 'Ir al chat' : 'Crear mi cuenta' ?> →</a>
  </div>
</section>

<?php require __DIR__ . '/includes/footer.php'; ?>
