/// Config FFI API -- load and save the YAML config.

use crate::config;

/// Load config from disk (or return defaults if no file exists).
pub fn load_config() -> Result<config::Config, String> {
    config::load_config()
}

/// Save config to disk.
pub fn save_config(cfg: config::Config) -> Result<(), String> {
    config::save_config(&cfg)
}
