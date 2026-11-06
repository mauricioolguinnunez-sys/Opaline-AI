/// Historial de pagos / factura (RF-MOV-03).
class Invoice {
  final String id;
  final String subscriptionId;
  final double amountMXN;
  final String status; // paid, pending, failed, refunded
  final DateTime issuedAt;
  final String? pdfUrl;

  Invoice({
    required this.id,
    required this.subscriptionId,
    required this.amountMXN,
    required this.status,
    required this.issuedAt,
    this.pdfUrl,
  });

  factory Invoice.fromJson(Map<String, dynamic> json) => Invoice(
        id: json['id'] ?? '',
        subscriptionId: json['subscription_id'] ?? '',
        amountMXN: (json['amount_mxn'] ?? 0).toDouble(),
        status: json['status'] ?? 'pending',
        issuedAt: json['issued_at'] != null ? DateTime.tryParse(json['issued_at']) ?? DateTime.now() : DateTime.now(),
        pdfUrl: json['pdf_url'],
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'subscription_id': subscriptionId,
        'amount_mxn': amountMXN,
        'status': status,
        'issued_at': issuedAt.toIso8601String(),
        'pdf_url': pdfUrl,
      };

  String get amountFormatted => '\$${amountMXN.toStringAsFixed(2)} MXN';
}
