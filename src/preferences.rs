use std::fs::{self, File};
use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde_json::Value;
use tempfile::NamedTempFile;

use crate::platform::Channel;

/// The path within the Preferences JSON where shield exceptions live.
const EXCEPTIONS_PATH: &[&str] = &[
    "profile",
    "content_settings",
    "exceptions",
];

/// Read and parse a Preferences JSON file.
pub fn read_preferences(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read Preferences at {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| "failed to parse Preferences as JSON")
}

/// Navigate to the exceptions object, returning None if any intermediate key is missing.
pub fn get_exceptions(prefs: &Value) -> Option<&serde_json::Map<String, Value>> {
    let mut current = prefs;
    for key in EXCEPTIONS_PATH {
        current = current.get(*key)?;
    }
    current.as_object()
}

/// Navigate to the exceptions object, creating intermediate objects as needed.
pub fn get_exceptions_mut(prefs: &mut Value) -> &mut serde_json::Map<String, Value> {
    let mut current = prefs;
    for key in EXCEPTIONS_PATH {
        if !current.get(*key).is_some_and(|v| v.is_object()) {
            current[*key] = serde_json::json!({});
        }
        current = current.get_mut(*key).unwrap();
    }
    current.as_object_mut().unwrap()
}

/// Get a specific exception type (e.g. "braveShields") from the exceptions object.
pub fn get_exception_entries<'a>(prefs: &'a Value, key: &str) -> Option<&'a serde_json::Map<String, Value>> {
    get_exceptions(prefs)?.get(key)?.as_object()
}

/// Get a mutable reference to a specific exception type, creating it if missing.
pub fn get_exception_entries_mut<'a>(
    prefs: &'a mut Value,
    key: &str,
) -> &'a mut serde_json::Map<String, Value> {
    let exceptions = get_exceptions_mut(prefs);
    if !exceptions.get(key).is_some_and(|v| v.is_object()) {
        exceptions.insert(key.to_string(), serde_json::json!({}));
    }
    exceptions.get_mut(key).unwrap().as_object_mut().unwrap()
}

/// Write preferences to disk atomically using tempfile + persist, with backup and verification.
pub fn write_preferences(path: &Path, prefs: &Value) -> Result<()> {
    let content = serde_json::to_string_pretty(prefs)?;
    let dir = path
        .parent()
        .context("Preferences path has no parent directory")?;

    // #4: Create a backup of the current file if it exists and no backup exists yet.
    let bak_path = dir.join("Preferences.bak");
    if path.exists() && !bak_path.exists() {
        fs::copy(path, &bak_path)
            .with_context(|| format!("failed to create backup at {}", bak_path.display()))?;
    }

    // #3: Use tempfile crate for atomic write.
    let tmp = NamedTempFile::new_in(dir)
        .context("failed to create temp file for atomic write")?;
    fs::write(tmp.path(), &content)
        .with_context(|| format!("failed to write temp file at {}", tmp.path().display()))?;
    tmp.persist(path)
        .with_context(|| format!("failed to persist temp file to {}", path.display()))?;

    // #7: Post-write verification -- read back and parse to confirm valid JSON.
    let readback = fs::read_to_string(path)
        .with_context(|| format!("post-write verification: failed to read back {}", path.display()))?;
    serde_json::from_str::<Value>(&readback)
        .with_context(|| "post-write verification: file is not valid JSON after write")?;

    Ok(())
}

/// Open the Preferences file with an exclusive lock, read-modify-write, then release the lock.
pub fn locked_read_modify_write(
    path: &Path,
    modify: impl FnOnce(&mut Value),
) -> Result<()> {
    // Ensure the file exists so we can lock it.
    if !path.exists() {
        anyhow::bail!("Preferences file not found at {}", path.display());
    }

    let file = File::open(path)
        .with_context(|| format!("failed to open {} for locking", path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to acquire exclusive lock on {}", path.display()))?;

    // Read while holding the lock.
    let mut content = String::new();
    // Re-read via the filesystem (not the locked fd) to get full content reliably.
    let content = {
        let mut f = File::open(path)?;
        f.read_to_string(&mut content)?;
        content
    };
    let mut prefs: Value =
        serde_json::from_str(&content).with_context(|| "failed to parse Preferences as JSON")?;

    modify(&mut prefs);

    write_preferences(path, &prefs)?;

    // Lock is released when `file` is dropped.
    drop(file);
    Ok(())
}

/// Process names for each Brave channel per platform.
fn brave_process_names(channel: Channel) -> &'static [&'static str] {
    match channel {
        Channel::Release => {
            #[cfg(target_os = "macos")]
            { &["Brave Browser"] }
            #[cfg(target_os = "linux")]
            { &["brave-browser"] }
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            { &[] }
        }
        Channel::Beta => {
            #[cfg(target_os = "macos")]
            { &["Brave Browser Beta"] }
            #[cfg(target_os = "linux")]
            { &["brave-browser-beta"] }
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            { &[] }
        }
        Channel::Nightly => {
            #[cfg(target_os = "macos")]
            { &["Brave Browser Nightly"] }
            #[cfg(target_os = "linux")]
            { &["brave-browser-nightly"] }
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            { &[] }
        }
        Channel::Dev => {
            #[cfg(target_os = "macos")]
            { &["Brave Browser Development"] }
            #[cfg(target_os = "linux")]
            { &["brave-browser-dev"] }
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            { &[] }
        }
    }
}

/// Check if a specific Brave channel is currently running.
/// Errors by default; pass `force: true` to downgrade to a warning.
pub fn check_brave_not_running(channel: Channel, force: bool) -> Result<()> {
    use sysinfo::System;
    let s = System::new_all();
    let expected_names = brave_process_names(channel);
    let brave_running = s.processes().values().any(|p| {
        let name = p.name().to_string_lossy();
        expected_names.iter().any(|expected| name == *expected)
    });
    if brave_running {
        if force {
            eprintln!(
                "warning: Brave {} appears to be running. \
                 Changes may be overwritten by the browser.",
                channel
            );
        } else {
            anyhow::bail!(
                "Brave {} is running. Quit the browser first, or use --force to write anyway.",
                channel
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_read_exceptions() {
        let prefs = json!({
            "profile": {
                "content_settings": {
                    "exceptions": {
                        "braveShields": {
                            "example.com,*": {
                                "last_modified": "123",
                                "setting": 2
                            }
                        }
                    }
                }
            }
        });
        let entries = get_exception_entries(&prefs, "braveShields").unwrap();
        assert!(entries.contains_key("example.com,*"));
        assert_eq!(entries["example.com,*"]["setting"], 2);
    }

    #[test]
    fn test_write_new_entry() {
        let mut prefs = json!({});
        let entries = get_exception_entries_mut(&mut prefs, "braveShields");
        entries.insert(
            "example.com,*".to_string(),
            json!({"setting": 2, "last_modified": "123"}),
        );
        let entries = get_exception_entries(&prefs, "braveShields").unwrap();
        assert_eq!(entries["example.com,*"]["setting"], 2);
    }

    #[test]
    fn test_write_preserves_existing() {
        let mut prefs = json!({
            "profile": {
                "content_settings": {
                    "exceptions": {
                        "braveShields": {
                            "existing.com,*": {
                                "last_modified": "100",
                                "setting": 1
                            }
                        }
                    }
                }
            }
        });
        let entries = get_exception_entries_mut(&mut prefs, "braveShields");
        entries.insert(
            "new.com,*".to_string(),
            json!({"setting": 2, "last_modified": "200"}),
        );
        let entries = get_exception_entries(&prefs, "braveShields").unwrap();
        assert_eq!(entries["existing.com,*"]["setting"], 1);
        assert_eq!(entries["new.com,*"]["setting"], 2);
    }

    #[test]
    fn test_create_missing_intermediates() {
        let mut prefs = json!({"some_other_key": true});
        let entries = get_exception_entries_mut(&mut prefs, "braveShields");
        entries.insert(
            "test.com,*".to_string(),
            json!({"setting": 1, "last_modified": "0"}),
        );
        assert!(prefs
            .pointer("/profile/content_settings/exceptions/braveShields/test.com,*")
            .is_some());
        assert_eq!(prefs["some_other_key"], true);
    }

    #[test]
    fn test_atomic_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Preferences");
        let prefs = json!({"test": true});
        write_preferences(&path, &prefs).unwrap();
        let read_back = read_preferences(&path).unwrap();
        assert_eq!(read_back["test"], true);
    }

    #[test]
    fn test_read_nonexistent_file_errors() {
        let result = read_preferences(Path::new("/nonexistent/path/Preferences"));
        assert!(result.is_err());
    }

    #[test]
    fn test_cosmetic_filtering_nested_format() {
        let mut prefs = json!({});
        let entries = get_exception_entries_mut(&mut prefs, "cosmeticFilteringV2");
        entries.insert(
            "example.com,*".to_string(),
            json!({
                "last_modified": "123",
                "setting": {"cosmeticFilteringV2": 2}
            }),
        );
        let entries = get_exception_entries(&prefs, "cosmeticFilteringV2").unwrap();
        let setting = &entries["example.com,*"]["setting"];
        assert_eq!(setting["cosmeticFilteringV2"], 2);
    }

    #[test]
    fn test_backup_created_on_first_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Preferences");
        let prefs = json!({"original": true});
        // Write the initial file.
        fs::write(&path, serde_json::to_string_pretty(&prefs).unwrap()).unwrap();
        // Now write via write_preferences -- should create backup.
        let new_prefs = json!({"updated": true});
        write_preferences(&path, &new_prefs).unwrap();
        let bak_path = tmp.path().join("Preferences.bak");
        assert!(bak_path.exists());
        let bak_content: Value =
            serde_json::from_str(&fs::read_to_string(&bak_path).unwrap()).unwrap();
        assert_eq!(bak_content["original"], true);
    }

    #[test]
    fn test_backup_not_overwritten_on_second_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Preferences");
        let prefs = json!({"original": true});
        fs::write(&path, serde_json::to_string_pretty(&prefs).unwrap()).unwrap();
        // First write creates backup.
        write_preferences(&path, &json!({"second": true})).unwrap();
        // Second write should NOT overwrite the backup.
        write_preferences(&path, &json!({"third": true})).unwrap();
        let bak_path = tmp.path().join("Preferences.bak");
        let bak_content: Value =
            serde_json::from_str(&fs::read_to_string(&bak_path).unwrap()).unwrap();
        assert_eq!(bak_content["original"], true);
    }

    #[test]
    fn test_post_write_verification() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Preferences");
        let prefs = json!({"verify": true});
        write_preferences(&path, &prefs).unwrap();
        // Verify the file is valid JSON after write.
        let content = fs::read_to_string(&path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["verify"], true);
    }

    #[test]
    fn test_locked_read_modify_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Preferences");
        let prefs = json!({"counter": 1});
        fs::write(&path, serde_json::to_string_pretty(&prefs).unwrap()).unwrap();

        locked_read_modify_write(&path, |prefs| {
            prefs["counter"] = json!(2);
        })
        .unwrap();

        let result = read_preferences(&path).unwrap();
        assert_eq!(result["counter"], 2);
    }
}
