import 'package:connectivity_plus/connectivity_plus.dart';

/// Servicio de conectividad para manejar errores de red con reintento
/// y modo solo lectura offline (RF-MOV-01 / RNF-MOV-03).
class ConnectivityService {
  ConnectivityService._internal();
  static final ConnectivityService instance = ConnectivityService._internal();

  final Connectivity _connectivity = Connectivity();
  bool _hasConnection = true;

  bool get hasConnection => _hasConnection;

  Stream<bool> get connectionStream =>
      _connectivity.onConnectivityChanged.map((results) {
        final connected = results.any((r) => r != ConnectivityResult.none);
        _hasConnection = connected;
        return connected;
      });

  Future<bool> check() async {
    final results = await _connectivity.checkConnectivity();
    _hasConnection = results.any((r) => r != ConnectivityResult.none);
    return _hasConnection;
  }
}
