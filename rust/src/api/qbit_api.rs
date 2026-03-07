/// qBittorrent FFI API -- test connection, auto-detect, poll for completed torrents.

use crate::config;
use crate::frb_generated::StreamSink;
use crate::qbittorrent;

/// Test qBittorrent connection with the given config.
pub fn test_qbittorrent(qbit_config: config::QbitConfig) -> Result<qbittorrent::QbitTestResult, String> {
    qbittorrent::test_connection(&qbit_config)
}

/// Auto-detect qBittorrent on common ports.
pub fn auto_detect_qbittorrent() -> Result<config::QbitConfig, String> {
    qbittorrent::auto_detect()
}

/// Get list of completed torrents.
pub fn get_completed_torrents() -> Result<Vec<qbittorrent::TorrentInfo>, String> {
    let cfg = config::load_config()?;
    qbittorrent::get_completed_torrents(&cfg.qbittorrent)
}

/// Start the qBittorrent poller. Events emitted as JSON strings via StreamSink.
pub fn start_qbit_poller(
    sink: StreamSink<String>,
) -> Result<(), String> {
    let cfg = config::load_config()?;
    qbittorrent::start_poller(cfg, move |event, payload| {
        let msg = format!("{}:{}", event, payload);
        let _ = sink.add(msg);
    })
}

/// Stop the qBittorrent poller.
pub fn stop_qbit_poller() -> Result<(), String> {
    qbittorrent::stop_poller()
}

/// Check if the qBittorrent poller is running.
pub fn is_qbit_poller_running() -> bool {
    qbittorrent::is_poller_running()
}
