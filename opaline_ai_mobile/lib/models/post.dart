/// Publicacion del muro (RF-MOV-01 / RF-WEB-04 reglas).
class WallPost {
  final String id;
  final String userId;
  final String userName;
  final String? userAvatar;
  final String content;
  final int likes;
  final bool likedByMe;
  final int commentsCount;
  final DateTime createdAt;
  final bool reported;

  WallPost({
    required this.id,
    required this.userId,
    required this.userName,
    this.userAvatar,
    required this.content,
    required this.likes,
    required this.likedByMe,
    required this.commentsCount,
    required this.createdAt,
    this.reported = false,
  });

  factory WallPost.fromJson(Map<String, dynamic> json) => WallPost(
        id: json['id'] ?? '',
        userId: json['user_id'] ?? '',
        userName: json['user_name'] ?? '',
        userAvatar: json['user_avatar'],
        content: json['content'] ?? '',
        likes: json['likes'] ?? 0,
        likedByMe: json['liked_by_me'] ?? false,
        commentsCount: json['comments_count'] ?? 0,
        createdAt: json['created_at'] != null
            ? DateTime.tryParse(json['created_at']) ?? DateTime.now()
            : DateTime.now(),
        reported: json['reported'] ?? false,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'user_id': userId,
        'user_name': userName,
        'user_avatar': userAvatar,
        'content': content,
        'likes': likes,
        'liked_by_me': likedByMe,
        'comments_count': commentsCount,
        'created_at': createdAt.toIso8601String(),
        'reported': reported,
      };

  WallPost copyWith({
    String? id,
    String? userId,
    String? userName,
    String? userAvatar,
    String? content,
    int? likes,
    bool? likedByMe,
    int? commentsCount,
    DateTime? createdAt,
    bool? reported,
  }) {
    return WallPost(
      id: id ?? this.id,
      userId: userId ?? this.userId,
      userName: userName ?? this.userName,
      userAvatar: userAvatar ?? this.userAvatar,
      content: content ?? this.content,
      likes: likes ?? this.likes,
      likedByMe: likedByMe ?? this.likedByMe,
      commentsCount: commentsCount ?? this.commentsCount,
      createdAt: createdAt ?? this.createdAt,
      reported: reported ?? this.reported,
    );
  }

  String get timeAgo {
    final diff = DateTime.now().difference(createdAt);
    if (diff.inDays > 0) return 'hace ${diff.inDays} d';
    if (diff.inHours > 0) return 'hace ${diff.inHours} h';
    if (diff.inMinutes > 0) return 'hace ${diff.inMinutes} min';
    return 'ahora';
  }
}
