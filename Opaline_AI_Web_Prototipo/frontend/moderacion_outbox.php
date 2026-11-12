<?php
require_once __DIR__ . '/config.php';
require_login();

$user = current_user();
if (($user['role'] ?? '') !== 'admin') {
    flash('No tienes permisos para acceder a la auditoría Outbox.', 'error');
    redirect('cuenta.php');
}

$pageTitle = 'Auditoría Outbox';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Administración</span>
    <h1 style="font-size:clamp(30px,4vw,42px);margin-top:12px">Auditoría Transactional Outbox</h1>
    <p>Supervisa discrepancias entre MySQL y MongoDB y ejecuta recuperaciones administrativas controladas.</p>
  </div>
  <div style="display:flex;gap:10px;flex-wrap:wrap">
    <a class="btn btn-ghost" href="panel.php">Panel</a>
    <a class="btn btn-ghost" href="moderacion.php">Moderar muro</a>
  </div>
</div>

<div id="outbox-toast" class="flash" style="display:none;position:static;margin:0 0 16px"></div>

<div class="admin-grid" style="margin-bottom:18px">
  <div class="card metric"><strong id="metric-total">—</strong><span>Discrepancias</span></div>
  <div class="card metric"><strong id="metric-pending">—</strong><span>Pendientes</span></div>
  <div class="card metric"><strong id="metric-processing">—</strong><span>Procesamiento vencido</span></div>
  <div class="card metric"><strong id="metric-failed">—</strong><span>Fallidos</span></div>
  <div class="card metric"><strong id="metric-missing">—</strong><span>Procesados sin auditoría</span></div>
  <div class="card metric"><strong id="metric-orphan">—</strong><span>Auditorías huérfanas</span></div>
</div>

<section class="card" style="margin-bottom:18px">
  <div style="display:flex;gap:12px;flex-wrap:wrap;align-items:end">
    <div class="field" style="margin:0;min-width:180px">
      <label for="filter-status">Estado</label>
      <select class="input" id="filter-status">
        <option value="">Todos</option>
        <option value="pending">Pendiente</option>
        <option value="processing">Procesando</option>
        <option value="failed">Fallido</option>
        <option value="processed">Procesado</option>
      </select>
    </div>
    <div class="field" style="margin:0;min-width:220px">
      <label for="filter-type">Tipo de evento</label>
      <select class="input" id="filter-type">
        <option value="">Todos</option>
        <option value="wall.post.created">wall.post.created</option>
        <option value="wall.post.updated">wall.post.updated</option>
        <option value="wall.post.deleted">wall.post.deleted</option>
        <option value="wall.post.reported">wall.post.reported</option>
        <option value="wall.post.auto_hidden">wall.post.auto_hidden</option>
        <option value="wall.post.moderated">wall.post.moderated</option>
      </select>
    </div>
    <div class="field" style="margin:0;flex:1;min-width:220px">
      <label for="filter-id">ID del evento</label>
      <input class="input" id="filter-id" placeholder="UUID del evento">
    </div>
    <button class="btn btn-primary btn-sm" type="button" id="apply-filters">Aplicar</button>
    <button class="btn btn-ghost btn-sm" type="button" id="clear-filters">Limpiar</button>
  </div>
</section>

<section class="card">
  <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:14px;flex-wrap:wrap">
    <h2 style="font-size:22px;margin:0">Discrepancias</h2>
    <div style="display:flex;gap:10px;align-items:center">
      <span id="page-info" class="hint"></span>
      <button class="btn btn-soft btn-sm" type="button" id="prev-page">Anterior</button>
      <button class="btn btn-soft btn-sm" type="button" id="next-page">Siguiente</button>
    </div>
  </div>

  <div id="outbox-loading" class="empty" style="padding:24px">
    <p>Cargando eventos…</p>
  </div>
  <div id="outbox-empty" class="empty" style="padding:24px;display:none">
    <h3>Sin discrepancias</h3>
    <p>No se detectaron inconsistencias entre MySQL y MongoDB.</p>
  </div>

  <div style="overflow-x:auto">
    <table class="table" id="outbox-table" style="display:none">
      <thead>
        <tr>
          <th>Evento</th>
          <th>Tipo</th>
          <th>Estado</th>
          <th>Intentos</th>
          <th>Clasificación</th>
          <th>Auditoría</th>
          <th>Acciones</th>
        </tr>
      </thead>
      <tbody id="outbox-rows"></tbody>
    </table>
  </div>
</section>

<section class="card" style="margin-top:18px" id="detail-card" style="display:none">
  <h2 style="font-size:22px;margin:0 0 12px">Detalle del evento</h2>
  <div id="detail-content" class="hint">Selecciona un evento para ver su detalle.</div>
</section>

<section class="card" style="margin-top:18px">
  <h2 style="font-size:22px;margin:0 0 12px">Historial administrativo</h2>
  <div id="history-content" class="hint">Selecciona un evento para ver su historial.</div>
</section>

<script>
const toast = document.getElementById('outbox-toast');
function showToast(message, type = 'success') {
  toast.textContent = message;
  toast.className = 'flash flash-' + type;
  toast.style.display = 'block';
  clearTimeout(window.__outboxToast);
  window.__outboxToast = setTimeout(() => { toast.style.display = 'none'; }, 4500);
}

async function proxy(payload) {
  const csrf = document.querySelector('meta[name="csrf-token"]')?.content || '';
  const res = await fetch('api_proxy.php', {
    method: 'POST',
    headers: {'Content-Type': 'application/json', 'X-CSRF-Token': csrf},
    body: JSON.stringify(payload)
  });
  let data = {};
  try { data = await res.json(); } catch (e) {}
  if (!res.ok) throw {status: res.status, data};
  return data;
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

let currentPage = 1;
const pageSize = 20;
let allDiscrepancies = [];

function classificationBadge(value) {
  const colors = {
    pending_with_document: ['#fff6e8', '#8a5a12'],
    pending_without_document: ['#fdecec', '#a63a3a'],
    processing_expired_with_document: ['#fff6e8', '#8a5a12'],
    processing_expired_without_document: ['#fdecec', '#a63a3a'],
    failed_with_document: ['#fdecec', '#a63a3a'],
    failed_without_document: ['#fdecec', '#a63a3a'],
    processed_missing_document: ['#fdecec', '#a63a3a'],
    audit_without_outbox: ['#efe7ff', '#4e2ea2'],
  };
  const pair = colors[value] || ['#f5f0ff', '#5c4a7a'];
  return '<span class="post-badge" style="background:' + pair[0] + ';color:' + pair[1] + '">' + escapeHtml(value) + '</span>';
}

async function loadMetrics() {
  const pages = [];
  let page = 1;
  while (page <= 10) {
    const data = await proxy({action: 'outbox_discrepancies', limit: 50, offset: (page - 1) * 50});
    const items = Array.isArray(data.items) ? data.items : [];
    pages.push(...items);
    if (items.length < 50) break;
    page++;
  }
  allDiscrepancies = pages;
  const counts = {
    total: pages.length,
    pending: pages.filter(x => x.classification.startsWith('pending')).length,
    processing: pages.filter(x => x.classification.startsWith('processing')).length,
    failed: pages.filter(x => x.classification.startsWith('failed')).length,
    missing: pages.filter(x => x.classification === 'processed_missing_document').length,
    orphan: pages.filter(x => x.classification === 'audit_without_outbox').length,
  };
  document.getElementById('metric-total').textContent = counts.total;
  document.getElementById('metric-pending').textContent = counts.pending;
  document.getElementById('metric-processing').textContent = counts.processing;
  document.getElementById('metric-failed').textContent = counts.failed;
  document.getElementById('metric-missing').textContent = counts.missing;
  document.getElementById('metric-orphan').textContent = counts.orphan;
}

async function loadQueue() {
  const loading = document.getElementById('outbox-loading');
  const empty = document.getElementById('outbox-empty');
  const table = document.getElementById('outbox-table');
  const rows = document.getElementById('outbox-rows');
  loading.style.display = 'block';
  empty.style.display = 'none';
  table.style.display = 'none';
  rows.innerHTML = '';

  const status = document.getElementById('filter-status').value;
  const type = document.getElementById('filter-type').value;
  const id = document.getElementById('filter-id').value.trim();

  const payload = {action: 'outbox_discrepancies', limit: pageSize, offset: (currentPage - 1) * pageSize};
  if (status) payload.status = status;
  if (type) payload.event_type = type;

  try {
    const data = await proxy(payload);
    const items = Array.isArray(data.items) ? data.items : [];
    const filtered = id ? items.filter(x => String(x.id || '').includes(id)) : items;

    if (!filtered.length) {
      loading.style.display = 'none';
      empty.style.display = 'block';
      document.getElementById('page-info').textContent = '';
      return;
    }

    filtered.forEach(item => {
      const tr = document.createElement('tr');
      const actions = [];
      if (item.classification !== 'consistent') {
        actions.push('<button class="btn btn-soft btn-sm action-btn" data-action="recover" data-id="' + escapeHtml(item.id) + '">Recuperar</button>');
      }
      actions.push('<button class="btn btn-ghost btn-sm detail-btn" data-id="' + escapeHtml(item.id) + '">Detalle</button>');
      actions.push('<button class="btn btn-ghost btn-sm history-btn" data-id="' + escapeHtml(item.id) + '">Historial</button>');
      tr.innerHTML =
        '<td><code style="font-size:12px">' + escapeHtml(item.id) + '</code><div class="hint">' + escapeHtml(item.aggregate_type || '') + ' · ' + escapeHtml(item.aggregate_id || '') + '</div></td>' +
        '<td>' + escapeHtml(item.event_type || '') + '</td>' +
        '<td>' + escapeHtml(item.status || '') + '</td>' +
        '<td>' + (item.attempts ?? 0) + '</td>' +
        '<td>' + classificationBadge(item.classification) + '</td>' +
        '<td>' + (item.has_audit_document ? 'Sí' : 'No') + '</td>' +
        '<td style="white-space:nowrap">' + actions.join(' ') + '</td>';
      rows.appendChild(tr);
    });

    loading.style.display = 'none';
    table.style.display = 'table';
    document.getElementById('page-info').textContent = 'Página ' + currentPage;

    rows.querySelectorAll('.detail-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        await showDetail(btn.dataset.id);
      });
    });
    rows.querySelectorAll('.history-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        await showHistory(btn.dataset.id);
      });
    });
    rows.querySelectorAll('.action-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const id = btn.dataset.id;
        const action = btn.dataset.action;
        let reason = null;
        if (action !== 'recover') return;
        reason = prompt('Motivo de la recuperación (obligatorio):');
        if (!reason || !reason.trim()) {
          showToast('El motivo es obligatorio.', 'error');
          return;
        }
        if (!confirm('¿Ejecutar la recuperación controlada de este evento?')) return;
        btn.disabled = true;
        try {
          const operationId = id + ':' + action;
          let key = sessionStorage.getItem('outbox:key:' + operationId);
          if (!key) {
            key = (typeof crypto !== 'undefined' && crypto.randomUUID) ? crypto.randomUUID() : (Date.now().toString(36) + Math.random().toString(36).slice(2));
            sessionStorage.setItem('outbox:key:' + operationId, key);
          }
          const result = await proxy({action: 'outbox_action', post_id: id, event_id: id, operation: action, reason: reason, idempotency_key: key, operation_id: operationId});
          sessionStorage.removeItem('outbox:key:' + operationId);
          showToast('Acción aplicada: ' + (result.result || 'ok'), 'success');
          await loadQueue();
          await loadMetrics();
        } catch (err) {
          const msg = err.data?.error || 'No se pudo aplicar la acción.';
          showToast(msg, 'error');
          btn.disabled = false;
        }
      });
    });
  } catch (err) {
    loading.style.display = 'none';
    showToast(err.data?.error || 'No se pudo cargar la auditoría Outbox.', 'error');
  }
}

async function showDetail(id) {
  const card = document.getElementById('detail-card');
  const content = document.getElementById('detail-content');
  card.style.display = 'block';
  content.textContent = 'Cargando detalle…';
  try {
    const data = await proxy({action: 'outbox_event', id: id});
    content.innerHTML =
      '<div class="kv" style="color:var(--ink-soft)">' +
      '<div style="border-bottom:1px solid var(--border)"><span>ID</span><strong><code style="font-size:12px">' + escapeHtml(data.id) + '</code></strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Tipo</span><strong>' + escapeHtml(data.event_type) + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Estado</span><strong>' + escapeHtml(data.status) + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Intentos</span><strong>' + (data.attempts ?? 0) + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Clasificación</span><strong>' + escapeHtml(data.classification || '') + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Auditoría</span><strong>' + (data.has_audit_document ? 'Sí' : 'No') + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Creado</span><strong>' + escapeHtml(String(data.created_at || '').slice(0, 19)) + '</strong></div>' +
      '<div style="border-bottom:1px solid var(--border)"><span>Último error</span><strong>' + escapeHtml(data.last_error || '—') + '</strong></div>' +
      '</div>';
  } catch (err) {
    content.textContent = err.data?.error || 'No se pudo cargar el detalle.';
  }
}

async function showHistory(id) {
  const content = document.getElementById('history-content');
  content.textContent = 'Cargando historial…';
  try {
    const data = await proxy({action: 'outbox_history', id: id});
    const items = Array.isArray(data) ? data : [];
    if (!items.length) {
      content.textContent = 'Sin intervenciones registradas.';
      return;
    }
    content.innerHTML = items.map(item =>
      '<div style="border-bottom:1px solid var(--border);padding:10px 0">' +
      '<strong>' + escapeHtml(item.action_type || '') + '</strong> · ' + escapeHtml(item.result || '') +
      '<div class="hint">' + escapeHtml(item.reason || '') + '</div>' +
      '<div class="hint">' + escapeHtml(String(item.created_at || '').slice(0, 19)) + ' · ' + escapeHtml(item.admin_user_id || '') + '</div>' +
      '</div>'
    ).join('');
  } catch (err) {
    content.textContent = err.data?.error || 'No se pudo cargar el historial.';
  }
}

document.getElementById('apply-filters')?.addEventListener('click', () => { currentPage = 1; loadQueue(); });
document.getElementById('clear-filters')?.addEventListener('click', () => {
  document.getElementById('filter-status').value = '';
  document.getElementById('filter-type').value = '';
  document.getElementById('filter-id').value = '';
  currentPage = 1;
  loadQueue();
});
document.getElementById('prev-page')?.addEventListener('click', () => {
  if (currentPage > 1) { currentPage--; loadQueue(); }
});
document.getElementById('next-page')?.addEventListener('click', () => {
  currentPage++; loadQueue();
});

loadMetrics().catch(() => {});
loadQueue();
</script>

<?php require __DIR__ . '/includes/footer.php'; ?>
PHP
echo 'created'