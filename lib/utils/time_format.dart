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
  } catch (_) {
    return timestamp;
  }
}
