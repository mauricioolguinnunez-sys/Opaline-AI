import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../core/api/api_client.dart';
import '../core/services/cache_service.dart';
import '../core/services/connectivity_service.dart';
import '../models/post.dart';

/// Proveedor del muro publico (RF-MOV-01 / RF-WEB-04).
/// Lectura publica, publicacion/edicion/eliminacion propia y reporte de contenido.
/// Si la API no esta disponible se muestran publicaciones de demostracion y se
/// permite publicar localmente para poder probar la interfaz.
class WallProvider extends ChangeNotifier {
  List<WallPost> _posts = [];
  bool _loading = false;
  String? _error;
  bool _initialized = false;
  bool _demo = false;

  List<WallPost> get posts => _posts;
  bool get loading => _loading;
  String? get error => _error;
  bool get initialized => _initialized;
  bool get isDemoData => _demo;

  void _setLoading(bool v) {
    _loading = v;
    notifyListeners();
  }

  void _setError(String? e) {
    _error = e;
    notifyListeners();
  }

  Future<void> init() async {
    if (_initialized) return;
    _initialized = true;
    await loadPosts(forceNetwork: false);
  }

  /// Lee el muro en modo solo lectura sin conexion con cache cifrada (RF-MOV-01).
  Future<void> loadPosts({bool forceNetwork = false}) async {
    if (!forceNetwork && _posts.isNotEmpty) return;
    _setLoading(true);
    _setError(null);

    // Sin conexion: solo lectura desde cache (RF-MOV-01).
    final connected = await ConnectivityService.instance
        .check()
        .catchError((_) => true);
    if (!connected) {
      final cached = CacheService.instance.get('wall', 'feed');
      final cachedPosts = _tryParsePosts(cached);
      if (cachedPosts != null && cachedPosts.isNotEmpty) {
        _posts = cachedPosts;
        notifyListeners();
        _setLoading(false);
        return;
      }
      _applyDemoPosts();
      _setLoading(false);
      return;
    }

    try {
      final res = await ApiClient().get('/muro');
      if (res.statusCode == 200) {
        final list = jsonDecode(res.body) as List;
        final parsed = list.map((e) => WallPost.fromJson(e)).toList();
        if (parsed.isNotEmpty) {
          _demo = false;
          _posts = parsed;
          await CacheService.instance.put(
            'wall',
            'feed',
            jsonEncode(_posts.map((p) => p.toJson()).toList()),
          );
          _setLoading(false);
          return;
        }
      }
    } catch (_) {
      // API no disponible -> muro demo.
    }
    _applyDemoPosts();
    _setLoading(false);
  }

  List<WallPost>? _tryParsePosts(String? raw) {
    if (raw == null) return null;
    try {
      return (jsonDecode(raw) as List)
          .map((e) => WallPost.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return null;
    }
  }

  void _applyDemoPosts() {
    if (_posts.isNotEmpty) return;
    _demo = true;
    final now = DateTime.now();
    _posts = [
      WallPost(
        id: 'demo-post-1',
        userId: 'demo-ana',
        userName: 'Ana Rivera',
        content:
            'Estoy usando Opaline Research para organizar las fuentes de mi '
            'tesis. Las respuestas con citas me ahorran mucho tiempo.',
        likes: 24,
        likedByMe: false,
        commentsCount: 5,
        createdAt: now.subtract(const Duration(hours: 2)),
      ),
      WallPost(
        id: 'demo-post-2',
        userId: 'demo-luis',
        userName: 'Luis Mendoza',
        content:
            'Probe Opaline Plus con el analisis de documentos de mi trabajo y '
            'la verdad es que el resumen quedo muy claro.',
        likes: 18,
        likedByMe: false,
        commentsCount: 3,
        createdAt: now.subtract(const Duration(days: 1, hours: 3)),
      ),
      WallPost(
        id: 'demo-post-3',
        userId: 'demo-maria',
        userName: 'Maria Torres',
        content:
            'Con Opaline Starter pude empezar sin gastar de mas. Los 100 '
            'mensajes mensuales alcanzan bien para estudiar.',
        likes: 31,
        likedByMe: false,
        commentsCount: 7,
        createdAt: now.subtract(const Duration(days: 3)),
      ),
    ];
    notifyListeners();
  }

  /// Publica una experiencia (RF-MOV-01 / RF-WEB-04).
  Future<bool> createPost(String content, {String authorName = 'Usuario Opaline'}) async {
    if (content.trim().isEmpty || content.length > 2000) {
      _setError('La publicacion debe tener entre 1 y 2000 caracteres.');
      return false;
    }
    _setLoading(true);
    _setError(null);

    if (_demo) {
      final post = WallPost(
        id: 'demo-local-${DateTime.now().millisecondsSinceEpoch}',
        userId: 'demo-user',
        userName: authorName,
        content: content,
        likes: 0,
        likedByMe: false,
        commentsCount: 0,
        createdAt: DateTime.now(),
      );
      _posts.insert(0, post);
      _setLoading(false);
      return true;
    }

    try {
      final res = await ApiClient().post('/muro', body: {'content': content});
      _setLoading(false);
      if (res.statusCode == 200 || res.statusCode == 201) {
        final data = jsonDecode(res.body);
        final post = WallPost.fromJson(data['post'] ?? {});
        _posts.insert(0, post);
        notifyListeners();
        return true;
      }
      _setError('No se pudo publicar.');
    } catch (_) {
      // Fallback local: la publicacion queda visible en este dispositivo.
      _demo = true;
      final post = WallPost(
        id: 'demo-local-${DateTime.now().millisecondsSinceEpoch}',
        userId: 'demo-user',
        userName: authorName,
        content: content,
        likes: 0,
        likedByMe: false,
        commentsCount: 0,
        createdAt: DateTime.now(),
      );
      _posts.insert(0, post);
      _setLoading(false);
      return true;
    }
    return false;
  }

  /// Edita la propia publicacion (RF-MOV-01 / RF-WEB-04).
  Future<bool> editPost(String postId, String newContent) async {
    if (newContent.trim().isEmpty || newContent.length > 2000) {
      _setError('Contenido invalido.');
      return false;
    }
    if (_demo) {
      final idx = _posts.indexWhere((p) => p.id == postId);
      if (idx == -1) return false;
      _posts[idx] = _posts[idx].copyWith(content: newContent);
      notifyListeners();
      return true;
    }
    try {
      final res =
          await ApiClient().put('/muro/$postId', body: {'content': newContent});
      if (res.statusCode == 200) {
        final idx = _posts.indexWhere((p) => p.id == postId);
        if (idx != -1) {
          _posts[idx] = _posts[idx].copyWith(content: newContent);
          notifyListeners();
        }
        return true;
      }
    } catch (_) {}
    return false;
  }

  /// Elimina la propia publicacion (RF-MOV-01 / RF-WEB-04).
  Future<bool> deletePost(String postId) async {
    if (_demo) {
      _posts.removeWhere((p) => p.id == postId);
      notifyListeners();
      return true;
    }
    try {
      final res = await ApiClient().delete('/muro/$postId');
      if (res.statusCode == 200) {
        _posts.removeWhere((p) => p.id == postId);
        notifyListeners();
        return true;
      }
    } catch (_) {}
    return false;
  }

  /// Reporta contenido (RF-MOV-01 / RF-WEB-04).
  Future<bool> reportPost(String postId, {String? motivo}) async {
    if (_demo) {
      final idx = _posts.indexWhere((p) => p.id == postId);
      if (idx != -1) {
        _posts[idx] = _posts[idx].copyWith(reported: true);
        notifyListeners();
      }
      return true;
    }
    try {
      final res = await ApiClient().post('/muro/$postId/reportar',
          body: {'motivo': motivo ?? 'contenido_inadecuado'});
      if (res.statusCode == 200) {
        final idx = _posts.indexWhere((p) => p.id == postId);
        if (idx != -1) {
          _posts[idx] = _posts[idx].copyWith(reported: true);
          notifyListeners();
        }
        return true;
      }
    } catch (_) {}
    return false;
  }

  void clearError() {
    _setError(null);
  }
}
