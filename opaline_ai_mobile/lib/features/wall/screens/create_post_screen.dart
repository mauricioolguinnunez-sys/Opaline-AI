import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';
import '../../../providers/wall_provider.dart';

/// Crear publicacion en el muro (RF-MOV-01 / RF-WEB-04).
class CreatePostScreen extends StatefulWidget {
  const CreatePostScreen({super.key});
  @override
  State<CreatePostScreen> createState() => _CreatePostScreenState();
}

class _CreatePostScreenState extends State<CreatePostScreen> {
  final _ctrl = TextEditingController();
  bool _posting = false;

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  Future<void> _publish() async {
    if (_posting) return;
    final wall = context.read<WallProvider>();
    final author = context.read<AuthProvider>().user?.name ?? 'Usuario Opaline';
    setState(() => _posting = true);
    final ok = await wall.createPost(_ctrl.text.trim(), authorName: author);
    if (!mounted) return;
    setState(() => _posting = false);
    if (ok) {
      context.pop(true);
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(wall.error ?? 'No se pudo publicar')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      appBar: AppBar(title: const Text('Publicar experiencia')),
      body: OpalineDecoration(
        child: Column(
          children: [
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: TextField(
                  controller: _ctrl,
                  maxLines: null,
                  expands: true,
                  textInputAction: TextInputAction.done,
                  onChanged: (_) => setState(() {}),
                  onSubmitted: (_) => _publish(),
                  decoration: InputDecoration(
                    hintText: 'Comparte tu experiencia con Opaline AI...',
                    hintStyle: TextStyle(color: isDark ? const Color(0xFF8B7DCB) : const Color(0xFF9A8CC7), fontFamily: 'Poppins'),
                    border: OutlineInputBorder(borderRadius: BorderRadius.circular(16), borderSide: BorderSide.none),
                    filled: true,
                    fillColor: isDark ? const Color(0xFF1A123E) : const Color(0xFFF1EAFC),
                    contentPadding: const EdgeInsets.all(16),
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.all(16),
              child: SizedBox(
                width: double.infinity,
                height: 52,
                child: ElevatedButton(
                  onPressed: _posting || _ctrl.text.trim().isEmpty ? null : _publish,
                  child: _posting
                      ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Publicar'),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
