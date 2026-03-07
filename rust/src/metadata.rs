// Thin re-export layer for FFI compatibility.
// frb_generated.rs references crate::metadata::TmdbMatch paths extensively.
// pipeline.rs and classifier.rs use metadata::* imports.
// Actual implementation lives in crate::identify::{tmdb, poster}.

pub use crate::identify::poster::get_poster_by_tmdb_id;
pub use crate::identify::tmdb::*;
