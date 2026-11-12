<?php
require_once __DIR__ . '/config.php';
require_login();

$user = current_user();
if (($user['role'] ?? '') !== 'admin') {
    flash('No tienes permisos para acceder a la moderación.', 'error');
    redirect('cuenta.php');
}

$pageTitle = 'Moderación del muro';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">Administración</span>
    <h1 style="font-size:clamp(30px,4vw,42px);margin-top:12px">Moderación del muro</h1>
    <p>Revisa publicaciones reportadas y decide si se aprueban, ocultan o eliminan.</p>
  </div>
  <a class="btn btn-ghost" href="panel.php">Volver al panel</a>
</div>

<div id="moderation-toast" class="flash" style="display:none;position:static;margin:0 0 16px"></div>

<section class="card">
  <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:14px;flex-wrap:wrap">
    <h2 style="font-size:22px;margin:0">Cola de moderación</h2>
    <button class="btn btn-soft btn-sm" type="button" id="refresh-queue">Actualizar</button>
  </div>

  <div id="moderation-loading" class="empty" style="padding:24px">
    <p>Cargando publicaciones…</p>
  </div>

  <div id="moderation-empty" class="empty" style="padding:24px;display:none">
    <h3>Sin publicaciones pendientes</h3>
    <p>No hay contenido que requiera moderación en este momento.</p>
  </div>

  <div style="overflow-x:auto">
    <table class="table" id="moderation-table" style="display:none">
      <thead>
        <tr>
          <th>Publicación</th>
          <th>Autor</th>
          <th>Estado</th>
          <th>Reportes</th>
          <th>Fecha</th>
          <th>Acciones</th>
        </tr>
      </thead>
      <tbody id="moderation-rows"></tbody>
    </table>
  </div>
</section>

<script>
const toast = document.getElementById('moderation-toast');
function showToast(message, type = 'success') {
  toast.textContent = message;
  toast.className = 'flash flash-' + type;
  toast.style.display = 'block';
  clearTimeout(window.__modToast);
  window.__modToast = setTimeout(() => { toast.style.display = 'none'; }, 4200);
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

function statusBadge(status) {
  const colors = {
    approved: ['#e8f8f0', '#147a50'],
    pending: ['#fff6e8', '#8a5a12'],
    hidden: ['#fdecec', '#a63a3a'],
    deleted: ['#f1f1f1', '#555'],
  };
  const pair = colors[status] || ['#f5f0ff', '#5c4a7a'];
  return '<span class="post-badge" style="background:' + pair[0] + ';color:' + pair[1] + '">' + (status || 'desconocido') + '</span>';
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

async function loadQueue() {
  const loading = document.getElementById('moderation-loading');
  const empty = document.getElementById('moderation-empty');
  const table = document.getElementById('moderation-table');
  const rows = document.getElementById('moderation-rows');
  loading.style.display = 'block';
  empty.style.display = 'none';
  table.style.display = 'none';
  rows.innerHTML = '';

  try {
    const data = await proxy({action: 'wall_moderation_list'});
    const posts = Array.isArray(data) ? data : [];
    if (!posts.length) {
      loading.style.display = 'none';
      empty.style.display = 'block';
      return;
    }

    posts.forEach(post => {
      const tr = document.createElement('tr');
      const actions = [];
      if (post.status !== 'approved') {
        actions.push('<button class="btn btn-soft btn-sm moderate-btn" data-action="approve" data-id="' + escapeHtml(post.id) + '">Aprobar</button>');
      }
      if (post.status !== 'hidden') {
        actions.push('<button class="btn btn-ghost btn-sm moderate-btn" data-action="hide" data-id="' + escapeHtml(post.id) + '">Ocultar</button>');
      }
      if (post.status !== 'deleted') {
        actions.push('<button class="btn btn-ghost btn-sm moderate-btn" data-action="delete" data-id="' + escapeHtml(post.id) + '" style="color:var(--danger)">Eliminar</button>');
      }
      tr.innerHTML =
        '<td><div style="max-width:360px"><strong>' + escapeHtml(post.content || '') + '</strong>' +
        (post.last_report_reason ? '<div class="hint" style="margin-top:6px">Último reporte: ' + escapeHtml(post.last_report_reason) + '</div>' : '') +
        '</div></td>' +
        '<td>' + escapeHtml(post.alias || 'Usuario') + '</td>' +
        '<td>' + statusBadge(post.status) + '</td>' +
        '<td>' + (post.report_count ?? 0) + '</td>' +
        '<td>' + escapeHtml(String(post.created_at || '').slice(0, 16)) + '</td>' +
        '<td style="white-space:nowrap">' + (actions.length ? actions.join(' ') : '—') + '</td>';
      rows.appendChild(tr);
    });

    loading.style.display = 'none';
    table.style.display = 'table';

    rows.querySelectorAll('.moderate-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const action = btn.dataset.action;
        const id = btn.dataset.id;
        let reason = null;
        if (action === 'hide' || action === 'delete') {
          reason = prompt('Motivo de la acción (obligatorio):');
          if (!reason || !reason.trim()) {
            showToast('El motivo es obligatorio.', 'error');
            return;
          }
        } else if (action === 'delete' && !confirm('¿Eliminar esta publicación de forma permanente?')) {
          return;
        }
        if (action === 'hide' && !confirm('¿Ocultar esta publicación?')) return;
        if (action === 'delete' && !confirm('¿Eliminar esta publicación?')) return;

        btn.disabled = true;
        try {
          await proxy({action: 'wall_moderate', post_id: id, action: action, reason: reason});
          showToast('Acción aplicada correctamente.', 'success');
          await loadQueue();
        } catch (err) {
          showToast(err.data?.error || 'No se pudo aplicar la acción.', 'error');
          btn.disabled = false;
        }
      });
    });
  } catch (err) {
    loading.style.display = 'none';
    showToast(err.data?.error || 'No se pudo cargar la cola de moderación.', 'error');
  }
}

document.getElementById('refresh-queue')?.addEventListener('click', loadQueue);
loadQueue();
</script>

<?php require __DIR__ . '/includes/footer.php'; ?>
PHP
echo 'created'