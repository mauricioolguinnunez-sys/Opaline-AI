/// Modelo de usuario compartido con la web (RF-MOV-01).
class AppUser {
  final String id;
  final String name;
  final String email;
  final String? avatarUrl;
  final String planId;
  final String planName;
  final bool subscriptionActive;
  final DateTime? nextBilling;
  final int dailyQuotaRemaining;
  final int dailyQuotaLimit;
  final bool biometricEnabled;
  final DateTime createdAt;

  AppUser({
    required this.id,
    required this.name,
    required this.email,
    this.avatarUrl,
    required this.planId,
    required this.planName,
    required this.subscriptionActive,
    this.nextBilling,
    required this.dailyQuotaRemaining,
    required this.dailyQuotaLimit,
    this.biometricEnabled = false,
    required this.createdAt,
  });

  factory AppUser.fromJson(Map<String, dynamic> json) => AppUser(
        id: json['id'] ?? '',
        name: json['name'] ?? '',
        email: json['email'] ?? '',
        avatarUrl: json['avatar_url'],
        planId: json['plan_id'] ?? '',
        planName: json['plan_name'] ?? 'Starter',
        subscriptionActive: json['subscription_active'] ?? false,
        nextBilling: json['next_billing'] != null
            ? DateTime.tryParse(json['next_billing'])
            : null,
        dailyQuotaRemaining: json['daily_quota_remaining'] ?? 0,
        dailyQuotaLimit: json['daily_quota_limit'] ?? 0,
        biometricEnabled: json['biometric_enabled'] ?? false,
        createdAt: json['created_at'] != null
            ? DateTime.tryParse(json['created_at']) ?? DateTime.now()
            : DateTime.now(),
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'email': email,
        'avatar_url': avatarUrl,
        'plan_id': planId,
        'plan_name': planName,
        'subscription_active': subscriptionActive,
        'next_billing': nextBilling?.toIso8601String(),
        'daily_quota_remaining': dailyQuotaRemaining,
        'daily_quota_limit': dailyQuotaLimit,
        'biometric_enabled': biometricEnabled,
        'created_at': createdAt.toIso8601String(),
      };
}

class SessionInfo {
  final String id;
  final String device;
  final String ip;
  final DateTime createdAt;
  final bool isCurrent;

  SessionInfo({
    required this.id,
    required this.device,
    required this.ip,
    required this.createdAt,
    this.isCurrent = false,
  });

  factory SessionInfo.fromJson(Map<String, dynamic> json) => SessionInfo(
        id: json['id'] ?? '',
        device: json['device'] ?? '',
        ip: json['ip'] ?? '',
        createdAt: json['created_at'] != null
            ? DateTime.tryParse(json['created_at']) ?? DateTime.now()
            : DateTime.now(),
        isCurrent: json['is_current'] ?? false,
      );
}
