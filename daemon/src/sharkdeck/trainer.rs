use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use unrar::Archive;

use super::types::TrainerInfo;

/// Returns the trainer cache directory: `~/.local/share/mugen/cache/trainers/`.
fn cache_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("mugen")
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
    let ext = if url_suggests_rar(resolved_url) {
        "rar"
    } else {
        "exe"
    };
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

/// Returns `true` if the URL suggests the download is a `.rar` archive.
///
/// GCW encodes filenames with `!rar` (URL-encoded `.rar`) or includes `.rar` directly.
fn url_suggests_rar(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("!rar") || lower.contains(".rar")
}

/// Extracts a `.rar` archive and returns the path to the first `.exe` found inside.
///
/// The archive is extracted into a subdirectory next to the `.rar` file (same name
/// without the `.rar` extension). Uses `unrar` which is a blocking C library, so
/// extraction runs inside `spawn_blocking`.
pub async fn extract_rar(rar_path: &str) -> Result<String> {
    let rar = PathBuf::from(rar_path);
    let extract_dir = rar.with_extension(""); // strip .rar to get directory name

    fs::create_dir_all(&extract_dir)
        .await
        .context("failed to create extraction directory")?;

    let rar_owned = rar_path.to_string();
    let dir_owned = extract_dir.clone();

    // unrar is a blocking C library — run in a blocking thread
    let exe_path = tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>> {
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
            // No .exe found — list what we extracted for debugging
            warn!(dir = %extract_dir.display(), "no .exe found in rar archive");
            anyhow::bail!("no .exe file found inside rar archive at {}", rar_path)
        }
    }
}

/// Downloads a trainer and extracts it if the download is a `.rar` archive.
///
/// - If the URL suggests a `.rar` file: downloads, extracts, returns path to the `.exe` inside.
/// - Otherwise: returns the downloaded file path directly (assumed to be an `.exe`).
pub async fn download_and_extract_trainer(
    client: &reqwest::Client,
    trainer: &TrainerInfo,
    resolved_url: &str,
) -> Result<String> {
    let cached_path = download_trainer(client, trainer, resolved_url).await?;

    if cached_path.ends_with(".rar") || url_suggests_rar(resolved_url) {
        debug!(path = %cached_path, "downloaded file is a rar archive, extracting");
        extract_rar(&cached_path).await
    } else {
        Ok(cached_path)
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
    fn test_url_suggests_rar_dot_rar() {
        assert!(url_suggests_rar(
            "https://mobiletarget.net/files/trainer.rar"
        ));
    }

    #[test]
    fn test_url_suggests_rar_bang_rar() {
        assert!(url_suggests_rar(
            "https://dl.gamecopyworld.com/dl/gcw_FLiNG_Elden_Ring_v1!2e12!2e3_Plus_50_Trainer!rar"
        ));
    }

    #[test]
    fn test_url_suggests_rar_exe() {
        assert!(!url_suggests_rar(
            "https://flingtrainer.com/download/trainer.exe"
        ));
    }

    #[test]
    fn test_url_suggests_rar_case_insensitive() {
        assert!(url_suggests_rar("https://example.com/file.RAR"));
    }
}
