/// Library browsing — scan library folders, list formats/genres/media, get details.

pub mod migrate;
pub mod rescan;
pub mod scanner;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FormatInfo {
    pub name: String,
    pub path: String,
    pub genre_count: u32,
    pub media_count: u32,
    pub poster_samples: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenreInfo {
    pub name: String,
    pub path: String,
    pub media_count: u32,
    pub media_samples: Vec<MediaInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaInfo {
    pub title: String,
    pub year: Option<u16>,
    pub path: String,
    pub poster_url: Option<String>,
    pub tmdb_id: Option<u64>,
    pub format: String,
    pub genre: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenrePage {
    pub items: Vec<MediaInfo>,
    pub total: u32,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaDetail {
    pub title: String,
    pub year: Option<u16>,
    pub path: String,
    pub poster_url: Option<String>,
    pub tmdb_id: Option<u64>,
    pub format: String,
    pub genre: String,
    pub media_type: String,
    pub files: Vec<MediaFile>,
    pub season_count: u16,
    pub episode_count: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaFile {
    pub path: String,
    pub filename: String,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    pub episode_title: Option<String>,
    pub size_bytes: u64,
    pub has_subtitles: bool,
}

// Public API — delegates to scanner submodule.
pub use scanner::{
    get_format_contents, get_genre_contents, get_library_contents, get_media_details,
    get_recently_added,
};
