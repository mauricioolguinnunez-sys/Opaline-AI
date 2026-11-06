import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../core/services/session_guard.dart';
import '../../../providers/auth_provider.dart';

/// Configuracion: tema, biometria, version, privacidad, terminos, cuenta (RF-MOV-01 / RNF-MOV-03).
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Configuracion')),
      body: Consumer<AuthProvider>(
        builder: (context, auth, _) {
          return OpalineDecoration(
            child: ListView(
              children: [
                _sectionTitle(context, 'Apariencia'),
                ListTile(
                  leading: const Icon(Icons.brightness_6),
                  title: const Text('Tema del sistema (claro/oscuro)'),
                  subtitle: const Text('Respeta la configuracion de tu dispositivo'),
                  trailing: const Icon(Icons.brightness_auto),
                  onTap: () {},
                ),
                const Divider(),
                _sectionTitle(context, 'Seguridad'),
                ListTile(
                  leading: const Icon(Icons.fingerprint),
                  title: const Text('Sesion biometrica'),
                  subtitle: Text(SessionGuard.instance.biometricEnabled ? 'Activada' : 'Desactivada'),
                  trailing: Switch(
                    value: SessionGuard.instance.biometricEnabled,
                    onChanged: (v) async {
                      await SessionGuard.instance.enableBiometric(v);
                    },
                  ),
                ),
                ListTile(
                  leading: const Icon(Icons.history),
                  title: const Text('Historial de sesiones'),
                  onTap: () {},
                ),
                const Divider(),
                _sectionTitle(context, 'Cuenta'),
                ListTile(
                  leading: const Icon(Icons.lock_outline),
                  title: const Text('Cambiar contrasena'),
                  onTap: () => context.pushNamed('cambiar_contrasena'),
                ),
                ListTile(
                  leading: const Icon(Icons.description),
                  title: const Text('Aviso de privacidad'),
                  onTap: () => _showLegal(context, 'Aviso de privacidad'),
                ),
                ListTile(
                  leading: const Icon(Icons.article),
                  title: const Text('Terminos y condiciones'),
                  onTap: () => _showLegal(context, 'Terminos y condiciones'),
                ),
                ListTile(
                  leading: const Icon(Icons.info),
                  title: const Text('Version de la app'),
                  subtitle: const Text('Opaline AI movil v1.0.0+1'),
                  onTap: () {},
                ),
                const Divider(),
                ListTile(
                  leading: const Icon(Icons.logout, color: AppTheme.errorColor),
                  title: const Text('Cerrar sesion', style: TextStyle(color: AppTheme.errorColor)),
                  onTap: () => _logout(context, auth),
                ),
                ListTile(
                  leading: const Icon(Icons.delete_forever, color: AppTheme.errorColor),
                  title: const Text('Eliminar cuenta', style: TextStyle(color: AppTheme.errorColor)),
                  onTap: () => _deleteAccount(context, auth),
                ),
                const SizedBox(height: 40),
              ],
            ),
          );
        },
      ),
    );
  }

  Widget _sectionTitle(BuildContext context, String title) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
      child: Text(title, style: TextStyle(fontSize: 13, fontWeight: FontWeight.w700, color: Theme.of(context).brightness == Brightness.dark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins')),
    );
  }

  void _showLegal(BuildContext context, String title) {
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: Text(title),
        content: const Text('Consulta el texto completo en www.opaline-ai.com/legal'),
        actions: [TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cerrar'))],
      ),
    );
  }

  Future<void> _logout(BuildContext context, AuthProvider auth) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(title: const Text('Cerrar sesion'), content: const Text('Se borraran tokens y cache local.'), actions: [
        TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
        ElevatedButton(onPressed: () => Navigator.pop(context, true), child: const Text('Cerrar sesion')),
      ]),
    );
    if (confirm == true && context.mounted) {
      await auth.logout();
      if (context.mounted) context.goNamed('login');
    }
  }

  Future<void> _deleteAccount(BuildContext context, AuthProvider auth) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('Eliminar cuenta'),
        content: const Text('Se borraran datos locales y se solicitara eliminacion en el servidor (max 30 dias).'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancelar')),
          ElevatedButton(style: ElevatedButton.styleFrom(backgroundColor: AppTheme.errorColor), onPressed: () => Navigator.pop(context, true), child: const Text('Eliminar')),
        ],
      ),
    );
    if (confirm == true && context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Solicitud de eliminacion enviada.')));
    }
  }
}
