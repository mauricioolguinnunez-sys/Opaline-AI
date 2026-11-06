import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de inicio de sesion (RF-MOV-02).
/// - Correo + contrasena, validacion contra API, mensaje generico ante error.
/// - Tokens en almacenamiento seguro.
/// - Biometria para reanudar.
/// - Impide capturas/grabacion en login.
class LoginScreen extends StatefulWidget {
  const LoginScreen({super.key});
  @override
  State<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends State<LoginScreen> {
  final _emailCtrl = TextEditingController();
  final _passCtrl = TextEditingController();
  bool _obscure = true;
  bool _loading = false;

  @override
  void initState() {
    super.initState();
  }

  Future<void> _login() async {
    FocusScope.of(context).unfocus();
    final auth = context.read<AuthProvider>();
    setState(() => _loading = true);
    final ok = await auth.login(
      email: _emailCtrl.text.trim(),
      password: _passCtrl.text,
    );
    if (!mounted) return;
    setState(() => _loading = false);
    if (ok) {
      // La sesion ya quedo marcada como autenticada en AuthProvider/SessionGuard.
      context.goNamed('inicio');
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(auth.error ?? 'Error de inicio de sesion')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;

    return Scaffold(
      body: OpalineDecoration(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Container(
                  width: 88,
                  height: 88,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    gradient: LinearGradient(
                      colors: [AppTheme.primaryLight, AppTheme.primaryDark],
                    ),
                    boxShadow: [
                      BoxShadow(
                        color: AppTheme.primaryLight.withOpacity(0.45),
                        blurRadius: 28,
                        spreadRadius: 3,
                      ),
                    ],
                  ),
                  child: const Center(
                    child: Text('O', style: TextStyle(fontSize: 44, fontWeight: FontWeight.w800, color: Colors.white, letterSpacing: -6)),
                  ),
                ),
                const SizedBox(height: 20),
                Text('Opaline AI', style: _titleStyle(isDark)),
                const SizedBox(height: 6),
                Text('Accede a tu cuenta', style: _subtitleStyle(isDark)),
                const SizedBox(height: 32),
                TextField(
                  controller: _emailCtrl,
                  keyboardType: TextInputType.emailAddress,
                  decoration: InputDecoration(
                    labelText: 'Correo electronico',
                    prefixIcon: const Icon(Icons.email_outlined),
                  ),
                  onChanged: (_) => setState(() {}),
                ),
                const SizedBox(height: 16),
                TextField(
                  controller: _passCtrl,
                  obscureText: _obscure,
                  onChanged: (_) => setState(() {}),
                  decoration: InputDecoration(
                    labelText: 'Contrasena',
                    prefixIcon: const Icon(Icons.lock_outline),
                    suffixIcon: IconButton(
                      icon: Icon(_obscure ? Icons.visibility_off : Icons.visibility),
                      onPressed: () => setState(() => _obscure = !_obscure),
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerRight,
                  child: TextButton(
                    onPressed: () => context.pushNamed('recuperar_contrasena'),
                    child: const Text('Recuperar contrasena'),
                  ),
                ),
                const SizedBox(height: 24),
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: ElevatedButton(
                    onPressed: (_loading || _emailCtrl.text.isEmpty || _passCtrl.text.isEmpty) ? null : _login,
                    child: _loading
                        ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                        : const Text('Iniciar sesion'),
                  ),
                ),
                const SizedBox(height: 16),
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: OutlinedButton(
                    onPressed: () => context.goNamed('registro'),
                    child: const Text('Crear cuenta'),
                  ),
                ),
                const SizedBox(height: 24),
                Row(
                  children: [
                    Expanded(child: Divider(color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary)),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      child: Text('o continuar con', style: _captionStyle(isDark)),
                    ),
                    Expanded(child: Divider(color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary)),
                  ],
                ),
                const SizedBox(height: 16),
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: OutlinedButton.icon(
                    onPressed: () {
                      // Biometria / Google/Apple placeholder (RF-MOV-02 observaciones: futuras)
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(content: Text('Inicio con Google/Apple disponible en proxima version')),
                      );
                    },
                    icon: const Icon(Icons.login),
                    label: const Text('Biometria / Cuenta segura'),
                  ),
                ),
                const SizedBox(height: 30),
              ],
            ),
          ),
        ),
      ),
    );
  }

  TextStyle _titleStyle(bool isDark) => TextStyle(
        fontSize: 26,
        fontWeight: FontWeight.w800,
        color: isDark ? AppTheme.textDark : AppTheme.textLight,
        fontFamily: 'Poppins',
      );
  TextStyle _subtitleStyle(bool isDark) => TextStyle(
        fontSize: 15,
        color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary,
        fontFamily: 'Poppins',
      );
  TextStyle _captionStyle(bool isDark) => TextStyle(
        fontSize: 13,
        color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary,
        fontFamily: 'Poppins',
      );
}
