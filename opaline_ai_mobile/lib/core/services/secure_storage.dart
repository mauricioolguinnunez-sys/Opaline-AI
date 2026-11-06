import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';

/// Almacenamiento local de credenciales y tokens.
///
/// Implementacion multiplataforma (compila en Linux, Android, iOS, Web, Windows, macOS)
/// usando shared_preferences con cifrado ligero (XOR + base64) para no guardar
/// credenciales en texto plano (RNF-MOV-02).
///
/// NOTA PARA PRODUCCION (Android/iOS): reemplazar esta clase por flutter_secure_storage
/// (Keychain en iOS / EncryptedSharedPreferences en Android) para cumplir RNF-MOV-02 al 100 %.
class SecureStorage {
  static SharedPreferences? _prefs;

  static Future<void> init() async {
    _prefs = await SharedPreferences.getInstance();
  }

  static Future<void> write(String key, String value) async {
    await _prefs!.setString(key, _encrypt(value));
  }

  static Future<String?> read(String key) async {
    final v = _prefs?.getString(key);
    if (v == null) return null;
    return _decrypt(v);
  }

  static Future<void> delete(String key) async {
    await _prefs!.remove(key);
  }

  static Future<void> deleteAll() async {
    await _prefs!.clear();
  }

  static bool get isInitialized => _prefs != null;

  // ---------- Cifrado ligero (no es sustituto de Keychain en produccion) ----------
  static const String _xorKey = 'opaline_secure_storage_v1';

  static String _encrypt(String plain) {
    final bytes = utf8.encode(plain);
    final key = utf8.encode(_xorKey);
    final enc = bytes
        .asMap()
        .entries
        .map((e) => e.value ^ key[e.key % key.length])
        .toList();
    return base64Url.encode(enc);
  }

  static String _decrypt(String cipher) {
    final bytes = base64Url.decode(cipher);
    final key = utf8.encode(_xorKey);
    final dec = bytes
        .asMap()
        .entries
        .map((e) => e.value ^ key[e.key % key.length])
        .toList();
    return utf8.decode(dec);
  }

  // Keys
  static const String accessToken = 'access_token';
  static const String refreshToken = 'refresh_token';
  static const String tokenExpiry = 'token_expiry';
  static const String userEmail = 'user_email';
  static const String userName = 'user_name';
  static const String userId = 'user_id';
  static const String planId = 'user_plan_id';
  static const String biometricEnabled = 'biometric_enabled';
  static const String lastAuthTime = 'last_auth_time';
  static const String deviceId = 'device_id';
}
