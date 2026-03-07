/// Watcher FFI API -- start/stop filesystem watcher for auto-import.
///
/// The watcher monitors a drop folder and auto-organizes new video files.
/// Events are sent back to Dart via StreamSink.

use crate::config;
use crate::frb_generated::StreamSink;
use crate::watcher;

/// Start the filesystem watcher. Events are emitted as JSON strings via StreamSink.
pub fn start_watcher(
    sink: StreamSink<String>,
) -> Result<(), String> {
    let cfg = config::load_config()?;
    let watch = cfg
        .watch_path
        .clone()
        .ok_or("No watch folder configured -- set one in Settings")?;
    watcher::start_watcher(&watch, cfg, move |event, payload| {
        let msg = format!("{}:{}", event, payload);
        let _ = sink.add(msg);
    })
}

/// Stop the filesystem watcher.
pub fn stop_watcher() -> Result<(), String> {
    watcher::stop_watcher()
}

/// Check if the watcher is currently running.
pub fn is_watcher_running() -> bool {
    watcher::is_watcher_running()
}
