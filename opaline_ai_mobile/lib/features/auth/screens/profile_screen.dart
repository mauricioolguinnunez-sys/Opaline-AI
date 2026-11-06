import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../core/services/session_guard.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de perfil / cuenta (RF-MOV-01 / RF-MOV-02).
/// Actualizar perfil, cambiar contrasena, historial de sesiones, cerrar sesion, eliminar cuenta.
class ProfileScreen extends StatelessWidget {
  const ProfileScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      appBar: AppBar(title: const Text('Mi cuenta')),
      body: Consumer<AuthProvider>(
        builder: (context, auth, _) {
          final user = auth.user;
          return OpalineDecoration(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  const SizedBox(height: 10),
                  CircleAvatar(
                    radius: 44,
                    backgroundColor: AppTheme.primaryLight.withOpacity(0.15),
                    child: Icon(Icons.person, size: 48, color: AppTheme.primaryLight),
                  ),
                  const SizedBox(height: 16),
                  Text(user?.name ?? 'Usuario', style: _title(isDark)),
                  const SizedBox(height: 4),
                  Text(user?.email ?? '', style: _sub(isDark)),
                  const SizedBox(height: 26),
                  _menuItem(context, Icons.person_outline, 'Editar perfil', () => _editProfile(context)),
                  _menuItem(context, Icons.lock_outline, 'Cambiar contrasena', () => context.pushNamed('cambiar_contrasena')),
                  _menuItem(context, Icons.history, 'Historial de sesiones', () => _viewSessions(context)),
                  _menuItem(context, Icons.security, 'Sesion biometrica', () => _toggleBiometric(context, auth)),
                  const Divider(),
                  _menuItem(context, Icons.subscriptions, 'Estado de suscripcion', () => context.pushNamed('suscripcion')),
                  const Divider(),
                  _menuItem(context, Icons.description, 'Aviso de privacidad', () => _showLegal(context, 'Aviso de privacidad')),
                  _menuItem(context, Icons.article, 'Terminos y condiciones', () => _showLegal(context, 'Terminos y condiciones')),
                  _menuItem(context, Icons.info, 'Version de la app', () => _showVersion(context)),
                  const Divider(),
                  _menuItem(context, Icons.logout, 'Cerrar sesion', () => _logout(context, auth), isDestructive: true),
                  _menuItem(context, Icons.delete_forever, 'Eliminar cuenta', () => _deleteAccount(context, auth), isDestructive: true),
                  const SizedBox(height: 30),
                ],
              ),
            ),
          );
        },
      ),
    );
  }

  Widget _menuItem(BuildContext context, IconData icon, String title, VoidCallback onTap, {bool isDestructive = false}) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return ListTile(
      leading: Icon(icon, color: isDestructive ? AppTheme.errorColor : (isDark ? AppTheme.primaryLight : AppTheme.primaryDark)),
      title: Text(title, style: TextStyle(color: isDestructive ? AppTheme.errorColor : null, fontFamily: 'Poppins', fontWeight: FontWeight.w500)),
      trailing: const Icon(Icons.chevron_right, size: 20),
      onTap: onTap,
    );
  }

  void _editProfile(BuildContext context) {
    showDialog(
      context: context,
      builder: (_) => const _EditProfileDialog(),
    );
  }

  void _viewSessions(BuildContext context) async {
    final sessions = await context.read<AuthProvider>().fetchSessions();
    showDialog(
      context: context,
      builder: (_) => _SessionsDialog(sessions: sessions),
    );
  }

  void _toggleBiometric(BuildContext context, AuthProvider auth) {
    final on = SessionGuard.instance.biometricEnabled;
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Sesion biometrica'),
        content: Text(on ? 'La biometria esta activada. ¿Desactivarla?' : '¿Activar biometria para desbloquear la app?'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancelar')),
          ElevatedButton(
            onPressed: () async {
              await SessionGuard.instance.enableBiometric(!on);
              Navigator.pop(context);
            },
            child: Text(on ? 'Desactivar' : 'Activar'),
          ),
        ],
      ),
    );
  }

  Future<void> _logout(BuildContext context, AuthProvider auth) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Cerrar sesion'),
        content: const Text('Se borraran los tokens y la cache local.'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
          ElevatedButton(onPressed: () => Navigator.pop(context, true), child: const Text('Cerrar sesion')),
        ],
      ),
    );
    if (confirm == true) {
      await auth.logout();
      if (context.mounted) context.goNamed('login');
    }
  }

  Future<void> _deleteAccount(BuildContext context, AuthProvider auth) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Eliminar cuenta'),
        content: const Text('Esto borrara tus datos locales y solicitara al servidor la eliminacion de tus datos personales (max 30 dias).'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: AppTheme.errorColor),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Eliminar cuenta'),
          ),
        ],
      ),
    );
    if (confirm == true && context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Solicitud de eliminacion enviada.')),
      );
    }
  }

  void _showLegal(BuildContext context, String title) {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: Text(title),
        content: const Text('Contenido legal de Opaline AI. Disponible completo en www.opaline-ai.com/legal'),
        actions: [TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cerrar'))],
      ),
    );
  }

  void _showVersion(BuildContext context) {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Version de la app'),
        content: const Text('Opaline AI movil v1.0.0+1'),
        actions: [TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cerrar'))],
      ),
    );
  }

  TextStyle _title(bool isDark) => TextStyle(fontSize: 20, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins');
  TextStyle _sub(bool isDark) => TextStyle(fontSize: 14, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins');
}

class _EditProfileDialog extends StatefulWidget {
  const _EditProfileDialog();
  @override
  State<_EditProfileDialog> createState() => _EditProfileDialogState();
}

class _EditProfileDialogState extends State<_EditProfileDialog> {
  final _name = TextEditingController();
  final _email = TextEditingController();
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return AlertDialog(
      title: const Text('Editar perfil'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(controller: _name, decoration: const InputDecoration(labelText: 'Nombre completo')),
          const SizedBox(height: 12),
          TextField(controller: _email, decoration: const InputDecoration(labelText: 'Correo electronico')),
        ],
      ),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancelar')),
        ElevatedButton(onPressed: () => Navigator.pop(context), child: const Text('Guardar')),
      ],
    );
  }
}

class _SessionsDialog extends StatelessWidget {
  final List<dynamic> sessions;
  const _SessionsDialog({required this.sessions});
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return AlertDialog(
      title: const Text('Historial de sesiones'),
      content: SizedBox(
        width: 300,
        height: 300,
        child: ListView.builder(
          itemCount: sessions.length,
          itemBuilder: (_, i) => ListTile(
            title: Text(sessions[i]['device'] ?? 'Dispositivo', style: const TextStyle(fontFamily: 'Poppins')),
            subtitle: Text(sessions[i]['ip'] ?? '', style: const TextStyle(fontSize: 12, fontFamily: 'Poppins')),
            trailing: IconButton(
              icon: const Icon(Icons.block, size: 20, color: AppTheme.errorColor),
              onPressed: () {},
            ),
          ),
        ),
      ),
      actions: [TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cerrar'))],
    );
  }
}
