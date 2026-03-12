import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/theme/app_theme.dart';

class TitleBar extends ConsumerWidget {
  final int trashCount;
  const TitleBar({super.key, this.trashCount = 0});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final nav = ref.read(navigationProvider.notifier);
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
            const SizedBox(width: 8),
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
            if (trashCount > 0 && currentPage is! TrashAppPage)
              _IconBadge(
                icon: Icons.delete_outline,
                count: trashCount,
                badgeColor: AppColors.textQuaternary,
                onTap: nav.goToTrash,
              ),
            if (currentPage is! HistoryAppPage)
              _TitleBarButton(
                icon: Icons.history,
                onTap: nav.goToHistory,
              ),
            if (currentPage is! SettingsAppPage)
              _TitleBarButton(
                icon: Icons.settings_outlined,
                onTap: nav.goToSettings,
              ),
            const SizedBox(width: 8),
            // Window controls
            const _WindowControls(),
          ],
        ),
      ),
    );
  }
}

class _WindowControls extends StatelessWidget {
  const _WindowControls();

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _WindowButton(
          icon: Icons.minimize,
          onTap: () => windowManager.minimize(),
        ),
        _WindowButton(
          icon: Icons.crop_square,
          onTap: () async {
            if (await windowManager.isMaximized()) {
              windowManager.unmaximize();
            } else {
              windowManager.maximize();
            }
          },
        ),
        _WindowButton(
          icon: Icons.close,
          hoverColor: AppColors.error,
          onTap: () => windowManager.close(),
        ),
        const SizedBox(width: 4),
      ],
    );
  }
}

class _WindowButton extends StatefulWidget {
  final IconData icon;
  final VoidCallback onTap;
  final Color? hoverColor;
  const _WindowButton({required this.icon, required this.onTap, this.hoverColor});

  @override
  State<_WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<_WindowButton> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final color = _hovering && widget.hoverColor != null
        ? widget.hoverColor!
        : AppColors.textTertiary;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() => _hovering = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Container(
          width: 34,
          height: 28,
          decoration: BoxDecoration(
            color: _hovering
                ? (widget.hoverColor ?? AppColors.textTertiary).withValues(alpha: 0.1)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(4),
          ),
          child: Icon(widget.icon, size: 16, color: color),
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

class _IconBadge extends StatelessWidget {
  final IconData icon;
  final int count;
  final Color badgeColor;
  final VoidCallback onTap;
  const _IconBadge({required this.icon, required this.count, required this.badgeColor, required this.onTap});

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
              Icon(icon, size: 18, color: AppColors.textTertiary),
              Positioned(
                top: -4,
                right: -6,
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                  decoration: BoxDecoration(
                    color: badgeColor,
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
