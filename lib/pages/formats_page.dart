import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:reel/providers/config_provider.dart';
import 'package:reel/providers/library_provider.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/components/format_card.dart';
import 'package:reel/components/media_card.dart';
import 'package:reel/components/dock_row.dart';
import 'package:reel/components/drop_zone.dart';
import 'package:reel/components/empty_state.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/utils/config_copy.dart';
import 'package:reel/utils/play_media.dart';
import 'package:reel/src/rust/api/pipeline_api.dart' as pipeline_api;
import 'package:reel/src/rust/api/config_api.dart' as config_api;

class FormatsPageWidget extends ConsumerStatefulWidget {
  const FormatsPageWidget({super.key});

  @override
  ConsumerState<FormatsPageWidget> createState() => _FormatsPageWidgetState();
}

class _FormatsPageWidgetState extends ConsumerState<FormatsPageWidget> {
  StreamSubscription<String>? _processSub;

  @override
  void dispose() {
    _processSub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final config = ref.watch(configProvider).value;
    final library = ref.watch(libraryProvider);
    final nav = ref.read(navigationProvider.notifier);

    if (config?.libraryPath == null) {
      return const _NoLibraryState();
    }

    if (library.loading) {
      return const _LoadingSkeleton();
    }

    return SingleChildScrollView(
      key: const PageStorageKey('formats'),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (config?.libraryPath != null)
            _LibraryPathRow(path: config!.libraryPath!),
          const SizedBox(height: 16),

          if (library.recentlyAdded.isNotEmpty) ...[
            DockRow(
              header: const Text(
                'Recently Added',
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                  color: AppColors.textPrimary,
                ),
              ),
              dockListInsets:
                  const EdgeInsets.symmetric(horizontal: -16),
              itemCount: library.recentlyAdded.length,
              itemBuilder: (context, index) {
                final media = library.recentlyAdded[index];
                return MediaCard.small(
                  media: media,
                  onTap: () => nav.goToMediaDetail(media),
                  onPlay: () => playMedia(media, ref),
                );
              },
            ),
            const SizedBox(height: 24),
          ],

          LayoutBuilder(
            builder: (context, constraints) {
              final crossAxisCount = constraints.maxWidth > 800 ? 3 : 2;
              return Wrap(
                spacing: 12,
                runSpacing: 12,
                children: library.formats.where((f) => f.name != 'Needs Review').map((format) {
                  final cardWidth =
                      (constraints.maxWidth - 12 * (crossAxisCount - 1)) /
                          crossAxisCount;
                  return SizedBox(
                    width: cardWidth,
                    child: FormatCard(
                      format: format,
                      onTap: () => nav.goToGenres(format),
                    ),
                  );
                }).toList(),
              );
            },
          ),

          const SizedBox(height: 24),
          DropZone(
            compact: library.formats.isNotEmpty,
            onDrop: (paths) => _confirmAndProcess(context, paths),
            onBrowse: () => _handleBrowse(context),
          ),
        ],
      ),
    );
  }

  Future<void> _confirmAndProcess(BuildContext context, List<String> paths) async {
    final itemCount = paths.length;
    final label = itemCount == 1 ? paths.first.split(RegExp(r'[\\/]')).last : '$itemCount item(s)';
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Organize files?', style: TextStyle(color: AppColors.textPrimary, fontSize: 16)),
        content: Text(
          'Process $label and organize into your library?',
          style: const TextStyle(color: AppColors.textSecondary, fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel', style: TextStyle(color: AppColors.textTertiary)),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Organize', style: TextStyle(color: AppColors.primary)),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      _handleDrop(paths);
    }
  }

  void _handleDrop(List<String> paths) {
    final toast = ref.read(toastProvider.notifier);
    toast.show('Processing ${paths.length} item(s)...', type: ToastType.info);

    // Cancel any prior subscription before starting a new one.
    _processSub?.cancel();
    _processSub = pipeline_api.processBackground(paths: paths).listen(
      (event) {
        if (!mounted) return;
        try {
          final data = event.isNotEmpty ? _parseJson(event) : <String, dynamic>{};
          final type = data['type'] as String?;
          if (type == 'done') {
            final succeeded = data['succeeded'] as int? ?? 0;
            final failed = data['failed'] as int? ?? 0;
            final total = data['total'] as int? ?? 0;
            final reviewCount = data['needs_review_count'] as int? ?? 0;
            final batchId = data['batch_id'] as String?;
            if (total == 0) {
              toast.show('No new files to organize', type: ToastType.info);
            } else if (reviewCount > 0) {
              final autoCount = succeeded - reviewCount;
              if (autoCount > 0) {
                toast.show('Organized $autoCount file(s), $reviewCount need${reviewCount == 1 ? 's' : ''} review');
              } else {
                toast.show('$reviewCount file(s) need review', type: ToastType.info);
              }
            } else if (failed == 0) {
              toast.show('Organized $succeeded file(s)');
            } else {
              toast.show('Organized $succeeded, $failed failed', type: ToastType.error);
            }
            ref.read(libraryProvider.notifier).refresh();
            // Navigate to review page when items need user confirmation
            if (reviewCount > 0 && batchId != null) {
              ref.read(navigationProvider.notifier).goToReview([batchId]);
            }
          } else if (type == 'error') {
            final msg = data['message'] as String? ?? 'Unknown error';
            toast.show('Processing error: $msg', type: ToastType.error);
          } else if (type == 'progress') {
            final processed = data['done'] as int? ?? 0;
            final total = data['total'] as int? ?? 0;
            final title = data['title'] as String?;
            if (total > 1) {
              toast.show('Processing $processed of $total${title != null ? ': $title' : ''}...', type: ToastType.info);
            }
            if (processed % 3 == 0 || processed == total) {
              ref.read(libraryProvider.notifier).refresh();
            }
          }
        } catch (e) {
          debugPrint('[formats] Failed to parse progress event: $e');
        }
      },
      onError: (e) {
        if (!mounted) return;
        toast.show('Processing failed: $e', type: ToastType.error);
      },
    );
  }

  static Map<String, dynamic> _parseJson(String json) {
    try {
      return Map<String, dynamic>.from(
        (const JsonDecoder().convert(json)) as Map,
      );
    } catch (e) {
      debugPrint('[formats] JSON parse error: $e');
      return {};
    }
  }

  Future<void> _handleBrowse(BuildContext context) async {
    final result = await FilePicker.platform.pickFiles(
      allowMultiple: true,
      dialogTitle: 'Select media files to organize',
    );
    if (result != null && result.files.isNotEmpty) {
      final paths = result.files
          .where((f) => f.path != null)
          .map((f) => f.path!)
          .toList();
      if (paths.isNotEmpty && context.mounted) {
        await _confirmAndProcess(context, paths);
      }
    }
  }
}

class _NoLibraryState extends ConsumerWidget {
  const _NoLibraryState();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: [
          EmptyState(
            icon: Icons.folder_open_outlined,
            title: 'Where should your library live?',
            subtitle:
                'Pick a folder and we\'ll create the structure and organize everything for you',
            actionLabel: 'Pick Location',
            onAction: () => _pickLibraryFolder(ref),
          ),
          const SizedBox(height: 32),
          DropZone(
            onDrop: (paths) {
              ref.read(toastProvider.notifier).show(
                    'Set up a library folder first',
                    type: ToastType.info,
                  );
            },
            onBrowse: () {
              ref.read(toastProvider.notifier).show(
                    'Set up a library folder first',
                    type: ToastType.info,
                  );
            },
          ),
        ],
      ),
    );
  }

  Future<void> _pickLibraryFolder(WidgetRef ref) async {
    final result = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Choose where to create your Reel library',
    );
    if (result != null) {
      try {
        final libraryRoot = await config_api.ensureLibraryRoot(parent: result);
        ref.read(configProvider.notifier).updateConfig(
          (cfg) => copyConfig(cfg, libraryPath: libraryRoot),
        );
      } catch (e) {
        ref.read(toastProvider.notifier).show(
          'Failed to create library: $e',
          type: ToastType.error,
        );
      }
    }
  }
}


class _LibraryPathRow extends StatelessWidget {
  final String path;
  const _LibraryPathRow({required this.path});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: () => pipeline_api.revealInFinder(path: path).catchError((e) {
          debugPrint('[formats] Failed to reveal file: $e');
        }),
        child: Row(
          children: [
            const Icon(Icons.folder_open_outlined, size: 12, color: AppColors.textQuaternary),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                path,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textQuaternary,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _LoadingSkeleton extends StatelessWidget {
  const _LoadingSkeleton();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        children: [
          Row(
            children: List.generate(
              4,
              (i) => Padding(
                padding: const EdgeInsets.only(right: 12),
                child: SkeletonBox(width: 120, height: 180, borderRadius: 8),
              ),
            ),
          ),
          const SizedBox(height: 24),
          Wrap(
            spacing: 12,
            runSpacing: 12,
            children: List.generate(
              4,
              (i) => const SkeletonBox(width: 200, height: 130, borderRadius: 12),
            ),
          ),
        ],
      ),
    );
  }
}
