use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::local_state::read_local_state;

/// Information about a Brave profile.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub dir_name: String,
    pub display_name: String,
}

/// List all profiles from the Local State info_cache.
pub fn list_profiles(brave_dir: &Path) -> Result<Vec<ProfileInfo>> {
    let local_state = read_local_state(brave_dir)?;
    let info_cache = local_state
        .pointer("/profile/info_cache")
        .and_then(|v| v.as_object())
        .context("Local State missing profile.info_cache")?;

    let mut profiles = Vec::new();
    for (dir_name, info) in info_cache {
        let display_name = info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(dir_name)
            .to_string();
        profiles.push(ProfileInfo {
            dir_name: dir_name.clone(),
            display_name,
        });
    }
    Ok(profiles)
}

/// Resolve the profile directory name from an optional user-specified name.
/// If no name is given, uses `profile.last_used` from Local State.
/// Accepts either a display name (e.g. "Work") or dir name (e.g. "Profile 1").
pub fn resolve_profile(brave_dir: &Path, profile_arg: Option<&str>) -> Result<ProfileInfo> {
    let local_state = read_local_state(brave_dir)?;

    match profile_arg {
        None => {
            // Use last_used profile
            let last_used = local_state
                .pointer("/profile/last_used")
                .and_then(|v| v.as_str())
                .context("Local State missing profile.last_used")?;

            let display_name = local_state
                .pointer(&format!("/profile/info_cache/{}/name", last_used))
                .and_then(|v| v.as_str())
                .unwrap_or(last_used);

            Ok(ProfileInfo {
                dir_name: last_used.to_string(),
                display_name: display_name.to_string(),
            })
        }
        Some(name) => {
            let info_cache = local_state
                .pointer("/profile/info_cache")
                .and_then(|v| v.as_object())
                .context("Local State missing profile.info_cache")?;

            // First try: exact match on dir name
            if info_cache.contains_key(name) {
                let display_name = info_cache[name]
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(name);
                return Ok(ProfileInfo {
                    dir_name: name.to_string(),
                    display_name: display_name.to_string(),
                });
            }

            // Second try: match on display name
            for (dir_name, info) in info_cache {
                if let Some(display) = info.get("name").and_then(|v| v.as_str()) {
                    if display == name {
                        return Ok(ProfileInfo {
                            dir_name: dir_name.clone(),
                            display_name: display.to_string(),
                        });
                    }
                }
            }

            anyhow::bail!("profile '{}' not found. Available profiles: {}", name, {
                let names: Vec<String> = info_cache
                    .iter()
                    .map(|(dir, info)| {
                        let display = info
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(dir);
                        format!("'{}' ({})", display, dir)
                    })
                    .collect();
                names.join(", ")
            })
        }
    }
}

/// Get the full path to the Preferences file for a resolved profile.
pub fn preferences_path(brave_dir: &Path, profile: &ProfileInfo) -> PathBuf {
    brave_dir.join(&profile.dir_name).join("Preferences")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_fixture(tmp: &Path) {
        let local_state = serde_json::json!({
            "profile": {
                "last_used": "Default",
                "info_cache": {
                    "Default": {"name": "Person 1"},
                    "Profile 1": {"name": "Work"},
                    "Profile 2": {"name": "Personal"}
                }
            }
        });
        fs::write(
            tmp.join("Local State"),
            serde_json::to_string_pretty(&local_state).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_resolve_by_display_name() {
        let tmp = tempfile::tempdir().unwrap();
        create_fixture(tmp.path());
        let profile = resolve_profile(tmp.path(), Some("Work")).unwrap();
        assert_eq!(profile.dir_name, "Profile 1");
        assert_eq!(profile.display_name, "Work");
    }

    #[test]
    fn test_resolve_by_dir_name() {
        let tmp = tempfile::tempdir().unwrap();
        create_fixture(tmp.path());
        let profile = resolve_profile(tmp.path(), Some("Profile 1")).unwrap();
        assert_eq!(profile.dir_name, "Profile 1");
        assert_eq!(profile.display_name, "Work");
    }

    #[test]
    fn test_resolve_default_profile() {
        let tmp = tempfile::tempdir().unwrap();
        create_fixture(tmp.path());
        let profile = resolve_profile(tmp.path(), None).unwrap();
        assert_eq!(profile.dir_name, "Default");
        assert_eq!(profile.display_name, "Person 1");
    }

    #[test]
    fn test_unknown_profile_errors() {
        let tmp = tempfile::tempdir().unwrap();
        create_fixture(tmp.path());
        let result = resolve_profile(tmp.path(), Some("Nonexistent"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_list_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        create_fixture(tmp.path());
        let profiles = list_profiles(tmp.path()).unwrap();
        assert_eq!(profiles.len(), 3);
        let names: Vec<&str> = profiles.iter().map(|p| p.display_name.as_str()).collect();
        assert!(names.contains(&"Work"));
        assert!(names.contains(&"Personal"));
        assert!(names.contains(&"Person 1"));
    }
}
