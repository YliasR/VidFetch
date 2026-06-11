//! Small file helpers that fall outside the fs-plugin scope.

/// Read a dropped text file (URL lists for drag-and-drop). Capped so a
/// stray multi-gigabyte drop can't balloon memory.
#[tauri::command]
pub async fn read_dropped_text(path: String) -> Result<String, String> {
    const MAX_BYTES: u64 = 1024 * 1024;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("cannot stat {path}: {e}"))?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if meta.len() > MAX_BYTES {
        return Err("file too large for a URL list".into());
    }

    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("cannot read {path}: {e}"))
}
