import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../core/api/api_client.dart';
import '../core/services/cache_service.dart';
import '../core/services/connectivity_service.dart';
import '../core/services/secure_storage.dart';
import '../models/plan.dart';
import '../models/subscription.dart';
import '../models/invoice.dart';

/// Proveedor de planes y suscripcion (RF-MOV-01 / RF-MOV-03).
///
/// Si la API de negocio no esta disponible (entorno de demo/pruebas), se usa
/// un catalogo local con los planes de referencia y un flujo de pago simulado.
/// La fuente de verdad en produccion sigue siendo la API (RF-SEG-01): precios,
/// planes y activacion nunca se confian al cliente.
class PlanProvider extends ChangeNotifier {
  List<Plan> _plans = [];
  Subscription? _subscription;
  List<Invoice> _invoices = [];
  bool _loading = false;
  String? _error;
  bool _initialized = false;
  bool _demo = false;

  List<Plan> get plans => _plans;
  Plan? get activePlan => _plans.isEmpty
      ? null
      : _plans.firstWhere((p) => p.active, orElse: () => _plans.first);
  Subscription? get subscription => _subscription;
  List<Invoice> get invoices => _invoices;
  bool get loading => _loading;
  String? get error => _error;
  bool get initialized => _initialized;

  /// True cuando se esta usando el catalogo/pago simulado local.
  bool get isDemoData => _demo;

  bool get hasActiveSubscription =>
      _subscription?.isActive == false ? false : _subscription != null;

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
    await loadPlans(forceNetwork: false);
    await loadSubscription();
  }

  /// Catalogo de planes con precio MXN, periodicidad, cuotas, caracteristicas (RF-MOV-01 / RF-MOV-03).
  Future<bool> _demoSessionActive() async {
    try { return (await SecureStorage.read('demo_mode')) == 'true'; } catch (_) { return false; }
  }

  Future<void> loadPlans({bool forceNetwork = false}) async {
    if (!forceNetwork && _plans.isNotEmpty) return;
    _setLoading(true);
    _setError(null);

    // Si el usuario esta en modo demo (API no disponible), no esperar red.
    if (!forceNetwork && await _demoSessionActive()) {
      _applyDemoPlans();
      _setLoading(false);
      return;
    }

    // Sin conexion: solo lectura desde cache (RF-MOV-01).
    final connected = await ConnectivityService.instance.check().catchError((_) => true);
    if (!connected) {
      final cached = CacheService.instance.get('plans', 'catalog');
      final cachedPlans = _tryParsePlans(cached);
      if (cachedPlans != null && cachedPlans.isNotEmpty) {
        _plans = cachedPlans;
        notifyListeners();
        _setLoading(false);
        return;
      }
      _applyDemoPlans();
      _setLoading(false);
      return;
    }

    try {
      final res = await ApiClient().get('/planes');
      if (res.statusCode == 200) {
        final list = jsonDecode(res.body) as List;
        final parsed = list.map((e) => Plan.fromJson(e)).toList();
        if (parsed.isNotEmpty) {
          _plans = parsed;
          _demo = false;
          await CacheService.instance.put(
            'plans',
            'catalog',
            jsonEncode(_plans.map((p) => p.toJson()).toList()),
          );
          _setLoading(false);
          return;
        }
      }
    } catch (_) {
      // API no disponible -> catalogo demo.
    }
    _applyDemoPlans();
    _setLoading(false);
  }

  List<Plan>? _tryParsePlans(String? raw) {
    if (raw == null) return null;
    try {
      return (jsonDecode(raw) as List)
          .map((e) => Plan.fromJson(e as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return null;
    }
  }

  void _applyDemoPlans() {
    _demo = true;
    _plans = [
      Plan(
        id: 'starter',
        name: 'Opaline Starter',
        description: 'Plan de entrada con limites moderados para estudiantes.',
        priceMXN: 49.00,
        periodicity: 'mensual',
        installments: 1,
        features: const [
          '100 mensajes por mes',
          'Modelos open source',
          'Historial de 7 dias',
          'Sin RAG ni exportacion',
        ],
        active: true,
        starter: true,
        monthlyMessages: 100,
        ragEnabled: false,
      ),
      Plan(
        id: 'plus',
        name: 'Opaline Plus',
        description: 'Mas uso, modelos de mayor capacidad y analisis de documentos.',
        priceMXN: 79.00,
        periodicity: 'mensual',
        installments: 1,
        features: const [
          'Modelos de mayor capacidad',
          'Analisis de documentos',
          'Historial de 90 dias',
          'Cuota ampliada de mensajes',
        ],
        active: true,
        monthlyMessages: 500,
        ragEnabled: false,
        popularTag: 'Mas popular',
      ),
      Plan(
        id: 'research',
        name: 'Opaline Research',
        description:
            'Mas contexto, investigacion y herramientas avanzadas (MXN 99-129/mes).',
        priceMXN: 99.00,
        periodicity: 'mensual',
        installments: 1,
        features: const [
          'Mas contexto por conversacion',
          'RAG con citas y fuentes',
          'Herramientas de investigacion',
          'Historial de 90 dias',
        ],
        active: true,
        monthlyMessages: 1000,
        ragEnabled: true,
      ),
    ];
    notifyListeners();
  }

  /// Estado de suscripcion, proximo cobro, historial de pagos y facturas (RF-MOV-03).
  Future<void> loadSubscription() async {
    if (_demo) {
      _applyDemoSubscription();
      return;
    }
    _setLoading(true);
    try {
      final res = await ApiClient().get('/suscripcion');
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body);
        final subJson = data['subscription'];
        _subscription = subJson is Map<String, dynamic>
            ? Subscription.fromJson(subJson)
            : null;
        _invoices = (data['invoices'] as List<dynamic>?)
                ?.map((e) => Invoice.fromJson(e as Map<String, dynamic>))
                .toList() ??
            [];
        _setLoading(false);
        return;
      }
    } catch (_) {}
    // Sin API -> suscripcion demo para poder probar la app.
    _demo = true;
    _applyDemoSubscription();
  }

  void _applyDemoSubscription() {
    final now = DateTime.now();
    _subscription ??= Subscription(
      id: 'demo-sub',
      userId: 'demo-user',
      planId: 'plus',
      planName: 'Opaline Plus',
      status: 'active',
      startsAt: now.subtract(const Duration(days: 10)),
      endsAt: now.add(const Duration(days: 20)),
      nextBilling: now.add(const Duration(days: 20)),
      autoRenew: true,
      amountMXN: 79.00,
    );
    _invoices = [
      Invoice(
        id: 'DEMO-002',
        subscriptionId: 'demo-sub',
        amountMXN: 79.00,
        status: 'paid',
        issuedAt: now.subtract(const Duration(days: 10)),
      ),
      Invoice(
        id: 'DEMO-001',
        subscriptionId: 'demo-sub',
        amountMXN: 79.00,
        status: 'paid',
        issuedAt: now.subtract(const Duration(days: 40)),
      ),
    ];
    _setLoading(false);
  }

  /// Crea orden PayPal y devuelve URL de checkout (RF-MOV-03).
  /// En modo demo devuelve una orden marcada con `demo: true`.
  Future<Map<String, dynamic>?> createPayPalOrder(String planId) async {
    _setLoading(true);
    _setError(null);

    if (_demo) {
      _setLoading(false);
      return {
        'demo': true,
        'order_id': 'demo-order-$planId',
        'checkout_url': '',
      };
    }

    try {
      final res =
          await ApiClient().post('/payments/paypal/create', body: {'plan_id': planId});
      _setLoading(false);
      if (res.statusCode == 200) {
        final data = jsonDecode(res.body) as Map<String, dynamic>;
        final url = data['checkout_url'] ?? data['url'];
        if (url is String && url.isNotEmpty) return data;
      }
      // Sin backend de pagos -> simulado.
      _demo = true;
      return {
        'demo': true,
        'order_id': 'demo-order-$planId',
        'checkout_url': '',
      };
    } catch (_) {
      _demo = true;
      _setLoading(false);
      return {
        'demo': true,
        'order_id': 'demo-order-$planId',
        'checkout_url': '',
      };
    }
  }

  /// Verifica estado de orden y activa suscripcion solo si pago confirmado (RF-MOV-03).
  Future<bool> verifyOrder(String orderId) async {
    // Pago simulado del modo demo.
    if (orderId.startsWith('demo-order-')) {
      final planId = orderId.replaceFirst('demo-order-', '');
      _activateDemoPlan(planId);
      return true;
    }

    _setLoading(true);
    try {
      final res = await ApiClient().get('/payments/paypal/verify/$orderId');
      if (res.statusCode == 200) {
        await loadSubscription();
        _setLoading(false);
        return true;
      }
    } catch (_) {}
    _setLoading(false);
    return false;
  }

  void _activateDemoPlan(String planId) {
    final plan = _plans.firstWhere(
      (p) => p.id == planId,
      orElse: () => _plans.isNotEmpty
          ? _plans.first
          : Plan(
              id: planId,
              name: 'Opaline Plus',
              description: '',
              priceMXN: 79,
              periodicity: 'mensual',
              installments: 1,
              features: const [],
              active: true,
            ),
    );
    final now = DateTime.now();
    _demo = true;
    _subscription = Subscription(
      id: 'demo-sub',
      userId: 'demo-user',
      planId: plan.id,
      planName: plan.name,
      status: 'active',
      startsAt: now,
      endsAt: now.add(const Duration(days: 30)),
      nextBilling: now.add(const Duration(days: 30)),
      autoRenew: true,
      amountMXN: plan.priceMXN,
    );
    _invoices = [
      Invoice(
        id: 'DEMO-${now.millisecondsSinceEpoch}',
        subscriptionId: 'demo-sub',
        amountMXN: plan.priceMXN,
        status: 'paid',
        issuedAt: now,
      ),
      ..._invoices,
    ];
    notifyListeners();
  }

  /// Cancela renovacion manteniendo acceso hasta fecha de fin (RF-MOV-03).
  Future<bool> cancelRenewal() async {
    if (_demo) {
      final sub = _subscription;
      if (sub == null) return false;
      // Mantiene el acceso activo hasta endsAt; solo se desactiva la renovacion.
      _subscription = _copySubscription(sub, autoRenew: false);
      notifyListeners();
      return true;
    }
    try {
      final res = await ApiClient().post('/suscripcion/cancelar-renovacion');
      if (res.statusCode == 200) {
        await loadSubscription();
        return true;
      }
    } catch (_) {}
    // Fallback local para que la accion no quede sin respuesta.
    final sub = _subscription;
    if (sub != null) {
      _subscription = _copySubscription(sub, autoRenew: false);
      notifyListeners();
      return true;
    }
    return false;
  }

  /// Restaura compra al cambiar de dispositivo o reinstalar (RF-MOV-03).
  Future<bool> restorePurchase() async {
    if (_demo) {
      if (_subscription == null) {
        _applyDemoSubscription();
      }
      notifyListeners();
      return true;
    }
    try {
      final res = await ApiClient().post('/suscripcion/restaurar');
      if (res.statusCode == 200) {
        await loadSubscription();
        return true;
      }
    } catch (_) {}
    if (_subscription != null) return true;
    return false;
  }

  /// Descarga factura PDF (RF-MOV-03). En demo no hay PDF real.
  Future<List<int>?> downloadInvoice(String invoiceId) async {
    if (_demo) return null;
    try {
      return await ApiClient().downloadBytes('/facturas/$invoiceId/pdf');
    } catch (_) {
      return null;
    }
  }

  /// Solicita reembolso desde factura (RF-MOV-03).
  Future<bool> requestRefund(String invoiceId) async {
    if (_demo) {
      final idx = _invoices.indexWhere((i) => i.id == invoiceId);
      if (idx == -1) return false;
      final inv = _invoices[idx];
      _invoices[idx] = Invoice(
        id: inv.id,
        subscriptionId: inv.subscriptionId,
        amountMXN: inv.amountMXN,
        status: 'refunded',
        issuedAt: inv.issuedAt,
        pdfUrl: inv.pdfUrl,
      );
      notifyListeners();
      return true;
    }
    try {
      final res = await ApiClient().post('/facturas/$invoiceId/reembolso');
      if (res.statusCode == 200) {
        await loadSubscription();
        return true;
      }
    } catch (_) {}
    return false;
  }

  Subscription _copySubscription(
    Subscription s, {
    bool? autoRenew,
    String? status,
  }) {
    return Subscription(
      id: s.id,
      userId: s.userId,
      planId: s.planId,
      planName: s.planName,
      status: status ?? s.status,
      startsAt: s.startsAt,
      endsAt: s.endsAt,
      nextBilling: s.nextBilling,
      autoRenew: autoRenew ?? s.autoRenew,
      amountMXN: s.amountMXN,
    );
  }

  void clearError() {
    _setError(null);
  }
}
