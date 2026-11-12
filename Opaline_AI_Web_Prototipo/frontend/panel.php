<?php
require_once __DIR__ . '/config.php';
require_login();

$user = current_user();
if (($user['role'] ?? '') !== 'admin') {
    flash('No tienes permisos para acceder al panel de administración.', 'error');
    redirect('cuenta.php');
}

$expenseError = '';
if ($_SERVER['REQUEST_METHOD'] === 'POST' && isset($_POST['expense'])) {
    require_csrf();
    $category = trim((string) ($_POST['category'] ?? ''));
    $amount = (float) ($_POST['amount'] ?? 0);
    $note = trim((string) ($_POST['note'] ?? ''));
    if ($category === '' || $amount <= 0) {
        $expenseError = 'Escribe una categoría y un importe mayor a cero.';
    } else {
        $res = api('POST', '/admin/expenses', [
            'category' => $category,
            'amount' => $amount,
            'currency' => 'MXN',
            'note' => $note,
        ]);
        if ($res['status'] === 200) {
            flash('Gasto registrado correctamente.', 'success');
            redirect('panel.php');
        }
        $expenseError = api_error($res, 'No se pudo registrar el gasto.');
    }
}

$metrics = api('GET', '/admin/metrics')['data'] ?? [];
$ai = api('GET', '/admin/ai-stats')['data'] ?? [];
$financial = api('GET', '/admin/reports/financial')['data'] ?? [];
$users = api('GET', '/admin/users')['data'] ?? [];
$expenses = api('GET', '/admin/expenses')['data'] ?? [];
$revenue = (float) ($financial['revenue'] ?? 0);
$expenseTotal = (float) ($financial['expenses'] ?? 0);
$profit = $revenue - $expenseTotal;
$pageTitle = 'Panel de administración';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Administración</span>
    <h1 style="font-size:clamp(30px,4vw,42px);margin-top:12px">Panel de Opaline AI</h1>
    <p>Métricas del negocio, usuarios, uso de IA y control de gastos.</p>
  </div>
  <a class="btn btn-ghost" href="chat.php">Ir al chat</a>
  <a class="btn btn-ghost" href="moderacion.php">Moderar muro</a>
  <a class="btn btn-ghost" href="moderacion_outbox.php">Auditoría Outbox</a>
</div>

<div class="admin-grid" style="margin-bottom:18px">
  <div class="card metric"><strong><?= (int) ($metrics['users'] ?? count($users)) ?></strong><span>Usuarios registrados</span></div>
  <div class="card metric"><strong><?= (int) ($metrics['active_subscriptions'] ?? 0) ?></strong><span>Suscripciones activas</span></div>
  <div class="card metric"><strong><?= money($revenue) ?></strong><span>Ingresos completados</span></div>
  <div class="card metric"><strong><?= (int) ($ai['ai_requests'] ?? 0) ?></strong><span>Solicitudes de IA</span></div>
  <div class="card metric"><strong><?= (int) ($ai['conversations'] ?? 0) ?></strong><span>Conversaciones</span></div>
  <div class="card metric"><strong style="color:<?= $profit >= 0 ? 'var(--success)' : 'var(--danger)' ?>"><?= money($profit) ?></strong><span>Utilidad estimada</span></div>
</div>

<div class="grid grid-2" style="align-items:start">
  <section class="card">
    <h2 style="font-size:22px">Usuarios</h2>
    <div style="overflow-x:auto">
      <table class="table">
        <thead><tr><th>Nombre</th><th>Correo</th><th>Rol</th><th>Alta</th></tr></thead>
        <tbody>
          <?php foreach ($users as $u): ?>
            <tr>
              <td><strong><?= e($u['name'] ?? '') ?></strong></td>
              <td><?= e($u['email'] ?? '') ?></td>
              <td><span class="post-badge" style="background:<?= ($u['role'] ?? '') === 'admin' ? '#efe7ff' : '#f5f0ff' ?>;color:<?= ($u['role'] ?? '') === 'admin' ? '#4e2ea2' : '#5c4a7a' ?>"><?= e($u['role'] ?? '') ?></span></td>
              <td><?= e(substr((string) ($u['created_at'] ?? ''), 0, 10)) ?></td>
            </tr>
          <?php endforeach; ?>
          <?php if (!$users): ?><tr><td colspan="4" class="hint">Sin usuarios.</td></tr><?php endif; ?>
        </tbody>
      </table>
    </div>
  </section>

  <section class="card">
    <h2 style="font-size:22px">Registrar gasto</h2>
    <?php if ($expenseError): ?><div class="alert alert-error" style="margin-bottom:12px"><?= e($expenseError) ?></div><?php endif; ?>
    <form method="post">
      <input type="hidden" name="expense" value="1">
      <input type="hidden" name="csrf_token" value="<?= e(csrf_token()) ?>">
      <div class="field"><label for="category">Categoría</label><input class="input" id="category" name="category" placeholder="Infraestructura, marketing…" required></div>
      <div class="field"><label for="amount">Importe (MXN)</label><input class="input" id="amount" name="amount" type="number" min="0.01" step="0.01" required></div>
      <div class="field"><label for="note">Nota (opcional)</label><input class="input" id="note" name="note" placeholder="Detalle del gasto"></div>
      <button class="btn btn-primary btn-block" type="submit">Guardar gasto</button>
    </form>
    <div class="kv" style="margin-top:18px;color:var(--ink-soft)">
      <div style="border-bottom:1px solid var(--border)"><span>Ingresos</span><strong><?= money($revenue) ?></strong></div>
      <div style="border-bottom:1px solid var(--border)"><span>Gastos</span><strong><?= money($expenseTotal) ?></strong></div>
      <div style="border-bottom:1px solid var(--border)"><span>Utilidad</span><strong style="color:<?= $profit >= 0 ? 'var(--success)' : 'var(--danger)' ?>"><?= money($profit) ?></strong></div>
    </div>
  </section>
</div>

<section class="card" style="margin-top:18px">
  <div style="display:flex;justify-content:space-between;align-items:center;gap:12px">
    <h2 style="font-size:22px;margin:0">Gastos registrados</h2>
    <span class="hint"><?= count($expenses) ?> registros</span>
  </div>
  <?php if (!$expenses): ?>
    <div class="empty" style="padding:24px"><p>Sin gastos registrados todavía.</p></div>
  <?php else: ?>
    <div style="overflow-x:auto">
      <table class="table">
        <thead><tr><th>Fecha</th><th>Categoría</th><th>Importe</th><th>Nota</th></tr></thead>
        <tbody>
          <?php foreach ($expenses as $ex): ?>
            <tr><td><?= e(substr((string) ($ex['date'] ?? ''), 0, 10)) ?></td><td><strong><?= e($ex['category'] ?? '') ?></strong></td><td><?= money($ex['amount'] ?? 0, $ex['currency'] ?? 'MXN') ?></td><td class="hint"><?= e($ex['note'] ?? '—') ?></td></tr>
          <?php endforeach; ?>
        </tbody>
      </table>
    </div>
  <?php endif; ?>
</section>

<?php require __DIR__ . '/includes/footer.php'; ?>
