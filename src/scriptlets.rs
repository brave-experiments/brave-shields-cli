use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use fs2::FileExt;
use rusty_leveldb::{DB, Options};
use serde_json::{json, Value};

const SCRIPTLETS_KEY: &[u8] = b"SCRIPTLETS";
const DB_DIR_NAME: &str = "AdBlock Custom Resources";

/// A single custom scriptlet.
pub struct Scriptlet {
    pub name: String,
    pub content: String,
}

/// Resolve the path to the AdBlock Custom Resources LevelDB.
/// This is always under Default/, regardless of active profile.
pub fn db_path(brave_dir: &Path) -> PathBuf {
    brave_dir.join("Default").join(DB_DIR_NAME)
}

/// Acquire an exclusive file lock on a sibling lock file before accessing the DB.
/// rusty-leveldb does not implement the LOCK file that C++ LevelDB uses, so we
/// must protect against concurrent access ourselves.
fn with_db_lock<T>(brave_dir: &Path, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let lock_path = db_path(brave_dir).with_extension("lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let file = File::create(&lock_path)
        .with_context(|| format!("failed to create lock file at {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to acquire exclusive lock on {}", lock_path.display()))?;
    let result = f();
    drop(file);
    result
}

/// Read all custom scriptlets from the LevelDB.
pub fn read_scriptlets(brave_dir: &Path) -> Result<Vec<Scriptlet>> {
    let path = db_path(brave_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    with_db_lock(brave_dir, || read_scriptlets_unlocked(&path))
}

/// Read scriptlets without acquiring the lock. Called from within `with_db_lock`.
fn read_scriptlets_unlocked(path: &Path) -> Result<Vec<Scriptlet>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut opts = Options::default();
    opts.create_if_missing = false;
    let mut db = DB::open(path, opts)
        .map_err(|e| anyhow::anyhow!("failed to open LevelDB at {}: {}", path.display(), e))?;

    let raw = match db.get(SCRIPTLETS_KEY) {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let json_str = String::from_utf8(raw.to_vec())
        .context("SCRIPTLETS value is not valid UTF-8")?;
    let entries: Vec<Value> = serde_json::from_str(&json_str)
        .context("failed to parse SCRIPTLETS JSON")?;

    let mut scriptlets = Vec::new();
    for entry in &entries {
        let name = entry["name"]
            .as_str()
            .context("scriptlet missing name")?
            .to_string();
        let content_b64 = entry["content"]
            .as_str()
            .context("scriptlet missing content")?;
        let content = if content_b64.is_empty() {
            String::new()
        } else {
            let bytes = BASE64
                .decode(content_b64)
                .context("failed to decode scriptlet content")?;
            String::from_utf8(bytes)
                .context("scriptlet content is not valid UTF-8")?
        };
        scriptlets.push(Scriptlet { name, content });
    }
    Ok(scriptlets)
}

/// Write the full set of scriptlets to the LevelDB.
fn write_scriptlets(brave_dir: &Path, scriptlets: &[Scriptlet]) -> Result<()> {
    let path = db_path(brave_dir);

    // Ensure the Default profile directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let entries: Vec<Value> = scriptlets
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "content": BASE64.encode(s.content.as_bytes()),
                "kind": {"mime": "application/javascript"},
            })
        })
        .collect();

    let json_str = serde_json::to_string(&entries)?;

    let mut opts = Options::default();
    opts.create_if_missing = true;
    let mut db = DB::open(&path, opts)
        .map_err(|e| anyhow::anyhow!("failed to open LevelDB at {}: {}", path.display(), e))?;

    db.put(SCRIPTLETS_KEY, json_str.as_bytes())
        .map_err(|e| anyhow::anyhow!("failed to write SCRIPTLETS: {}", e))?;
    db.flush()
        .map_err(|e| anyhow::anyhow!("failed to flush LevelDB: {}", e))?;

    Ok(())
}

/// Validate a scriptlet name matches Brave's requirements.
fn validate_name(name: &str) -> Result<()> {
    anyhow::ensure!(!name.is_empty(), "scriptlet name must not be empty");
    anyhow::ensure!(name.is_ascii(), "scriptlet name must be ASCII");
    anyhow::ensure!(
        !name.contains('/') && !name.contains('\\') && !name.contains(".."),
        "scriptlet name must not contain path separators or traversal sequences (got '{}')",
        name
    );
    anyhow::ensure!(
        name.starts_with("user-"),
        "scriptlet name must start with 'user-' (got '{}')",
        name
    );
    anyhow::ensure!(
        name.ends_with(".js"),
        "scriptlet name must end with '.js' (got '{}')",
        name
    );
    Ok(())
}

/// Add a scriptlet. If one with the same name exists, it is replaced.
pub fn add_scriptlet(brave_dir: &Path, name: &str, content: &str) -> Result<()> {
    validate_name(name)?;
    let path = db_path(brave_dir);
    with_db_lock(brave_dir, || {
        let mut scriptlets = read_scriptlets_unlocked(&path)?;
        scriptlets.retain(|s| s.name != name);
        scriptlets.push(Scriptlet {
            name: name.to_string(),
            content: content.to_string(),
        });
        write_scriptlets(brave_dir, &scriptlets)
    })
}

/// Remove a scriptlet by name. Returns true if it was found.
pub fn remove_scriptlet(brave_dir: &Path, name: &str) -> Result<bool> {
    let path = db_path(brave_dir);
    with_db_lock(brave_dir, || {
        let mut scriptlets = read_scriptlets_unlocked(&path)?;
        let before = scriptlets.len();
        scriptlets.retain(|s| s.name != name);
        if scriptlets.len() == before {
            return Ok(false);
        }
        write_scriptlets(brave_dir, &scriptlets)?;
        Ok(true)
    })
}

/// Remove all custom scriptlets.
pub fn clear_scriptlets(brave_dir: &Path) -> Result<()> {
    with_db_lock(brave_dir, || {
        write_scriptlets(brave_dir, &[])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_brave_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("Default")).unwrap();
        tmp
    }

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("user-test.js").is_ok());
        assert!(validate_name("user-my-script.js").is_ok());
    }

    #[test]
    fn test_validate_name_missing_prefix() {
        assert!(validate_name("test.js").is_err());
    }

    #[test]
    fn test_validate_name_missing_suffix() {
        assert!(validate_name("user-test").is_err());
    }

    #[test]
    fn test_validate_name_empty() {
        assert!(validate_name("").is_err());
    }

    #[test]
    fn test_validate_name_non_ascii() {
        assert!(validate_name("user-t\u{00e9}st.js").is_err());
    }

    #[test]
    fn test_validate_name_rejects_path_separators() {
        assert!(validate_name("user-../../etc/passwd.js").is_err());
        assert!(validate_name("user-foo/bar.js").is_err());
        assert!(validate_name("user-foo\\bar.js").is_err());
    }

    #[test]
    fn test_read_empty_db() {
        let tmp = setup_brave_dir();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert!(scriptlets.is_empty());
    }

    #[test]
    fn test_add_and_read() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-test.js", "console.log('hi')").unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert_eq!(scriptlets.len(), 1);
        assert_eq!(scriptlets[0].name, "user-test.js");
        assert_eq!(scriptlets[0].content, "console.log('hi')");
    }

    #[test]
    fn test_add_multiple() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-a.js", "// a").unwrap();
        add_scriptlet(tmp.path(), "user-b.js", "// b").unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert_eq!(scriptlets.len(), 2);
    }

    #[test]
    fn test_add_replaces_existing() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-test.js", "old").unwrap();
        add_scriptlet(tmp.path(), "user-test.js", "new").unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert_eq!(scriptlets.len(), 1);
        assert_eq!(scriptlets[0].content, "new");
    }

    #[test]
    fn test_remove() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-a.js", "// a").unwrap();
        add_scriptlet(tmp.path(), "user-b.js", "// b").unwrap();
        let removed = remove_scriptlet(tmp.path(), "user-a.js").unwrap();
        assert!(removed);
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert_eq!(scriptlets.len(), 1);
        assert_eq!(scriptlets[0].name, "user-b.js");
    }

    #[test]
    fn test_remove_nonexistent() {
        let tmp = setup_brave_dir();
        let removed = remove_scriptlet(tmp.path(), "user-nope.js").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_add_invalid_name_rejected() {
        let tmp = setup_brave_dir();
        assert!(add_scriptlet(tmp.path(), "bad-name.js", "").is_err());
        assert!(add_scriptlet(tmp.path(), "user-test", "").is_err());
    }

    #[test]
    fn test_nonexistent_db_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        // No Default/ directory at all
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert!(scriptlets.is_empty());
    }

    #[test]
    fn test_add_empty_content() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-empty.js", "").unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert_eq!(scriptlets.len(), 1);
        assert_eq!(scriptlets[0].name, "user-empty.js");
        assert_eq!(scriptlets[0].content, "");
    }

    #[test]
    fn test_clear_scriptlets() {
        let tmp = setup_brave_dir();
        add_scriptlet(tmp.path(), "user-a.js", "// a").unwrap();
        add_scriptlet(tmp.path(), "user-b.js", "// b").unwrap();
        clear_scriptlets(tmp.path()).unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert!(scriptlets.is_empty());
    }

    #[test]
    fn test_clear_empty_db() {
        let tmp = setup_brave_dir();
        clear_scriptlets(tmp.path()).unwrap();
        let scriptlets = read_scriptlets(tmp.path()).unwrap();
        assert!(scriptlets.is_empty());
    }

    #[test]
    fn test_read_corrupted_db() {
        let tmp = setup_brave_dir();
        let path = db_path(tmp.path());
        fs::create_dir_all(&path).unwrap();
        let mut opts = Options::default();
        opts.create_if_missing = true;
        let mut db = DB::open(&path, opts).unwrap();
        db.put(SCRIPTLETS_KEY, b"not valid json").unwrap();
        db.flush().unwrap();
        drop(db);
        let result = read_scriptlets(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_read_missing_name_field() {
        let tmp = setup_brave_dir();
        let path = db_path(tmp.path());
        fs::create_dir_all(&path).unwrap();
        let mut opts = Options::default();
        opts.create_if_missing = true;
        let mut db = DB::open(&path, opts).unwrap();
        let data = serde_json::to_string(&serde_json::json!([
            {"content": "Y29uc29sZS5sb2coJ2hpJyk="}
        ])).unwrap();
        db.put(SCRIPTLETS_KEY, data.as_bytes()).unwrap();
        db.flush().unwrap();
        drop(db);
        let result = read_scriptlets(tmp.path());
        assert!(result.is_err());
    }
}
