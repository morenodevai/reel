/// Shared HTTP client for all API calls (TMDb, OpenSubtitles, etc.).
///
/// Created on a dedicated OS thread to avoid conflicts with tokio runtime —
/// `reqwest::blocking::Client` creates its own internal tokio runtime, which
/// panics if created inside an existing runtime.

use once_cell::sync::Lazy;

static HTTP_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        let _ = tx.send(client);
    });
    rx.recv().expect("Failed to create HTTP client")
});

pub fn client() -> &'static reqwest::blocking::Client {
    &HTTP_CLIENT
}

/// User-Agent header value, built from Cargo.toml version at compile time.
pub const USER_AGENT: &str = concat!("Reel v", env!("CARGO_PKG_VERSION"));
