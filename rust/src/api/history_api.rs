/// History/Transaction FFI API -- paginated history, undo operations.

use crate::transaction;

/// Get paginated transaction history.
pub fn get_history(offset: u32, limit: u32) -> Result<transaction::HistoryPage, String> {
    Ok(transaction::get_history(offset, limit)?)
}

/// Undo a single transaction (move file back to original location).
pub fn undo_transaction(id: String) -> Result<transaction::UndoResult, String> {
    Ok(transaction::undo_transaction(&id)?)
}

/// Undo an entire batch of transactions.
pub fn undo_batch(batch_id: String) -> Result<transaction::UndoBatchResult, String> {
    Ok(transaction::undo_batch(&batch_id)?)
}
