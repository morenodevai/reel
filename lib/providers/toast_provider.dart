import 'dart:async';
import 'package:flutter_riverpod/flutter_riverpod.dart';

enum ToastType { success, error, info }

class ToastMessage {
  static int _nextId = 0;
  final String id;
  final String message;
  final ToastType type;

  ToastMessage({required this.message, this.type = ToastType.success})
      : id = '${_nextId++}';
}

class ToastNotifier extends Notifier<List<ToastMessage>> {
  final Map<String, Timer> _timers = {};

  @override
  List<ToastMessage> build() {
    ref.onDispose(() {
      for (final t in _timers.values) {
        t.cancel();
      }
      _timers.clear();
    });
    return [];
  }

  void show(String message, {ToastType type = ToastType.success}) {
    final toast = ToastMessage(message: message, type: type);
    state = [...state, toast];

    // Auto-dismiss after 4 seconds
    _timers[toast.id] = Timer(const Duration(seconds: 4), () {
      dismiss(toast.id);
    });
  }

  void dismiss(String id) {
    _timers[id]?.cancel();
    _timers.remove(id);
    state = state.where((t) => t.id != id).toList();
  }
}

final toastProvider =
    NotifierProvider<ToastNotifier, List<ToastMessage>>(ToastNotifier.new);
