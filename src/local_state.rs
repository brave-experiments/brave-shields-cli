use std::fs::{self, File};
use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde_json::Value;
use tempfile::NamedTempFile;

/// Read and parse the Local State file from the Brave data directory.
pub fn read_local_state(brave_dir: &Path) -> Result<Value> {
    let path = brave_dir.join("Local State");
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read Local State at {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| "failed to parse Local State as JSON")
}

/// Extract custom filter rules from Local State as a list of non-empty lines.
pub fn get_custom_filters(local_state: &Value) -> Vec<String> {
    local_state
        .pointer("/brave/ad_block/custom_filters")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect()
}

/// Write Local State to disk atomically using tempfile + persist, with backup and verification.
fn write_local_state(path: &Path, state: &Value) -> Result<()> {
    let content = serde_json::to_string_pretty(state)?;
    let dir = path
        .parent()
        .context("Local State path has no parent directory")?;

    let bak_path = dir.join("Local State.bak");
    if path.exists() && !bak_path.exists() {
        fs::copy(path, &bak_path)
            .with_context(|| format!("failed to create backup at {}", bak_path.display()))?;
    }

    let tmp = NamedTempFile::new_in(dir).context("failed to create temp file for atomic write")?;
    fs::write(tmp.path(), &content)
        .with_context(|| format!("failed to write temp file at {}", tmp.path().display()))?;
    tmp.persist(path)
        .with_context(|| format!("failed to persist temp file to {}", path.display()))?;

    let readback = fs::read_to_string(path)
        .with_context(|| format!("post-write verification: failed to read back {}", path.display()))?;
    serde_json::from_str::<Value>(&readback)
        .with_context(|| "post-write verification: file is not valid JSON after write")?;

    Ok(())
}

/// Open the Local State file with an exclusive lock, read-modify-write, then release the lock.
pub fn locked_read_modify_write(
    path: &Path,
    modify: impl FnOnce(&mut Value),
) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Local State file not found at {}", path.display());
    }

    let file = File::open(path)
        .with_context(|| format!("failed to open {} for locking", path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to acquire exclusive lock on {}", path.display()))?;

    let mut content = String::new();
    let content = {
        let mut f = File::open(path)?;
        f.read_to_string(&mut content)?;
        content
    };
    let mut state: Value =
        serde_json::from_str(&content).with_context(|| "failed to parse Local State as JSON")?;

    modify(&mut state);

    write_local_state(path, &state)?;

    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_custom_filters_with_entries() {
        let state = json!({
            "brave": {
                "ad_block": {
                    "custom_filters": "||example.com^\nexample.com##h1\n\n!comment"
                }
            }
        });
        let filters = get_custom_filters(&state);
        assert_eq!(filters, vec!["||example.com^", "example.com##h1", "!comment"]);
    }

    #[test]
    fn test_get_custom_filters_empty() {
        let state = json!({});
        let filters = get_custom_filters(&state);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_get_custom_filters_empty_string() {
        let state = json!({
            "brave": {
                "ad_block": {
                    "custom_filters": ""
                }
            }
        });
        let filters = get_custom_filters(&state);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_locked_read_modify_write() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Local State");
        let state = json!({"brave": {"ad_block": {"custom_filters": "||old.com^"}}});
        fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

        locked_read_modify_write(&path, |state| {
            state["brave"]["ad_block"]["custom_filters"] =
                Value::String("||old.com^\n||new.com^".to_string());
        })
        .unwrap();

        let result = read_local_state(tmp.path()).unwrap();
        let filters = get_custom_filters(&result);
        assert_eq!(filters, vec!["||old.com^", "||new.com^"]);
    }

    #[test]
    fn test_backup_created() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Local State");
        let state = json!({"original": true});
        fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

        locked_read_modify_write(&path, |state| {
            state["modified"] = json!(true);
        })
        .unwrap();

        let bak_path = tmp.path().join("Local State.bak");
        assert!(bak_path.exists());
        let bak: Value =
            serde_json::from_str(&fs::read_to_string(&bak_path).unwrap()).unwrap();
        assert_eq!(bak["original"], true);
    }
}
