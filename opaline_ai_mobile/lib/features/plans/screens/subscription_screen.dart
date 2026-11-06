import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/plan_provider.dart';
import '../../../models/invoice.dart';

/// Estado de suscripcion, proximo cobro, historial de pagos, facturas,
/// cancelar renovacion, restaurar compra, descargar factura (RF-MOV-03).
class SubscriptionScreen extends StatelessWidget {
  const SubscriptionScreen({super.key});

  String _fmt(DateTime d) =>
      '${d.day.toString().padLeft(2, '0')}/${d.month.toString().padLeft(2, '0')}/${d.year}';

  Future<void> _downloadPdf(BuildContext context, Invoice inv) async {
    final bytes = await context.read<PlanProvider>().downloadInvoice(inv.id);
    if (!context.mounted) return;
    if (bytes == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Factura no disponible para descarga.')),
      );
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Factura descargada.')),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Mi suscripcion')),
      body: Consumer<PlanProvider>(
        builder: (context, provider, _) {
          final sub = provider.subscription;
          final invoices = provider.invoices;
          return OpalineDecoration(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(22),
                    decoration: BoxDecoration(
                      color: (sub?.isActive == true
                              ? AppTheme.successColor
                              : AppTheme.warningColor)
                          .withOpacity(0.12),
                      borderRadius: BorderRadius.circular(20),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            Icon(
                              sub?.isActive == true
                                  ? Icons.check_circle
                                  : Icons.error_outline,
                              color: sub?.isActive == true
                                  ? AppTheme.successColor
                                  : AppTheme.warningColor,
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Text(
                                sub?.planName ?? 'Sin plan activo',
                                style: TextStyle(
                                    fontSize: 18,
                                    fontWeight: FontWeight.w700,
                                    color: Theme.of(context).brightness ==
                                            Brightness.dark
                                        ? AppTheme.textDark
                                        : AppTheme.textLight,
                                    fontFamily: 'Poppins'),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 8),
                        Text(
                          sub == null
                              ? 'No tienes una suscripcion activa.'
                              : 'Estado: ${sub.status}',
                          style: TextStyle(
                              fontSize: 14,
                              color: Theme.of(context).brightness ==
                                      Brightness.dark
                                  ? AppTheme.textDarkSecondary
                                  : AppTheme.textLightSecondary,
                              fontFamily: 'Poppins'),
                        ),
                        if (sub != null && sub.nextBilling != null) ...[
                          const SizedBox(height: 8),
                          Text('Proximo cobro: ${_fmt(sub.nextBilling!)}',
                              style: const TextStyle(
                                  fontSize: 14, fontFamily: 'Poppins')),
                        ],
                        if (sub != null && sub.endsAt != null)
                          Text('Acceso hasta: ${_fmt(sub.endsAt!)}',
                              style: const TextStyle(
                                  fontSize: 14, fontFamily: 'Poppins')),
                      ],
                    ),
                  ),
                  const SizedBox(height: 24),
                  if (sub != null && sub.isActive && sub.autoRenew)
                    SizedBox(
                      width: double.infinity,
                      height: 48,
                      child: OutlinedButton(
                        style: OutlinedButton.styleFrom(
                            foregroundColor: AppTheme.errorColor,
                            side: const BorderSide(color: AppTheme.errorColor)),
                        onPressed: () async {
                          final ok = await provider.cancelRenewal();
                          if (ok && context.mounted) {
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(
                                  content: Text(
                                      'Renovacion cancelada. Mantienes acceso hasta la fecha de fin.')),
                            );
                          }
                        },
                        child: const Text('Cancelar renovacion'),
                      ),
                    )
                  else if (sub != null && sub.isActive && !sub.autoRenew)
                    const Text('La renovacion esta cancelada.',
                        style: TextStyle(
                            color: AppTheme.errorColor, fontFamily: 'Poppins')),
                  const SizedBox(height: 16),
                  SizedBox(
                    width: double.infinity,
                    height: 48,
                    child: OutlinedButton(
                      onPressed: () async {
                        final ok = await provider.restorePurchase();
                        if (ok && context.mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(content: Text('Compra restaurada.')),
                          );
                        }
                      },
                      child: const Text('Restaurar compra'),
                    ),
                  ),
                  const SizedBox(height: 28),
                  Text('Historial de pagos y facturas',
                      style: TextStyle(
                          fontSize: 18,
                          fontWeight: FontWeight.w700,
                          color: Theme.of(context).brightness ==
                                  Brightness.dark
                              ? AppTheme.textDark
                              : AppTheme.textLight,
                          fontFamily: 'Poppins')),
                  const SizedBox(height: 12),
                  if (invoices.isEmpty)
                    const Text('No hay facturas disponibles.',
                        style: TextStyle(fontSize: 14, fontFamily: 'Poppins'))
                  else
                    ...invoices.map((inv) => _InvoiceCard(
                        invoice: inv, onDownloadPdf: () => _downloadPdf(context, inv))),
                  const SizedBox(height: 30),
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}

class _InvoiceCard extends StatelessWidget {
  final Invoice invoice;
  final VoidCallback onDownloadPdf;
  const _InvoiceCard({required this.invoice, required this.onDownloadPdf});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: ListTile(
        title: Text('Factura #${invoice.id}',
            style: const TextStyle(fontWeight: FontWeight.w600, fontFamily: 'Poppins')),
        subtitle: Text('${invoice.amountFormatted}  •  ${invoice.status}',
            style: const TextStyle(fontSize: 13, fontFamily: 'Poppins')),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (invoice.pdfUrl != null || invoice.status == 'paid')
              IconButton(
                  icon: const Icon(Icons.picture_as_pdf), onPressed: onDownloadPdf),
            if (invoice.status == 'paid')
              IconButton(
                icon: const Icon(Icons.send, color: AppTheme.warningColor),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (_) => AlertDialog(
                      title: const Text('Solicitar reembolso'),
                      content: const Text(
                          '¿Deseas solicitar el reembolso de esta factura?'),
                      actions: [
                        TextButton(
                            onPressed: () => Navigator.pop(context),
                            child: const Text('Cancelar')),
                        ElevatedButton(
                          onPressed: () async {
                            final ok = await context
                                .read<PlanProvider>()
                                .requestRefund(invoice.id);
                            if (ok && context.mounted) {
                              Navigator.pop(context);
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(
                                    content: Text(
                                        'Solicitud de reembolso enviada.')),
                              );
                            }
                          },
                          child: const Text('Solicitar'),
                        ),
                      ],
                    ),
                  );
                },
              ),
          ],
        ),
      ),
    );
  }
}
