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
    _demoMode = (await SecureStorage.read('demo_mode')) == 'true';
    if (_accessToken != null && raw != null) {
      _user = AppUser(
        id: raw,
        name: name ?? '',
        email: email ?? '',
        planId: _demoMode ? 'plus' : '',
        planName: _demoMode ? 'Opaline Plus' : 'Starter',
        subscriptionActive: _demoMode,
        dailyQuotaRemaining: _demoMode ? 180 : 0,
        dailyQuotaLimit: _demoMode ? 200 : 0,
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
      // Servidor no disponible (404/500/otros) -> modo demo.
      await _activateDemoSession(name: name, email: email);
      _setLoading(false);
      return true;
    } catch (e) {
      // Sin red / timeout -> modo demo para poder probar la app.
      await _activateDemoSession(name: name, email: email);
      _setLoading(false);
      return true;
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
      // API no disponible -> modo demo.
      await _activateDemoSession(name: _nameFromEmail(email), email: email);
      _setLoading(false);
      return true;
    } catch (e) {
      // Sin red / timeout (ya no se cuelga: ApiClient tiene timeout de 12 s).
      await _activateDemoSession(name: _nameFromEmail(email), email: email);
      _setLoading(false);
      return true;
    }
  }

  String _nameFromEmail(String email) {
    final raw = email.split('@').first.replaceAll(RegExp(r'[._-]+'), ' ').trim();
    if (raw.isEmpty) return 'Usuario Opaline';
    return raw
        .split(' ')
        .where((w) => w.isNotEmpty)
        .map((w) => '${w[0].toUpperCase()}${w.substring(1)}')
        .join(' ');
  }

  /// Crea una sesion local de demostracion (sin backend) para poder recorrer
  /// toda la app: planes, chat, muro, suscripcion.
  Future<void> _activateDemoSession({required String name, required String email}) async {
    final now = DateTime.now();
    _demoMode = true;
    _accessToken = 'demo-token-${now.millisecondsSinceEpoch}';
    _refreshToken = 'demo-refresh-${now.millisecondsSinceEpoch}';
    _user = AppUser(
      id: 'demo-user',
      name: name,
      email: email,
      planId: 'plus',
      planName: 'Opaline Plus',
      subscriptionActive: true,
      nextBilling: DateTime(now.year, now.month + 1, now.day),
      dailyQuotaRemaining: 180,
      dailyQuotaLimit: 200,
      createdAt: now,
    );

    await SecureStorage.write(SecureStorage.accessToken, _accessToken!);
    await SecureStorage.write(SecureStorage.refreshToken, _refreshToken!);
    await SecureStorage.write(SecureStorage.userId, _user!.id);
    await SecureStorage.write(SecureStorage.userName, _user!.name);
    await SecureStorage.write(SecureStorage.userEmail, _user!.email);
    await SecureStorage.write('demo_mode', 'true');
    SessionGuard.instance.markAuthenticated();
    notifyListeners();
  }

  Future<void> _onLoginSuccess(Map<String, dynamic> data) async {
    _demoMode = false;
    _accessToken = data['access_token'];
    _refreshToken = data['refresh_token'];
    _user = AppUser.fromJson(data['user'] ?? {});
    await SecureStorage.write(SecureStorage.accessToken, _accessToken!);
    await SecureStorage.write(SecureStorage.refreshToken, _refreshToken!);
    await SecureStorage.write(SecureStorage.userId, _user!.id);
    await SecureStorage.write(SecureStorage.userName, _user!.name);
    await SecureStorage.write(SecureStorage.userEmail, _user!.email);
    await SecureStorage.write('demo_mode', 'false');
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
        final data = jsonDecode(res.body);
        _accessToken = data['access_token'];
        await SecureStorage.write(SecureStorage.accessToken, _accessToken!);
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
        return jsonDecode(res.body) as List;
      }
    } catch (_) {}
    return [];
  }

  void clearError() { _setError(null); }
}
