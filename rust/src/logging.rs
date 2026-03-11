/// File-based logging — writes to config_dir()/reel.log + stderr.
///
/// Uses `chrono::Local` for local-time timestamps (env_logger's built-in
/// timestamps are UTC only, which is confusing in a desktop app's log file).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024; // 5 MB

struct TeeWriter {
    file: File,
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // File write is best-effort — never block stderr
        let _ = self.file.write_all(buf);
        io::stderr().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = self.file.flush();
        io::stderr().flush()
    }
}

/// Initialize logging to file + stderr.
/// Must be called before `flutter_rust_bridge::setup_default_user_utils()`.
/// FRB does not set up a logger on desktop platforms; we do it here.
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let dir = crate::config::config_dir();
    fs::create_dir_all(&dir)?;

    let log_path = dir.join("reel.log");
    let old_path = dir.join("reel.log.old");

    // Rotate if current log exceeds 5MB — keeps one generation of history
    if log_path.exists() {
        if let Ok(meta) = fs::metadata(&log_path) {
            if meta.len() > MAX_LOG_SIZE {
                let _ = fs::rename(&log_path, &old_path);
            }
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let tee = TeeWriter { file };

    env_logger::Builder::new()
        .filter_module("rust_lib_reel", log::LevelFilter::Info)
        .filter_level(log::LevelFilter::Warn)
        .write_style(env_logger::WriteStyle::Never)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}:{} {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args(),
            )
        })
        .target(env_logger::Target::Pipe(Box::new(tee)))
        .try_init()?;

    Ok(())
}
