import 'package:flutter/material.dart';
import 'package:reel/src/rust/config.dart';
import 'package:reel/theme/app_theme.dart';

// ---------------------------------------------------------------------------
// Section Header
// ---------------------------------------------------------------------------

class SectionHeader extends StatelessWidget {
  final String title;
  const SectionHeader(this.title, {super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Text(
        title.toUpperCase(),
        style: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: AppColors.textTertiary,
          letterSpacing: 1,
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

class SettingsCard extends StatelessWidget {
  final List<Widget> children;
  const SettingsCard({super.key, required this.children});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surfaceElevated.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: AppColors.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: children,
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Setting Row
// ---------------------------------------------------------------------------

class SettingRow extends StatelessWidget {
  final String label;
  final String? subtitle;
  final Widget trailing;
  final bool isSubsetting;

  const SettingRow({
    super.key,
    required this.label,
    this.subtitle,
    required this.trailing,
    this.isSubsetting = false,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                label,
                style: TextStyle(
                  fontSize: isSubsetting ? 12 : 14,
                  color: isSubsetting ? AppColors.textSecondary : AppColors.textPrimary,
                ),
              ),
              if (subtitle != null)
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Text(
                    subtitle!,
                    style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                  ),
                ),
            ],
          ),
        ),
        trailing,
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// Path Text
// ---------------------------------------------------------------------------

class PathText extends StatelessWidget {
  final String path;
  const PathText(this.path, {super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Text(
        path,
        style: const TextStyle(fontSize: 12, color: AppColors.textQuaternary),
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Toggle Switch
// ---------------------------------------------------------------------------

class ToggleSwitch extends StatelessWidget {
  final bool value;
  final ValueChanged<bool> onChanged;
  const ToggleSwitch({super.key, required this.value, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: () => onChanged(!value),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          width: 40,
          height: 24,
          decoration: BoxDecoration(
            color: value ? AppColors.primary : AppColors.surfaceElevated,
            borderRadius: BorderRadius.circular(12),
          ),
          child: AnimatedAlign(
            duration: const Duration(milliseconds: 200),
            alignment: value ? Alignment.centerRight : Alignment.centerLeft,
            child: Container(
              width: 16,
              height: 16,
              margin: const EdgeInsets.symmetric(horizontal: 4),
              decoration: const BoxDecoration(
                color: Colors.white,
                shape: BoxShape.circle,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Small Button
// ---------------------------------------------------------------------------

class SmallButton extends StatelessWidget {
  final IconData? icon;
  final String label;
  final VoidCallback? onTap;
  const SmallButton({super.key, this.icon, required this.label, this.onTap});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: onTap != null ? SystemMouseCursors.click : SystemMouseCursors.basic,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
          decoration: BoxDecoration(
            color: AppColors.surfaceElevated,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[
                Icon(icon, size: 14, color: AppColors.textSecondary),
                const SizedBox(width: 6),
              ],
              Text(
                label,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textSecondary,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// API Key Field
// ---------------------------------------------------------------------------

class ApiKeyField extends StatelessWidget {
  final String label;
  final String value;
  final String hint;
  final String helpText;
  final bool reveal;
  final VoidCallback onToggleReveal;
  final ValueChanged<String> onChanged;

  const ApiKeyField({
    super.key,
    required this.label,
    required this.value,
    required this.hint,
    required this.helpText,
    required this.reveal,
    required this.onToggleReveal,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(label,
                style: const TextStyle(fontSize: 12, color: AppColors.textSecondary)),
            MouseRegion(
              cursor: SystemMouseCursors.click,
              child: GestureDetector(
                onTap: onToggleReveal,
                child: Icon(
                  reveal ? Icons.visibility_off : Icons.visibility,
                  size: 12,
                  color: AppColors.textQuaternary,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        TextFormField(
          initialValue: value,
          obscureText: !reveal,
          style: const TextStyle(
            fontSize: 12,
            color: AppColors.textPrimary,
            fontFamily: 'monospace',
          ),
          decoration: InputDecoration(hintText: hint),
          onChanged: onChanged,
        ),
        const SizedBox(height: 4),
        Text(
          helpText,
          style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
        ),
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// qBittorrent Manual Config
// ---------------------------------------------------------------------------

class QbitManualConfig extends StatelessWidget {
  final QbitConfig config;
  final ValueChanged<QbitConfig> onUpdate;
  final VoidCallback onTest;

  const QbitManualConfig({
    super.key,
    required this.config,
    required this.onUpdate,
    required this.onTest,
  });

  void _emit({String? host, int? port, String? username, String? password}) {
    onUpdate(QbitConfig(
      host: host ?? config.host,
      port: port ?? config.port,
      username: username ?? config.username,
      password: password ?? config.password,
      autoRemove: config.autoRemove,
    ));
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Row(
          children: [
            Expanded(
              flex: 2,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Host',
                      style: TextStyle(fontSize: 10, color: AppColors.textQuaternary)),
                  const SizedBox(height: 4),
                  TextFormField(
                    initialValue: config.host,
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                    decoration: const InputDecoration(hintText: 'localhost'),
                    onChanged: (v) => _emit(host: v),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Port',
                      style: TextStyle(fontSize: 10, color: AppColors.textQuaternary)),
                  const SizedBox(height: 4),
                  TextFormField(
                    initialValue: config.port.toString(),
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                    keyboardType: TextInputType.number,
                    onChanged: (v) => _emit(port: int.tryParse(v) ?? 0),
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Username',
                      style: TextStyle(fontSize: 10, color: AppColors.textQuaternary)),
                  const SizedBox(height: 4),
                  TextFormField(
                    initialValue: config.username,
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                    decoration: const InputDecoration(hintText: 'admin'),
                    onChanged: (v) => _emit(username: v),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Password',
                      style: TextStyle(fontSize: 10, color: AppColors.textQuaternary)),
                  const SizedBox(height: 4),
                  TextFormField(
                    initialValue: config.password,
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                    obscureText: true,
                    onChanged: (v) => _emit(password: v),
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        SmallButton(label: 'Test Connection', onTap: onTest),
      ],
    );
  }
}
