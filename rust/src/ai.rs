//! Local AI classification using bundled llama.cpp.
//!
//! Provides automatic media type classification using a TinyLlama 1.1B model.
//! The model is deployed to the app's config directory on first launch.

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Mutex;

/// Global AI classifier instance
static AI_CLASSIFIER: Lazy<Mutex<Option<LocalClassifier>>> = Lazy::new(|| Mutex::new(None));

/// Classification result
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub media_type: String,
    pub confidence: f32,
}

/// Local AI classifier using llama.cpp
pub struct LocalClassifier {
    #[allow(dead_code)] // RAII: backend must outlive model, stored here to prevent drop
    backend: LlamaBackend,
    model: LlamaModel,
}

impl LocalClassifier {
    /// Initialize the classifier with a model
    pub fn new(model_path: &str) -> Result<Self, String> {
        let backend = LlamaBackend::init()
            .map_err(|e| format!("Failed to init llama backend: {}", e))?;

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| format!("Failed to load model: {}", e))?;

        Ok(Self { backend, model })
    }

    /// Classify a filename
    pub fn classify(&self, filename: &str) -> Result<ClassificationResult, String> {
        let prompt = format!(
            r#"Classify this media filename into one category.
Categories: movie, tv_show, anime, anime_movie

Filename: {}

Reply with ONLY the category name, nothing else."#,
            filename
        );

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(std::num::NonZeroU32::new(512));

        let mut ctx = self.model.new_context(&self.backend, ctx_params)
            .map_err(|e| format!("Failed to create context: {}", e))?;

        // Tokenize the prompt
        let tokens = self.model.str_to_token(&prompt, llama_cpp_2::model::AddBos::Always)
            .map_err(|e| format!("Failed to tokenize: {}", e))?;

        let mut batch = LlamaBatch::new(512, 1);

        // Add tokens to batch
        for (i, token) in tokens.iter().enumerate() {
            batch.add(*token, i as i32, &[0], i == tokens.len() - 1)
                .map_err(|e| format!("Failed to add token: {}", e))?;
        }

        // Decode
        ctx.decode(&mut batch)
            .map_err(|e| format!("Failed to decode: {}", e))?;

        // Sample output
        let mut output = String::new();
        let mut n_cur = batch.n_tokens();
        let mut decoder = encoding_rs::UTF_8.new_decoder();

        for _ in 0..32 {
            // Get token data array and sample from it
            let mut token_data = ctx.token_data_array_ith(n_cur - 1);
            let new_token_id = token_data.sample_token_greedy();

            // Check for end of generation
            if self.model.is_eog_token(new_token_id) {
                break;
            }

            // Convert token to string using token_to_piece
            if let Ok(token_str) = self.model.token_to_piece(new_token_id, &mut decoder, false, None) {
                output.push_str(&token_str);
            }

            batch.clear();
            batch.add(new_token_id, n_cur, &[0], true)
                .map_err(|e| format!("Failed to add token: {}", e))?;

            ctx.decode(&mut batch)
                .map_err(|e| format!("Failed to decode: {}", e))?;

            n_cur += 1;
        }

        // Parse the output and assign confidence based on match quality
        let output_lower = output.trim().to_lowercase();
        let (media_type, confidence) = if output_lower.contains("anime_movie") {
            // Exact category match from the LLM
            ("anime_movie", 0.85)
        } else if output_lower.contains("tv_show") {
            // Exact category match from the LLM
            ("tv_show", 0.85)
        } else if output_lower.contains("anime") {
            // Exact category match from the LLM
            ("anime", 0.85)
        } else if output_lower.contains("movie") {
            // Explicit "movie" mention
            ("movie", 0.70)
        } else if output_lower.contains("tv") || output_lower.contains("episode") || output_lower.contains("series") {
            // Keyword inference -- less certain
            ("tv_show", 0.60)
        } else {
            // Nothing matched -- fallback guess
            ("movie", 0.30)
        };

        Ok(ClassificationResult {
            media_type: media_type.to_string(),
            confidence,
        })
    }
}

/// Model filename for the bundled TinyLlama classifier.
pub const MODEL_FILENAME: &str = "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf";

/// Get the model path in the app's config directory.
pub fn get_model_path() -> Result<PathBuf, String> {
    let model_path = crate::config::config_dir()
        .join("models")
        .join(MODEL_FILENAME);

    if !model_path.exists() {
        let msg = format!(
            "AI model not found at {} — classification will use heuristics only",
            model_path.display()
        );
        log::error!("[ai] {}", msg);
        return Err(msg);
    }

    Ok(model_path)
}

/// Initialize the global classifier
pub fn init_classifier() -> Result<(), String> {
    let model_path = get_model_path()?;
    log::info!("[ai] Loading model from {}", model_path.display());

    let classifier = LocalClassifier::new(model_path.to_str().unwrap())?;

    let mut guard = AI_CLASSIFIER.lock().map_err(|_| "Lock error")?;
    *guard = Some(classifier);

    log::info!("[ai] Classifier ready");
    Ok(())
}

/// Classify a filename using the global classifier
pub fn classify_filename(filename: &str) -> Result<ClassificationResult, String> {
    let guard = AI_CLASSIFIER.lock().map_err(|_| "Lock error")?;

    match &*guard {
        Some(classifier) => classifier.classify(filename),
        None => Err("Classifier not initialized".to_string()),
    }
}

/// Check if the classifier is ready
pub fn is_classifier_ready() -> bool {
    AI_CLASSIFIER.lock().map(|g| g.is_some()).unwrap_or(false)
}
