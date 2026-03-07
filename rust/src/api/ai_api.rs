/// AI classifier FFI API -- init and query the bundled llama.cpp model.

use crate::ai;

/// Initialize the AI classifier. Call once at startup (in background).
/// Will fail if the model file is not present at the expected path.
pub fn init_ai() -> Result<(), String> {
    ai::init_classifier()
}

/// Check if the AI classifier is ready for use.
pub fn is_ai_ready() -> bool {
    ai::is_classifier_ready()
}
