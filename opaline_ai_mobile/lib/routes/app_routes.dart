import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:url_launcher/url_launcher.dart';

import '../core/services/session_guard.dart';
import '../features/auth/screens/blocked_screen.dart';
import '../features/auth/screens/change_password_screen.dart';
import '../features/auth/screens/login_screen.dart';
import '../features/auth/screens/profile_screen.dart';
import '../features/auth/screens/register_screen.dart';
import '../features/auth/screens/welcome_screen.dart';
import '../features/chat/screens/chat_screen.dart';
import '../features/home/screens/home_screen.dart';
import '../features/plans/screens/checkout_screen.dart';
import '../features/plans/screens/plan_detail_screen.dart';
import '../features/plans/screens/plans_screen.dart';
import '../features/plans/screens/subscription_screen.dart';
import '../features/settings/screens/settings_screen.dart';
import '../features/wall/screens/create_post_screen.dart';
import '../features/wall/screens/wall_screen.dart';

final GoRouter appRouter = GoRouter(
  initialLocation: '/bienvenida',
  debugLogDiagnostics: true,
  refreshListenable: SessionGuard.instance.lockNotifier,
  redirect: (BuildContext context, GoRouterState state) {
    final guard = SessionGuard.instance;
    final location = state.matchedLocation;

    // Rutas publicas: no requieren sesion activa.
    const publicRoutes = {
      '/bienvenida',
      '/login',
      '/registro',
      '/recuperar-contrasena',
    };
    final isPublic = publicRoutes.contains(location);

    // Sesion bloqueada (sin login o por inactividad):
    // - rutas publicas y /desbloqueo se permiten
    // - cualquier otra redirige a /desbloqueo solo si habia una sesion previa;
    //   si no, a /login.
    if (guard.isLocked && !isPublic && location != '/desbloqueo') {
      return guard.hasPreviousSession ? '/desbloqueo' : '/login';
    }

    // Ya autenticado: salir de bienvenida/login/registro hacia el inicio.
    if (!guard.isLocked &&
        (location == '/bienvenida' ||
            location == '/login' ||
            location == '/registro')) {
      return '/inicio';
    }

    return null;
  },
  routes: <RouteBase>[
    GoRoute(
      path: '/bienvenida',
      name: 'bienvenida',
      builder: (context, state) => const WelcomeScreen(),
    ),
    GoRoute(
      path: '/desbloqueo',
      name: 'desbloqueo',
      builder: (context, state) => const BlockedScreen(),
    ),
    GoRoute(
      path: '/login',
      name: 'login',
      builder: (context, state) => const LoginScreen(),
    ),
    GoRoute(
      path: '/registro',
      name: 'registro',
      builder: (context, state) => const RegisterScreen(),
    ),
    StatefulShellRoute.indexedStack(
      builder: (context, state, navigationShell) =>
          HomeScreen(navigationShell: navigationShell),
      branches: <StatefulShellBranch>[
        StatefulShellBranch(
          routes: <RouteBase>[
            GoRoute(
              path: '/inicio',
              name: 'inicio',
              builder: (context, state) => const PlansScreen(),
            ),
          ],
        ),
        StatefulShellBranch(
          routes: <RouteBase>[
            GoRoute(
              path: '/chat',
              name: 'chat',
              builder: (context, state) => const ChatScreen(),
            ),
          ],
        ),
        StatefulShellBranch(
          routes: <RouteBase>[
            GoRoute(
              path: '/muro',
              name: 'muro',
              builder: (context, state) => const WallScreen(),
            ),
          ],
        ),
        StatefulShellBranch(
          routes: <RouteBase>[
            GoRoute(
              path: '/cuenta',
              name: 'cuenta',
              builder: (context, state) => const ProfileScreen(),
            ),
          ],
        ),
      ],
    ),
    GoRoute(
      path: '/plan/:id',
      name: 'plan_detalle',
      builder: (context, state) =>
          PlanDetailScreen(planId: state.pathParameters['id']!),
    ),
    GoRoute(
      path: '/checkout',
      name: 'checkout',
      builder: (context, state) => const CheckoutScreen(),
    ),
    GoRoute(
      path: '/suscripcion',
      name: 'suscripcion',
      builder: (context, state) => const SubscriptionScreen(),
    ),
    GoRoute(
      path: '/muro/crear',
      name: 'muro_crear',
      builder: (context, state) => const CreatePostScreen(),
    ),
    GoRoute(
      path: '/cambiar-contrasena',
      name: 'cambiar_contrasena',
      builder: (context, state) => const ChangePasswordScreen(),
    ),
    GoRoute(
      path: '/configuracion',
      name: 'configuracion',
      builder: (context, state) => const SettingsScreen(),
    ),
    GoRoute(
      path: '/recuperar-contrasena',
      name: 'recuperar_contrasena',
      builder: (context, state) => const _RecoverRedirectScreen(),
    ),
  ],
);

class _RecoverRedirectScreen extends StatelessWidget {
  const _RecoverRedirectScreen();

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      final url = 'https://www.opaline-ai.com/recuperar-contrasena';
      if (await canLaunchUrl(Uri.parse(url))) {
        await launchUrl(Uri.parse(url));
      }
    });
    return const Scaffold(
      body: Center(child: Text('Abriendo recuperacion de contrasena...')),
    );
  }
}
