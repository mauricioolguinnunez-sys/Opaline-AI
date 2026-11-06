import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../models/post.dart';
import '../../../providers/wall_provider.dart';

/// Muro publico: leer, publicar, editar/eliminar propia, reportar (RF-MOV-01 / RF-WEB-04).
class WallScreen extends StatefulWidget {
  const WallScreen({super.key});
  @override
  State<WallScreen> createState() => _WallScreenState();
}

class _WallScreenState extends State<WallScreen> {
  Future<void> _refresh() async {
    await context.read<WallProvider>().loadPosts(forceNetwork: true);
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      appBar: AppBar(title: const Text('Muro'), actions: [
        IconButton(icon: const Icon(Icons.add), onPressed: () => context.pushNamed('muro_crear')),
      ]),
      body: Consumer<WallProvider>(
        builder: (context, provider, _) {
          if (provider.error != null && provider.posts.isEmpty) {
            return Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(Icons.error_outline, size: 48),
                  const SizedBox(height: 12),
                  Text(provider.error!, textAlign: TextAlign.center),
                  const SizedBox(height: 12),
                  ElevatedButton(onPressed: _refresh, child: const Text('Reintentar')),
                ],
              ),
            );
          }
          return OpalineDecoration(
            child: RefreshIndicator(
              onRefresh: _refresh,
              child: provider.posts.isEmpty
                  ? const Center(child: Text('No hay publicaciones aun.'))
                  : ListView.builder(
                      itemCount: provider.posts.length,
                      itemBuilder: (_, i) => _PostCard(post: provider.posts[i]),
                    ),
            ),
          );
        },
      ),
    );
  }
}

class _PostCard extends StatelessWidget {
  final WallPost post;
  const _PostCard({required this.post});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final provider = context.read<WallProvider>();
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ListTile(
            leading: CircleAvatar(child: Text(post.userName.isNotEmpty ? post.userName[0] : '?', style: const TextStyle(fontFamily: 'Poppins'))),
            title: Text(post.userName, style: const TextStyle(fontWeight: FontWeight.w600, fontFamily: 'Poppins')),
            subtitle: Text(post.timeAgo, style: TextStyle(fontSize: 12, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
            trailing: post.reported
                ? const Icon(Icons.flag, color: AppTheme.warningColor, size: 20)
                : IconButton(icon: const Icon(Icons.flag_outlined, size: 20), onPressed: () => _report(provider, post.id)),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text(post.content, style: TextStyle(fontSize: 15, fontFamily: 'Poppins', height: 1.4)),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              IconButton(icon: Icon(post.likedByMe ? Icons.favorite : Icons.favorite_border, color: post.likedByMe ? Colors.red : null), onPressed: () {}, iconSize: 22),
              const SizedBox(width: 8),
              Text('${post.likes}', style: TextStyle(fontSize: 13, fontFamily: 'Poppins')),
              const SizedBox(width: 16),
              const Icon(Icons.comment_outlined, size: 22),
              const SizedBox(width: 8),
              Text('${post.commentsCount}', style: TextStyle(fontSize: 13, fontFamily: 'Poppins')),
            ],
          ),
          const SizedBox(height: 8),
        ],
      ),
    );
  }

  void _report(WallProvider provider, String postId) async {
    final ok = await provider.reportPost(postId, motivo: 'contenido_inadecuado');
    if (ok) {
      // notificacion de moderacion (RF-MOV-01)
    }
  }
}
