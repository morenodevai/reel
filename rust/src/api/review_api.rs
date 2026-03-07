/// Review FFI API -- review recently processed files, approve/reject/undo.

use crate::transaction;

/// Get all unlocked (pending review) transactions for the given batch IDs.
pub fn get_review_items(batch_ids: Vec<String>) -> Result<Vec<transaction::Transaction>, String> {
    Ok(transaction::get_review_items(&batch_ids)?)
}

/// Get count of unlocked (pending review) transactions for the given batch IDs.
pub fn get_review_count(batch_ids: Vec<String>) -> Result<u32, String> {
    Ok(transaction::get_review_count(&batch_ids)?)
}

/// Lock (approve) transactions -- they will no longer appear in review.
pub fn lock_transactions(ids: Vec<String>) -> Result<u32, String> {
    Ok(transaction::lock_transactions(&ids)?)
}

/// Undo all pending (unlocked) transactions. Returns (succeeded, failed).
pub fn clear_all_pending() -> Result<ClearResult, String> {
    let (succeeded, failed) = transaction::undo_all_pending()?;
    Ok(ClearResult { succeeded, failed })
}

/// Result of clearing all pending reviews.
#[derive(Debug, Clone)]
pub struct ClearResult {
    pub succeeded: u32,
    pub failed: u32,
}
