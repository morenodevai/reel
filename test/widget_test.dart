import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:reel/theme/app_theme.dart';

void main() {
  test('buildAppTheme returns dark theme', () {
    final theme = buildAppTheme();
    expect(theme.brightness, Brightness.dark);
    expect(theme.scaffoldBackgroundColor, AppColors.background);
  });
}
