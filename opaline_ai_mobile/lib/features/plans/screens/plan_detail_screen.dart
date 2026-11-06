import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../models/plan.dart';
import '../../../providers/plan_provider.dart';

/// Detalle de plan y inicio de compra PayPal (RF-MOV-03).
class PlanDetailScreen extends StatefulWidget {
  final String planId;
  const PlanDetailScreen({super.key, required this.planId});
  @override
  State<PlanDetailScreen> createState() => _PlanDetailScreenState();
}

class _PlanDetailScreenState extends State<PlanDetailScreen> {
  Plan? _plan;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final provider = context.read<PlanProvider>();
    await provider.loadPlans();
    if (!mounted) return;
    setState(() {
      _plan = provider.plans.isEmpty
          ? null
          : provider.plans.firstWhere(
              (p) => p.id == widget.planId,
              orElse: () => provider.plans.first,
            );
    });
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    if (_plan == null) {
      return Scaffold(appBar: AppBar(title: const Text('Plan')), body: const Center(child: CircularProgressIndicator()));
    }
    final plan = _plan!;
    final provider = context.watch<PlanProvider>();
    final alreadyActive = provider.subscription?.planId == plan.id && provider.subscription?.isActive == true;

    return Scaffold(
      appBar: AppBar(title: Text(plan.name)),
      body: OpalineDecoration(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                width: double.infinity,
                padding: const EdgeInsets.all(24),
                decoration: BoxDecoration(
                  color: AppTheme.primaryLight.withOpacity(0.12),
                  borderRadius: BorderRadius.circular(24),
                ),
                child: Column(
                  children: [
                    Text(plan.name, style: TextStyle(fontSize: 22, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins')),
                    const SizedBox(height: 8),
                    Text(plan.description, textAlign: TextAlign.center, style: TextStyle(fontSize: 15, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins', height: 1.5)),
                    const SizedBox(height: 16),
                    Text(plan.priceFormatted, style: TextStyle(fontSize: 32, fontWeight: FontWeight.w800, color: AppTheme.primaryLight, fontFamily: 'Poppins')),
                    const SizedBox(height: 4),
                    Text(plan.installmentText, style: TextStyle(fontSize: 15, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
                  ],
                ),
              ),
              const SizedBox(height: 24),
              Text('Caracteristicas', style: TextStyle(fontSize: 18, fontWeight: FontWeight.w700, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins')),
              const SizedBox(height: 12),
              ...plan.features.map((f) => Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: Row(children: [
                      Icon(Icons.check_circle, color: AppTheme.primaryLight, size: 20),
                      const SizedBox(width: 12),
                      Expanded(child: Text(f, style: TextStyle(fontSize: 15, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins'))),
                    ]),
                  )),
              const SizedBox(height: 24),
              if (alreadyActive)
                Container(
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(color: AppTheme.successColor.withOpacity(0.12), borderRadius: BorderRadius.circular(16)),
                  child: Row(children: [
                    const Icon(Icons.check_circle, color: AppTheme.successColor),
                    const SizedBox(width: 12),
                    Expanded(child: Text('Ya tienes este plan activo. Puedes cambiar de plan desde tu suscripcion.', style: TextStyle(fontSize: 14, color: AppTheme.successColor, fontFamily: 'Poppins'))),
                  ]),
                )
              else ...[
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: ElevatedButton(
                    onPressed: () => context.pushNamed(
                      'checkout',
                      queryParameters: {'plan': plan.id},
                    ),
                    child: const Text('Pagar con PayPal'),
                  ),
                ),
                const SizedBox(height: 12),
                TextButton(
                  onPressed: () => context.pop(),
                  child: const Text('Cancelar y ver otros planes'),
                ),
              ],
              const SizedBox(height: 30),
            ],
          ),
        ),
      ),
    );
  }
}
