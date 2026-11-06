import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../core/services/session_guard.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de desbloqueo de sesion tras 5 min de inactividad o segundo plano (RF-MOV-02).
/// Biometria o contrasena.
class BlockedScreen extends StatefulWidget {
  const BlockedScreen({super.key});
  @override
  State<BlockedScreen> createState() => _BlockedScreenState();
}

class _BlockedScreenState extends State<BlockedScreen> {
  bool _unlocking = false;

  Future<void> _unlock() async {
    setState(() => _unlocking = true);
    final ok = await SessionGuard.instance.unlock(context);
    setState(() => _unlocking = false);
    if (ok && mounted) {
      // Volver a la pantalla anterior / inicio
      if (Navigator.canPop(context)) Navigator.pop(context);
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('No se pudo autenticar. Intenta de nuevo.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final hasBio = SessionGuard.instance.biometricAvailable;
    final bioOn = SessionGuard.instance.biometricEnabled;

    return Scaffold(
      body: OpalineDecoration(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                width: 80,
                height: 80,
                decoration: BoxDecoration(
                  color: (isDark ? AppTheme.primaryDark : AppTheme.primaryLight).withOpacity(0.15),
                  shape: BoxShape.circle,
                ),
                child: Icon(Icons.lock_outline, size: 40, color: isDark ? AppTheme.primaryLight : AppTheme.primaryDark),
              ),
              const SizedBox(height: 24),
              Text('Sesion bloqueada', style: _title(isDark)),
              const SizedBox(height: 8),
              Text(
                'Autentica para reanudar tu sesion de Opaline AI',
                textAlign: TextAlign.center,
                style: _sub(isDark),
              ),
              const SizedBox(height: 32),
              SizedBox(
                width: double.infinity,
                height: 52,
                child: ElevatedButton.icon(
                  onPressed: _unlocking ? null : _unlock,
                  icon: _unlocking
                      ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Icon(Icons.fingerprint),
                  label: Text(hasBio ? 'Desbloquear con biometria' : 'Desbloquear sesion'),
                ),
              ),
              const SizedBox(height: 16),
              if (!bioOn)
                TextButton(
                  onPressed: () async {
                    await SessionGuard.instance.enableBiometric(true);
                    if (mounted) setState(() {});
                  },
                  child: const Text('Activar biometria para futuros desbloqueos'),
                ),
              const SizedBox(height: 24),
              OutlinedButton(
                onPressed: () async {
                  await context.read<AuthProvider>().logout();
                  if (mounted) context.goNamed('login');
                },
                child: const Text('Cerrar sesion'),
              ),
              const SizedBox(height: 30),
            ],
          ),
        ),
      ),
    );
  }

  TextStyle _title(bool isDark) => TextStyle(fontSize: 22, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins');
  TextStyle _sub(bool isDark) => TextStyle(fontSize: 14, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins');
}
