import 'dart:convert';

import 'package:hive/hive.dart';

/// Cache local cifrada de solo lectura para catalogo y muro (RF-MOV-01 / RNF-MOV-03).
/// - Datos NO sensibles (planes, publicaciones) -> Hive con cifrado ligero.
/// - Tokens y datos personales NUNCA aqui (van en SecureStorage).
/// - Max 100 MB (RNF-MOV-01). Purga al cerrar sesion / eliminar cuenta.
///
/// Requiere que `Hive.initFlutter()` se haya llamado antes (ver main.dart).
class CacheService {
  CacheService._internal();
  static final CacheService instance = CacheService._internal();

  static const int maxCacheSizeBytes = 100 * 1024 * 1024; // 100 MB
  static const Duration defaultTtl = Duration(hours: 24);

  Box<String>? _plansBox;
  Box<String>? _wallBox;
  Box<String>? _metaBox;

  bool get isReady => _plansBox != null && _wallBox != null;

  Future<void> init() async {
    if (isReady) return;
    try {
      _plansBox ??= await Hive.openBox<String>('cache_plans');
      _wallBox ??= await Hive.openBox<String>('cache_wall');
      _metaBox ??= await Hive.openBox<String>('cache_meta');
    } catch (_) {
      // Si el almacenamiento no esta disponible, la app sigue sin cache.
    }
  }

  Future<void> open() async {
    await init();
  }

  List<int> _xor(List<int> bytes) {
    final key = utf8.encode('opaline_cache_key_2026');
    return List<int>.generate(
      bytes.length,
      (i) => bytes[i] ^ key[i % key.length],
    );
  }

  String _encrypt(String plain) => base64Url.encode(_xor(utf8.encode(plain)));

  String _decrypt(String cipher) => utf8.decode(_xor(base64Url.decode(cipher)));

  /// Guarda datos con TTL.
  Future<void> put(String boxName, String key, String value, {Duration? ttl}) async {
    final box = _boxFor(boxName);
    if (box == null) return;
    final entry = jsonEncode({
      'v': _encrypt(value),
      'e': (DateTime.now().add(ttl ?? defaultTtl)).millisecondsSinceEpoch,
    });
    await box.put(key, entry);
    await _enforceSize(box);
  }

  /// Lee datos si no expiraron. Devuelve null si no existe o expiro.
  String? get(String boxName, String key) {
    final box = _boxFor(boxName);
    if (box == null) return null;
    final raw = box.get(key);
    if (raw == null) return null;
    try {
      final entry = jsonDecode(raw) as Map;
      final expiry = entry['e'] as int;
      if (DateTime.now().millisecondsSinceEpoch > expiry) {
        box.delete(key);
        return null;
      }
      return _decrypt(entry['v'] as String);
    } catch (_) {
      box.delete(key);
      return null;
    }
  }

  Future<void> remove(String boxName, String key) async {
    final box = _boxFor(boxName);
    if (box == null) return;
    await box.delete(key);
  }

  /// Purga todo el cache (al cerrar sesion o eliminar cuenta).
  Future<void> clearAll() async {
    await _plansBox?.clear();
    await _wallBox?.clear();
    await _metaBox?.clear();
  }

  Box<String>? _boxFor(String name) {
    switch (name) {
      case 'plans': return _plansBox;
      case 'wall': return _wallBox;
      case 'meta': return _metaBox;
      default: return null;
    }
  }

  Future<void> _enforceSize(Box<String> box) async {
    // Hive no expone tamano directo; limitamos por cantidad de entradas.
    if (box.length > 500) {
      final keys = box.keys.toList()..sort();
      final toDelete = keys.take(box.length - 500);
      for (final k in toDelete) {
        await box.delete(k);
      }
    }
  }

  /// Marca contenido cacheado con fecha (RNF-MOV-03 observaciones).
  static String cacheDateLabel() => 'Actualizado: ${_format(DateTime.now())}';

  static String _format(DateTime d) =>
      '${d.day.toString().padLeft(2, '0')}/${d.month.toString().padLeft(2, '0')}/${d.year} ${d.hour.toString().padLeft(2, '0')}:${d.minute.toString().padLeft(2, '0')}';
}
