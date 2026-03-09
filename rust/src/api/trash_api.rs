/// Trash FFI API -- manage companion files sent to the library trash bin.

use crate::companion;

/// Get all items in the trash bin.
pub fn get_trash_items() -> Result<Vec<companion::TrashItem>, String> {
    Ok(companion::get_all_trash()?)
}

/// Restore a single trash item to its original location.
pub fn restore_trash_item(id: String) -> Result<String, String> {
    Ok(companion::restore_trash_item(&id)?)
}

/// Permanently delete a single trash item.
pub fn delete_trash_item(id: String) -> Result<(), String> {
    Ok(companion::delete_trash_item(&id)?)
}

/// Permanently delete all trash items.
pub fn empty_trash() -> Result<u32, String> {
    Ok(companion::empty_trash()?)
}

/// Get the number of items in the trash.
pub fn get_trash_count() -> Result<u32, String> {
    Ok(companion::get_trash_count()?)
}
