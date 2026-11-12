<?php
require_once __DIR__ . '/config.php';
require_login();

$modelsRes = api('GET', '/ai/models');
$models = $modelsRes['data']['models'] ?? [];
$defaultModel = $modelsRes['data']['default'] ?? ($models[0] ?? '');
$convsRes = api('GET', '/ai/conversations');
$conversations = is_array($convsRes['data']) ? $convsRes['data'] : [];
$statusRes = api('GET', '/ai/status', null, false);
$aiOnline = (bool) ($statusRes['data']['online'] ?? false);
$user = current_user();
$pageTitle = 'Chat IA';
require __DIR__ . '/includes/header.php';
?>

<div class="section-head" style="margin-top:4px">
  <div>
    <span class="eyebrow">✦ Opaline Chat</span>
    <h1 style="font-size:clamp(28px,3.5vw,40px);margin-top:12px">Conversa con tu IA local</h1>
    <p>Modelos open source servidos por LM Studio. Tus mensajes se guardan en tu propia base de datos.</p>
  </div>
  <span class="status" style="font-size:13px;color:var(--ink-soft)">
    <span class="dot <?= $aiOnline ? 'online' : 'offline' ?>"></span>
    <?= $aiOnline ? 'LM Studio en línea' : 'LM Studio sin conexión' ?>
  </span>
</div>

<?php if (!$aiOnline): ?>
  <div class="alert alert-warning" style="margin-bottom:16px">
    LM Studio no responde en este momento. Abre LM Studio, inicia el servidor local en el puerto <strong>1234</strong> y recarga esta página.
  </div>
<?php endif; ?>

<div class="chat-layout">
  <aside class="card chat-sidebar">
    <div style="display:flex;justify-content:space-between;align-items:center;gap:8px">
      <h3>Tus conversaciones</h3>
      <button class="btn btn-soft btn-sm" id="new-chat" type="button">+ Nueva</button>
    </div>
    <div class="conv-list" id="conv-list">
      <?php if (!$conversations): ?>
        <p class="hint" style="padding:8px">Aún no tienes conversaciones. ¡Empieza la primera!</p>
      <?php else: ?>
        <?php foreach ($conversations as $conv): ?>
          <button class="conv-item" type="button" data-conv-id="<?= e($conv['id'] ?? '') ?>">
            <span style="display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap"><?= e($conv['title'] ?? 'Conversación') ?></span>
            <small style="color:var(--muted)"><?= e(substr((string) ($conv['updated_at'] ?? ''), 0, 16)) ?></small>
          </button>
        <?php endforeach; ?>
      <?php endif; ?>
    </div>
    <div class="quota" style="margin-top:auto;border-top:1px solid var(--border);padding-top:12px">
      <strong style="color:var(--ink)"><?= e($user['name'] ?? 'Usuario') ?></strong><br>
      <?= e($user['email'] ?? '') ?>
    </div>
  </aside>

  <section class="card chat-main">
    <div class="chat-head">
      <div>
        <strong>Opaline AI</strong>
        <div class="status"><span class="dot <?= $aiOnline ? 'online' : 'offline' ?>"></span> <?= $aiOnline ? 'Listo para conversar' : 'Sin conexión a LM Studio' ?></div>
      </div>
      <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;justify-content:flex-end">
        <button class="btn btn-soft btn-sm" id="new-chat-top" type="button">+ Nueva</button>
        <label for="model" class="hint" style="margin:0">Modelo</label>
        <select id="model" class="input" style="padding:8px 12px;min-width:190px">
          <?php foreach ($models as $m): ?>
            <option value="<?= e($m) ?>" <?= $m === $defaultModel ? 'selected' : '' ?>><?= e($m) ?></option>
          <?php endforeach; ?>
          <?php if (!$models): ?><option value="qwen/qwen3.5-9b">qwen/qwen3.5-9b</option><?php endif; ?>
        </select>
      </div>
    </div>

    <div class="chat-messages" id="chat-messages">
      <div class="msg-row">
        <div class="msg-avatar ai">O</div>
        <div>
          <div class="msg assistant">¡Hola, <?= e(explode(' ', $user['name'] ?? 'Usuario')[0]) ?>! Soy Opaline, tu asistente con IA local. Puedo ayudarte a redactar, programar, estudiar o simplemente conversar. ¿Qué quieres hacer hoy? ✨</div>
          <div class="msg-meta">Opaline AI · ahora</div>
        </div>
      </div>
    </div>

    <div class="chat-tools">
      <button class="chip" type="button" data-prompt="Explícame en palabras sencillas cómo funciona un modelo de lenguaje.">¿Cómo funciona un LLM?</button>
      <button class="chip" type="button" data-prompt="Ayúdame a escribir un correo profesional para pedir una reunión.">Redactar un correo</button>
      <button class="chip" type="button" data-prompt="Dame ideas de nombres para un proyecto de inteligencia artificial.">Ideas creativas</button>
    </div>

    <form class="chat-compose" id="chat-form">
      <textarea class="input" id="message" rows="1" placeholder="Escribe tu mensaje… (Enter para enviar, Shift+Enter para salto de línea)" maxlength="2000" required></textarea>
      <button class="btn btn-primary" type="submit" id="send-btn">Enviar</button>
    </form>
  </section>
</div>

<script>
const CONVERSATIONS = <?= json_encode($conversations, JSON_HEX_TAG | JSON_HEX_AMP | JSON_HEX_APOS | JSON_HEX_QUOT) ?>;
const DEMO_NAME = <?= json_encode($user['name'] ?? 'Usuario', JSON_HEX_TAG | JSON_HEX_AMP | JSON_HEX_APOS | JSON_HEX_QUOT) ?>;
let conversationId = null;
let history = [];

const messagesEl = document.getElementById('chat-messages');
const form = document.getElementById('chat-form');
const input = document.getElementById('message');
const sendBtn = document.getElementById('send-btn');
const modelEl = document.getElementById('model');
const convList = document.getElementById('conv-list');

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}[c]));
}
function scrollBottom() { messagesEl.scrollTop = messagesEl.scrollHeight; }
function appendMessage(role, text) {
  const row = document.createElement('div');
  row.className = 'msg-row' + (role === 'user' ? ' user' : '');
  const avatar = document.createElement('div');
  avatar.className = 'msg-avatar ' + (role === 'user' ? 'you' : 'ai');
  avatar.textContent = role === 'user' ? (DEMO_NAME[0] || 'T') : 'O';
  const wrap = document.createElement('div');
  const bubble = document.createElement('div');
  bubble.className = 'msg ' + (role === 'user' ? 'user' : 'assistant');
  bubble.textContent = text;
  const meta = document.createElement('div');
  meta.className = 'msg-meta';
  meta.textContent = (role === 'user' ? 'Tú' : 'Opaline AI') + ' · ' + new Date().toLocaleTimeString('es-MX', {hour:'2-digit', minute:'2-digit'});
  wrap.appendChild(bubble); wrap.appendChild(meta);
  row.appendChild(avatar); row.appendChild(wrap);
  messagesEl.appendChild(row);
  scrollBottom();
}
function appendTyping() {
  const row = document.createElement('div');
  row.className = 'msg-row';
  row.id = 'typing-row';
  row.innerHTML = '<div class="msg-avatar ai">O</div><div class="msg assistant"><span class="typing"><span></span><span></span><span></span></span></div>';
  messagesEl.appendChild(row);
  scrollBottom();
}
function removeTyping() { document.getElementById('typing-row')?.remove(); }
function setConversation(id, title) {
  conversationId = id || null;
  document.querySelectorAll('.conv-item').forEach(el => el.classList.toggle('active', el.dataset.convId === id));
  if (id && title) {
    const existing = [...convList.querySelectorAll('.conv-item')].find(el => el.dataset.convId === id);
    if (!existing) {
      const btn = document.createElement('button');
      btn.type = 'button'; btn.className = 'conv-item active'; btn.dataset.convId = id;
      btn.innerHTML = '<span style="display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + escapeHtml(title) + '</span><small style="color:var(--muted)">ahora</small>';
      convList.prepend(btn);
      btn.addEventListener('click', () => loadConversation(id));
    }
  }
}
async function apiProxy(payload) {
  const csrf = document.querySelector('meta[name="csrf-token"]')?.content || '';
  const res = await fetch('api_proxy.php', {
    method: 'POST',
    headers: {'Content-Type': 'application/json', 'Accept': 'application/json', 'X-CSRF-Token': csrf},
    body: JSON.stringify(payload)
  });
  let data = {};
  try { data = await res.json(); } catch (e) {}
  if (!res.ok) throw {status: res.status, data};
  return data;
}
let pendingIdempotencyKey = localStorage.getItem('opaline_pending_chat_key');
let pendingOperationText = localStorage.getItem('opaline_pending_chat_text');

async function sendMessage(text) {
  const normalizedText = text.trim();
  if (!pendingIdempotencyKey || pendingOperationText !== normalizedText) {
    pendingOperationText = normalizedText;
    pendingIdempotencyKey = (typeof crypto !== 'undefined' && crypto.randomUUID)
      ? crypto.randomUUID()
      : (Date.now().toString(16) + Math.random().toString(16).slice(2));
    localStorage.setItem('opaline_pending_chat_key', pendingIdempotencyKey);
    localStorage.setItem('opaline_pending_chat_text', pendingOperationText);
  }
  const idempotencyKey = pendingIdempotencyKey;
  if (!text.trim()) return;
  appendMessage('user', text.trim());
  input.value = '';
  input.style.height = 'auto';
  sendBtn.disabled = true;
  appendTyping();
  try {
    const data = await apiProxy({
      action: 'chat',
      message: text.trim(),
      model: modelEl.value,
      conversation_id: conversationId,
      history: history.slice(-16),
      idempotency_key: idempotencyKey
    });
    removeTyping();
    pendingIdempotencyKey = null;
    pendingOperationText = null;
    localStorage.removeItem('opaline_pending_chat_key');
    localStorage.removeItem('opaline_pending_chat_text');
    const reply = data.reply || 'No recibí respuesta del modelo.';
    appendMessage('assistant', reply);
    history.push({role:'user', content:text.trim()});
    history.push({role:'assistant', content:reply});
    if (data.conversation_id) setConversation(data.conversation_id, text.trim().slice(0, 40));
  } catch (err) {
    removeTyping();
    // Conserva la clave solo si fue un fallo de red/transporte; si el backend
    // respondio, la operacion ya tiene un estado terminal recuperable.
    if (err && typeof err.status === 'number') {
      pendingIdempotencyKey = null;
      pendingOperationText = null;
      localStorage.removeItem('opaline_pending_chat_key');
      localStorage.removeItem('opaline_pending_chat_text');
    }
    let msg = 'No se pudo obtener respuesta.';
    if (err.status === 401) msg = 'Tu sesión expiró. Inicia sesión de nuevo para continuar.';
    else if (err.status === 503) msg = 'LM Studio no está disponible. Inicia el servidor local en el puerto 1234 y vuelve a intentar.';
    else if (err.status === 400) msg = 'El mensaje no es válido. Revisa la longitud e inténtalo otra vez.';
    else if (err.data?.error) msg = err.data.error;
    appendMessage('assistant', '⚠️ ' + msg);
  } finally {
    sendBtn.disabled = false;
    input.focus();
  }
}
function loadConversation(id) {
  const conv = CONVERSATIONS.find(c => c.id === id);
  if (!conv) return;
  conversationId = conv.id;
  history = (conv.messages || []).map(m => ({role:m.role, content:m.content}));
  messagesEl.innerHTML = '';
  if (!history.length) {
    appendMessage('assistant', 'Esta conversación está vacía. Escribe tu primer mensaje.');
  } else {
    history.forEach(m => appendMessage(m.role === 'user' ? 'user' : 'assistant', m.content));
  }
  document.querySelectorAll('.conv-item').forEach(el => el.classList.toggle('active', el.dataset.convId === id));
}

form.addEventListener('submit', e => { e.preventDefault(); sendMessage(input.value); });
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMessage(input.value); }
});
input.addEventListener('input', () => {
  input.style.height = 'auto';
  input.style.height = Math.min(input.scrollHeight, 140) + 'px';
});
document.querySelectorAll('.chip[data-prompt]').forEach(btn => {
  btn.addEventListener('click', () => { input.value = btn.dataset.prompt; input.focus(); });
});
function startNewChat() {
  conversationId = null; history = []; messagesEl.innerHTML = '';
  appendMessage('assistant', 'Nueva conversación lista. ¿Qué te gustaría hacer? ✨');
  document.querySelectorAll('.conv-item').forEach(el => el.classList.remove('active'));
  input.focus();
}
document.getElementById('new-chat').addEventListener('click', startNewChat);
document.getElementById('new-chat-top').addEventListener('click', startNewChat);
convList.querySelectorAll('.conv-item').forEach(btn => btn.addEventListener('click', () => loadConversation(btn.dataset.convId)));
scrollBottom();
</script>

<?php require __DIR__ . '/includes/footer.php'; ?>
