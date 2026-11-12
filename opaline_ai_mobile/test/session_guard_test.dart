import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:opaline_ai_mobile/core/services/session_guard.dart';

void main() {
  testWidgets('unlock no concede acceso sin un factor real', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: Scaffold(body: SizedBox.shrink())));
    final context = tester.element(find.byType(Scaffold));

    final result = await SessionGuard.instance.unlock(context);

    expect(result, isFalse);
    expect(SessionGuard.instance.isLocked, isTrue);
  });
}