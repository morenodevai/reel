import 'package:flutter/foundation.dart';

/// Format a Duration as HH:MM:SS or MM:SS.
String formatDuration(Duration d) {
  final h = d.inHours;
  final m = d.inMinutes.remainder(60);
  final s = d.inSeconds.remainder(60);
  if (h > 0) {
    return '${h.toString().padLeft(2, '0')}:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
  }
  return '${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
}

String timeAgo(String timestamp) {
  try {
    final then = DateTime.parse(timestamp);
    final now = DateTime.now();
    final diff = now.difference(then);
    if (diff.inMinutes < 1) return 'Just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    if (diff.inDays < 7) return '${diff.inDays}d ago';
    return '${then.month}/${then.day}/${then.year}';
  } catch (e) {
    debugPrint('[timeAgo] Failed to parse timestamp "$timestamp": $e');
    return timestamp;
  }
}
