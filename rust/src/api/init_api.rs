/// Initialization functions called once at app startup.

/// Initialize the database. Must be called before any transaction operations.
pub fn init_db() -> Result<(), String> {
    Ok(crate::transaction::init_db()?)
}

/// Simple ping to verify FFI bridge is alive.
pub fn ping() -> String {
    "reel_core alive".to_string()
}
