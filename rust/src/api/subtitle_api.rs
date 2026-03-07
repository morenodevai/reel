/// Subtitle FFI API -- search and download subtitles for a media file.

use crate::config;

/// Search for subtitles for the given media path and download them.
/// Returns a status message.
pub fn search_subtitles(path: String) -> Result<String, String> {
    let cfg = config::load_config()?;
    crate::subtitles::search_and_download(
        std::path::Path::new(&path),
        &cfg.subtitle_languages,
        &cfg.opensubs_api_key,
    )
}
