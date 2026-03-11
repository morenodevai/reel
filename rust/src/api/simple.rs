#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Initialize file+stderr logging before FRB utilities.
    // FRB does not set up a logger on desktop platforms; we do it here.
    if let Err(e) = crate::logging::init() {
        eprintln!("Failed to initialize logger: {e}");
    }
    flutter_rust_bridge::setup_default_user_utils();
}
