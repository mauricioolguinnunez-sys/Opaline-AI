<?php
require_once __DIR__ . '/config.php';
require_login();

$user = current_user();
$subRes = api('GET', '/suscripcion');
$sub = $subRes['data'] ?? ['status' => 'none'];
$payRes = api('GET', '/payments/history');
$payments = is_array($payRes['data']) ? $payRes['data'] : [];
$status = (string) ($sub['status'] ?? 'none');
$statusLabel = [
    'active' => 'Activa',
    'pending' => 'Pendiente de pago',
    'cancelled' => 'Cancelada',
    'grace' => 'En periodo de gracia',
    'none' => 'Sin suscripción',
][$status] ?? ucfirst($status);
$pageTitle = 'Mi cuenta';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Mi cuenta</span>
    <h1 style="font-size:clamp(30px,4vw,42px);margin-top:12px">Hola, <?= e(explode(' ', (string) ($user['name'] ?? 'Usuario'))[0]) ?> 👋</h1>
    <p>Administra tu suscripción, revisa tus pagos y mantén tu cuenta segura.</p>
  </div>
</div>

<div id="account-toast" class="flash" style="display:none;position:static;margin:0 0 16px"></div>

<div class="account-grid">
  <div>
    <section class="sub-hero">
      <span class="status-pill">● <?= e($statusLabel) ?></span>
      <h2><?= e($sub['plan_name'] ?? 'Sin plan activo') ?></h2>
      <p><?= $status === 'active' ? 'Tu suscripción está al día. Disfruta de Opaline AI sin interrupciones.' : 'Elige un plan para desbloquear todos los beneficios de Opaline AI.' ?></p>
      <div class="kv">
        <div><span>Precio</span><strong><?= isset($sub['price']) ? money($sub['price'], 'MXN') . ' / mes' : '—' ?></strong></div>
        <div><span>Inicio</span><strong><?= e(substr((string) ($sub['start_date'] ?? '—'), 0, 10)) ?></strong></div>
        <div><span>Próxima renovación</span><strong><?= e(substr((string) ($sub['end_date'] ?? '—'), 0, 10)) ?></strong></div>
      </div>
      <div style="display:flex;gap:10px;flex-wrap:wrap;margin-top:22px">
        <?php if ($status === 'active'): ?>
          <button class="btn" style="background:#fff;color:#4e2ea2" type="button" id="cancel-renewal">Cancelar renovación</button>
          <a class="btn" style="background:rgba(255,255,255,.16);color:#fff;border-color:rgba(255,255,255,.25)" href="planes.php">Cambiar de plan</a>
        <?php elseif ($status === 'cancelled'): ?>
          <button class="btn" style="background:#fff;color:#4e2ea2" type="button" id="restore-sub">Reactivar suscripción</button>
          <a class="btn" style="background:rgba(255,255,255,.16);color:#fff;border-color:rgba(255,255,255,.25)" href="planes.php">Ver planes</a>
        <?php else: ?>
          <a class="btn" style="background:#fff;color:#4e2ea2" href="planes.php">Elegir un plan</a>
        <?php endif; ?>
      </div>
    </section>

    <section class="card" style="margin-top:18px">
      <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:8px">
        <h2 style="font-size:22px;margin:0">Historial de pagos</h2>
        <a class="btn btn-ghost btn-sm" href="planes.php">Nueva compra</a>
      </div>
      <?php if (!$payments): ?>
        <div class="empty" style="padding:24px">
          <p>Aún no tienes pagos registrados.</p>
          <a class="btn btn-soft btn-sm" href="planes.php">Ver planes</a>
        </div>
      <?php else: ?>
        <div style="overflow-x:auto">
          <table class="table">
            <thead><tr><th>Fecha</th><th>Plan</th><th>Importe</th><th>Estado</th><th>Factura</th></tr></thead>
            <tbody>
              <?php foreach ($payments as $p): ?>
                <tr>
                  <td><?= e(substr((string) ($p['created_at'] ?? ''), 0, 16)) ?></td>
                  <td><strong><?= e($p['plan_name'] ?? 'Plan') ?></strong></td>
                  <td><?= money($p['amount'] ?? 0, $p['currency'] ?? 'MXN') ?></td>
                  <td><span class="post-badge" style="background:<?= ($p['status'] ?? '') === 'completed' ? '#e8f8f0' : '#fff6e8' ?>;color:<?= ($p['status'] ?? '') === 'completed' ? '#147a50' : '#8a5a12' ?>"><?= e($p['status'] ?? '') ?></span></td>
                  <td>
                    <?php if (!empty($p['invoice_id'])): ?>
                      <a href="factura.php?id=<?= e($p['invoice_id']) ?>" target="_blank" rel="noopener">Ver / <?= e($p['folio'] ?? 'factura') ?></a>
                    <?php else: ?>—<?php endif; ?>
                  </td>
                </tr>
              <?php endforeach; ?>
            </tbody>
          </table>
        </div>
      <?php endif; ?>
    </section>
  </div>

  <aside>
    <section class="card">
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:16px">
        <span class="avatar" style="width:48px;height:48px;font-size:18px"><?= e(strtoupper(substr((string) ($user['name'] ?? 'U'), 0, 1))) ?></span>
        <div>
          <strong style="display:block"><?= e($user['name'] ?? 'Usuario') ?></strong>
          <span class="hint"><?= e($user['email'] ?? '') ?></span>
        </div>
      </div>
      <div class="kv" style="margin-top:0">
        <div style="border-bottom:1px solid var(--border);color:var(--ink-soft)"><span>Rol</span><strong><?= e($user['role'] === 'admin' ? 'Administrador' : 'Cliente') ?></strong></div>
        <div style="border-bottom:1px solid var(--border);color:var(--ink-soft)"><span>Correo</span><strong><?= !empty($user['email_verified']) ? 'Verificado' : 'Sin verificar' ?></strong></div>
      </div>
      <a class="btn btn-ghost btn-block" style="margin-top:16px" href="logout.php">Cerrar sesión</a>
    </section>

    <section class="card" style="margin-top:16px">
      <h3>Cambiar contraseña</h3>
      <form id="password-form">
        <div class="field"><label for="current_password">Contraseña actual</label><input class="input" type="password" id="current_password" name="current_password" required></div>
        <div class="field"><label for="new_password">Nueva contraseña</label><input class="input" type="password" id="new_password" name="new_password" minlength="10" required></div>
        <button class="btn btn-soft btn-block" type="submit">Actualizar contraseña</button>
      </form>
      <p class="hint" style="margin:12px 0 0">Al cambiarla se cerrarán las demás sesiones activas.</p>
    </section>

    <section class="card" style="margin-top:16px">
      <h3>Soporte</h3>
      <p class="hint">¿Problemas con tu cuenta o pagos? Escríbenos a <a href="mailto:soporte@opaline-ai.com">soporte@opaline-ai.com</a>.</p>
    </section>
  </aside>
</div>

<script>
const toast = document.getElementById('account-toast');
function showToast(msg, type = 'success') { toast.textContent = msg; toast.className = 'flash flash-' + type; toast.style.display = 'block'; setTimeout(() => toast.style.display = 'none', 4200); }
async function proxy(payload) {
  const csrf = document.querySelector('meta[name="csrf-token"]')?.content || '';
  const res = await fetch('api_proxy.php', {method:'POST', headers:{'Content-Type':'application/json', 'X-CSRF-Token': csrf}, body:JSON.stringify(payload)});
  let data = {}; try { data = await res.json(); } catch (e) {}
  if (!res.ok) throw {status:res.status, data};
  return data;
}
document.getElementById('cancel-renewal')?.addEventListener('click', async () => {
  if (!confirm('¿Cancelar la renovación? Mantendrás el acceso hasta el final del periodo.')) return;
  try { await proxy({action:'cancel_renewal'}); showToast('Renovación cancelada. Tu acceso sigue activo hasta la fecha de fin.'); setTimeout(() => location.reload(), 900); }
  catch (e) { showToast('No se pudo cancelar la renovación.', 'error'); }
});
document.getElementById('restore-sub')?.addEventListener('click', async () => {
  try { await proxy({action:'restore_subscription'}); showToast('Suscripción reactivada.'); setTimeout(() => location.reload(), 900); }
  catch (e) { showToast('No se pudo reactivar la suscripción.', 'error'); }
});
document.getElementById('password-form')?.addEventListener('submit', async e => {
  e.preventDefault();
  try {
    await proxy({action:'change_password', current_password: document.getElementById('current_password').value, new_password: document.getElementById('new_password').value});
    showToast('Contraseña actualizada correctamente.');
    e.target.reset();
  } catch (err) {
    showToast(err.data?.error || 'No se pudo cambiar la contraseña. Verifica tu contraseña actual.', 'error');
  }
});
</script>

<?php require __DIR__ . '/includes/footer.php'; ?>
