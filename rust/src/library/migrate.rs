/// Library relocation — move all format folders from one path to another.

use std::fs;
use std::path::Path;

/// Move library contents from one location to another.
/// Copies all top-level directories (format folders) from `from` to `to`,
/// then removes the originals. Creates `to` if it doesn't exist.
pub fn move_library(from: &str, to: &str) -> Result<u32, String> {
    let src = Path::new(from);
    let dst = Path::new(to);

    if !src.exists() {
        return Err("Source library does not exist".to_string());
    }

    fs::create_dir_all(dst).map_err(|e| format!("Failed to create destination: {}", e))?;

    let mut moved = 0u32;
    let entries =
        fs::read_dir(src).map_err(|e| format!("Failed to read source: {}", e))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_dir() {
            if fs::rename(&src_path, &dst_path).is_ok() {
                moved += 1;
            } else {
                copy_dir_recursive(&src_path, &dst_path)?;
                fs::remove_dir_all(&src_path)
                    .map_err(|e| format!("Failed to remove {}: {}", src_path.display(), e))?;
                moved += 1;
            }
        }
    }

    Ok(moved)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create {}: {}", dst.display(), e))?;

    for entry in fs::read_dir(src)
        .map_err(|e| format!("Failed to read {}: {}", src.display(), e))?
        .flatten()
    {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            let src_size = fs::metadata(&src_path)
                .map(|m| m.len())
                .map_err(|e| format!("Failed to read metadata for {}: {}", src_path.display(), e))?;
            let copied = fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy {}: {}", src_path.display(), e))?;
            if copied != src_size {
                let _ = fs::remove_file(&dst_path);
                return Err(format!(
                    "Copy size mismatch for {}: expected {} bytes, got {}",
                    src_path.display(), src_size, copied
                ));
            }
        }
    }

    Ok(())
}
