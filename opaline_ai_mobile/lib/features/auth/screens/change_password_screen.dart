import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../core/app_theme.dart';
import '../../../providers/auth_provider.dart';

/// Pantalla de cambio de contrasena (RF-MOV-02).
class ChangePasswordScreen extends StatefulWidget {
  const ChangePasswordScreen({super.key});
  @override
  State<ChangePasswordScreen> createState() => _ChangePasswordScreenState();
}

class _ChangePasswordScreenState extends State<ChangePasswordScreen> {
  final _current = TextEditingController();
  final _new = TextEditingController();
  bool _loading = false;

  @override
  void initState() {
    super.initState();
  }

  Future<void> _change() async {
    final auth = context.read<AuthProvider>();
    setState(() => _loading = true);
    final ok = await auth.changePassword(current: _current.text, newPassword: _new.text);
    if (!mounted) return;
    setState(() => _loading = false);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Contrasena cambiada correctamente')));
      Navigator.pop(context);
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(auth.error ?? 'Error')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Scaffold(
      appBar: AppBar(title: const Text('Cambiar contrasena')),
      body: OpalineDecoration(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Cambia tu contrasena', style: _title(isDark)),
              const SizedBox(height: 24),
              TextField(
                controller: _current,
                obscureText: true,
                decoration: InputDecoration(
                  labelText: 'Contrasena actual',
                  prefixIcon: const Icon(Icons.lock_outline),
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _new,
                obscureText: true,
                decoration: InputDecoration(
                  labelText: 'Nueva contrasena',
                  prefixIcon: const Icon(Icons.lock_outline),
                ),
              ),
              const SizedBox(height: 10),
              Text(
                '10-64 caracteres, mayuscula, minuscula, un digito y un caracter especial.',
                style: TextStyle(fontSize: 12, color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary, fontFamily: 'Poppins'),
              ),
              const SizedBox(height: 28),
              SizedBox(
                width: double.infinity,
                height: 52,
                child: ElevatedButton(
                  onPressed: _loading ? null : _change,
                  child: _loading ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2)) : const Text('Cambiar contrasena'),
                ),
              ),
              const SizedBox(height: 30),
            ],
          ),
        ),
      ),
    );
  }

  TextStyle _title(bool isDark) => TextStyle(fontSize: 22, fontWeight: FontWeight.w800, color: isDark ? AppTheme.textDark : AppTheme.textLight, fontFamily: 'Poppins');
}
