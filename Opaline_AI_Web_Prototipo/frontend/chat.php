<?php require __DIR__ . '/includes/header.php'; ?>
<?php require __DIR__ . '/api_client.php'; ?>
<?php
if (empty($_SESSION['user'])) { header('Location: login.php'); exit; }
if (!isset($_SESSION['chat_history'])) $_SESSION['chat_history'] = [];
if (!isset($_SESSION['chat_model'])) { $ms = call_lm_studio_models(); $_SESSION['chat_model'] = $ms[0] ?? 'local-model'; }
$models = call_lm_studio_models();
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    if (isset($_POST['model'])) {
        $_SESSION['chat_model'] = $_POST['model'];
    }
    $msg = trim($_POST['message'] ?? '');
    if ($msg !== '') {
        // Historial previo (sin el mensaje actual) para dar contexto al modelo
        $hist = array_slice($_SESSION['chat_history'], 0);
        $_SESSION['chat_history'][] = ['role'=>'user','content'=>$msg];
        $r = call_lm_studio_chat($msg, $_SESSION['chat_model'], $hist);
        if ($r === null) $r = "[prototipo] Eco: " . $msg;
        $_SESSION['chat_history'][] = ['role'=>'assistant','content'=>$r];
    }
    if (isset($_POST['clear'])) {
        $_SESSION['chat_history'] = [];
    }
}
?>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.9/dist/contrib/auto-render.min.js"
  onload="renderMathInElement(document.body,{delimiters:[{left:'$$',right:'$$',display:true},{left:'$',right:'$',display:false}]});"></script>
<style>
body{margin:0;font-family:system-ui,Segoe UI,Roboto,Arial,sans-serif;background:#0f0f0f;color:#ececec;}
.wrap{display:flex;height:100vh;}
.sidebar{width:260px;background:#171717;padding:12px;border-right:1px solid #2a2a2a;display:flex;flex-direction:column;}
.sidebar h2{font-size:14px;margin:8px 0 12px;color:#aaa;}
.sidebar .model{width:100%;padding:8px;border-radius:6px;border:1px solid #3a3a3a;background:#222;color:#ececec;margin-bottom:12px;}
.sidebar .newchat{padding:8px;border-radius:6px;border:1px solid #3a3a3a;background:transparent;color:#ececec;cursor:pointer;margin-bottom:12px;}
.main{flex:1;display:flex;flex-direction:column;}
.messages{flex:1;overflow:auto;padding:24px;max-width:800px;margin:0 auto;width:100%;box-sizing:border-box;}
.msg{margin:16px 0;line-height:1.6;}
.msg.user{text-align:right;}
.msg.user .bubble{display:inline-block;background:#2b2b2b;padding:10px 14px;border-radius:12px;max-width:70%;}
.msg.assistant .bubble{display:inline-block;background:#1e1e1e;padding:10px 14px;border-radius:12px;max-width:80%;}
.inputbar{border-top:1px solid #2a2a2a;padding:16px;background:#171717;}
.inputbar form{max-width:800px;margin:0 auto;display:flex;gap:8px;}
.inputbar input{flex:1;padding:12px;border-radius:8px;border:1px solid #3a3a3a;background:#222;color:#ececec;}
.inputbar button{padding:12px 18px;border-radius:8px;border:none;background:#10a37f;color:#fff;cursor:pointer;}
.topbar{padding:10px 24px;border-bottom:1px solid #2a2a2a;display:flex;justify-content:space-between;align-items:center;}
.topbar a{color:#aaa;text-decoration:none;font-size:13px;}
</style>
<div class="wrap">
  <div class="sidebar">
    <h2>Opaline AI</h2>
    <form method="post"><button class="newchat" name="clear" value="1">+ Nuevo chat</button></form>
    <label style="font-size:12px;color:#aaa;">Modelo</label>
    <form method="post">
      <select class="model" name="model" onchange="this.form.submit()">
        <?php foreach ($models as $m): ?>
          <option value="<?php echo htmlspecialchars($m); ?>" <?php if ($m===$_SESSION['chat_model']) echo 'selected'; ?>>
            <?php echo htmlspecialchars($m); ?>
          </option>
        <?php endforeach; ?>
      </select>
    </form>
    <div style="margin-top:auto;font-size:12px;color:#666;">
      Usuario: <?php echo htmlspecialchars($_SESSION['user']); ?><br>
      <a href="home.php" style="color:#aaa;">Inicio</a> · <a href="logout.php" style="color:#aaa;">Salir</a>
    </div>
  </div>
  <div class="main">
    <div class="topbar">
      <span><?php echo htmlspecialchars($_SESSION['chat_model']); ?></span>
      <a href="home.php">← Volver</a>
    </div>
    <div class="messages">
      <?php if (empty($_SESSION['chat_history'])): ?>
        <div class="msg assistant"><div class="bubble">Hola, soy el asistente de Opaline AI. ¿En qué puedo ayudarte?</div></div>
      <?php endif; ?>
      <?php foreach ($_SESSION['chat_history'] as $m): ?>
        <div class="msg <?php echo $m['role']; ?>"><div class="bubble"><?php echo nl2br(htmlspecialchars($m['content'])); ?></div></div>
      <?php endforeach; ?>
    </div>
    <div class="inputbar">
      <form method="post">
        <input type="hidden" name="model" value="<?php echo htmlspecialchars($_SESSION['chat_model']); ?>">
        <input type="text" name="message" placeholder="Escribe un mensaje..." required autofocus>
        <button type="submit">Enviar</button>
      </form>
    </div>
  </div>
</div>
<?php require __DIR__ . '/includes/footer.php'; ?>
