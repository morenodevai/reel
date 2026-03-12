import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/db/watch_progress.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/utils/format_bytes.dart';

// ---------------------------------------------------------------------------
// Hero Section
// ---------------------------------------------------------------------------

class HeroSection extends StatelessWidget {
  final MediaDetail detail;
  final bool isTV;
  final VoidCallback onPlay;
  final VoidCallback onReveal;
  final VoidCallback onDownloadSubs;
  final bool downloadingSubs;
  final VoidCallback? onRestartSeason;
  final VoidCallback? onRestartShow;
  final VoidCallback? onEdit;

  const HeroSection({
    super.key,
    required this.detail,
    required this.isTV,
    required this.onPlay,
    required this.onReveal,
    required this.onDownloadSubs,
    required this.downloadingSubs,
    this.onRestartSeason,
    this.onRestartShow,
    this.onEdit,
  });

  @override
  Widget build(BuildContext context) {
    final gradient = AppColors.formatGradients[detail.format] ??
        AppColors.formatGradientDefault;

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Poster
        ClipRRect(
          borderRadius: BorderRadius.circular(12),
          child: SizedBox(
            width: 180,
            height: 270,
            child: detail.posterUrl != null
                ? CachedNetworkImage(
                    imageUrl: detail.posterUrl!,
                    fit: BoxFit.cover,
                    placeholder: (_, __) => Container(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: gradient,
                        ),
                      ),
                    ),
                    errorWidget: (_, __, ___) => PosterFallback(
                      title: detail.title,
                      gradient: gradient,
                    ),
                  )
                : PosterFallback(
                    title: detail.title,
                    gradient: gradient,
                  ),
          ),
        ),

        const SizedBox(width: 24),

        // Info column
        Expanded(
          child: Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // Title
                Text(
                  detail.title,
                  style: const TextStyle(
                    fontSize: 24,
                    fontWeight: FontWeight.w700,
                    color: AppColors.textPrimary,
                    letterSpacing: -0.3,
                  ),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),

                const SizedBox(height: 6),

                // Year / Format / Genre
                Row(
                  children: [
                    if (detail.year != null) ...[
                      Text(
                        '${detail.year}',
                        style: const TextStyle(
                          fontSize: 14,
                          color: AppColors.textTertiary,
                        ),
                      ),
                      const DotSeparator(),
                    ],
                    Text(
                      detail.format,
                      style: const TextStyle(
                        fontSize: 14,
                        color: AppColors.textTertiary,
                      ),
                    ),
                    const DotSeparator(),
                    Text(
                      detail.genre,
                      style: const TextStyle(
                        fontSize: 14,
                        color: AppColors.textTertiary,
                      ),
                    ),
                  ],
                ),

                const SizedBox(height: 4),

                // Season/episode count or file size
                if (isTV)
                  Text(
                    '${detail.seasonCount} Season${detail.seasonCount != 1 ? 's' : ''} '
                    '- ${detail.episodeCount} Episode${detail.episodeCount != 1 ? 's' : ''}',
                    style: const TextStyle(
                      fontSize: 13,
                      color: AppColors.textTertiary,
                    ),
                  ),
                if (!isTV && detail.files.isNotEmpty)
                  Text(
                    formatBytes(detail.files.first.sizeBytes),
                    style: const TextStyle(
                      fontSize: 13,
                      color: AppColors.textTertiary,
                    ),
                  ),

                const SizedBox(height: 20),

                // Action buttons
                Wrap(
                  spacing: 10,
                  runSpacing: 10,
                  children: [
                    DetailActionButton(
                      icon: Icons.play_arrow_rounded,
                      label: 'Play',
                      primary: true,
                      onTap: onPlay,
                    ),
                    DetailActionButton(
                      icon: Icons.folder_open_outlined,
                      label: 'Reveal in Explorer',
                      onTap: onReveal,
                    ),
                    DetailActionButton(
                      icon: Icons.subtitles_outlined,
                      label: downloadingSubs ? 'Downloading...' : 'Get Subtitles',
                      onTap: downloadingSubs ? null : onDownloadSubs,
                    ),
                    if (onEdit != null)
                      DetailActionButton(
                        icon: Icons.edit_outlined,
                        label: 'Fix ID',
                        onTap: onEdit,
                      ),
                    if (onRestartSeason != null)
                      DetailActionButton(
                        icon: Icons.replay_rounded,
                        label: 'Restart Season',
                        onTap: onRestartSeason,
                      ),
                    if (onRestartShow != null)
                      DetailActionButton(
                        icon: Icons.restart_alt_rounded,
                        label: 'Restart Show',
                        onTap: onRestartShow,
                      ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// Season Selector
// ---------------------------------------------------------------------------

class SeasonSelector extends StatelessWidget {
  final List<int> seasons;
  final int activeSeason;
  final MediaDetail detail;
  final bool isOpen;
  final VoidCallback onToggle;
  final void Function(int season) onSelect;

  const SeasonSelector({
    super.key,
    required this.seasons,
    required this.activeSeason,
    required this.detail,
    required this.isOpen,
    required this.onToggle,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    final seasonEpCount =
        detail.files.where((f) => f.season == activeSeason).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            MouseRegion(
              cursor: SystemMouseCursors.click,
              child: GestureDetector(
                onTap: onToggle,
                child: Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                  decoration: BoxDecoration(
                    color: AppColors.surface,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        'Season $activeSeason',
                        style: const TextStyle(
                          fontSize: 14,
                          fontWeight: FontWeight.w500,
                          color: AppColors.textPrimary,
                        ),
                      ),
                      const SizedBox(width: 4),
                      Text(
                        '($seasonEpCount)',
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppColors.textQuaternary,
                        ),
                      ),
                      const SizedBox(width: 6),
                      AnimatedRotation(
                        turns: isOpen ? 0.5 : 0,
                        duration: const Duration(milliseconds: 200),
                        child: const Icon(
                          Icons.expand_more,
                          size: 16,
                          color: AppColors.textTertiary,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),

        // Dropdown items
        if (isOpen) ...[
          const SizedBox(height: 4),
          Container(
            constraints: const BoxConstraints(maxWidth: 200),
            decoration: BoxDecoration(
              color: AppColors.surfaceElevated,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppColors.border),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.3),
                  blurRadius: 12,
                  offset: const Offset(0, 4),
                ),
              ],
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: seasons.map((s) {
                final sCount =
                    detail.files.where((f) => f.season == s).length;
                final isActive = s == activeSeason;
                return MouseRegion(
                  cursor: SystemMouseCursors.click,
                  child: GestureDetector(
                    onTap: () => onSelect(s),
                    child: Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 10,
                      ),
                      decoration: BoxDecoration(
                        color: isActive
                            ? AppColors.primary.withValues(alpha: 0.1)
                            : Colors.transparent,
                      ),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            'Season $s',
                            style: TextStyle(
                              fontSize: 13,
                              color: isActive
                                  ? AppColors.primary
                                  : AppColors.textPrimary,
                            ),
                          ),
                          Text(
                            '$sCount ep',
                            style: const TextStyle(
                              fontSize: 11,
                              color: AppColors.textQuaternary,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
        ],
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// Episode Row
// ---------------------------------------------------------------------------

class EpisodeRow extends StatefulWidget {
  final MediaFile file;
  final bool isTV;
  final String showTitle;
  final WatchProgress? progress;
  final VoidCallback onPlay;

  const EpisodeRow({
    super.key,
    required this.file,
    required this.isTV,
    required this.showTitle,
    this.progress,
    required this.onPlay,
  });

  @override
  State<EpisodeRow> createState() => _EpisodeRowState();
}

class _EpisodeRowState extends State<EpisodeRow> {
  bool _hovered = false;

  String get _label {
    if (widget.isTV &&
        widget.file.season != null &&
        widget.file.episode != null) {
      final ep =
          'S${widget.file.season.toString().padLeft(2, '0')}E${widget.file.episode.toString().padLeft(2, '0')}';
      if (widget.file.episodeTitle != null) {
        return '$ep - ${widget.file.episodeTitle}';
      }
      return ep;
    }
    return widget.file.filename;
  }

  @override
  Widget build(BuildContext context) {
    final wp = widget.progress;
    final isCompleted = wp != null && wp.completed;
    final isInProgress = wp != null && !wp.completed && wp.positionSeconds > 5.0;
    final progressFraction = (wp != null && wp.durationSeconds > 0)
        ? (wp.positionSeconds / wp.durationSeconds).clamp(0.0, 1.0)
        : 0.0;

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onPlay,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          margin: const EdgeInsets.only(bottom: 2),
          decoration: BoxDecoration(
            color: _hovered ? AppColors.surface : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                child: Row(
                  children: [
                    // Episode number, checkmark, or play icon
                    SizedBox(
                      width: 32,
                      height: 32,
                      child: Center(
                        child: _hovered
                            ? const Icon(
                                Icons.play_arrow_rounded,
                                size: 20,
                                color: Colors.white,
                              )
                            : isCompleted
                                ? const Icon(
                                    Icons.check_circle_rounded,
                                    size: 18,
                                    color: AppColors.success,
                                  )
                                : widget.isTV && widget.file.episode != null
                                    ? Text(
                                        '${widget.file.episode}',
                                        style: TextStyle(
                                          fontSize: 14,
                                          fontWeight: FontWeight.w500,
                                          color: isInProgress
                                              ? AppColors.primary
                                              : AppColors.textTertiary,
                                        ),
                                      )
                                    : const Icon(
                                        Icons.play_arrow_rounded,
                                        size: 18,
                                        color: AppColors.textQuaternary,
                                      ),
                      ),
                    ),

                    const SizedBox(width: 12),

                    // Title
                    Expanded(
                      child: Text(
                        _label,
                        style: TextStyle(
                          fontSize: 14,
                          color: isCompleted
                              ? AppColors.textTertiary
                              : AppColors.textPrimary,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),

                    // Subtitle badge
                    if (widget.file.hasSubtitles) ...[
                      const SizedBox(width: 8),
                      const Icon(
                        Icons.subtitles_outlined,
                        size: 14,
                        color: AppColors.textQuaternary,
                      ),
                    ],

                    // File size
                    const SizedBox(width: 12),
                    Text(
                      formatBytes(widget.file.sizeBytes),
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppColors.textQuaternary,
                        fontFeatures: [FontFeature.tabularFigures()],
                      ),
                    ),
                  ],
                ),
              ),

              // Progress bar for in-progress episodes
              if (isInProgress)
                Padding(
                  padding: const EdgeInsets.only(left: 60, right: 16, bottom: 4),
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(2),
                    child: LinearProgressIndicator(
                      value: progressFraction,
                      minHeight: 3,
                      backgroundColor: AppColors.surface,
                      valueColor: const AlwaysStoppedAnimation<Color>(AppColors.primary),
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

// ---------------------------------------------------------------------------
// Shared Widgets
// ---------------------------------------------------------------------------

class PosterFallback extends StatelessWidget {
  final String title;
  final List<Color> gradient;
  const PosterFallback({super.key, required this.title, required this.gradient});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: gradient,
        ),
      ),
      alignment: Alignment.bottomLeft,
      padding: const EdgeInsets.all(16),
      child: Text(
        title,
        style: const TextStyle(
          fontSize: 14,
          fontWeight: FontWeight.w500,
          color: AppColors.textSecondary,
          height: 1.3,
        ),
      ),
    );
  }
}

class DotSeparator extends StatelessWidget {
  const DotSeparator({super.key});

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 6),
      child: Text(
        '\u00B7',
        style: TextStyle(
          fontSize: 14,
          color: AppColors.textQuaternary,
        ),
      ),
    );
  }
}

class DetailActionButton extends StatefulWidget {
  final IconData icon;
  final String label;
  final bool primary;
  final VoidCallback? onTap;

  const DetailActionButton({
    super.key,
    required this.icon,
    required this.label,
    this.primary = false,
    this.onTap,
  });

  @override
  State<DetailActionButton> createState() => _DetailActionButtonState();
}

class _DetailActionButtonState extends State<DetailActionButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final isDisabled = widget.onTap == null;
    final bgColor = widget.primary
        ? (_hovered ? AppColors.primaryHover : AppColors.primary)
        : (_hovered ? AppColors.surfaceHover : AppColors.surface);
    final textColor = widget.primary
        ? Colors.white
        : (isDisabled ? AppColors.textQuaternary : AppColors.textSecondary);

    return MouseRegion(
      cursor: isDisabled ? SystemMouseCursors.basic : SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
          decoration: BoxDecoration(
            color: isDisabled ? bgColor.withValues(alpha: 0.5) : bgColor,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(widget.icon, size: 16, color: textColor),
              const SizedBox(width: 8),
              Text(
                widget.label,
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                  color: textColor,
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
// Loading Skeleton
// ---------------------------------------------------------------------------

class DetailLoadingSkeleton extends StatelessWidget {
  const DetailLoadingSkeleton({super.key});

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SkeletonBox(width: 180, height: 270, borderRadius: 12),
              const SizedBox(width: 24),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const SizedBox(height: 4),
                    SkeletonBox(width: 280, height: 28, borderRadius: 6),
                    const SizedBox(height: 12),
                    SkeletonBox(width: 180, height: 16, borderRadius: 4),
                    const SizedBox(height: 8),
                    SkeletonBox(width: 140, height: 16, borderRadius: 4),
                    const SizedBox(height: 24),
                    Row(
                      children: [
                        SkeletonBox(width: 90, height: 40, borderRadius: 8),
                        const SizedBox(width: 10),
                        SkeletonBox(width: 140, height: 40, borderRadius: 8),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 32),
          ...List.generate(
            6,
            (i) => Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: SkeletonBox(height: 48, borderRadius: 8),
            ),
          ),
        ],
      ),
    );
  }
}
