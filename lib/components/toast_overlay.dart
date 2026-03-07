import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/theme/app_theme.dart';

class ToastOverlay extends ConsumerWidget {
  const ToastOverlay({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final toasts = ref.watch(toastProvider);
    if (toasts.isEmpty) return const SizedBox.shrink();

    return Positioned(
      bottom: 16,
      right: 16,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: toasts.map((toast) {
          return _ToastCard(
            toast: toast,
            onDismiss: () => ref.read(toastProvider.notifier).dismiss(toast.id),
          );
        }).toList(),
      ),
    );
  }
}

class _ToastCard extends StatelessWidget {
  final ToastMessage toast;
  final VoidCallback onDismiss;
  const _ToastCard({required this.toast, required this.onDismiss});

  @override
  Widget build(BuildContext context) {
    final (bgColor, borderColor, iconColor, icon) = switch (toast.type) {
      ToastType.success => (
        AppColors.success.withValues(alpha: 0.15),
        AppColors.success.withValues(alpha: 0.3),
        AppColors.success,
        Icons.check_circle_outline,
      ),
      ToastType.error => (
        AppColors.error.withValues(alpha: 0.15),
        AppColors.error.withValues(alpha: 0.3),
        AppColors.error,
        Icons.error_outline,
      ),
      ToastType.info => (
        AppColors.primary.withValues(alpha: 0.15),
        AppColors.primary.withValues(alpha: 0.3),
        AppColors.primary,
        Icons.info_outline,
      ),
    };

    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Container(
        constraints: const BoxConstraints(minWidth: 260, maxWidth: 360),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          color: bgColor,
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 16, color: iconColor),
            const SizedBox(width: 10),
            Flexible(
              child: Text(
                toast.message,
                style: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                  color: AppColors.textPrimary,
                ),
              ),
            ),
            const SizedBox(width: 8),
            MouseRegion(
              cursor: SystemMouseCursors.click,
              child: GestureDetector(
                onTap: onDismiss,
                child: const Icon(Icons.close, size: 14, color: AppColors.textTertiary),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
