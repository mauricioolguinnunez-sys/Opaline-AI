import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de inicio de sesion (RF-MOV-02).
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
  void dispose() {
    _emailCtrl.dispose();
    _passCtrl.dispose();
    super.dispose();
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
      context.goNamed('inicio');
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(auth.error ?? 'No se pudo iniciar sesión')),
      );
    }
  }

  Future<void> _demoLogin() async {
    _emailCtrl.text = 'demo@opaline-ai.com';
    _passCtrl.text = 'demo1234';
    await _login();
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      body: OpalineDecoration(
        child: Stack(
          children: [
            Positioned(
              top: -90,
              right: -80,
              child: _Orb(size: 260, color: AppTheme.primaryLight.withOpacity(.25)),
            ),
            Positioned(
              bottom: -100,
              left: -90,
              child: _Orb(size: 280, color: AppTheme.primaryDark.withOpacity(.18)),
            ),
            Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 460),
                  child: Container(
                    padding: const EdgeInsets.fromLTRB(26, 30, 26, 24),
                    decoration: BoxDecoration(
                      color: isDark ? AppTheme.surfaceDark : Colors.white.withOpacity(.92),
                      borderRadius: BorderRadius.circular(28),
                      border: Border.all(
                        color: isDark ? const Color(0xFF2C205A) : const Color(0xFFE3DAF6),
                      ),
                      boxShadow: [
                        BoxShadow(
                          color: AppTheme.primaryDark.withOpacity(.14),
                          blurRadius: 40,
                          offset: const Offset(0, 20),
                        ),
                      ],
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Center(
                          child: Container(
                            width: 78,
                            height: 78,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              gradient: const LinearGradient(
                                begin: Alignment.topLeft,
                                end: Alignment.bottomRight,
                                colors: [AppTheme.primaryLight, AppTheme.primaryDark],
                              ),
                              boxShadow: [
                                BoxShadow(
                                  color: AppTheme.primaryLight.withOpacity(.4),
                                  blurRadius: 24,
                                  spreadRadius: 2,
                                ),
                              ],
                            ),
                            child: const Center(
                              child: Text('O', style: TextStyle(fontSize: 38, fontWeight: FontWeight.w800, color: Colors.white)),
                            ),
                          ),
                        ),
                        const SizedBox(height: 18),
                        Text(
                          'Bienvenido a Opaline',
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            fontSize: 25,
                            fontWeight: FontWeight.w800,
                            letterSpacing: -.4,
                            color: isDark ? AppTheme.textDark : AppTheme.textLight,
                          ),
                        ),
                        const SizedBox(height: 6),
                        Text(
                          'Tu IA privada, en todas tus pantallas.',
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            fontSize: 13.5,
                            color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary,
                          ),
                        ),
                        const SizedBox(height: 26),
                        TextField(
                          controller: _emailCtrl,
                          keyboardType: TextInputType.emailAddress,
                          textInputAction: TextInputAction.next,
                          onChanged: (_) => setState(() {}),
                          decoration: const InputDecoration(
                            labelText: 'Correo electrónico',
                            prefixIcon: Icon(Icons.email_outlined),
                          ),
                        ),
                        const SizedBox(height: 14),
                        TextField(
                          controller: _passCtrl,
                          obscureText: _obscure,
                          onChanged: (_) => setState(() {}),
                          onSubmitted: (_) => _login(),
                          decoration: InputDecoration(
                            labelText: 'Contraseña',
                            prefixIcon: const Icon(Icons.lock_outline),
                            suffixIcon: IconButton(
                              icon: Icon(_obscure ? Icons.visibility_off_outlined : Icons.visibility_outlined),
                              onPressed: () => setState(() => _obscure = !_obscure),
                            ),
                          ),
                        ),
                        const SizedBox(height: 8),
                        Align(
                          alignment: Alignment.centerRight,
                          child: TextButton(
                            onPressed: () => context.pushNamed('recuperar_contrasena'),
                            child: const Text('¿Olvidaste tu contraseña?', style: TextStyle(fontSize: 12.5)),
                          ),
                        ),
                        const SizedBox(height: 8),
                        SizedBox(
                          height: 52,
                          child: ElevatedButton(
                            onPressed: (_loading || _emailCtrl.text.isEmpty || _passCtrl.text.isEmpty) ? null : _login,
                            child: _loading
                                ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                                : const Text('Iniciar sesión'),
                          ),
                        ),
                        const SizedBox(height: 10),
                        SizedBox(
                          height: 48,
                          child: OutlinedButton.icon(
                            onPressed: _loading ? null : _demoLogin,
                            icon: const Icon(Icons.bolt_outlined, size: 18),
                            label: const Text('Probar con cuenta demo', style: TextStyle(fontSize: 14)),
                          ),
                        ),
                        const SizedBox(height: 18),
                        Row(
                          children: [
                            Expanded(child: Divider(color: isDark ? const Color(0xFF3A2A63) : const Color(0xFFE3DAF6))),
                            Padding(
                              padding: const EdgeInsets.symmetric(horizontal: 10),
                              child: Text('o', style: TextStyle(fontSize: 12, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary)),
                            ),
                            Expanded(child: Divider(color: isDark ? const Color(0xFF3A2A63) : const Color(0xFFE3DAF6))),
                          ],
                        ),
                        const SizedBox(height: 14),
                        SizedBox(
                          height: 48,
                          child: OutlinedButton(
                            onPressed: () => context.goNamed('registro'),
                            child: const Text('Crear una cuenta gratis'),
                          ),
                        ),
                        const SizedBox(height: 8),
                        Text(
                          'Demo: demo@opaline-ai.com · demo1234',
                          textAlign: TextAlign.center,
                          style: TextStyle(fontSize: 11, color: isDark ? AppTheme.textDarkSecondary : const Color(0xFF9A8CC7)),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Orb extends StatelessWidget {
  final double size;
  final Color color;
  const _Orb({required this.size, required this.color});
  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(shape: BoxShape.circle, color: color),
    );
  }
}
