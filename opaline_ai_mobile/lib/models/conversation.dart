import 'message.dart';

/// Conversacion del historial (RF-MOV-04).
class Conversation {
  final String id;
  String title;
  final String? lastMessage;
  final DateTime updatedAt;
  final List<ChatMessage> messages;

  Conversation({
    required this.id,
    required this.title,
    this.lastMessage,
    required this.updatedAt,
    List<ChatMessage>? messages,
  }) : messages = messages ?? [];

  factory Conversation.fromJson(Map<String, dynamic> json) => Conversation(
        id: json['id'] ?? '',
        title: json['title'] ?? 'Conversacion',
        lastMessage: json['last_message'],
        updatedAt: json['updated_at'] != null
            ? DateTime.tryParse(json['updated_at']) ?? DateTime.now()
            : DateTime.now(),
        messages: (json['messages'] as List<dynamic>?)
            ?.map((e) => ChatMessage.fromJson(e as Map<String, dynamic>))
            .toList()
            .cast<ChatMessage>() ?? [],
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'last_message': lastMessage,
        'updated_at': updatedAt.toIso8601String(),
        'messages': messages.map((m) => m.toJson()).toList(),
      };
}
