use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Detects which Proton the game is using by reading /proc/<pid>/cmdline.
///
/// The game's command line contains the Proton path, e.g.:
/// `.../Proton 10.0/proton waitforexitandrun .../game.exe`
/// We extract the Proton directory from this.
pub async fn detect_game_proton(game_pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", game_pid);
    let data = fs::read(&cmdline_path).await.ok()?;
    // /proc/<pid>/cmdline is null-byte separated
    let cmdline = String::from_utf8_lossy(&data);
    let args: Vec<&str> = cmdline.split('\0').collect();

    for arg in &args {
        // Look for a path containing "/proton" (the proton script)
        if arg.ends_with("/proton") || arg.contains("/proton\0") {
            // The Proton dir is the parent of the "proton" script
            let path = PathBuf::from(arg);
            if let Some(parent) = path.parent() {
                let parent_str = parent.to_string_lossy().to_string();
                info!(path = %parent_str, pid = game_pid, "detected game's Proton version");
                return Some(parent_str);
            }
        }
    }

    // Also check for the pattern in the full command line
    let full = cmdline.replace('\0', " ");
    if let Some(idx) = full.find("/proton ") {
        let prefix = &full[..idx];
        let start = prefix.rfind(' ').map(|i| i + 1).unwrap_or(0);
        let proton_dir = &full[start..idx];
        let proton_dir = proton_dir.trim();
        if !proton_dir.is_empty() {
            info!(path = %proton_dir, pid = game_pid, "detected game's Proton version (from cmdline)");
            return Some(proton_dir.to_string());
        }
    }

    warn!(
        pid = game_pid,
        "could not detect game's Proton version from cmdline"
    );
    None
}

/// Finds the latest Proton installation (fallback when game Proton can't be detected).
///
/// Searches in:
/// 1. `~/.local/share/Steam/compatibilitytools.d/` (custom Proton / GE)
/// 2. `~/.local/share/Steam/steamapps/common/` (official Proton)
pub async fn find_proton() -> Result<String> {
    let home = std::env::var("HOME").context("HOME not set")?;

    // Check custom Proton installations first (GE-Proton, etc.)
    let custom_dir = PathBuf::from(&home).join(".local/share/Steam/compatibilitytools.d");

    if let Ok(mut entries) = fs::read_dir(&custom_dir).await {
        let mut proton_dirs = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("proton") || name.contains("ge") {
                let proton_bin = entry.path().join("proton");
                if fs::metadata(&proton_bin).await.is_ok() {
                    proton_dirs.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
        proton_dirs.sort();
        proton_dirs.reverse();
        if let Some(path) = proton_dirs.first() {
            debug!(path = %path, "found custom Proton");
            return Ok(path.clone());
        }
    }

    // Check official Steam Proton
    let steam_common = PathBuf::from(&home).join(".local/share/Steam/steamapps/common");

    if let Ok(mut entries) = fs::read_dir(&steam_common).await {
        let mut proton_dirs = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("Proton") {
                let proton_bin = entry.path().join("proton");
                if fs::metadata(&proton_bin).await.is_ok() {
                    proton_dirs.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
        proton_dirs.sort();
        proton_dirs.reverse();
        if let Some(path) = proton_dirs.first() {
            debug!(path = %path, "found official Proton");
            return Ok(path.clone());
        }
    }

    anyhow::bail!("no Proton installation found")
}

/// Checks whether .NET Framework + VC++ Runtime have been installed
/// for the given app_id (marker file at `~/.local/share/sharkdeck/cache/deps/<id>.done`).
pub async fn deps_installed(app_id: &str) -> bool {
    if let Ok(home) = std::env::var("HOME") {
        let marker = PathBuf::from(&home)
            .join(".local/share/sharkdeck/cache/deps")
            .join(format!("{}.done", app_id));
        fs::metadata(&marker).await.is_ok()
    } else {
        false
    }
}

/// Records that deps were successfully installed for the given app_id.
async fn mark_deps_done(app_id: &str) -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let dir = PathBuf::from(&home).join(".local/share/sharkdeck/cache/deps");
    fs::create_dir_all(&dir).await?;
    let marker = dir.join(format!("{}.done", app_id));
    fs::write(&marker, "dotnet48 vcrun2019\n").await?;
    Ok(())
}

/// Finds the winetricks script at `~/.local/bin/winetricks`.
async fn find_winetricks() -> Result<String> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let wt = PathBuf::from(&home).join(".local/bin/winetricks");
    if fs::metadata(&wt).await.is_ok() {
        return Ok(wt.to_string_lossy().to_string());
    }
    anyhow::bail!(
        "winetricks not found at {} — re-run the SharkDeck installer",
        wt.display()
    )
}

/// Finds the wine binary inside a Proton installation directory.
async fn find_wine_in_proton(proton_path: &str) -> Result<(String, String)> {
    let proton_dir = PathBuf::from(proton_path);

    let candidates = [
        ("files/bin/wine", "files/bin/wineserver"),
        ("files/bin/wine64", "files/bin/wineserver"),
        ("dist/bin/wine", "dist/bin/wineserver"),
        ("dist/bin/wine64", "dist/bin/wineserver"),
    ];

    for (wine, server) in &candidates {
        let wine_path = proton_dir.join(wine);
        let server_path = proton_dir.join(server);
        if fs::metadata(&wine_path).await.is_ok() && fs::metadata(&server_path).await.is_ok() {
            return Ok((
                wine_path.to_string_lossy().to_string(),
                server_path.to_string_lossy().to_string(),
            ));
        }
    }

    anyhow::bail!(
        "no wine/wineserver found in {} (checked files/bin/ and dist/bin/)",
        proton_path
    )
}

/// Installs .NET Framework 4.8 and Visual C++ Runtime 2019 into the
/// game's Wine prefix via winetricks.
///
/// Fling trainers post-July 2023 use the .NET Framework embedding API
/// which Wine Mono cannot handle. Without native dotnet, the trainer
/// window appears but the cheat engine never initializes.
///
/// This is a one-time operation per game that takes a few minutes.
pub async fn install_prefix_deps(app_id: &str, proton_path: &str) -> Result<()> {
    if deps_installed(app_id).await {
        debug!(app_id = %app_id, "prefix deps already installed — skipping");
        return Ok(());
    }

    let wt = find_winetricks().await?;
    let (wine_bin, wineserver_bin) = find_wine_in_proton(proton_path).await?;

    let home = std::env::var("HOME").context("HOME not set")?;
    let prefix_pfx = PathBuf::from(&home)
        .join(".local/share/Steam/steamapps/compatdata")
        .join(app_id)
        .join("pfx");

    info!(
        app_id = %app_id,
        wine = %wine_bin,
        prefix = %prefix_pfx.display(),
        "installing .NET Framework + VC++ Runtime (one-time, may take a few minutes)"
    );

    let output = Command::new(&wt)
        .arg("-q")
        .arg("dotnet48")
        .env("WINE", &wine_bin)
        .env("WINESERVER", &wineserver_bin)
        .env("WINEPREFIX", prefix_pfx.to_string_lossy().as_ref())
        .env("WINEFSYNC", "1")
        .env("WINEESYNC", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .context("failed to run winetricks")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        debug!(stdout = %stdout, "winetricks stdout");
    }
    if !stderr.is_empty() {
        debug!(stderr = %stderr, "winetricks stderr");
    }

    if output.status.success() {
        mark_deps_done(app_id).await?;
        info!(app_id = %app_id, "prefix deps installed successfully");
        Ok(())
    } else {
        let err_line = stderr
            .lines()
            .chain(stdout.lines())
            .rfind(|l| {
                let t = l.trim();
                !t.is_empty() && !t.chars().all(|c| c == '-')
            })
            .unwrap_or("unknown error")
            .trim();
        warn!(
            app_id = %app_id,
            error = %err_line,
            "winetricks failed — trainer may not work without .NET"
        );
        Err(anyhow::anyhow!(
            "dependency installation failed: {}",
            err_line
        ))
    }
}
