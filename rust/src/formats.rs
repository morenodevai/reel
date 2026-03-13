pub struct FormatDefinition {
    pub name: &'static str,
    pub genres: &'static [&'static str],
    pub auto_create: bool,
}

pub const FORMATS: &[FormatDefinition] = &[
    FormatDefinition {
        name: "Movies",
        genres: &[
            "Action",
            "Adventure",
            "Comedy",
            "Crime",
            "Documentary",
            "Drama",
            "Family",
            "Fantasy",
            "History",
            "Horror",
            "Music",
            "Mystery",
            "Romance",
            "Sci-Fi",
            "Thriller",
            "War",
            "Western",
        ],
        auto_create: false,
    },
    FormatDefinition {
        name: "Shows",
        genres: &[
            "Action & Adventure",
            "Comedy",
            "Crime",
            "Documentary",
            "Drama",
            "Family",
            "Kids",
            "Mystery",
            "Reality",
            "Sci-Fi & Fantasy",
            "War & Politics",
            "Western",
        ],
        auto_create: false,
    },
    FormatDefinition {
        name: "Anime",
        genres: &[
            "Action",
            "Adventure",
            "Comedy",
            "Drama",
            "Fantasy",
            "Horror",
            "Isekai",
            "Mecha",
            "Music",
            "Mystery",
            "Psychological",
            "Romance",
            "School",
            "Sci-Fi",
            "Slice of Life",
            "Sports",
            "Supernatural",
            "Thriller",
        ],
        auto_create: true,
    },
    FormatDefinition {
        name: "Anime Movies",
        genres: &[
            "Action",
            "Adventure",
            "Comedy",
            "Drama",
            "Fantasy",
            "Mecha",
            "Mystery",
            "Psychological",
            "Romance",
            "Sci-Fi",
            "Slice of Life",
            "Supernatural",
        ],
        auto_create: true,
    },
    FormatDefinition {
        name: "Animated Movies",
        genres: &[
            "Family",
            "Adventure",
            "Comedy",
            "Fantasy",
            "Action",
            "Musical",
            "Sci-Fi",
        ],
        auto_create: true,
    },
    FormatDefinition {
        name: "Animated Shows",
        genres: &[
            "Family",
            "Kids",
            "Comedy",
            "Fantasy",
            "Action & Adventure",
            "Sci-Fi & Fantasy",
        ],
        auto_create: true,
    },
    FormatDefinition {
        name: "Documentary",
        genres: &[
            "Biographical",
            "Crime",
            "Historical",
            "Nature",
            "Political",
            "Science",
            "Social",
            "Sports",
            "True Crime",
        ],
        auto_create: true,
    },
    FormatDefinition {
        name: "Needs Review",
        genres: &["Pending"],
        auto_create: true,
    },
];

/// Map a TMDb genre ID to a genre name appropriate for the given format.
pub fn map_tmdb_genre(tmdb_genre_id: u32, format: &str) -> Option<&'static str> {
    // Find the format definition
    let format_def = FORMATS.iter().find(|f| f.name == format)?;

    // Map TMDb genre IDs to names, then check if the format supports that genre
    let genre_name = match tmdb_genre_id {
        // Movie genre IDs
        28 => {
            if format_def.genres.contains(&"Action") {
                "Action"
            } else if format_def.genres.contains(&"Action & Adventure") {
                "Action & Adventure"
            } else {
                return None;
            }
        }
        12 => "Adventure",
        16 => return None, // Animation - handled at format level, not genre
        35 => "Comedy",
        80 => "Crime",
        99 => "Documentary",
        18 => "Drama",
        10751 => "Family",
        14 => "Fantasy",
        36 => {
            if format_def.genres.contains(&"History") {
                "History"
            } else if format_def.genres.contains(&"Historical") {
                "Historical"
            } else {
                return None;
            }
        }
        27 => "Horror",
        10402 => "Music",
        9648 => "Mystery",
        10749 => "Romance",
        878 => "Sci-Fi",
        53 => "Thriller",
        10752 => "War",
        37 => "Western",
        // TV genre IDs
        10759 => "Action & Adventure",
        10762 => "Kids",
        10764 => "Reality",
        10765 => "Sci-Fi & Fantasy",
        10768 => "War & Politics",
        _ => return None,
    };

    if format_def.genres.contains(&genre_name) {
        Some(genre_name)
    } else {
        None
    }
}

/// Get the default formats that should always be shown (even if empty).
pub fn default_format_names() -> Vec<&'static str> {
    FORMATS
        .iter()
        .filter(|f| !f.auto_create)
        .map(|f| f.name)
        .collect()
}

/// Check if a format name is valid.
pub fn is_valid_format(name: &str) -> bool {
    FORMATS.iter().any(|f| f.name == name)
}

/// Serializable format option for the review UI's dropdowns.
#[derive(Debug, serde::Serialize)]
pub struct FormatOption {
    pub name: String,
    pub genres: Vec<String>,
}

/// Get all format names and their genres for the edit UI.
pub fn get_format_options() -> Vec<FormatOption> {
    FORMATS
        .iter()
        .map(|f| FormatOption {
            name: f.name.to_string(),
            genres: f.genres.iter().map(|g| g.to_string()).collect(),
        })
        .collect()
}
