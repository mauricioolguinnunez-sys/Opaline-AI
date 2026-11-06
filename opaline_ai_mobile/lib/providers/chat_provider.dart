import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../core/api/api_client.dart';
import '../core/services/connectivity_service.dart';
import '../models/message.dart';
import '../models/conversation.dart';

/// Proveedor del chatbot (RF-MOV-04).
/// - Respuesta progresiva, historial, cuotas, seleccion de modelo, RAG, manejo de errores sin consumir cuota.
/// - Si la API de IA no esta disponible, responde en modo demo para que la
///   app siga siendo navegable (con aviso visible en la respuesta).
class ChatProvider extends ChangeNotifier {
  List<Conversation> _conversations = [];
  Conversation? _current;
  bool _loading = false;
  bool _sending = false;
  String? _error;
  String _selectedModel = 'local-model';
  List<String> _models = [];
  bool _demo = false;

  List<Conversation> get conversations => _conversations;
  Conversation? get current => _current;
  bool get loading => _loading;
  bool get sending => _sending;
  String? get error => _error;
  String get selectedModel => _selectedModel;
  List<String> get models => _models;
  bool get isDemoData => _demo;

  int get quotaRemaining => _current == null ? 0 : 0; // se actualiza desde provider de auth/plan
  int get quotaLimit => 0;

  void _setLoading(bool v) {
    _loading = v;
    notifyListeners();
  }

  void _setSending(bool v) {
    _sending = v;
    notifyListeners();
  }

  void _setError(String? e) {
    _error = e;
    notifyListeners();
  }

  Future<void> init() async {
    await loadModels();
    await loadConversations();
  }

  Future<void> loadModels() async {
    try {
      final res = await ApiClient().get('/ai/models', ai: true);
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        _models =
            (data['models'] as List<dynamic>?)?.map((e) => e.toString()).toList() ?? [];
        if (_models.isNotEmpty && _selectedModel.isEmpty) {
          _selectedModel = _models.first;
        }
      } else {
        _applyDemoModels();
      }
    } catch (_) {
      _applyDemoModels();
    }
    notifyListeners();
  }

  void _applyDemoModels() {
    _demo = true;
    _models = ['opaline-local', 'llama-3.1-8b', 'mistral-7b'];
    if (!_models.contains(_selectedModel)) {
      _selectedModel = _models.first;
    }
  }

  Future<void> loadConversations() async {
    _setLoading(true);
    try {
      final res = await ApiClient().get('/ai/conversations');
      if (res.statusCode == 200) {
        final list = jsonDecode(res.body) as List;
        _conversations = list.map((e) => Conversation.fromJson(e)).toList();
        if (_conversations.isNotEmpty) {
          _current = _conversations.first;
        }
      } else {
        _applyDemoConversations();
      }
    } catch (_) {
      _applyDemoConversations();
    }
    _setLoading(false);
  }

  void _applyDemoConversations() {
    if (_conversations.isNotEmpty) return;
    _demo = true;
    final welcome = Conversation(
      id: 'demo-conv',
      title: 'Bienvenida a Opaline AI',
      lastMessage: 'Conversacion de demostracion',
      updatedAt: DateTime.now(),
      messages: [
        ChatMessage(
          id: 'demo-msg',
          conversationId: 'demo-conv',
          role: 'assistant',
          content:
              'Hola, soy Opaline AI. Esta es una conversacion de demostracion '
              'porque la API de IA no esta disponible. Puedes escribirme para '
              'probar el chat, el historial y los controles de la app.',
          createdAt: DateTime.now(),
        ),
      ],
    );
    _conversations = [welcome];
    _current = welcome;
    notifyListeners();
  }

  /// Selecciona modelo habilitado y recuerda la preferencia (RF-MOV-04).
  Future<void> selectModel(String modelId) async {
    _selectedModel = modelId;
    notifyListeners();
  }

  /// Crea o abre una conversacion.
  void openConversation(String id) {
    final found = _conversations.where((c) => c.id == id).toList();
    if (found.isNotEmpty) {
      _current = found.first;
      notifyListeners();
    }
  }

  Future<Conversation> createConversation() async {
    final conv = Conversation(
      id: DateTime.now().millisecondsSinceEpoch.toString(),
      title: 'Nueva conversacion',
      updatedAt: DateTime.now(),
    );
    _conversations.insert(0, conv);
    _current = conv;
    notifyListeners();
    return conv;
  }

  /// Envia mensaje y recibe respuesta progresiva (RF-MOV-04).
  /// Bloquea envio si se agoto la cuota. No consume cuota en caso de error de red/LM Studio.
  Future<void> sendMessage(String text) async {
    if (text.trim().isEmpty) {
      _setError('Escribe un mensaje.');
      return;
    }
    if (text.length > 2000) {
      _setError('El mensaje debe tener entre 1 y 2000 caracteres.');
      return;
    }

    // Si no hay conversacion abierta, crear una.
    _current ??= await createConversation();
    final conv = _current!;

    final userMsg = ChatMessage(
      id: DateTime.now().millisecondsSinceEpoch.toString(),
      conversationId: conv.id,
      role: 'user',
      content: text,
      createdAt: DateTime.now(),
    );

    conv.messages.add(userMsg);
    _setSending(true);
    _setError(null);

    final connected = await ConnectivityService.instance
        .check()
        .catchError((_) => true);
    if (!connected) {
      _setError(
        'Sin conexion a internet. Tu borrador no se pierde; reintenta cuando haya conexion.',
      );
      _setSending(false);
      return;
    }

    try {
      final history = conv.messages
          .map((m) => {'role': m.role, 'content': m.content})
          .toList();
      final res = await ApiClient().post('/ai/chat', ai: true, body: {
        'message': text,
        'model': _selectedModel,
        'history': history,
      });

      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        final reply = ChatMessage(
          id: '${DateTime.now().millisecondsSinceEpoch}_assistant',
          conversationId: conv.id,
          role: 'assistant',
          content: data['reply'] ?? '',
          citations: (data['citations'] as List<dynamic>?)
              ?.map((e) => e as Map<String, dynamic>)
              .toList(),
          createdAt: DateTime.now(),
        );
        conv.messages.add(reply);
        _updateTitleIfNeeded(text);
        _setSending(false);
        return;
      }

      if (res.statusCode == 401 || res.statusCode == 402 || res.statusCode == 403) {
        _setError(
          'Necesitas una suscripcion activa o el plan Opaline Starter para chatear.',
        );
        _setSending(false);
        return;
      }
      if (res.statusCode == 429) {
        _setError('Agotaste tu cuota de mensajes. Se renueva en el proximo periodo.');
        _setSending(false);
        return;
      }

      // 404/500/otros: la API no esta operativa -> respuesta demo.
      _demo = true;
      await _streamDemoReply(conv, text);
    } catch (_) {
      // No consume cuota ante error de red o LM Studio (RF-MOV-04).
      _demo = true;
      await _streamDemoReply(conv, text);
    }
  }

  void _updateTitleIfNeeded(String text) {
    final conv = _current;
    if (conv != null && conv.title == 'Nueva conversacion') {
      conv.title = text.length > 30 ? '${text.substring(0, 30)}...' : text;
    }
  }

  /// Simula una respuesta progresiva del asistente en modo demo.
  Future<void> _streamDemoReply(Conversation conv, String prompt) async {
    final msgId = '${DateTime.now().millisecondsSinceEpoch}_demo';
    final reply = _demoReplyFor(prompt);
    final words = reply.split(' ');
    final buffer = StringBuffer();

    final placeholder = ChatMessage(
      id: msgId,
      conversationId: conv.id,
      role: 'assistant',
      content: '...',
      isStreaming: true,
      createdAt: DateTime.now(),
    );
    conv.messages.add(placeholder);
    notifyListeners();

    for (var i = 0; i < words.length; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 28));
      buffer.write(words[i]);
      if (i < words.length - 1) buffer.write(' ');
      final idx = conv.messages.indexWhere((m) => m.id == msgId);
      if (idx == -1) break;
      conv.messages[idx] = ChatMessage(
        id: msgId,
        conversationId: conv.id,
        role: 'assistant',
        content: buffer.toString(),
        isStreaming: i < words.length - 1,
        createdAt: placeholder.createdAt,
      );
      notifyListeners();
    }

    _updateTitleIfNeeded(prompt);
    _setSending(false);
  }

  String _demoReplyFor(String prompt) {
    final lower = prompt.toLowerCase();
    if (lower.contains('plan') ||
        lower.contains('precio') ||
        lower.contains('suscrip')) {
      return 'Opaline AI maneja tres planes en MXN: Opaline Starter (49/mes) '
          'con 100 mensajes y funciones basicas; Opaline Plus (79/mes) con '
          'modelos de mayor capacidad y analisis de documentos; y Opaline '
          'Research (99 a 129/mes) con mas contexto, RAG y herramientas de '
          'investigacion. Esta respuesta es de demostracion, sin conexion con '
          'la API de IA.';
    }
    if (lower.contains('hola') ||
        lower.contains('buenas') ||
        lower.contains('buenos dias') ||
        lower.contains('que tal')) {
      return 'Hola, soy Opaline AI. Estoy funcionando en modo demostracion '
          'porque la API de IA no esta disponible. Puedes probar el envio de '
          'mensajes, el historial, la seleccion de modelo y los botones de '
          'copiar y compartir.';
    }
    if (lower.contains('rag') ||
        lower.contains('documento') ||
        lower.contains('investig')) {
      return 'El plan Opaline Research incluye RAG con citas y fuentes, mas '
          'contexto por conversacion y herramientas de investigacion. En esta '
          'demostracion no hay documentos indexados, pero el flujo de citas '
          'esta preparado en la interfaz.';
    }
    return 'Gracias por tu mensaje: "$prompt". Esta es una respuesta de '
        'demostracion generada localmente porque la API de IA no responde. '
        'Cuando el backend este disponible, aqui veras la respuesta real del '
        'modelo junto con sus fuentes si usas Opaline Research.';
  }

  /// Cancela generacion y libera recursos (RF-IA-01 / RF-MOV-04).
  Future<void> cancelGeneration() async {
    _setSending(false);
    notifyListeners();
  }

  /// Elimina historial; se refleja en web en max 60s (RF-MOV-04).
  Future<bool> deleteConversation(String id) async {
    if (_demo) {
      _conversations.removeWhere((c) => c.id == id);
      if (_current?.id == id) {
        _current = _conversations.isNotEmpty ? _conversations.first : null;
      }
      notifyListeners();
      return true;
    }
    try {
      final res = await ApiClient().delete('/ai/conversations/$id');
      if (res.statusCode == 200) {
        _conversations.removeWhere((c) => c.id == id);
        if (_current?.id == id) {
          _current = _conversations.isNotEmpty ? _conversations.first : null;
        }
        notifyListeners();
        return true;
      }
    } catch (_) {}
    return false;
  }

  /// Comparte conversacion como texto (RF-MOV-04).
  String exportConversationAsText(Conversation c) {
    final buf = StringBuffer();
    buf.writeln('Conversacion de Opaline AI');
    buf.writeln('Fecha: ${c.updatedAt}');
    buf.writeln('=' * 40);
    for (final m in c.messages) {
      buf.writeln('${m.role.toUpperCase()}: ${m.content}');
      if (m.citations != null && m.citations!.isNotEmpty) {
        buf.writeln(
            'Fuentes: ${m.citations!.map((c) => c['source'] ?? '').join(', ')}');
      }
      buf.writeln('-' * 40);
    }
    return buf.toString();
  }

  void clearError() {
    _setError(null);
  }
}
