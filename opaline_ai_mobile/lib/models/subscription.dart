/// Estado de suscripcion del usuario (RF-MOV-03).
class Subscription {
  final String id;
  final String userId;
  final String planId;
  final String planName;
  final String status; // active, pending, cancelled, past_due, refunded
  final DateTime? startsAt;
  final DateTime? endsAt;
  final DateTime? nextBilling;
  final bool autoRenew;
  final double amountMXN;

  Subscription({
    required this.id,
    required this.userId,
    required this.planId,
    required this.planName,
    required this.status,
    this.startsAt,
    this.endsAt,
    this.nextBilling,
    required this.autoRenew,
    required this.amountMXN,
  });

  factory Subscription.fromJson(Map<String, dynamic> json) => Subscription(
        id: json['id'] ?? '',
        userId: json['user_id'] ?? '',
        planId: json['plan_id'] ?? '',
        planName: json['plan_name'] ?? '',
        status: json['status'] ?? 'inactive',
        startsAt: json['starts_at'] != null ? DateTime.tryParse(json['starts_at']) : null,
        endsAt: json['ends_at'] != null ? DateTime.tryParse(json['ends_at']) : null,
        nextBilling: json['next_billing'] != null ? DateTime.tryParse(json['next_billing']) : null,
        autoRenew: json['auto_renew'] ?? true,
        amountMXN: (json['amount_mxn'] ?? 0).toDouble(),
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'user_id': userId,
        'plan_id': planId,
        'plan_name': planName,
        'status': status,
        'starts_at': startsAt?.toIso8601String(),
        'ends_at': endsAt?.toIso8601String(),
        'next_billing': nextBilling?.toIso8601String(),
        'auto_renew': autoRenew,
        'amount_mxn': amountMXN,
      };

  bool get isActive => status == 'active';
  bool get canCancelRenewal => isActive && autoRenew;
}
