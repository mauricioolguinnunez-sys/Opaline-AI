import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../models/post.dart';
import '../../../providers/wall_provider.dart';

/// Muro publico: leer, publicar, editar/eliminar propia, reportar.
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
      appBar: AppBar(
        title: const Text('Muro', style: TextStyle(fontWeight: FontWeight.w700)),
        actions: [
          IconButton(
            tooltip: 'Nueva publicación',
            icon: const Icon(Icons.add_circle_outline),
            onPressed: () => context.pushNamed('muro_crear'),
          ),
        ],
      ),
      body: OpalineDecoration(
        child: Consumer<WallProvider>(
          builder: (context, provider, _) {
            if (provider.error != null && provider.posts.isEmpty) {
              return Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Icon(Icons.cloud_off, size: 48, color: AppTheme.primaryLight),
                    const SizedBox(height: 12),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 28),
                      child: Text(provider.error!, textAlign: TextAlign.center),
                    ),
                    const SizedBox(height: 12),
                    ElevatedButton(onPressed: _refresh, child: const Text('Reintentar')),
                  ],
                ),
              );
            }
            return RefreshIndicator(
              onRefresh: _refresh,
              child: ListView(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 32),
                children: [
                  _WallHeader(count: provider.posts.length),
                  const SizedBox(height: 16),
                  if (provider.posts.isEmpty)
                    _EmptyWall(onPublish: () => context.pushNamed('muro_crear'))
                  else
                    ...provider.posts.map((post) => _PostCard(post: post)),
                  if (provider.isDemoData)
                    Padding(
                      padding: const EdgeInsets.only(top: 12),
                      child: Text(
                        'Mostrando publicaciones de demostración.',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          fontSize: 12,
                          color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary,
                        ),
                      ),
                    ),
                ],
              ),
            );
          },
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => context.pushNamed('muro_crear'),
        backgroundColor: AppTheme.primaryLight,
        foregroundColor: Colors.white,
        icon: const Icon(Icons.edit_outlined),
        label: const Text('Publicar'),
      ),
    );
  }
}

class _WallHeader extends StatelessWidget {
  final int count;
  const _WallHeader({required this.count});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(22),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(24),
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [Color(0xFF2A1668), Color(0xFF4E2EA2), Color(0xFF9369E5)],
        ),
        boxShadow: [
          BoxShadow(
            color: AppTheme.primaryDark.withOpacity(.28),
            blurRadius: 28,
            offset: const Offset(0, 14),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
              color: Colors.white.withOpacity(.16),
              borderRadius: BorderRadius.circular(999),
            ),
            child: const Text('Comunidad Opaline', style: TextStyle(color: Colors.white, fontSize: 11, fontWeight: FontWeight.w600)),
          ),
          const SizedBox(height: 14),
          const Text(
            'Muro de experiencias',
            style: TextStyle(color: Colors.white, fontSize: 26, fontWeight: FontWeight.w800, letterSpacing: -.5),
          ),
          const SizedBox(height: 6),
          Text(
            'Descubre cómo otras personas usan Opaline AI y comparte tu historia. $count publicaciones.',
            style: const TextStyle(color: Color(0xFFE2D8FF), fontSize: 13, height: 1.5),
          ),
        ],
      ),
    );
  }
}

class _EmptyWall extends StatelessWidget {
  final VoidCallback onPublish;
  const _EmptyWall({required this.onPublish});
  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(28),
      decoration: BoxDecoration(
        color: Colors.white.withOpacity(.75),
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: const Color(0xFFE3DAF6)),
      ),
      child: Column(
        children: [
          const Icon(Icons.forum_outlined, size: 46, color: AppTheme.primaryLight),
          const SizedBox(height: 12),
          const Text('Aún no hay publicaciones', style: TextStyle(fontSize: 18, fontWeight: FontWeight.w700, fontFamily: 'Poppins')),
          const SizedBox(height: 6),
          Text('Sé la primera persona en compartir su experiencia con Opaline AI.', textAlign: TextAlign.center, style: TextStyle(color: AppTheme.textLightSecondary, fontSize: 13)),
          const SizedBox(height: 16),
          ElevatedButton.icon(onPressed: onPublish, icon: const Icon(Icons.edit_outlined), label: const Text('Crear publicación')),
        ],
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
    return Container(
      margin: const EdgeInsets.only(bottom: 14),
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: isDark ? AppTheme.surfaceDark : Colors.white.withOpacity(.92),
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: isDark ? const Color(0xFF2C205A) : const Color(0xFFE3DAF6)),
        boxShadow: [
          BoxShadow(
            color: AppTheme.primaryDark.withOpacity(isDark ? .18 : .06),
            blurRadius: 18,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 42,
                height: 42,
                decoration: const BoxDecoration(
                  shape: BoxShape.circle,
                  gradient: LinearGradient(colors: [AppTheme.primaryLight, AppTheme.primaryDark]),
                ),
                child: Center(
                  child: Text(
                    post.userName.isNotEmpty ? post.userName[0].toUpperCase() : '?',
                    style: const TextStyle(color: Colors.white, fontWeight: FontWeight.w700, fontFamily: 'Poppins'),
                  ),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(post.userName, style: TextStyle(fontWeight: FontWeight.w700, fontSize: 14, color: isDark ? AppTheme.textDark : AppTheme.textLight)),
                    Text(post.timeAgo, style: TextStyle(fontSize: 11, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary)),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Reportar',
                icon: Icon(post.reported ? Icons.flag : Icons.flag_outlined, size: 19, color: post.reported ? AppTheme.warningColor : AppTheme.primaryLight),
                onPressed: post.reported ? null : () => _report(context, provider, post.id),
              ),
            ],
          ),
          if (post.rating > 0) ...[
            const SizedBox(height: 8),
            Text(
              List.filled(post.rating, '★').join() + List.filled(5 - post.rating, '☆').join(),
              style: const TextStyle(color: Color(0xFFF4B740), fontSize: 15, letterSpacing: 1),
            ),
          ],
          const SizedBox(height: 12),
          Text(
            post.content,
            style: TextStyle(
              fontSize: 14.5,
              height: 1.6,
              color: isDark ? AppTheme.textDark : AppTheme.textLight,
            ),
          ),
        ],
      ),
    );
  }

  void _report(BuildContext context, WallProvider provider, String postId) async {
    final ok = await provider.reportPost(postId, motivo: 'contenido_inadecuado');
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(ok ? 'Reporte enviado. Gracias por cuidar la comunidad.' : 'No se pudo enviar el reporte.')),
    );
  }
}
