import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../core/app_theme.dart';
import '../../../models/plan.dart';
import '../../../providers/plan_provider.dart';
import 'package:provider/provider.dart';

/// Checkout de PayPal en WebView con barra de direccion visible (RF-MOV-03).
/// Retorno por enlace profundo y validacion en servidor.
/// Si el backend de pagos no esta disponible, muestra un pago simulado (demo).
class CheckoutScreen extends StatefulWidget {
  const CheckoutScreen({super.key});
  @override
  State<CheckoutScreen> createState() => _CheckoutScreenState();
}

class _CheckoutScreenState extends State<CheckoutScreen> {
  WebViewController? _controller;
  bool _loading = true;
  String? _error;
  String? _demoOrderId;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final provider = context.read<PlanProvider>();
    final requestedId = GoRouterState.of(context).uri.queryParameters['plan'];
    Plan? plan;
    if (requestedId != null) {
      for (final p in provider.plans) {
        if (p.id == requestedId) {
          plan = p;
          break;
        }
      }
    }
    plan ??= provider.activePlan;
    if (plan == null) {
      setState(() {
        _loading = false;
        _error = 'No hay un plan disponible para comprar.';
      });
      return;
    }
    final selectedPlan = plan;

    final order = await provider.createPayPalOrder(selectedPlan.id);
    if (!mounted) return;
    if (order == null) {
      setState(() {
        _loading = false;
        _error = 'No se pudo iniciar el pago con PayPal.';
      });
      return;
    }

    final checkoutUrl =
        (order['checkout_url'] ?? order['url'] ?? '').toString();
    if (order['demo'] == true || checkoutUrl.isEmpty) {
      // Backend de pagos no disponible: pago simulado.
      setState(() {
        _loading = false;
        _demoOrderId =
            (order['order_id'] ?? 'demo-order-${selectedPlan.id}').toString();
      });
      return;
    }

    try {
      final controller = WebViewController()
        ..setJavaScriptMode(JavaScriptMode.unrestricted)
        ..setNavigationDelegate(
          NavigationDelegate(
            onPageStarted: (url) {
              if (mounted) setState(() => _loading = true);
            },
            onPageFinished: (url) {
              if (mounted) setState(() => _loading = false);
            },
            onNavigationRequest: (request) async {
              // Enlace profundo de retorno (RF-MOV-03)
              if (request.url.startsWith('opalineai://') ||
                  request.url.startsWith(
                      'https://www.opaline-ai.com/pago-retorno')) {
                final orderId = _extractOrderId(request.url);
                if (orderId != null) {
                  await _verifyAndFinish(orderId);
                }
                return NavigationDecision.prevent;
              }
              return NavigationDecision.navigate;
            },
          ),
        )
        ..loadRequest(Uri.parse(checkoutUrl));
      if (mounted) {
        setState(() => _controller = controller);
      }
    } catch (_) {
      // WebView no disponible (p. ej. Linux): abrir en navegador externo.
      setState(() => _loading = false);
      await _openExternal(checkoutUrl);
    }
  }

  Future<void> _verifyAndFinish(String orderId) async {
    final verified = await context.read<PlanProvider>().verifyOrder(orderId);
    if (!mounted) return;
    if (verified) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Pago verificado y suscripcion activada')),
      );
      context.goNamed('suscripcion');
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('No se pudo verificar el pago.')),
      );
    }
  }

  String? _extractOrderId(String url) {
    try {
      final uri = Uri.parse(url);
      return uri.queryParameters['order_id'] ?? uri.queryParameters['token'];
    } catch (_) {
      return null;
    }
  }

  Future<void> _openExternal(String url) async {
    final uri = Uri.tryParse(url);
    if (uri == null) return;
    if (await canLaunchUrl(uri)) {
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final provider = context.watch<PlanProvider>();
    final requestedId = GoRouterState.of(context).uri.queryParameters['plan'];
    Plan? selected;
    for (final p in provider.plans) {
      if (p.id == requestedId) {
        selected = p;
        break;
      }
    }
    selected ??= provider.activePlan;
    return Scaffold(
      appBar: AppBar(title: const Text('Pago con PayPal')),
      body: OpalineDecoration(
        child: _buildBody(isDark, selected),
      ),
    );
  }

  Widget _buildBody(bool isDark, Plan? plan) {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.error_outline, size: 48),
              const SizedBox(height: 12),
              Text(_error!, textAlign: TextAlign.center),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: () {
                  setState(() {
                    _loading = true;
                    _error = null;
                  });
                  _init();
                },
                child: const Text('Reintentar'),
              ),
            ],
          ),
        ),
      );
    }

    if (_demoOrderId != null) {
      return Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.payments_outlined,
                  size: 64, color: AppTheme.primaryLight),
              const SizedBox(height: 16),
              Text(
                'Pago simulado (modo demo)',
                style: TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w800,
                  fontFamily: 'Poppins',
                  color: isDark ? AppTheme.textDark : AppTheme.textLight,
                ),
              ),
              const SizedBox(height: 10),
              Text(
                'PayPal y la API de pagos no estan disponibles en este entorno. '
                'Puedes simular un pago aprobado para probar la activacion de la '
                'suscripcion y el historial de facturas.\n\n'
                'Plan: ${plan?.name ?? 'Opaline Plus'}\n'
                'Importe: ${plan?.priceFormatted ?? '\$79.00 MXN'}',
                textAlign: TextAlign.center,
                style: TextStyle(
                  fontSize: 14,
                  height: 1.5,
                  fontFamily: 'Poppins',
                  color: isDark
                      ? AppTheme.textDarkSecondary
                      : AppTheme.textLightSecondary,
                ),
              ),
              const SizedBox(height: 24),
              SizedBox(
                width: double.infinity,
                height: 52,
                child: ElevatedButton.icon(
                  onPressed: () => _verifyAndFinish(_demoOrderId!),
                  icon: const Icon(Icons.check_circle_outline),
                  label: const Text('Simular pago aprobado'),
                ),
              ),
              const SizedBox(height: 10),
              TextButton(
                onPressed: () => context.pop(),
                child: const Text('Cancelar'),
              ),
            ],
          ),
        ),
      );
    }

    return Stack(
      children: [
        if (_controller != null) WebViewWidget(controller: _controller!),
        if (_loading)
          Container(
            color: (isDark ? AppTheme.backgroundDark : AppTheme.backgroundLight)
                .withOpacity(0.9),
            child: const Center(child: CircularProgressIndicator()),
          ),
      ],
    );
  }
}
