/// Library browsing FFI API -- read library structure from disk.

use crate::pipeline;

/// Get top-level format folders (Movies, Shows, Anime, etc.) from the library.
pub fn get_library_contents(library_path: String) -> Result<Vec<pipeline::FormatInfo>, String> {
    pipeline::get_library_contents(&library_path)
}

/// Get genre folders within a format folder.
pub fn get_format_contents(format_path: String) -> Result<Vec<pipeline::GenreInfo>, String> {
    pipeline::get_format_contents(&format_path)
}

/// Get paginated media items within a genre folder.
pub fn get_genre_contents(
    genre_path: String,
    offset: u32,
    limit: u32,
) -> Result<pipeline::GenrePage, String> {
    pipeline::get_genre_contents(&genre_path, offset, limit)
}

/// Get detailed info about a single media item (movie or show folder).
pub fn get_media_details(media_path: String) -> Result<pipeline::MediaDetail, String> {
    pipeline::get_media_details(&media_path)
}

/// Get recently added media from the library.
pub fn get_recently_added(library_path: String, limit: u32) -> Result<Vec<pipeline::MediaInfo>, String> {
    pipeline::get_recently_added(&library_path, limit)
}
