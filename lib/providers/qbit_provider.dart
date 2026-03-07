import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/api/qbit_api.dart' as qbit_api;
import 'package:reel/providers/config_provider.dart';
import 'package:reel/providers/library_provider.dart';

/// Manages the qBittorrent poller lifecycle.
/// Automatically starts/stops based on qbitEnabled config.
class QbitNotifier extends Notifier<QbitState> {
  StreamSubscription<String>? _pollerSub;
  bool _starting = false;
  bool _cancelled = false;
  int _retryCount = 0;
  Timer? _retryTimer;

  @override
  QbitState build() {
    final config = ref.watch(configProvider).value;
    final shouldRun = config != null && config.qbitEnabled;
    final isRunning = _pollerSub != null;

    if (shouldRun && !isRunning && !_starting) {
      _starting = true;
      _cancelled = false;
      Future.microtask(() => _startPoller());
    } else if (!shouldRun && (isRunning || _starting)) {
      _stopPoller();
    }

    ref.onDispose(_stopPoller);
    return QbitState(running: isRunning, status: isRunning ? 'Connected' : '');
  }

  Future<void> _startPoller() async {
    if (_cancelled) {
      _starting = false;
      return;
    }
    try {
      _pollerSub = qbit_api.startQbitPoller().listen(
        (event) {
          debugPrint('[qbit] $event');
          if (event.startsWith('qbit-imported:') || event.startsWith('done:')) {
            ref.read(libraryProvider.notifier).refresh();
          }
        },
        onError: (e) {
          debugPrint('[qbit] Poller error: $e');
          _starting = false;
          _pollerSub?.cancel();
          _pollerSub = null;
          state = const QbitState(running: false, status: 'Error');
          _scheduleRetry();
        },
        onDone: () {
          _starting = false;
          _pollerSub = null;
          final config = ref.read(configProvider).value;
          if (config != null && config.qbitEnabled) {
            debugPrint('[qbit] Poller stream ended, retrying...');
            state = const QbitState(running: false, status: 'Reconnecting...');
            _scheduleRetry();
          } else {
            state = const QbitState(running: false, status: '');
          }
        },
      );
      _starting = false;
      _retryCount = 0;
      state = const QbitState(running: true, status: 'Connected');
    } catch (e) {
      debugPrint('[qbit] Failed to start poller: $e');
      _starting = false;
      _pollerSub?.cancel();
      _pollerSub = null;
      state = QbitState(running: false, status: 'Error: $e');
      _scheduleRetry();
    }
  }

  void _scheduleRetry() {
    if (_retryCount >= 5) {
      debugPrint('[qbit] Max retries reached');
      state = const QbitState(running: false, status: 'Failed after 5 retries');
      return;
    }
    final delay = Duration(seconds: 2 << _retryCount);
    _retryCount++;
    _retryTimer?.cancel();
    _retryTimer = Timer(delay, () => _startPoller());
  }

  void _stopPoller() {
    _retryTimer?.cancel();
    _retryTimer = null;
    _pollerSub?.cancel();
    _pollerSub = null;
    _starting = false;
    _cancelled = true;
    _retryCount = 0;
    qbit_api.stopQbitPoller().catchError((_) {});
  }
}

/// qBit poller state — persists across page navigations.
class QbitState {
  final bool running;
  final String status;
  const QbitState({required this.running, required this.status});

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is QbitState && running == other.running && status == other.status;

  @override
  int get hashCode => running.hashCode ^ status.hashCode;
}

final qbitProvider = NotifierProvider<QbitNotifier, QbitState>(QbitNotifier.new);
