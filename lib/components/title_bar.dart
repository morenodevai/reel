import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/theme/app_theme.dart';

class TitleBar extends ConsumerWidget {
  final int reviewCount;
  const TitleBar({super.key, this.reviewCount = 0});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final nav = ref.watch(navigationProvider.notifier);
    final pages = ref.watch(navigationProvider);
    final canGoBack = pages.length > 1;
    final title = nav.title;
    final currentPage = nav.current;

    return GestureDetector(
      onPanStart: (_) => windowManager.startDragging(),
      child: Container(
        height: 48,
        decoration: BoxDecoration(
          color: AppColors.background.withValues(alpha: 0.8),
          border: const Border(
            bottom: BorderSide(color: AppColors.border, width: 0.5),
          ),
        ),
        child: Row(
          children: [
            // macOS traffic light space + back button
            SizedBox(width: Platform.isMacOS ? 78 : 8),
            if (canGoBack)
              _TitleBarButton(
                icon: Icons.chevron_left,
                onTap: nav.pop,
              ),
            const SizedBox(width: 4),
            Expanded(
              child: Text(
                title,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: AppColors.textPrimary,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            // Review badge
            if (reviewCount > 0 && currentPage is! ReviewAppPage)
              _ReviewBadge(
                count: reviewCount,
                onTap: () => nav.goToReview([]),
              ),
            // History
            if (currentPage is! HistoryAppPage)
              _TitleBarButton(
                icon: Icons.history,
                onTap: nav.goToHistory,
              ),
            // Settings
            if (currentPage is! SettingsAppPage)
              _TitleBarButton(
                icon: Icons.settings_outlined,
                onTap: nav.goToSettings,
              ),
            const SizedBox(width: 12),
          ],
        ),
      ),
    );
  }
}

class _TitleBarButton extends StatelessWidget {
  final IconData icon;
  final VoidCallback onTap;
  const _TitleBarButton({required this.icon, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 8),
          child: Icon(icon, size: 18, color: AppColors.textTertiary),
        ),
      ),
    );
  }
}

class _ReviewBadge extends StatelessWidget {
  final int count;
  final VoidCallback onTap;
  const _ReviewBadge({required this.count, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 8),
          child: Stack(
            clipBehavior: Clip.none,
            children: [
              const Icon(Icons.assignment_outlined, size: 18, color: AppColors.textTertiary),
              Positioned(
                top: -4,
                right: -6,
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                  decoration: BoxDecoration(
                    color: AppColors.error,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  constraints: const BoxConstraints(minWidth: 16),
                  child: Text(
                    count > 99 ? '99+' : '$count',
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                      fontSize: 9,
                      fontWeight: FontWeight.bold,
                      color: Colors.white,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
