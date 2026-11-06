/// Plan de suscripcion (RF-MOV-01 / RF-MOV-03). Precios en MXN.
class Plan {
  final String id;
  final String name; // Opaline Starter / Plus / Research
  final String description;
  final double priceMXN;
  final String periodicity; // mensual / anual
  final int installments; // cuotas
  final List<String> features;
  final bool active;
  final bool starter; // plan Opaline Starter
  final int monthlyMessages; // cuota de mensajes
  final bool ragEnabled;
  final String? popularTag;

  Plan({
    required this.id,
    required this.name,
    required this.description,
    required this.priceMXN,
    required this.periodicity,
    required this.installments,
    required this.features,
    required this.active,
    this.starter = false,
    this.monthlyMessages = 0,
    this.ragEnabled = false,
    this.popularTag,
  });

  factory Plan.fromJson(Map<String, dynamic> json) => Plan(
        id: json['id'] ?? '',
        name: json['name'] ?? '',
        description: json['description'] ?? '',
        priceMXN: (json['price_mxn'] ?? 0).toDouble(),
        periodicity: json['periodicity'] ?? 'mensual',
        installments: json['installments'] ?? 1,
        features: (json['features'] as List<dynamic>?)
            ?.map((e) => e.toString())
            .toList() ?? [],
        active: json['active'] ?? true,
        starter: json['starter'] ?? false,
        monthlyMessages: json['monthly_messages'] ?? 0,
        ragEnabled: json['rag_enabled'] ?? false,
        popularTag: json['popular_tag'],
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'description': description,
        'price_mxn': priceMXN,
        'periodicity': periodicity,
        'installments': installments,
        'features': features,
        'active': active,
        'starter': starter,
        'monthly_messages': monthlyMessages,
        'rag_enabled': ragEnabled,
        'popular_tag': popularTag,
      };

  String get priceFormatted => '\$${priceMXN.toStringAsFixed(2)} MXN';
  String get installmentText {
    if (installments <= 1) return periodicity;
    return '$installments cuotas de \$${(priceMXN / installments).toStringAsFixed(2)} MXN';
  }
}
