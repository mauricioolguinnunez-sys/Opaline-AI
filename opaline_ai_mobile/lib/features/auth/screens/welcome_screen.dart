import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../../../core/app_theme.dart';

/// Pantalla de bienvenida / onboarding al estilo Style.png.
/// "Opaline AI - Inteligencia accesible. Potencial ilimitado."
class WelcomeScreen extends StatelessWidget {
  const WelcomeScreen({super.key});

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
                const SizedBox(height: 40),
                // Logo estilo Style.png (anillo entrelazado)
                Container(
                  width: 110,
                  height: 110,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    gradient: LinearGradient(
                      colors: [
                        AppTheme.primaryLight,
                        AppTheme.primaryDark,
                      ],
                    ),
                    boxShadow: [
                      BoxShadow(
                        color: AppTheme.primaryLight.withOpacity(0.45),
                        blurRadius: 30,
                        spreadRadius: 4,
                      ),
                    ],
                  ),
                  child: const Center(
                    child: Text(
                      'O',
                      style: TextStyle(
                        fontSize: 56,
                        fontWeight: FontWeight.w800,
                        color: Colors.white,
                        letterSpacing: -8,
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 28),
                Text(
                  'Opaline AI',
                  style: googleFontsPoppins(
                    fontSize: 34,
                    fontWeight: FontWeight.w800,
                    color: isDark ? AppTheme.textDark : AppTheme.textLight,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 10),
                Text(
                  'Inteligencia accesible. Potencial ilimitado.',
                  style: googleFontsPoppins(
                    fontSize: 15,
                    fontWeight: FontWeight.w400,
                    color: isDark ? AppTheme.textDarkSecondary : AppTheme.textLightSecondary,
                    height: 1.5,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 40),
                _FeatureRow(
                  icon: Icons.auto_awesome,
                  label: 'IA al alcance\nde todos',
                ),
                _FeatureRow(
                  icon: Icons.security,
                  label: 'Sesion segura y\nbiometria',
                ),
                _FeatureRow(
                  icon: Icons.chat_bubble_outline,
                  label: 'Chatbot con\nhistorial',
                ),
                const SizedBox(height: 40),
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: ElevatedButton(
                    onPressed: () => context.pushNamed('login'),
                    child: const Text('Iniciar sesion'),
                  ),
                ),
                const SizedBox(height: 12),
                SizedBox(
                  width: double.infinity,
                  height: 52,
                  child: OutlinedButton(
                    onPressed: () => context.pushNamed('registro'),
                    child: const Text('Crear cuenta'),
                  ),
                ),
                const SizedBox(height: 24),
                TextButton(
                  onPressed: () => context.pushNamed('recuperar_contrasena'),
                  child: Text(
                    '¿Olvidaste tu contraseña?',
                    style: googleFontsPoppins(
                      color: isDark ? AppTheme.primaryLight : AppTheme.primaryDark,
                      fontWeight: FontWeight.w600,
                    ),
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
}

TextStyle googleFontsPoppins({
  double fontSize = 14,
  FontWeight fontWeight = FontWeight.w400,
  Color? color,
  double? height,
}) {
  return TextStyle(
    fontSize: fontSize,
    fontWeight: fontWeight,
    color: color,
    height: height,
    fontFamily: 'Poppins',
  );
}

class _FeatureRow extends StatelessWidget {
  final IconData icon;
  final String label;
  const _FeatureRow({required this.icon, required this.label});
  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Container(
            width: 40,
            height: 40,
            decoration: BoxDecoration(
              color: (isDark ? AppTheme.primaryDark : AppTheme.primaryLight).withOpacity(0.15),
              borderRadius: BorderRadius.circular(12),
            ),
            child: Icon(icon, size: 20, color: isDark ? AppTheme.primaryLight : AppTheme.primaryDark),
          ),
          const SizedBox(width: 14),
          Flexible(
            child: Text(
              label,
              textAlign: TextAlign.center,
              style: googleFontsPoppins(
                fontSize: 14,
                fontWeight: FontWeight.w500,
                color: isDark ? AppTheme.textDark : AppTheme.textLight,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
