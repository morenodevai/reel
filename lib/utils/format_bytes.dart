import 'dart:math' as math;

/// Formats a byte count into a human-readable string (e.g. "1.5 GB").
String formatBytes(BigInt bytes) {
  final b = bytes.toDouble();
  if (b <= 0) return '0 B';
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  final i = (math.log(b) / math.log(1024)).floor().clamp(0, sizes.length - 1);
  final val = b / math.pow(1024, i);
  return '${val.toStringAsFixed(i > 1 ? 1 : 0)} ${sizes[i]}';
}
