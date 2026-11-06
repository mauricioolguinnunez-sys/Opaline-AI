import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/plan_provider.dart';
import '../../../models/plan.dart';

/// Catalogo de planes con precio MXN, periodicidad, cuotas y caracteristicas (RF-MOV-01 / RF-MOV-03).
class PlansScreen extends StatelessWidget {
  const PlansScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      appBar: AppBar(title: const Text('Planes'), actions: [
        IconButton(icon: const Icon(Icons.refresh), onPressed: () => context.read<PlanProvider>().loadPlans(forceNetwork: true)),
      ]),
      body: Consumer<PlanProvider>(
        builder: (context, provider, _) {
          if (provider.error != null && provider.plans.isEmpty) {
            return _ErrorState(message: provider.error!, onRetry: () => provider.loadPlans(forceNetwork: true));
          }
          return OpalineDecoration(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Elige tu plan', style: _title(isDark)),
                      const SizedBox(height: 6),
                      Text('Precios en MXN. Una sola suscripcion para web y movil.', style: _sub(isDark)),
                    ],
                  ),
                ),
                Expanded(
                  child: provider.plans.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : ListView.builder(
                          padding: const EdgeInsets.symmetric(horizontal: 16),
                          itemCount: provider.plans.length,
                          itemBuilder: (_, i) => _PlanCard(plan: provider.plans[i]),
                        ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  TextStyle _title(bool isDark) => TextStyle(fontSize: 22, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins');
  TextStyle _sub(bool isDark) => TextStyle(fontSize: 14, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins');
}

class _PlanCard extends StatelessWidget {
  final Plan plan;
  const _PlanCard({required this.plan});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Consumer<PlanProvider>(
      builder: (context, provider, _) {
        final alreadyActive = provider.subscription?.planId == plan.id && provider.subscription?.isActive == true;
        return Card(
          margin: const EdgeInsets.only(bottom: 14),
          child: InkWell(
            borderRadius: BorderRadius.circular(20),
            onTap: alreadyActive
                ? null
                : () => context.pushNamed('plan_detalle', pathParameters: {'id': plan.id}),
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Flexible(
                        child: Text(
                          plan.name,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(fontSize: 18, fontWeight: FontWeight.w700, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins'),
                        ),
                      ),
                      if (plan.popularTag != null)
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                          decoration: BoxDecoration(color: AppTheme.primaryLight.withOpacity(0.15), borderRadius: BorderRadius.circular(8)),
                          child: Text(plan.popularTag!, style: TextStyle(fontSize: 11, color: AppTheme.primaryDark, fontWeight: FontWeight.w700, fontFamily: 'Poppins')),
                        ),
                    ],
                  ),
                  if (plan.starter) ...[
                    const SizedBox(height: 6),
                    Text('Plan de entrada con limites moderados para estudiantes.', style: TextStyle(fontSize: 12, color: AppTheme.warningColor, fontFamily: 'Poppins')),
                  ],
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      Text(plan.priceFormatted, style: TextStyle(fontSize: 24, fontWeight: FontWeight.w800, color: AppTheme.primaryLight, fontFamily: 'Poppins')),
                      const SizedBox(width: 10),
                      Text(plan.installmentText, style: TextStyle(fontSize: 13, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text('${plan.periodicity} • ${plan.monthlyMessages} mensajes/mes', style: TextStyle(fontSize: 13, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
                  const SizedBox(height: 12),
                  ...plan.features.map((f) => Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Row(
                          children: [
                            Icon(Icons.check_circle_outline, size: 16, color: AppTheme.primaryLight),
                            const SizedBox(width: 8),
                            Expanded(child: Text(f, style: TextStyle(fontSize: 13, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins'))),
                          ],
                        ),
                      )),
                  const SizedBox(height: 16),
                  SizedBox(
                    width: double.infinity,
                    height: 48,
                    child: alreadyActive
                        ? OutlinedButton(
                            onPressed: null,
                            style: OutlinedButton.styleFrom(
                              side: const BorderSide(color: AppTheme.successColor),
                              foregroundColor: AppTheme.successColor,
                              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
                            ),
                            child: const Text('Plan activo', style: TextStyle(fontWeight: FontWeight.w600, fontFamily: 'Poppins')),
                          )
                        : ElevatedButton(
                            onPressed: () => context.pushNamed('plan_detalle', pathParameters: {'id': plan.id}),
                            child: const Text('Seleccionar'),
                          ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

class _ErrorState extends StatelessWidget {
  final String message;
  final VoidCallback onRetry;
  const _ErrorState({required this.message, required this.onRetry});
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.error_outline, size: 48, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary),
          const SizedBox(height: 12),
          Text(message, textAlign: TextAlign.center, style: TextStyle(color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
          const SizedBox(height: 16),
          ElevatedButton(onPressed: onRetry, child: const Text('Reintentar')),
        ],
      ),
    );
  }
}
