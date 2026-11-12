import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:http/io_client.dart';

import '../services/secure_storage.dart';

/// Cliente HTTP centralizado para la API de negocio (PHP) y la API de IA (FastAPI).
/// - HTTPS obligatorio; en release rechaza certificados invalidos (RNF-MOV-02).
/// - Inyecta access token en cada peticion (RF-MOV-02).
/// - Timeout corto para que la UI nunca se quede "pasamada" si la API no responde.
/// - Circuit breaker: tras un fallo de red, las siguientes peticiones fallan de
///   inmediato durante 30 s. Asi el modo demo no espera 8 s por cada pantalla.
class ApiClient {
  /// En debug apunta al backend Rust local. El emulador Android debe usar
  /// 10.0.2.2 (el host real) en lugar de 127.0.0.1.
  static String get _debugHost {
    if (defaultTargetPlatform == TargetPlatform.android) return 'http://10.0.2.2:8090/api/v1';
    return 'http://127.0.0.1:8090/api/v1';
  }

  static String get businessBaseUrl =>
      kDebugMode ? _debugHost : 'https://www.opaline-ai.com/api/v1';
  static String get aiBaseUrl =>
      kDebugMode ? _debugHost : 'https://www.opaline-ai.com/api/v1';

  /// Timeout de red para la API de negocio (login, planes, muro, pagos).
  static const Duration businessTimeout = Duration(seconds: 12);

  /// Timeout de red para la API de IA (LM Studio puede tardar con modelos grandes).
  static const Duration aiTimeout = Duration(seconds: 180);

  static DateTime? _offlineUntil;

  static final ApiClient _instance = ApiClient._internal();
  factory ApiClient() => _instance;
  ApiClient._internal();

  http.Client _createClient(Duration timeout) {
    final client = HttpClient()
      ..connectionTimeout = timeout
      ..badCertificateCallback = (cert, host, port) {
        if (kDebugMode) return true;
        return false;
      };
    return IOClient(client);
  }

  Map<String, String> _headers(String? token, Map<String, String>? extra) => {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        if (token != null) 'Authorization': 'Bearer $token',
        ...?extra,
      };

  Future<http.Response> get(String path,
          {bool ai = false, Map<String, String>? extraHeaders}) =>
      _send('GET', path, ai: ai, extraHeaders: extraHeaders);

  Future<http.Response> post(String path,
          {Object? body,
          bool ai = false,
          Map<String, String>? extraHeaders}) =>
      _send('POST', path, body: body, ai: ai, extraHeaders: extraHeaders);

  Future<http.Response> put(String path,
          {Object? body,
          bool ai = false,
          Map<String, String>? extraHeaders}) =>
      _send('PUT', path, body: body, ai: ai, extraHeaders: extraHeaders);

  Future<http.Response> delete(String path,
          {bool ai = false, Map<String, String>? extraHeaders}) =>
      _send('DELETE', path, ai: ai, extraHeaders: extraHeaders);

  /// Descarga un archivo (factura PDF) y devuelve los bytes.
  Future<List<int>?> downloadBytes(String path, {bool ai = false}) async {
    final res = await _send('GET', path, ai: ai);
    if (res.statusCode == 200) return res.bodyBytes;
    return null;
  }

  Future<http.Response> _send(
    String method,
    String path, {
    Object? body,
    bool ai = false,
    Map<String, String>? extraHeaders,
    bool retry = true,
  }) async {
    // Circuit breaker: si hace poco la API no respondio, fallar rapido.
    final offlineUntil = _offlineUntil;
    if (offlineUntil != null && DateTime.now().isBefore(offlineUntil)) {
      throw const SocketException('API no disponible (fallo reciente de red)');
    }

    final timeout = ai ? aiTimeout : businessTimeout;
    final token = await _readToken();
    final uri = Uri.parse('${ai ? aiBaseUrl : businessBaseUrl}$path');
    final client = _createClient(timeout);

    try {
      final request = http.Request(method, uri)
        ..headers.addAll(_headers(token, extraHeaders));
      if (body != null) request.body = jsonEncode(body);

      final streamed = await client.send(request).timeout(timeout);
      final response = await http.Response.fromStream(streamed).timeout(timeout);
      _offlineUntil = null; // La API respondio: desactivar circuit breaker.

      // La sesion de la API dura 15 minutos. Renovar y reintentar una sola vez
      // para que el usuario no vea errores inesperados a mitad de una tarea.
      const noRefreshPaths = {
        '/auth/login',
        '/auth/register',
        '/auth/refresh',
        '/auth/forgot-password',
      };
      if (response.statusCode == 401 &&
          retry &&
          token != null &&
          !noRefreshPaths.contains(path)) {
        final refreshed = await _refreshSession();
        if (refreshed) {
          return await _send(
            method,
            path,
            body: body,
            ai: ai,
            extraHeaders: extraHeaders,
            retry: false,
          );
        }
      }
      return response;
    } catch (e) {
      _offlineUntil = DateTime.now().add(const Duration(seconds: 30));
      rethrow;
    } finally {
      client.close();
    }
  }

  Future<bool> _refreshSession() async {
    try {
      final refresh = await SecureStorage.read(SecureStorage.refreshToken);
      if (refresh == null || refresh.isEmpty) return false;
      final res = await _send(
        'POST',
        '/auth/refresh',
        body: {'refresh_token': refresh},
        retry: false,
      );
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body) as Map<String, dynamic>;
        final access = data['access_token']?.toString();
        if (access == null || access.isEmpty) return false;
        await SecureStorage.write(SecureStorage.accessToken, access);
        final newRefresh = data['refresh_token']?.toString();
        if (newRefresh != null && newRefresh.isNotEmpty) {
          await SecureStorage.write(SecureStorage.refreshToken, newRefresh);
        }
        return true;
      }
    } catch (_) {}
    return false;
  }

  Future<String?> _readToken() async {
    try {
      return await _tokenReader?.call();
    } catch (_) {
      return null;
    }
  }

  /// Inyectado desde main.dart para leer el token de SecureStorage sin acoplar capas.
  static Future<String?> Function()? _tokenReader;
  static void setTokenReader(Future<String?> Function() reader) {
    _tokenReader = reader;
  }

  /// Fuerza el reintento inmediato (por ejemplo, al pulsar "Reintentar").
  static void resetCircuitBreaker() {
    _offlineUntil = null;
  }
}

class ApiException implements Exception {
  final int statusCode;
  final String message;
  ApiException({required this.statusCode, required this.message});
  @override
  String toString() => 'ApiException($statusCode): $message';
}
