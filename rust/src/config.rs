use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub library_path: Option<String>,
    #[serde(default)]
    pub watch_path: Option<String>,
    #[serde(default = "default_tmdb_key")]
    pub tmdb_api_key: String,
    #[serde(default = "default_opensubs_key")]
    pub opensubs_api_key: String,
    #[serde(default)]
    pub tvdb_api_key: String,
    #[serde(default = "default_subtitle_languages")]
    pub subtitle_languages: Vec<String>,
    #[serde(default = "default_true")]
    pub auto_download_subs: bool,
    #[serde(default)]
    pub qbittorrent: QbitConfig,
    #[serde(default = "default_true")]
    pub qbit_enabled: bool,
    #[serde(default = "default_true")]
    pub watcher_enabled: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QbitConfig {
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_qbit_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub auto_remove: bool,
}

impl Default for QbitConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 8080,
            username: String::new(),
            password: String::new(),
            auto_remove: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            library_path: None,
            watch_path: None,
            tmdb_api_key: default_tmdb_key(),
            opensubs_api_key: default_opensubs_key(),
            tvdb_api_key: String::new(),
            subtitle_languages: default_subtitle_languages(),
            auto_download_subs: true,
            qbittorrent: QbitConfig::default(),
            qbit_enabled: true,
            watcher_enabled: true,
            theme: "dark".to_string(),
        }
    }
}

fn default_tmdb_key() -> String {
    String::new()
}
fn default_opensubs_key() -> String {
    String::new()
}
fn default_subtitle_languages() -> Vec<String> {
    vec!["eng".to_string()]
}
fn default_true() -> bool {
    true
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_qbit_port() -> u16 {
    8080
}

pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        // %APPDATA%\Reel → C:\Users\Name\AppData\Roaming\Reel
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Reel")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".media-sort")
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.yaml")
}

pub fn db_path() -> PathBuf {
    config_dir().join("transactions.db")
}

pub fn load_config() -> Result<Config, String> {
    // On Windows, migrate from old ~/.media-sort to %APPDATA%\Reel if needed
    if cfg!(windows) {
        migrate_windows_config();
    }

    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {}", e))?;
    serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

/// One-time migration: copy config.yaml and transactions.db from ~/.media-sort
/// to %APPDATA%\Reel on Windows, if the new dir doesn't exist but the old one does.
fn migrate_windows_config() {
    let new_dir = config_dir();
    if new_dir.exists() {
        return; // Already migrated or fresh install
    }

    let old_dir = match dirs::home_dir() {
        Some(h) => h.join(".media-sort"),
        None => return,
    };
    if !old_dir.exists() {
        return; // Nothing to migrate
    }

    // Create new config directory
    if fs::create_dir_all(&new_dir).is_err() {
        return;
    }

    // Copy config.yaml
    let old_config = old_dir.join("config.yaml");
    if old_config.exists() {
        if let Err(e) = fs::copy(&old_config, new_dir.join("config.yaml")) {
            eprintln!("Migration warning: failed to copy config.yaml: {}", e);
        }
    }

    // Copy transactions.db
    let old_db = old_dir.join("transactions.db");
    if old_db.exists() {
        if let Err(e) = fs::copy(&old_db, new_dir.join("transactions.db")) {
            eprintln!("Migration warning: failed to copy transactions.db: {}", e);
        }
    }

    // Copy processed_torrents.txt
    let old_torrents = old_dir.join("processed_torrents.txt");
    if old_torrents.exists() {
        if let Err(e) = fs::copy(&old_torrents, new_dir.join("processed_torrents.txt")) {
            eprintln!("Migration warning: failed to copy processed_torrents.txt: {}", e);
        }
    }
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let content =
        serde_yaml::to_string(config).map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&path, &content).map_err(|e| format!("Failed to write config: {}", e))?;

    // Restrict file permissions to owner-only on Unix (config may contain API keys)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perms)
            .map_err(|e| format!("Failed to set config permissions: {}", e))?;
    }

    Ok(())
}
