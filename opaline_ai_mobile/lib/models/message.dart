/// Mensaje del chat (RF-MOV-04).
class ChatMessage {
  final String id;
  final String conversationId;
  final String role; // user / assistant
  final String content;
  final bool isStreaming;
  final List<Map<String, dynamic>>? citations; // citas RAG
  final DateTime createdAt;

  ChatMessage({
    required this.id,
    required this.conversationId,
    required this.role,
    required this.content,
    this.isStreaming = false,
    this.citations,
    required this.createdAt,
  });

  factory ChatMessage.fromJson(Map<String, dynamic> json) => ChatMessage(
        id: json['id'] ?? '',
        conversationId: json['conversation_id'] ?? '',
        role: json['role'] ?? 'user',
        content: json['content'] ?? '',
        isStreaming: json['is_streaming'] ?? false,
        citations: (json['citations'] as List<dynamic>?)
            ?.map((e) => e as Map<String, dynamic>)
            .toList(),
        createdAt: json['created_at'] != null
            ? DateTime.tryParse(json['created_at']) ?? DateTime.now()
            : DateTime.now(),
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'conversation_id': conversationId,
        'role': role,
        'content': content,
        'is_streaming': isStreaming,
        'citations': citations,
        'created_at': createdAt.toIso8601String(),
      };

  bool get isUser => role == 'user';
}
