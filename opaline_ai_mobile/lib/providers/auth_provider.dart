import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../core/api/api_client.dart';
import '../core/services/secure_storage.dart';
import '../core/services/session_guard.dart';
import '../models/user.dart';

/// Proveedor de autenticacion y sesion (RF-MOV-02).
///
/// - Los tokens se guardan en almacenamiento cifrado (SecureStorage).
/// - Si la API no responde (entorno de demo/pruebas sin backend), se activa un
///   modo demo para que la app siga siendo navegable y no se quede "pasamada".
class AuthProvider extends ChangeNotifier {
  AppUser? _user;
  String? _accessToken;
  String? _refreshToken;
  bool _loading = false;
  String? _error;
  bool _initialized = false;
  bool _demoMode = false;

  static final List<Future<void> Function()> _sessionClearers = <Future<void> Function()>[];

  static void registerSessionClearer(Future<void> Function() clearer) {
    if (!_sessionClearers.contains(clearer)) {
      _sessionClearers.add(clearer);
    }
  }

  AppUser? get user => _user;
  String? get accessToken => _accessToken;
  bool get loading => _loading;
  String? get error => _error;
  bool get initialized => _initialized;
  bool get demoMode => _demoMode;
  bool get isAuthenticated => _user != null && _accessToken != null;

  Future<void> init() async {
    if (_initialized) return;
    _initialized = true;
    _accessToken = await SecureStorage.read(SecureStorage.accessToken);
    _refreshToken = await SecureStorage.read(SecureStorage.refreshToken);
    final raw = await SecureStorage.read(SecureStorage.userId);
    final name = await SecureStorage.read(SecureStorage.userName);
    final email = await SecureStorage.read(SecureStorage.userEmail);
    // El modo demo anterior no es una sesion autenticada valida. Se elimina
    // para que un valor legado no conceda acceso sin backend.
    _demoMode = false;
    await SecureStorage.delete('demo_mode');

    if (_accessToken != null && raw != null) {
      _user = AppUser(
        id: raw,
        name: name ?? '',
        email: email ?? '',
        planId: '',
        planName: 'Starter',
        subscriptionActive: false,
        dailyQuotaRemaining: 0,
        dailyQuotaLimit: 0,
        createdAt: DateTime.now(),
      );
      SessionGuard.instance.markAuthenticated();
    }
    notifyListeners();
  }

  void _setLoading(bool v) { _loading = v; notifyListeners(); }
  void _setError(String? e) { _error = e; notifyListeners(); }

  /// Registro con nombre, correo y contrasena 10-64 con mayuscula, minuscula, digito, especial (RF-MOV-02).
  Future<bool> register({
    required String name,
    required String email,
    required String password,
  }) async {
    _setLoading(true);
    _setError(null);

    final pwRegex = RegExp(r'^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[\W_]).{10,64}$');
    if (!pwRegex.hasMatch(password)) {
      _setError('La contrasena debe tener 10-64 caracteres, mayuscula, minuscula, un digito y un caracter especial.');
      _setLoading(false);
      return false;
    }

    try {
      final res = await ApiClient().post('/auth/register', body: {
        'name': name,
        'email': email,
        'password': password,
      });
      if (res.statusCode == 200 || res.statusCode == 201) {
        final data = jsonDecode(res.body);
        await _onLoginSuccess(data);
        _setLoading(false);
        return true;
      }
      if (res.statusCode == 400 || res.statusCode == 409 || res.statusCode == 422) {
        _setError('No se pudo crear la cuenta. Revisa los datos o usa otro correo.');
        _setLoading(false);
        return false;
      }
      _setError('El servidor no pudo crear la cuenta (${res.statusCode}).');
      _setLoading(false);
      return false;
    } catch (_) {
      _setError('No se pudo crear la cuenta. Verifica tu conexion e intenta de nuevo.');
      _setLoading(false);
      return false;
    }
  }

  /// Inicio de sesion con correo y contrasena; mensaje generico ante error (RF-MOV-02).
  Future<bool> login({required String email, required String password}) async {
    _setLoading(true);
    _setError(null);

    if (email.trim().isEmpty || password.isEmpty) {
      _setError('Ingresa tu correo y contrasena.');
      _setLoading(false);
      return false;
    }

    try {
      final res = await ApiClient().post('/auth/login', body: {
        'email': email,
        'password': password,
      });
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        await _onLoginSuccess(data);
        _setLoading(false);
        return true;
      }
      if (res.statusCode == 401 || res.statusCode == 403) {
        // Mensaje generico para no revelar si el correo existe (RF-MOV-02)
        _setError('Credenciales invalidas.');
        _setLoading(false);
        return false;
      }
      _setError('El servidor no pudo iniciar sesión (${res.statusCode}).');
      _setLoading(false);
      return false;
    } catch (_) {
      _setError('No se pudo iniciar sesion. Verifica tu conexion e intenta de nuevo.');
      _setLoading(false);
      return false;
    }
  }


  Future<void> _onLoginSuccess(Map<String, dynamic> data) async {
    final accessToken = data['access_token']?.toString();
    final refreshToken = data['refresh_token']?.toString();
    final rawUser = data['user'];
    if (accessToken == null || accessToken.isEmpty ||
        refreshToken == null || refreshToken.isEmpty ||
        rawUser is! Map<String, dynamic>) {
      throw const FormatException('Respuesta de autenticacion incompleta');
    }

    final user = AppUser.fromJson(rawUser);
    if (user.id.isEmpty) {
      throw const FormatException('Usuario invalido');
    }

    _demoMode = false;
    _accessToken = accessToken;
    _refreshToken = refreshToken;
    _user = user;

    await SecureStorage.write(SecureStorage.accessToken, _accessToken!);
    await SecureStorage.write(SecureStorage.refreshToken, _refreshToken!);
    await SecureStorage.write(SecureStorage.userId, _user!.id);
    await SecureStorage.write(SecureStorage.userName, _user!.name);
    await SecureStorage.write(SecureStorage.userEmail, _user!.email);
    await SecureStorage.delete('demo_mode');
    SessionGuard.instance.markAuthenticated();
    notifyListeners();
  }

  /// Cierra sesion, borra tokens y limpia cache local (RF-MOV-02).
  Future<void> logout() async {
    try {
      await ApiClient().post('/auth/logout').timeout(const Duration(seconds: 3));
    } catch (_) {}
    _accessToken = null;
    _refreshToken = null;
    _user = null;
    _demoMode = false;
    await SecureStorage.delete(SecureStorage.accessToken);
    await SecureStorage.delete(SecureStorage.refreshToken);
    await SecureStorage.delete(SecureStorage.userId);
    await SecureStorage.delete(SecureStorage.userName);
    await SecureStorage.delete(SecureStorage.userEmail);
    await SecureStorage.delete('demo_mode');

    // Limpia datos privados de los demas providers sin depender de su ciclo.
    for (final clearer in List<Future<void> Function()>.from(_sessionClearers)) {
      try {
        await clearer();
      } catch (_) {}
    }

    await SessionGuard.instance.logout();
    notifyListeners();
  }

  /// Renueva access token antes de expirar (RF-MOV-02).
  Future<bool> refreshAccessToken() async {
    if (_demoMode) return true;
    final refresh = await SecureStorage.read(SecureStorage.refreshToken);
    if (refresh == null) return false;
    try {
      final res = await ApiClient().post('/auth/refresh', body: {'refresh_token': refresh});
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body) as Map<String, dynamic>;
        _accessToken = data['access_token'];
        _refreshToken = data['refresh_token'] ?? _refreshToken;
        if (_accessToken != null) {
          await SecureStorage.write(SecureStorage.accessToken, _accessToken!);
        }
        if (_refreshToken != null && _refreshToken!.isNotEmpty) {
          await SecureStorage.write(SecureStorage.refreshToken, _refreshToken!);
        }
        notifyListeners();
        return true;
      }
    } catch (_) {}
    await logout();
    return false;
  }

  /// Cambia contrasena con la actual y revoca demas sesiones (RF-MOV-02).
  Future<bool> changePassword({required String current, required String newPassword}) async {
    final pwRegex = RegExp(r'^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[\W_]).{10,64}$');
    if (!pwRegex.hasMatch(newPassword)) {
      _setError('La nueva contrasena no cumple los requisitos.');
      return false;
    }
    _setLoading(true);
    _setError(null);
    if (_demoMode) {
      await Future<void>.delayed(const Duration(milliseconds: 400));
      _setLoading(false);
      return true;
    }
    try {
      final res = await ApiClient().post('/auth/change-password', body: {
        'current_password': current,
        'new_password': newPassword,
      });
      if (res.statusCode == 200) {
        _setLoading(false);
        return true;
      }
      _setError('Contrasena actual incorrecta.');
    } catch (e) {
      _setError('No se pudo cambiar la contrasena.');
    }
    _setLoading(false);
    return false;
  }

  /// Solicita recuperacion por correo (abre enlace en navegador) (RF-MOV-02).
  Future<bool> requestPasswordReset(String email) async {
    _setLoading(true);
    _setError(null);
    try {
      final res = await ApiClient().post('/auth/forgot-password', body: {'email': email});
      _setLoading(false);
      return res.statusCode == 200;
    } catch (e) {
      _setError('No se pudo solicitar la recuperacion.');
      _setLoading(false);
      return false;
    }
  }

  /// Historial de sesiones y cerrar sesion remota (RF-MOV-02).
  Future<List<dynamic>> fetchSessions() async {
    if (_demoMode) {
      return [
        {
          'id': 's1',
          'device': 'Este dispositivo',
          'ip': '127.0.0.1',
          'is_current': true,
          'created_at': DateTime.now().toIso8601String(),
        },
        {
          'id': 's2',
          'device': 'Navegador web',
          'ip': '192.168.1.10',
          'is_current': false,
          'created_at': DateTime.now()
              .subtract(const Duration(days: 2))
              .toIso8601String(),
        },
      ];
    }
    try {
      final res = await ApiClient().get('/auth/sessions');
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        if (data is List) return data;
        if (data is Map<String, dynamic> && data['sessions'] is List) {
          return data['sessions'] as List;
        }
      }
    } catch (_) {}
    return [];
  }

  void clearError() { _setError(null); }
}
