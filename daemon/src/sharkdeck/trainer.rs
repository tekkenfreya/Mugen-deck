use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use unrar::Archive;

use super::types::TrainerInfo;

/// Supported archive types for trainer downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveType {
    Rar,
    Zip,
    SevenZ,
    /// Not an archive — assumed to be a raw `.exe`.
    Exe,
}

impl ArchiveType {
    /// File extension for this archive type.
    fn extension(&self) -> &'static str {
        match self {
            Self::Rar => "rar",
            Self::Zip => "zip",
            Self::SevenZ => "7z",
            Self::Exe => "exe",
        }
    }
}

/// Returns the trainer cache directory: `~/.local/share/sharkdeck/cache/trainers/`.
fn cache_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("sharkdeck")
        .join("cache")
        .join("trainers"))
}

/// Sanitizes a trainer name into a safe filename.
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .take(100)
        .collect()
}

/// Detects the archive type from a URL.
///
/// GCW encodes filenames with `!` instead of `.` — e.g. `Trainer!rar`, `Trainer!7z`, `Trainer!zip`.
fn detect_archive_type(url: &str) -> ArchiveType {
    let lower = url.to_lowercase();
    if lower.contains("!rar") || lower.contains(".rar") {
        ArchiveType::Rar
    } else if lower.contains("!7z") || lower.contains(".7z") {
        ArchiveType::SevenZ
    } else if lower.contains("!zip") || lower.contains(".zip") {
        ArchiveType::Zip
    } else {
        ArchiveType::Exe
    }
}

/// Downloads a trainer to the cache directory.
///
/// Returns the local file path of the downloaded trainer.
pub async fn download_trainer(
    client: &reqwest::Client,
    trainer: &TrainerInfo,
    resolved_url: &str,
) -> Result<String> {
    let dir = cache_dir()?;
    fs::create_dir_all(&dir)
        .await
        .context("failed to create trainer cache dir")?;

    let safe_name = sanitize_filename(&trainer.name);

    // Check both the resolved URL and original download URL for archive hints
    let archive_type = match detect_archive_type(resolved_url) {
        ArchiveType::Exe => detect_archive_type(&trainer.download_url),
        t => t,
    };
    let ext = archive_type.extension();

    let file_path = dir.join(format!("{}.{}", safe_name, ext));
    let file_path_str = file_path.to_string_lossy().to_string();

    // Check if already cached
    if file_path.exists() {
        if let Ok(meta) = fs::metadata(&file_path).await {
            if meta.is_file() && meta.len() > 0 {
                // Verify checksum if available
                if let Some(ref expected) = trainer.checksum {
                    if verify_checksum_file(&file_path, expected).await? {
                        debug!(path = %file_path_str, "trainer already cached, checksum valid");
                        return Ok(file_path_str);
                    }
                    // Invalid checksum — re-download
                    fs::remove_file(&file_path).await.ok();
                } else {
                    debug!(path = %file_path_str, "trainer already cached");
                    return Ok(file_path_str);
                }
            }
        }
    }

    info!(url = %resolved_url, path = %file_path_str, "downloading trainer");

    let response = client
        .get(resolved_url)
        .send()
        .await
        .context("failed to download trainer")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "download failed: {} {}",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("unknown")
        );
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read trainer download body")?;

    // Verify the response isn't an HTML error page (Cloudflare challenge, 404, etc.)
    if bytes.len() > 15 {
        let header = &bytes[..15];
        let header_str = String::from_utf8_lossy(header);
        if header_str.contains("<!DOCTYPE") || header_str.contains("<html") || header_str.contains("<HTML") {
            let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
            anyhow::bail!(
                "download returned HTML instead of a trainer file (likely blocked by Cloudflare): {}",
                preview
            );
        }
    }

    fs::write(&file_path, &bytes)
        .await
        .context("failed to write trainer to disk")?;

    // Verify checksum if provided
    if let Some(ref expected) = trainer.checksum {
        if !verify_checksum_file(&file_path, expected).await? {
            fs::remove_file(&file_path).await.ok();
            anyhow::bail!("checksum verification failed");
        }
    }

    info!(path = %file_path_str, size = bytes.len(), "trainer downloaded");
    Ok(file_path_str)
}

/// Verifies the SHA256 checksum of a file.
async fn verify_checksum_file(path: &Path, expected: &str) -> Result<bool> {
    let data = fs::read(path)
        .await
        .context("failed to read file for checksum")?;
    Ok(verify_checksum(&data, expected))
}

/// Verifies the SHA256 checksum of a byte slice.
pub fn verify_checksum(data: &[u8], expected: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hex::encode(hasher.finalize());
    hash == expected.to_lowercase()
}

/// Extracts a `.rar` archive and returns the path to the first `.exe` found inside.
///
/// Uses `unrar` which is a blocking C library, so extraction runs inside `spawn_blocking`.
pub async fn extract_rar(rar_path: &str) -> Result<String> {
    let rar = PathBuf::from(rar_path);
    let extract_dir = rar.with_extension("");

    fs::create_dir_all(&extract_dir)
        .await
        .context("failed to create extraction directory")?;

    let rar_owned = rar_path.to_string();
    let dir_owned = extract_dir.clone();

    let exe_path = tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>> {
        // Verify the file looks like a RAR archive (magic bytes: Rar!\x1a\x07)
        let file_bytes = std::fs::read(&rar_owned)
            .map_err(|e| anyhow::anyhow!("failed to read rar file: {}", e))?;
        if file_bytes.len() < 7 || &file_bytes[..4] != b"Rar!" {
            let preview = String::from_utf8_lossy(&file_bytes[..file_bytes.len().min(100)]);
            anyhow::bail!(
                "file is not a valid RAR archive (size={}, starts with: {})",
                file_bytes.len(),
                preview
            );
        }

        let mut archive = Archive::new(&rar_owned)
            .open_for_processing()
            .map_err(|e| anyhow::anyhow!("failed to open rar archive: {}", e))?;

        let mut first_exe: Option<PathBuf> = None;

        while let Some(header) = archive
            .read_header()
            .map_err(|e| anyhow::anyhow!("failed to read rar header: {}", e))?
        {
            let filename = header.entry().filename.clone();
            let is_file = header.entry().is_file();

            archive = header
                .extract_with_base(&dir_owned)
                .map_err(|e| anyhow::anyhow!("failed to extract rar entry: {}", e))?;

            if is_file && first_exe.is_none() {
                let name_lower = filename.to_string_lossy().to_lowercase();
                if name_lower.ends_with(".exe") {
                    first_exe = Some(dir_owned.join(&filename));
                }
            }
        }

        Ok(first_exe)
    })
    .await
    .context("rar extraction task panicked")?
    .context("rar extraction failed")?;

    match exe_path {
        Some(path) => {
            let result = path.to_string_lossy().to_string();
            info!(exe = %result, "extracted trainer exe from rar");
            Ok(result)
        }
        None => {
            warn!(dir = %extract_dir.display(), "no .exe found in rar archive");
            anyhow::bail!("no .exe file found inside rar archive at {}", rar_path)
        }
    }
}

/// Extracts a `.zip` archive and returns the path to the first `.exe` found inside.
pub async fn extract_zip(zip_path: &str) -> Result<String> {
    let zip_file = PathBuf::from(zip_path);
    let extract_dir = zip_file.with_extension("");

    fs::create_dir_all(&extract_dir)
        .await
        .context("failed to create extraction directory")?;

    let zip_owned = zip_path.to_string();
    let dir_owned = extract_dir.clone();

    let exe_path = tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>> {
        let file = std::fs::File::open(&zip_owned)
            .context("failed to open zip file")?;
        let mut archive = zip::ZipArchive::new(file)
            .context("failed to read zip archive")?;

        let mut first_exe: Option<PathBuf> = None;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| anyhow::anyhow!("failed to read zip entry: {}", e))?;

            let Some(enclosed_name) = entry.enclosed_name() else {
                warn!(name = ?entry.name(), "skipping zip entry with unsafe path");
                continue;
            };
            let out_path = dir_owned.join(&enclosed_name);

            if entry.is_dir() {
                std::fs::create_dir_all(&out_path).ok();
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut outfile = std::fs::File::create(&out_path)
                    .with_context(|| format!("failed to create {}", out_path.display()))?;
                std::io::copy(&mut entry, &mut outfile)
                    .with_context(|| format!("failed to extract {}", out_path.display()))?;

                if first_exe.is_none() {
                    let name_lower = enclosed_name.to_string_lossy().to_lowercase();
                    if name_lower.ends_with(".exe") {
                        first_exe = Some(out_path);
                    }
                }
            }
        }

        Ok(first_exe)
    })
    .await
    .context("zip extraction task panicked")?
    .context("zip extraction failed")?;

    match exe_path {
        Some(path) => {
            let result = path.to_string_lossy().to_string();
            info!(exe = %result, "extracted trainer exe from zip");
            Ok(result)
        }
        None => {
            warn!(dir = %extract_dir.display(), "no .exe found in zip archive");
            anyhow::bail!("no .exe file found inside zip archive at {}", zip_path)
        }
    }
}

/// Extracts a `.7z` archive and returns the path to the first `.exe` found inside.
pub async fn extract_7z(sevenz_path: &str) -> Result<String> {
    let archive_file = PathBuf::from(sevenz_path);
    let extract_dir = archive_file.with_extension("");

    fs::create_dir_all(&extract_dir)
        .await
        .context("failed to create extraction directory")?;

    let sevenz_owned = sevenz_path.to_string();
    let dir_owned = extract_dir.clone();

    let exe_path = tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>> {
        sevenz_rust2::decompress_file(&sevenz_owned, &dir_owned)
            .map_err(|e| anyhow::anyhow!("failed to extract 7z archive: {}", e))?;

        // Walk extracted directory to find the first .exe
        find_first_exe(&dir_owned)
    })
    .await
    .context("7z extraction task panicked")?
    .context("7z extraction failed")?;

    match exe_path {
        Some(path) => {
            let result = path.to_string_lossy().to_string();
            info!(exe = %result, "extracted trainer exe from 7z");
            Ok(result)
        }
        None => {
            warn!(dir = %extract_dir.display(), "no .exe found in 7z archive");
            anyhow::bail!("no .exe file found inside 7z archive at {}", sevenz_path)
        }
    }
}

/// Recursively finds the first `.exe` file in a directory.
fn find_first_exe(dir: &Path) -> Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(dir).context("failed to read extraction directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(Some(exe)) = find_first_exe(&path) {
                return Ok(Some(exe));
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("exe") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Downloads a trainer and extracts it if the download is an archive.
///
/// Supports `.rar`, `.zip`, and `.7z` archives. For each, extracts and returns
/// the path to the first `.exe` found inside. Non-archive downloads are returned
/// directly (assumed to be `.exe`).
///
/// `original_url` is an optional secondary URL to check for archive hints. GCW's
/// resolved URL (mobiletarget.net) has no file extension, but the original
/// dl.gamecopyworld.com URL contains `!rar`/`!7z`/`!zip` in the filename parameter.
pub async fn download_and_extract_trainer(
    client: &reqwest::Client,
    trainer: &TrainerInfo,
    resolved_url: &str,
    original_url: Option<&str>,
) -> Result<String> {
    // Determine archive type from all available URL hints
    let archive_type = match detect_archive_type(resolved_url) {
        ArchiveType::Exe => {
            original_url
                .map(|u| detect_archive_type(u))
                .unwrap_or(ArchiveType::Exe)
        }
        t => t,
    };

    let cached_path = download_trainer(client, trainer, resolved_url).await?;

    match archive_type {
        ArchiveType::Rar => {
            debug!(path = %cached_path, "downloaded file is a rar archive, extracting");
            extract_rar(&cached_path).await
        }
        ArchiveType::Zip => {
            debug!(path = %cached_path, "downloaded file is a zip archive, extracting");
            extract_zip(&cached_path).await
        }
        ArchiveType::SevenZ => {
            debug!(path = %cached_path, "downloaded file is a 7z archive, extracting");
            extract_7z(&cached_path).await
        }
        ArchiveType::Exe => Ok(cached_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(
            sanitize_filename("Elden Ring v1.2 Trainer"),
            "Elden_Ring_v1.2_Trainer"
        );
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(
            sanitize_filename("Game: The (Re)birth!"),
            "Game__The__Re_birth_"
        );
    }

    #[test]
    fn test_sanitize_filename_truncates() {
        let long_name = "A".repeat(200);
        assert_eq!(sanitize_filename(&long_name).len(), 100);
    }

    #[test]
    fn test_sanitize_filename_collapses_whitespace() {
        assert_eq!(sanitize_filename("Game   Name   Here"), "Game_Name_Here");
    }

    #[test]
    fn test_verify_checksum_valid() {
        let data = b"hello world";
        // SHA256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_checksum(data, expected));
    }

    #[test]
    fn test_verify_checksum_invalid() {
        let data = b"hello world";
        let expected = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!verify_checksum(data, expected));
    }

    #[test]
    fn test_verify_checksum_case_insensitive() {
        let data = b"hello world";
        let expected = "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9";
        assert!(verify_checksum(data, expected));
    }

    #[test]
    fn test_detect_rar() {
        assert_eq!(
            detect_archive_type("https://dl.gamecopyworld.com/dl/gcw_Trainer!rar"),
            ArchiveType::Rar
        );
        assert_eq!(
            detect_archive_type("https://example.com/file.rar"),
            ArchiveType::Rar
        );
    }

    #[test]
    fn test_detect_zip() {
        assert_eq!(
            detect_archive_type("https://dl.gamecopyworld.com/dl/gcw_Trainer!zip"),
            ArchiveType::Zip
        );
        assert_eq!(
            detect_archive_type("https://example.com/file.zip"),
            ArchiveType::Zip
        );
    }

    #[test]
    fn test_detect_7z() {
        assert_eq!(
            detect_archive_type("https://dl.gamecopyworld.com/dl/gcw_Trainer!7z"),
            ArchiveType::SevenZ
        );
        assert_eq!(
            detect_archive_type("https://example.com/file.7z"),
            ArchiveType::SevenZ
        );
    }

    #[test]
    fn test_detect_exe() {
        assert_eq!(
            detect_archive_type("https://flingtrainer.com/download/trainer.exe"),
            ArchiveType::Exe
        );
        assert_eq!(
            detect_archive_type("https://g06.mobiletarget.net/?y=abc&x=def"),
            ArchiveType::Exe
        );
    }

    #[test]
    fn test_detect_case_insensitive() {
        assert_eq!(
            detect_archive_type("https://example.com/file.RAR"),
            ArchiveType::Rar
        );
        assert_eq!(
            detect_archive_type("https://example.com/file.ZIP"),
            ArchiveType::Zip
        );
        assert_eq!(
            detect_archive_type("https://example.com/file.7Z"),
            ArchiveType::SevenZ
        );
    }
}
