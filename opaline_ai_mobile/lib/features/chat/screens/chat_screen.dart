import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_math_fork/flutter_math.dart';
import 'package:go_router/go_router.dart';
import 'package:share_plus/share_plus.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';
import '../../../providers/chat_provider.dart';
import '../../../models/message.dart';

/// Pantalla de chatbot (RF-MOV-04).
/// Chat solo autenticados con suscripcion o Starter vigente.
/// Respuesta progresiva, historial, cuota, seleccion de modelo, RAG,
/// manejo de errores sin consumir cuota, texto enriquecido, copia/compartir.
class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key});
  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _ctrl = TextEditingController();
  final _scroll = ScrollController();
  bool _showModelPicker = false;

  @override
  void initState() {
    super.initState();
    context.read<ChatProvider>().init();
  }

  void _scrollToBottom() {
    if (_scroll.hasClients) {
      _scroll.animateTo(0, duration: const Duration(milliseconds: 300), curve: Curves.easeOut);
    }
  }

  @override
  void dispose() {
    _ctrl.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final provider = context.read<ChatProvider>();
    await provider.sendMessage(_ctrl.text.trim());
    if (provider.error == null) {
      // Solo limpiar el borrador cuando el envio fue aceptado (RF-MOV-04).
      _ctrl.clear();
      if (mounted) _scrollToBottom();
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(provider.error!)),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final auth = context.watch<AuthProvider>();
    final chat = context.watch<ChatProvider>();

    // Chat solo autenticados con suscripcion o Starter vigente (RF-MOV-04)
    if (!auth.isAuthenticated) {
      return _AccessRequiredScreen(onBuy: () => context.goNamed('inicio'));
    }

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Chat Opaline AI', style: TextStyle(fontWeight: FontWeight.w700, fontFamily: 'Poppins')),
            Text('Modelo: ${chat.selectedModel}', style: const TextStyle(fontSize: 12, fontFamily: 'Poppins')),
          ],
        ),
        actions: [
          IconButton(icon: const Icon(Icons.tune), onPressed: () => setState(() => _showModelPicker = !_showModelPicker)),
        ],
      ),
      body: OpalineDecoration(
        child: Column(
          children: [
            if (_showModelPicker) _ModelPicker(chat: chat),
            Expanded(
              child: chat.current == null
                  ? const Center(child: Text('Sin conversaciones'))
                  : ListView.builder(
                      controller: _scroll,
                      reverse: true,
                      padding: const EdgeInsets.all(12),
                      itemCount: chat.current!.messages.length,
                      itemBuilder: (_, i) {
                        final m = chat.current!.messages.reversed.toList()[i];
                        return _MessageBubble(message: m, chat: chat);
                      },
                    ),
            ),
            if (chat.sending) const Padding(padding: EdgeInsets.all(8), child: CircularProgressIndicator()),
            _Composer(ctrl: _ctrl, onSend: _send, chat: chat),
          ],
        ),
      ),
    );
  }
}

class _ModelPicker extends StatelessWidget {
  final ChatProvider chat;
  const _ModelPicker({required this.chat});
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      color: (isDark ? AppTheme.surfaceDark : AppTheme.surfaceLight),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: chat.models.map((m) {
          final selected = chat.selectedModel == m;
          return ChoiceChip(
            label: Text(m, style: TextStyle(fontSize: 12, fontFamily: 'Poppins', color: selected ? Colors.white : null)),
            selected: selected,
            selectedColor: AppTheme.primaryLight,
            onSelected: (_) => chat.selectModel(m),
          );
        }).toList(),
      ),
    );
  }
}

class _MessageBubble extends StatelessWidget {
  final ChatMessage message;
  final ChatProvider chat;
  const _MessageBubble({required this.message, required this.chat});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final isUser = message.isUser;

    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        constraints: BoxConstraints(maxWidth: MediaQuery.of(context).size.width * 0.88),
        margin: const EdgeInsets.symmetric(vertical: 6, horizontal: 4),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: BoxDecoration(
          color: isUser ? AppTheme.primaryLight : (isDark ? AppTheme.surfaceDark : AppTheme.surfaceLight),
          borderRadius: BorderRadius.circular(16),
          border: isUser ? null : Border.all(color: isDark ? const Color(0xFF2C205A) : const Color(0xFFE3DAF6)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (!isUser && message.citations != null && message.citations!.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(bottom: 6),
                child: Text(
                  'Fuentes (RAG): ${message.citations!.map((c) => c['source'] ?? 'doc').join(', ')}',
                  style: TextStyle(fontSize: 11, color: AppTheme.primaryDark, fontStyle: FontStyle.italic, fontFamily: 'Poppins'),
                ),
              ),
            _MathText(text: message.content),
            if (!isUser)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    IconButton(icon: const Icon(Icons.copy, size: 18), onPressed: () => _copy(context, message.content), constraints: const BoxConstraints(), padding: EdgeInsets.zero),
                    IconButton(icon: const Icon(Icons.share, size: 18), onPressed: () => _share(context, message), constraints: const BoxConstraints(), padding: EdgeInsets.zero),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }

  void _copy(BuildContext context, String text) {
    Clipboard.setData(ClipboardData(text: text));
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Copiado al portapapeles')));
  }

  void _share(BuildContext context, ChatMessage msg) {
    Share.share(chat.exportConversationAsText(chat.current!), subject: 'Conversacion Opaline AI');
  }
}

class _MathText extends StatelessWidget {
  final String text;
  const _MathText({required this.text});
  @override
  Widget build(BuildContext context) {
    final reg = RegExp(r'\$\$(.+?)\$\$|\$(.+?)\$', dotAll: true);
    final spans = <InlineSpan>[];
    int last = 0;
    for (final m in reg.allMatches(text)) {
      if (m.start > last) {
        spans.add(TextSpan(text: text.substring(last, m.start)));
      }
      final expr = (m.group(1) ?? m.group(2) ?? '').trim();
      spans.add(WidgetSpan(
        alignment: PlaceholderAlignment.middle,
        child: Math.tex(expr, textStyle: const TextStyle(fontSize: 16)),
      ));
      last = m.end;
    }
    if (last < text.length) spans.add(TextSpan(text: text.substring(last)));
    return RichText(
      text: TextSpan(
        style: TextStyle(fontSize: 16, fontFamily: 'Poppins', color: Theme.of(context).brightness == Brightness.dark ? Colors.white : Colors.black),
        children: spans,
      ),
    );
  }
}

class _Composer extends StatelessWidget {
  final TextEditingController ctrl;
  final VoidCallback onSend;
  final ChatProvider chat;
  const _Composer({required this.ctrl, required this.onSend, required this.chat});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: isDark ? AppTheme.surfaceDark : AppTheme.surfaceLight,
        border: Border(top: BorderSide(color: isDark ? const Color(0xFF2C205A) : const Color(0xFFE3DAF6))),
      ),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: ctrl,
              maxLines: 5,
              minLines: 1,
              textInputAction: TextInputAction.send,
              onSubmitted: (_) => onSend(),
              decoration: InputDecoration(
                hintText: 'Escribe tu mensaje (1-2000 caracteres)',
                hintStyle: TextStyle(color: isDark ? const Color(0xFF8B7DCB) : const Color(0xFF9A8CC7), fontFamily: 'Poppins'),
                border: OutlineInputBorder(borderRadius: BorderRadius.circular(16), borderSide: BorderSide.none),
                filled: true,
                fillColor: isDark ? const Color(0xFF1A123E) : const Color(0xFFF1EAFC),
                contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
              ),
            ),
          ),
          const SizedBox(width: 8),
          Container(
            decoration: BoxDecoration(color: AppTheme.primaryLight, shape: BoxShape.circle),
            child: IconButton(
              icon: const Icon(Icons.send, color: Colors.white),
              onPressed: chat.sending ? null : onSend,
            ),
          ),
        ],
      ),
    );
  }
}

class _AccessRequiredScreen extends StatelessWidget {
  final VoidCallback onBuy;
  const _AccessRequiredScreen({required this.onBuy});
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      body: OpalineDecoration(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.lock_outline, size: 64, color: AppTheme.primaryLight),
              const SizedBox(height: 20),
              Text('Suscripcion requerida', style: TextStyle(fontSize: 20, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins')),
              const SizedBox(height: 10),
              Text('Necesitas una suscripcion activa o el plan Opaline Starter para chatear.', textAlign: TextAlign.center, style: TextStyle(fontSize: 14, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
              const SizedBox(height: 24),
              SizedBox(width: double.infinity, height: 52, child: ElevatedButton(onPressed: onBuy, child: const Text('Ver planes y comprar'))),
            ],
          ),
        ),
      ),
    );
  }
}
