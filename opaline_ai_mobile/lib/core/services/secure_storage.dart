import 'dart:convert';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Almacenamiento de credenciales y tokens respaldado por mecanismos nativos.
///
/// - Android: EncryptedSharedPreferences respaldado por Android Keystore.
/// - iOS/macOS: Keychain.
/// - Linux/Windows/Web: backend nativo de flutter_secure_storage.
///
/// Solo se conserva shared_preferences para migrar una unica vez los valores
/// legados cifrados con XOR. Tras la migracion se eliminan de preferences.
class SecureStorage {
  static const FlutterSecureStorage _storage = FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
    iOptions: IOSOptions(accessibility: KeychainAccessibility.first_unlock_this_device),
  );

  static SharedPreferences? _legacyPrefs;

  static const List<String> _keys = <String>[
    accessToken,
    refreshToken,
    tokenExpiry,
    userEmail,
    userName,
    userId,
    planId,
    biometricEnabled,
    lastAuthTime,
    deviceId,
    'demo_mode',
  ];

  static Future<void> init() async {
    _legacyPrefs = await SharedPreferences.getInstance();
    await _migrateLegacyValues();
  }

  static Future<void> write(String key, String value) async {
    await _storage.write(key: key, value: value);
  }

  static Future<String?> read(String key) async {
    return _storage.read(key: key);
  }

  static Future<void> delete(String key) async {
    await _storage.delete(key: key);
    await _legacyPrefs?.remove(key);
  }

  static Future<void> deleteAll() async {
    for (final key in _keys) {
      await _storage.delete(key: key);
      await _legacyPrefs?.remove(key);
    }
  }

  static bool get isInitialized => true;

  static Future<void> _migrateLegacyValues() async {
    for (final key in _keys) {
      try {
        final secureValue = await _storage.read(key: key);
        final legacyValue = _legacyPrefs?.getString(key);
        if (secureValue != null) {
          await _legacyPrefs?.remove(key);
          continue;
        }
        if (legacyValue == null) continue;

        String value;
        try {
          value = _decryptLegacy(legacyValue);
        } catch (_) {
          // Si el valor legado no era cifrado XOR, se migra tal cual.
          value = legacyValue;
        }
        await _storage.write(key: key, value: value);
        await _legacyPrefs?.remove(key);
      } catch (_) {
        // La migracion es best-effort: un fallo no debe bloquear el arranque.
      }
    }
  }

  // ---------- Compatibilidad temporal con el almacenamiento legado XOR ----------
  static const String _xorKey = 'opaline_secure_storage_v1';

  static String _decryptLegacy(String cipher) {
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