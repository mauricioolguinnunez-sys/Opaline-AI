import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de registro (RF-MOV-02).
/// Nombre, correo y contrasena 10-64 con mayuscula, minuscula, digito y caracter especial.
class RegisterScreen extends StatefulWidget {
  const RegisterScreen({super.key});
  @override
  State<RegisterScreen> createState() => _RegisterScreenState();
}

class _RegisterScreenState extends State<RegisterScreen> {
  final _nameCtrl = TextEditingController();
  final _emailCtrl = TextEditingController();
  final _passCtrl = TextEditingController();
  bool _obscure = true;
  bool _loading = false;

  @override
  void initState() {
    super.initState();
  }

  bool _passwordValid(String p) {
    return RegExp(r'^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[\W_]).{10,64}$').hasMatch(p);
  }

  Future<void> _register() async {
    if (_nameCtrl.text.trim().isEmpty || _emailCtrl.text.trim().isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Completa nombre y correo')),
      );
      return;
    }
    if (!_passwordValid(_passCtrl.text)) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('La contrasena debe tener 10-64 caracteres, mayuscula, minuscula, un digito y un caracter especial.'),
        ),
      );
      return;
    }

    final auth = context.read<AuthProvider>();
    setState(() => _loading = true);
    final ok = await auth.register(
      name: _nameCtrl.text.trim(),
      email: _emailCtrl.text.trim(),
      password: _passCtrl.text,
    );
    if (!mounted) return;
    setState(() => _loading = false);
    if (ok) {
      context.goNamed('inicio');
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(auth.error ?? 'No se pudo registrar')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;

    return Scaffold(
      appBar: AppBar(
        leading: IconButton(icon: const Icon(Icons.arrow_back), onPressed: () => context.goNamed('login')),
        title: const Text('Crear cuenta'),
      ),
      body: OpalineDecoration(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Registrate', style: _title(isDark)),
              const SizedBox(height: 6),
              Text('Crea tu cuenta de Opaline AI', style: _sub(isDark)),
              const SizedBox(height: 24),
              TextField(
                controller: _nameCtrl,
                decoration: InputDecoration(
                  labelText: 'Nombre completo',
                  prefixIcon: const Icon(Icons.person_outline),
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _emailCtrl,
                keyboardType: TextInputType.emailAddress,
                decoration: InputDecoration(
                  labelText: 'Correo electronico',
                  prefixIcon: const Icon(Icons.email_outlined),
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _passCtrl,
                obscureText: _obscure,
                decoration: InputDecoration(
                  labelText: 'Contrasena',
                  prefixIcon: const Icon(Icons.lock_outline),
                  suffixIcon: IconButton(
                    icon: Icon(_obscure ? Icons.visibility_off : Icons.visibility),
                    onPressed: () => setState(() => _obscure = !_obscure),
                  ),
                ),
              ),
              const SizedBox(height: 10),
              Text(
                '10-64 caracteres, mayuscula, minuscula, un digito y un caracter especial.',
                style: TextStyle(fontSize: 12, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins'),
              ),
              const SizedBox(height: 24),
              SizedBox(
                width: double.infinity,
                height: 52,
                child: ElevatedButton(
                  onPressed: _loading ? null : _register,
                  child: _loading
                      ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Registrarse'),
                ),
              ),
              const SizedBox(height: 16),
              TextButton(
                onPressed: () => context.goNamed('login'),
                child: const Text('Ya tengo cuenta, iniciar sesion'),
              ),
              const SizedBox(height: 24),
            ],
          ),
        ),
      ),
    );
  }

  TextStyle _title(bool isDark) => TextStyle(fontSize: 24, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins');
  TextStyle _sub(bool isDark) => TextStyle(fontSize: 15, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins');
}
