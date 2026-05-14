use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::config::entry::{
    unique_slug, ConfigEntry, ConfigMetadata, Source, CONFIG_FILENAME, METADATA_FILENAME,
};
use crate::config::persist;
use crate::core::paths::Paths;
use crate::log_bus::LogEvent;

/// Spec passed from the UI when the user finalizes the "Add config" dialog.
#[derive(Clone, Debug)]
pub struct NewConfigSpec {
    pub name: String,
    pub source: Source,
}

/// Create a brand-new config folder under `config/<slug>/`, populate it with
/// `config.json` (downloaded or copied) and `metadata.json`. Returns the
/// resulting entry's slug on success.
pub fn add_config(
    paths: &Paths,
    spec: &NewConfigSpec,
    log_tx: &Sender<LogEvent>,
) -> Result<String> {
    let name = spec.name.trim();
    if name.is_empty() {
        return Err(anyhow!("name is required"));
    }

    let slug = unique_slug(&paths.config_dir);
    let folder = paths.config_folder(&slug);
    std::fs::create_dir_all(&folder).context("failed to create config folder")?;

    let cfg_path = folder.join(CONFIG_FILENAME);
    if let Err(e) = fetch_into(&spec.source, &cfg_path, log_tx) {
        // Clean up the empty folder so we don't leave a half-built entry.
        let _ = std::fs::remove_dir_all(&folder);
        return Err(e);
    }

    let meta = ConfigMetadata {
        name: name.to_string(),
        source: spec.source.clone(),
        last_updated: Some(now_string()),
    };
    meta.save(&folder.join(METADATA_FILENAME))
        .context("failed to write metadata.json")?;

    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] added config '{name}' as '{slug}'"
    )));
    Ok(slug)
}

/// Re-fetch an existing entry from its recorded source.
pub fn refresh_entry(entry: &ConfigEntry, log_tx: &Sender<LogEvent>) -> Result<()> {
    let cfg_path = entry.config_file();
    fetch_into(&entry.metadata.source, &cfg_path, log_tx)?;

    let mut meta = entry.metadata.clone();
    meta.last_updated = Some(now_string());
    meta.save(&entry.metadata_file())
        .context("failed to update metadata.json")?;

    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] refreshed '{}'",
        entry.metadata.name
    )));
    Ok(())
}

/// Apply edits to an existing entry. The folder/slug stay the same (so the
/// stored selection remains valid). If `new_source` differs from the current
/// one, the config file is re-fetched and `last_updated` is bumped.
pub fn edit_entry(
    entry: &ConfigEntry,
    new_name: &str,
    new_source: &Source,
    log_tx: &Sender<LogEvent>,
) -> Result<()> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err(anyhow!("name is required"));
    }

    let mut meta = entry.metadata.clone();
    meta.name = new_name.to_string();

    let source_changed = &meta.source != new_source;
    if source_changed {
        fetch_into(new_source, &entry.config_file(), log_tx)?;
        meta.source = new_source.clone();
        meta.last_updated = Some(now_string());
    }

    meta.save(&entry.metadata_file())
        .context("failed to update metadata.json")?;

    let _ = log_tx.send(LogEvent::line(format!("[updater] edited '{}'", meta.name)));
    Ok(())
}

/// Delete an entry's folder entirely.
pub fn delete_entry(entry: &ConfigEntry, log_tx: &Sender<LogEvent>) -> Result<()> {
    if entry.folder.exists() {
        std::fs::remove_dir_all(&entry.folder).context("failed to delete folder")?;
    }
    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] deleted '{}'",
        entry.metadata.name
    )));
    Ok(())
}

// ---------- internals ----------

fn fetch_into(source: &Source, dest: &Path, log_tx: &Sender<LogEvent>) -> Result<()> {
    match source {
        Source::Remote { url } => download_to(url, dest, log_tx),
        Source::Local { path } => copy_local(path, dest, log_tx),
    }
}

fn download_to(url: &str, dest: &Path, log_tx: &Sender<LogEvent>) -> Result<()> {
    let url = url.trim();
    if url.is_empty() {
        return Err(anyhow!("URL is empty"));
    }

    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] downloading {url} -> {}",
        dest.display()
    )));

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("sing-box-for-windows/0.1")
        .build()
        .context("failed to build HTTP client")?;

    let resp = client.get(url).send().context("request failed")?;
    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow!("HTTP {status}"));
    }

    let bytes = resp.bytes().context("failed to read response")?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .context("downloaded content is not valid JSON")?;

    write_atomic(dest, &bytes)?;
    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] saved {} bytes",
        bytes.len()
    )));
    Ok(())
}

fn copy_local(src: &Path, dest: &Path, log_tx: &Sender<LogEvent>) -> Result<()> {
    if !src.exists() {
        return Err(anyhow!("source file not found: {}", src.display()));
    }
    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] copying {} -> {}",
        src.display(),
        dest.display()
    )));
    let bytes = std::fs::read(src).context("failed to read source file")?;
    serde_json::from_slice::<serde_json::Value>(&bytes).context("source file is not valid JSON")?;
    write_atomic(dest, &bytes)?;
    let _ = log_tx.send(LogEvent::line(format!(
        "[updater] copied {} bytes",
        bytes.len()
    )));
    Ok(())
}

fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    persist::write_atomic_with_backup(dest, bytes)
        .with_context(|| format!("failed to replace config file {}", dest.display()))
}

fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
