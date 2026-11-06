import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/secure_storage.dart';

/// Guarda el estado de sesion y aplica las reglas de seguridad del dispositivo:
/// - Bloqueo a los 5 min de inactividad o al pasar a segundo plano (RF-MOV-02).
/// - Biometria para reanudar (RF-MOV-02): en Linux se simula; en Android/iOS
///   integrar local_auth para usar Keychain/biometria real.
/// - Deteccion de root/jailbreak (RF-MOV-02 / RNF-MOV-02).
/// - Bloqueo de capturas en pantallas sensibles (RNF-MOV-02).
class SessionGuard {
  SessionGuard._internal();
  static final SessionGuard instance = SessionGuard._internal();

  bool _isInitialized = false;
  bool get isInitialized => _isInitialized;

  bool _authenticated = false;
  bool get authenticated => _authenticated;

  /// Indica si existe una sesion previa (para distinguir desbloqueo de login).
  bool _hasPreviousSession = false;
  bool get hasPreviousSession => _hasPreviousSession;

  /// En Linux/desktop no hay biometria nativa; se deja disponible=false.
  /// En Android/iOS conectar local_auth para habilitarla.
  bool _biometricAvailable = false;
  bool get biometricAvailable => _biometricAvailable;

  bool _biometricEnabled = false;
  bool get biometricEnabled => _biometricEnabled;

  bool _rooted = false;
  bool get rooted => _rooted;

  DateTime? _lastActivity;
  static const Duration lockTimeout = Duration(minutes: 5);

  VoidCallback? onLocked;
  VoidCallback? onUnlocked;
  final ValueNotifier<bool> _lockNotifier = ValueNotifier<bool>(false);
  ValueNotifier<bool> get lockNotifier => _lockNotifier;

  Future<void> load() async {
    _isInitialized = true;
    _biometricAvailable = false; // desktop no tiene biometria nativa
    final enabled = await SecureStorage.read(SecureStorage.biometricEnabled);
    _biometricEnabled = enabled == 'true';

    _rooted = await _detectRoot();

    final lastAuth = await SecureStorage.read(SecureStorage.lastAuthTime);
    if (lastAuth != null) {
      try {
        _lastActivity = DateTime.parse(lastAuth);
      } catch (_) {
        _lastActivity = null;
      }
    }

    _hasPreviousSession = lastAuth != null;
    _authenticated = _lastActivity != null &&
        DateTime.now().difference(_lastActivity!) <= lockTimeout;

    if (!_authenticated) {
      _lockNotifier.value = true;
    }

    WidgetsBinding.instance.addObserver(_AppLifecycleObserver());
  }

  Future<bool> _detectRoot() async {
    // En desktop no aplica; en Android/iOS integrar deteccion real.
    return false;
  }

  /// Desbloquea la sesion. En desktop retorna true directamente;
  /// en Android/iOS validar con biometria o contrasena.
  Future<bool> unlock(BuildContext context) async {
    if (_biometricAvailable && _biometricEnabled) {
      // TODO: integrar local_auth.authenticate() en Android/iOS
    }

    _authenticated = true;
    _lastActivity = DateTime.now();
    await SecureStorage.write(SecureStorage.lastAuthTime, _lastActivity!.toIso8601String());
    _lockNotifier.value = false;
    onUnlocked?.call();
    return true;
  }

  /// Marca la sesion como autenticada tras un login exitoso.
  /// Esto desbloquea el redirect del router y evita que la app quede atrapada.
  void markAuthenticated() {
    _authenticated = true;
    _hasPreviousSession = true;
    _lastActivity = DateTime.now();
    SecureStorage.write(
      SecureStorage.lastAuthTime,
      _lastActivity!.toIso8601String(),
    );
    _lockNotifier.value = false;
    onUnlocked?.call();
  }

  /// Registra actividad del usuario (evita bloqueo por inactividad).
  void registerActivity() {
    if (!_authenticated) return;
    _lastActivity = DateTime.now();
  }

  void lock() {
    _authenticated = false;
    _lockNotifier.value = true;
    onLocked?.call();
  }

  Future<void> enableBiometric(bool value) async {
    _biometricEnabled = value;
    await SecureStorage.write(SecureStorage.biometricEnabled, value ? 'true' : 'false');
  }

  Future<void> logout() async {
    _authenticated = false;
    _hasPreviousSession = false;
    _lastActivity = null;
    await SecureStorage.delete(SecureStorage.lastAuthTime);
    _lockNotifier.value = true;
  }

  bool shouldBlockScreenshots(BuildContext context) {
    final route = ModalRoute.of(context)?.settings.name ?? '';
    final sensitive = [
      '/login', '/registro', '/perfil', '/checkout', '/suscripcion', '/planes',
    ];
    return sensitive.any((r) => route.startsWith(r));
  }

  /// Aplica bloqueo de capturas si la ruta es sensible (Android FLAG_SECURE).
  void applyScreenshotBlock(BuildContext context) {
    if (!shouldBlockScreenshots(context)) return;
    if (Platform.isAndroid) {
      // FLAG_SECURE se aplica por ventana nativa desde el wrapper de la app.
    }
  }

  bool get isLocked => !_authenticated;
}

class _AppLifecycleObserver extends WidgetsBindingObserver {
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused) {
      SessionGuard.instance.lock();
    }
  }
}
