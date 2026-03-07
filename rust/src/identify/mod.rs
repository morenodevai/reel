/// Identification module — analyze video files to determine title, year, type, and metadata.

/// Base URL for TMDb poster images (w500 size).
pub(crate) const TMDB_POSTER_BASE: &str = "https://image.tmdb.org/t/p/w500";

pub mod hash;
pub mod poster;
pub mod probe;
pub mod tmdb;
