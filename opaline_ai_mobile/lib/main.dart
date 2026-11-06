import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:hive_flutter/hive_flutter.dart';
import 'package:provider/provider.dart';

import 'core/app_theme.dart';
import 'core/api/api_client.dart';
import 'core/services/cache_service.dart';
import 'core/services/secure_storage.dart';
import 'core/services/session_guard.dart';
import 'providers/auth_provider.dart';
import 'providers/plan_provider.dart';
import 'providers/chat_provider.dart';
import 'providers/wall_provider.dart';
import 'routes/app_routes.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await SecureStorage.init();
  // Permite que ApiClient inyecte el access token en cada peticion (RF-MOV-02).
  ApiClient.setTokenReader(() => SecureStorage.read(SecureStorage.accessToken));
  await Hive.initFlutter();
  await CacheService.instance.init();
  await SessionGuard.instance.load();

  runApp(const OpalineApp());
}

class OpalineApp extends StatelessWidget {
  const OpalineApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        // La inicializacion se difiere al primer frame para no notificar
        // durante la construccion del arbol de widgets (evita cuelgues).
        ChangeNotifierProvider(
          create: (_) {
            final p = AuthProvider();
            WidgetsBinding.instance.addPostFrameCallback((_) => p.init());
            return p;
          },
        ),
        ChangeNotifierProvider(
          create: (_) {
            final p = PlanProvider();
            WidgetsBinding.instance.addPostFrameCallback((_) => p.init());
            return p;
          },
        ),
        ChangeNotifierProvider(create: (_) => ChatProvider()),
        ChangeNotifierProvider(
          create: (_) {
            final p = WallProvider();
            WidgetsBinding.instance.addPostFrameCallback((_) => p.init());
            return p;
          },
        ),
      ],
      child: MaterialApp.router(
        title: 'Opaline AI',
        debugShowCheckedModeBanner: false,
        routerConfig: appRouter,
        theme: AppTheme.lightTheme(),
        darkTheme: AppTheme.darkTheme(),
        themeMode: ThemeMode.system,
        builder: (context, child) {
          return ScrollConfiguration(
            behavior: const OpalineScrollBehavior(),
            child: child!,
          );
        },
        localizationsDelegates: [
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: const [Locale('es', 'ES'), Locale('en', 'US')],
        locale: const Locale('es', 'ES'),
      ),
    );
  }
}

/// Permite arrastrar listas con mouse/trackpad en Linux/desktop.
class OpalineScrollBehavior extends MaterialScrollBehavior {
  const OpalineScrollBehavior();

  @override
  Set<PointerDeviceKind> get dragDevices => {
        PointerDeviceKind.touch,
        PointerDeviceKind.mouse,
        PointerDeviceKind.trackpad,
        PointerDeviceKind.stylus,
      };
}
