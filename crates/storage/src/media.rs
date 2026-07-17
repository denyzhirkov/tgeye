//! Safe on-disk layout for downloaded attachments.
//!
//! Physical path is `media/<category>/<sha256>.<ext>` where every component is
//! derived from a controlled source: category is a fixed enum, sha256 is hex, and
//! the extension comes from a whitelist keyed on MIME/kind — never from the
//! Telegram-supplied filename. Path traversal is therefore impossible by
//! construction (spec §18.2).

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::StorageError;

/// Storage subdirectory for a message content kind.
pub fn category_for_kind(kind: &str) -> &'static str {
    match kind {
        "photo" => "photos",
        "video" | "video_note" | "animation" => "video",
        "audio" => "audio",
        "voice" => "voice",
        "document" => "documents",
        _ => "other",
    }
}

/// Whitelisted extension for the stored file. MIME wins; otherwise fall back to a
/// per-kind default. The Telegram filename is deliberately ignored.
pub fn safe_extension(kind: &str, mime_type: Option<&str>) -> &'static str {
    if let Some(mime) = mime_type {
        match mime.split(';').next().unwrap_or("").trim() {
            "image/jpeg" => return "jpg",
            "image/png" => return "png",
            "image/gif" => return "gif",
            "image/webp" => return "webp",
            "video/mp4" => return "mp4",
            "video/webm" => return "webm",
            "video/quicktime" => return "mov",
            "audio/mpeg" => return "mp3",
            "audio/ogg" => return "ogg",
            "audio/mp4" | "audio/m4a" => return "m4a",
            "audio/wav" | "audio/x-wav" => return "wav",
            "application/pdf" => return "pdf",
            "application/zip" => return "zip",
            "text/plain" => return "txt",
            "application/json" => return "json",
            _ => {}
        }
    }
    match kind {
        "photo" => "jpg",
        "video" | "video_note" | "animation" => "mp4",
        "voice" => "ogg",
        "audio" => "mp3",
        _ => "bin",
    }
}

#[derive(Debug, Clone)]
pub struct StoredFile {
    pub sha256: String,
    pub byte_size: i64,
    pub extension: String,
    pub category: String,
    pub absolute_path: PathBuf,
    /// Path relative to the media root, for a stable resource identifier.
    pub relative_path: String,
    /// False when an identical file already existed (content dedup).
    pub newly_written: bool,
}

/// Persist bytes as `<media_root>/<category>/<sha256>.<ext>` atomically, deduping
/// by content. `bytes` is the fully-downloaded payload (size already checked).
pub fn store_bytes(
    media_root: &Path,
    category: &str,
    extension: &str,
    sha256: &str,
    bytes: &[u8],
) -> Result<StoredFile, StorageError> {
    let dir = media_root.join(category);
    std::fs::create_dir_all(&dir).map_err(io_err)?;
    let file_name = format!("{sha256}.{extension}");
    let final_path = dir.join(&file_name);

    let newly_written = if final_path.exists() {
        false
    } else {
        // Temp file in the same dir → rename is atomic and within the media root.
        let tmp_path = dir.join(format!(".{sha256}.{extension}.part"));
        let mut tmp = std::fs::File::create(&tmp_path).map_err(io_err)?;
        tmp.write_all(bytes).map_err(io_err)?;
        tmp.sync_all().map_err(io_err)?;
        std::fs::rename(&tmp_path, &final_path).map_err(io_err)?;
        true
    };

    Ok(StoredFile {
        sha256: sha256.to_owned(),
        byte_size: bytes.len() as i64,
        extension: extension.to_owned(),
        category: category.to_owned(),
        absolute_path: final_path,
        relative_path: format!("{category}/{file_name}"),
        newly_written,
    })
}

fn io_err(source: std::io::Error) -> StorageError {
    StorageError::Io(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_mapping() {
        assert_eq!(category_for_kind("photo"), "photos");
        assert_eq!(category_for_kind("voice"), "voice");
        assert_eq!(category_for_kind("sticker"), "other");
    }

    #[test]
    fn extension_prefers_mime_then_kind() {
        assert_eq!(safe_extension("document", Some("application/pdf")), "pdf");
        assert_eq!(safe_extension("photo", None), "jpg");
        assert_eq!(
            safe_extension("document", Some("application/x-evil")),
            "bin"
        );
        assert_eq!(
            safe_extension("document", Some("image/png; charset=x")),
            "png"
        );
    }

    #[test]
    fn store_and_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let first = store_bytes(dir.path(), "documents", "pdf", "abc123", b"hello").unwrap();
        assert!(first.newly_written);
        assert!(first.absolute_path.exists());
        assert_eq!(first.relative_path, "documents/abc123.pdf");

        let second = store_bytes(dir.path(), "documents", "pdf", "abc123", b"hello").unwrap();
        assert!(!second.newly_written, "identical content must dedup");
    }
}
