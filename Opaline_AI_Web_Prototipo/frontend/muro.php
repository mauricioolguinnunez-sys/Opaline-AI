<?php
$pageTitle = 'Muro de experiencias';
require __DIR__ . '/includes/header.php';

$res = api('GET', '/muro', null, false);
$posts = is_array($res['data']) ? $res['data'] : [];
$user = current_user();
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Comunidad Opaline</span>
    <h1 style="font-size:clamp(30px,4vw,44px);margin-top:12px">Muro de experiencias</h1>
    <p>Descubre cómo otras personas usan Opaline AI. Comparte tu propia experiencia y ayuda a la comunidad.</p>
  </div>
  <span class="pill" style="background:#efe7ff;color:var(--primary-deep);padding:8px 12px;border-radius:999px;font-size:13px"><?= count($posts) ?> publicaciones</span>
</div>

<div id="wall-toast" class="flash" style="display:none;position:static;margin:0 0 16px"></div>

<div class="wall-layout">
  <section>
    <div id="posts">
      <?php if (!$posts): ?>
        <div class="card empty">
          <h3>Aún no hay publicaciones</h3>
          <p>Sé la primera persona en compartir su experiencia con Opaline AI.</p>
        </div>
      <?php else: ?>
        <?php foreach ($posts as $post): ?>
          <?php $isOwner = $user && (($post['user_id'] ?? '') === ($user['id'] ?? '')); ?>
          <article class="card post" style="margin-bottom:14px" data-post-id="<?= e($post['id'] ?? '') ?>">
            <div class="post-head">
              <span class="avatar"><?= e(strtoupper(substr((string) ($post['alias'] ?? 'U'), 0, 1))) ?></span>
              <div>
                <div class="post-name"><?= e($post['alias'] ?? 'Usuario') ?></div>
                <div class="post-date"><?= e(substr((string) ($post['created_at'] ?? ''), 0, 16)) ?></div>
              </div>
              <span class="post-badge"><?= e($post['plan'] ?? 'Opaline') ?></span>
              <?php if ($post['rating'] ?? null): ?><span class="stars" style="margin-left:auto"><?= str_repeat('★', (int) $post['rating']) ?><?= str_repeat('☆', 5 - (int) $post['rating']) ?></span><?php endif; ?>
            </div>
            <p class="post-content" style="margin:0"><?= nl2br(e($post['content'] ?? '')) ?></p>
            <div class="post-actions">
              <?php if ($isOwner): ?>
                <button class="btn btn-ghost btn-sm delete-post" type="button" data-post-id="<?= e($post['id'] ?? '') ?>">Eliminar</button>
              <?php elseif ($user): ?>
                <button class="btn btn-ghost btn-sm report-post" type="button" data-post-id="<?= e($post['id'] ?? '') ?>">Reportar</button>
              <?php endif; ?>
            </div>
          </article>
        <?php endforeach; ?>
      <?php endif; ?>
    </div>
  </section>

  <aside>
    <?php if ($user): ?>
      <div class="card">
        <h3>Comparte tu experiencia</h3>
        <p class="hint">Cuéntanos qué te gusta de Opaline o cómo te está ayudando.</p>
        <form id="publish-form">
          <div class="field">
            <label for="content">Tu publicación</label>
            <textarea class="input" id="content" name="content" rows="5" minlength="20" maxlength="1000" placeholder="Escribe entre 20 y 1000 caracteres…" required></textarea>
            <div class="hint" style="display:flex;justify-content:space-between"><span>Mínimo 20 caracteres</span><span id="char-count">0 / 1000</span></div>
          </div>
          <div class="field">
            <label>Calificación (opcional)</label>
            <div class="rating-input" id="rating-input">
              <?php for ($i = 1; $i <= 5; $i++): ?><button type="button" data-value="<?= $i ?>" aria-label="<?= $i ?> estrellas">★</button><?php endfor; ?>
            </div>
            <input type="hidden" id="rating" name="rating" value="">
          </div>
          <button class="btn btn-primary btn-block" type="submit" id="publish-btn">Publicar en el muro</button>
        </form>
      </div>
      <div class="card" style="margin-top:14px">
        <h3>Normas de la comunidad</h3>
        <ul class="feature-list">
          <li>Sé respetuoso y constructivo.</li>
          <li>No compartas datos personales de terceros.</li>
          <li>Las publicaciones con 3 reportes se ocultan automáticamente.</li>
        </ul>
      </div>
    <?php else: ?>
      <div class="card" style="text-align:center">
        <div class="brand-mark" style="margin:0 auto 14px">✦</div>
        <h3>Únete a la conversación</h3>
        <p class="hint">Inicia sesión para publicar tu experiencia y reportar contenido inapropiado.</p>
        <a class="btn btn-primary btn-block" href="login.php">Iniciar sesión</a>
        <a class="btn btn-ghost btn-block" style="margin-top:8px" href="register.php">Crear cuenta</a>
      </div>
    <?php endif; ?>
  </aside>
</div>

<script>
const loggedIn = <?= $user ? 'true' : 'false' ?>;
const toastEl = document.getElementById('wall-toast');
function showToast(message, type = 'success') {
  toastEl.textContent = message;
  toastEl.className = 'flash flash-' + type;
  toastEl.style.display = 'block';
  clearTimeout(window.__wallToast);
  window.__wallToast = setTimeout(() => { toastEl.style.display = 'none'; }, 4200);
}
async function proxy(payload) {
  const csrf = document.querySelector('meta[name="csrf-token"]')?.content || '';
  const res = await fetch('api_proxy.php', {method:'POST', headers:{'Content-Type':'application/json', 'X-CSRF-Token': csrf}, body:JSON.stringify(payload)});
  let data = {}; try { data = await res.json(); } catch (e) {}
  if (!res.ok) throw {status:res.status, data};
  return data;
}

const contentEl = document.getElementById('content');
if (contentEl) {
  contentEl.addEventListener('input', () => {
    document.getElementById('char-count').textContent = contentEl.value.length + ' / 1000';
  });
}
document.querySelectorAll('.rating-input button').forEach(btn => {
  btn.addEventListener('click', () => {
    const value = Number(btn.dataset.value);
    const hidden = document.getElementById('rating');
    hidden.value = hidden.value == String(value) ? '' : String(value);
    document.querySelectorAll('.rating-input button').forEach(b => b.classList.toggle('on', Number(b.dataset.value) <= Number(hidden.value || 0)));
  });
});

document.getElementById('publish-form')?.addEventListener('submit', async e => {
  e.preventDefault();
  const btn = document.getElementById('publish-btn');
  const content = contentEl.value.trim();
  if (content.length < 20) { showToast('Tu publicación necesita al menos 20 caracteres.', 'error'); return; }
  btn.disabled = true; btn.textContent = 'Publicando…';
  try {
    const idempotencyKey = (typeof crypto !== 'undefined' && crypto.randomUUID) ? crypto.randomUUID() : (Date.now().toString(36) + Math.random().toString(36).slice(2));
    await proxy({action:'wall_publish', content, rating: document.getElementById('rating').value || null, idempotency_key: idempotencyKey});
    showToast('¡Publicación creada! Actualizando el muro…', 'success');
    setTimeout(() => location.reload(), 700);
  } catch (err) {
    const msg = err.data?.error || (err.status === 401 ? 'Inicia sesión para publicar.' : 'No se pudo publicar. Inténtalo de nuevo.');
    showToast(msg, 'error');
    btn.disabled = false; btn.textContent = 'Publicar en el muro';
  }
});

document.querySelectorAll('.delete-post').forEach(btn => {
  btn.addEventListener('click', async () => {
    if (!confirm('¿Eliminar esta publicación?')) return;
    try {
      await proxy({action:'wall_delete', post_id: btn.dataset.postId});
      showToast('Publicación eliminada.', 'success');
      setTimeout(() => location.reload(), 500);
    } catch (e) { showToast('No se pudo eliminar la publicación.', 'error'); }
  });
});
document.querySelectorAll('.report-post').forEach(btn => {
  btn.addEventListener('click', async () => {
    const reason = prompt('Motivo del reporte (spam, ofensivo, datos personales, otro):', 'otro');
    if (!reason) return;
    try {
      await proxy({action:'wall_report', post_id: btn.dataset.postId, reason});
      showToast('Reporte enviado. Gracias por cuidar la comunidad.', 'success');
    } catch (e) { showToast('No se pudo enviar el reporte.', 'error'); }
  });
});
</script>

<?php require __DIR__ . '/includes/footer.php'; ?>
