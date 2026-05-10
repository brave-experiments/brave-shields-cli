use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// Build the binary once and return its path.
fn binary_path() -> &'static std::path::PathBuf {
    static BIN: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    BIN.get_or_init(|| {
        let output = Command::new("cargo")
            .args(["build", "--quiet"])
            .output()
            .expect("failed to build");
        assert!(output.status.success(), "cargo build failed: {}", String::from_utf8_lossy(&output.stderr));
        std::env::current_dir()
            .unwrap()
            .join("target/debug/brave-shields-cli")
    })
}

/// Set up a temp directory with Local State and a profile Preferences file.
/// Returns (temp_dir, brave_dir_path).
fn setup_fixture(prefs: &Value) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let brave_dir = tmp.path();

    // Write Local State
    let local_state = json!({
        "profile": {
            "last_used": "Default",
            "info_cache": {
                "Default": {"name": "Test Profile"}
            }
        }
    });
    fs::write(
        brave_dir.join("Local State"),
        serde_json::to_string_pretty(&local_state).unwrap(),
    )
    .unwrap();

    // Create profile directory and write Preferences
    let profile_dir = brave_dir.join("Default");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_string_pretty(prefs).unwrap(),
    )
    .unwrap();

    tmp
}

fn run_cmd(args: &[&str], brave_dir: &Path) -> (String, String, bool) {
    let bin = binary_path();
    let output = Command::new(&bin)
        .args(args)
        .args(["--brave-dir", brave_dir.to_str().unwrap(), "--force"])
        .output()
        .expect("failed to run binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

fn run_cmd_with_stdin(args: &[&str], brave_dir: &Path, stdin_input: &str) -> (String, String, bool) {
    let bin = binary_path();
    let mut child = Command::new(&bin)
        .args(args)
        .args(["--brave-dir", brave_dir.to_str().unwrap(), "--force"])
        .current_dir(brave_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn binary");

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

fn setup_fixture_with_local_state(prefs: &Value, local_state: &Value) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let brave_dir = tmp.path();

    fs::write(
        brave_dir.join("Local State"),
        serde_json::to_string_pretty(local_state).unwrap(),
    )
    .unwrap();

    let profile_dir = brave_dir.join("Default");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_string_pretty(prefs).unwrap(),
    )
    .unwrap();

    tmp
}

fn read_local_state(brave_dir: &Path) -> Value {
    let path = brave_dir.join("Local State");
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn read_prefs(brave_dir: &Path) -> Value {
    let path = brave_dir.join("Default/Preferences");
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

// ==================== GET command tests ====================

#[test]
fn test_get_domain_with_overrides() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 2, "last_modified": "123"}
                        },
                        "fingerprintingV2": {
                            "example.com,*": {"setting": 1, "last_modified": "123"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["get", "example.com"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "off");
    assert_eq!(output["settings"]["fingerprinting"], "disabled");
}

#[test]
fn test_get_domain_no_overrides() {
    let prefs = json!({});
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["get", "example.com"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "default");
    assert_eq!(output["settings"]["ads"], "default");
    assert_eq!(output["settings"]["fingerprinting"], "default");
    assert_eq!(output["settings"]["https-upgrade"], "default");
    assert_eq!(output["settings"]["scripts"], "default");
}

#[test]
fn test_get_json_output_format() {
    let prefs = json!({});
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["get", "example.com", "--format", "json"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["domain"], "example.com");
    assert!(output["settings"].is_object());
}

#[test]
fn test_get_table_output_format() {
    let prefs = json!({});
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["get", "example.com", "--format", "table"], tmp.path());
    assert!(success);
    assert!(stdout.contains("Domain: example.com"));
    assert!(stdout.contains("Setting"));
    assert!(stdout.contains("Value"));
}

// ==================== SET command tests ====================

#[test]
fn test_set_shields_off() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_shields_on() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "on"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 1);
}

#[test]
fn test_set_fingerprinting_disabled() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "disabled"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 1);
}

#[test]
fn test_set_fingerprinting_aggressive() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "aggressive"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_fingerprinting_standard() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "standard"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 3);
}

#[test]
fn test_set_https_strict() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "https-upgrade", "strict"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["httpsUpgrades"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_scripts_block() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "scripts", "block"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["javascript"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_ads_standard() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "ads", "standard"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    assert_eq!(exceptions["shieldsAds"]["example.com,*"]["setting"], 2);
    assert_eq!(exceptions["trackers"]["example.com,*"]["setting"], 2);
    assert_eq!(exceptions["cosmeticFilteringV2"]["example.com,*"]["setting"]["cosmeticFilteringV2"], 2);
}

#[test]
fn test_set_ads_aggressive() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "ads", "aggressive"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    assert_eq!(exceptions["shieldsAds"]["example.com,*"]["setting"], 2);
    assert_eq!(exceptions["trackers"]["example.com,*"]["setting"], 2);
    assert_eq!(exceptions["cosmeticFilteringV2"]["example.com,*"]["setting"]["cosmeticFilteringV2"], 1);
}

#[test]
fn test_set_ads_disabled() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "ads", "disabled"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    assert_eq!(exceptions["shieldsAds"]["example.com,*"]["setting"], 1);
    assert_eq!(exceptions["trackers"]["example.com,*"]["setting"], 1);
    assert_eq!(exceptions["cosmeticFilteringV2"]["example.com,*"]["setting"]["cosmeticFilteringV2"], 0);
}

#[test]
fn test_set_overwrites_existing() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 1, "last_modified": "100"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_generates_last_modified() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "on"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    let lm = entry["last_modified"].as_str().unwrap();
    assert!(!lm.is_empty());
    assert!(lm.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_set_preserves_other_domains() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "other.com,*": {"setting": 1, "last_modified": "100"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let shields = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"];
    assert_eq!(shields["other.com,*"]["setting"], 1);
    assert_eq!(shields["example.com,*"]["setting"], 2);
}

// ==================== LIST command tests ====================

#[test]
fn test_list_empty() {
    let tmp = setup_fixture(&json!({}));
    let (stdout, _, success) = run_cmd(&["list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.as_array().unwrap().len(), 0);
}

#[test]
fn test_list_multiple_domains() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "a.com,*": {"setting": 2, "last_modified": "1"},
                            "b.com,*": {"setting": 1, "last_modified": "2"}
                        },
                        "fingerprintingV2": {
                            "c.com,*": {"setting": 1, "last_modified": "3"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 3);
    let domains: Vec<&str> = output.iter().map(|e| e["domain"].as_str().unwrap()).collect();
    assert!(domains.contains(&"a.com"));
    assert!(domains.contains(&"b.com"));
    assert!(domains.contains(&"c.com"));
}

#[test]
fn test_list_deduplicates_domains() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 2, "last_modified": "1"}
                        },
                        "fingerprintingV2": {
                            "example.com,*": {"setting": 1, "last_modified": "2"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["domain"], "example.com");
    assert_eq!(output[0]["overrides"]["shields"], "off");
    assert_eq!(output[0]["overrides"]["fingerprinting"], "disabled");
}

#[test]
fn test_list_json_format() {
    let tmp = setup_fixture(&json!({}));
    let (stdout, _, success) = run_cmd(&["list", "--format", "json"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert!(output.is_array());
}

// ==================== RESET command tests ====================

#[test]
fn test_reset_all_for_domain() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 2, "last_modified": "1"}
                        },
                        "fingerprintingV2": {
                            "example.com,*": {"setting": 1, "last_modified": "2"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    assert!(exceptions["braveShields"]["example.com,*"].is_null());
    assert!(exceptions["fingerprintingV2"]["example.com,*"].is_null());
}

#[test]
fn test_reset_single_setting() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 2, "last_modified": "1"}
                        },
                        "fingerprintingV2": {
                            "example.com,*": {"setting": 1, "last_modified": "2"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com", "fingerprinting"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    // fingerprinting should be removed
    assert!(exceptions["fingerprintingV2"]["example.com,*"].is_null());
    // shields should be preserved
    assert_eq!(exceptions["braveShields"]["example.com,*"]["setting"], 2);
}

#[test]
fn test_reset_ads_removes_all_three() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "shieldsAds": {
                            "example.com,*": {"setting": 2, "last_modified": "1"}
                        },
                        "trackers": {
                            "example.com,*": {"setting": 2, "last_modified": "1"}
                        },
                        "cosmeticFilteringV2": {
                            "example.com,*": {"setting": {"cosmeticFilteringV2": 2}, "last_modified": "1"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com", "ads"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["profile"]["content_settings"]["exceptions"];
    assert!(exceptions["shieldsAds"]["example.com,*"].is_null());
    assert!(exceptions["trackers"]["example.com,*"].is_null());
    assert!(exceptions["cosmeticFilteringV2"]["example.com,*"].is_null());
}

#[test]
fn test_reset_nonexistent_domain() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["reset", "nonexistent.com"], tmp.path());
    assert!(success);
}

#[test]
fn test_reset_preserves_other_domains() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "example.com,*": {"setting": 2, "last_modified": "1"},
                            "other.com,*": {"setting": 1, "last_modified": "2"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let shields = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"];
    assert!(shields["example.com,*"].is_null());
    assert_eq!(shields["other.com,*"]["setting"], 1);
}

// ==================== Round-trip tests ====================

#[test]
fn test_set_then_get() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let (stdout, _, success) = run_cmd(&["get", "example.com"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "off");
}

#[test]
fn test_set_then_list() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["set", "a.com", "shields", "off"], tmp.path());
    run_cmd(&["set", "b.com", "fingerprinting", "disabled"], tmp.path());
    let (stdout, _, success) = run_cmd(&["list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 2);
}

#[test]
fn test_set_then_reset_then_get() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    run_cmd(&["reset", "example.com"], tmp.path());
    let (stdout, _, success) = run_cmd(&["get", "example.com"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "default");
}

#[test]
fn test_multiple_sets_then_list() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    run_cmd(&["set", "example.com", "fingerprinting", "aggressive"], tmp.path());
    run_cmd(&["set", "example.com", "scripts", "block"], tmp.path());
    let (stdout, _, success) = run_cmd(&["list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["domain"], "example.com");
    assert_eq!(output[0]["overrides"]["shields"], "off");
    assert_eq!(output[0]["overrides"]["fingerprinting"], "aggressive");
    assert_eq!(output[0]["overrides"]["scripts"], "block");
}

// ==================== FILTERS command tests ====================

fn local_state_with_filters(filters: &str) -> Value {
    json!({
        "profile": {
            "last_used": "Default",
            "info_cache": {
                "Default": {"name": "Test Profile"}
            }
        },
        "brave": {
            "ad_block": {
                "custom_filters": filters
            }
        }
    })
}

#[test]
fn test_filters_list_empty() {
    let tmp = setup_fixture(&json!({}));
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["custom_filters"].as_array().unwrap().len(), 0);
}

#[test]
fn test_filters_list_with_entries() {
    let ls = local_state_with_filters("||example.com^\nexample.com##h1");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0], "||example.com^");
    assert_eq!(filters[1], "example.com##h1");
}

#[test]
fn test_filters_list_skips_blank_lines() {
    let ls = local_state_with_filters("||a.com^\n\n||b.com^\n\n");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
}

#[test]
fn test_filters_add() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["filters", "add", "||example.com^"], tmp.path());
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(filters.contains("||example.com^"));
}

#[test]
fn test_filters_add_to_existing() {
    let ls = local_state_with_filters("||old.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (_, _, success) = run_cmd(&["filters", "add", "||new.com^"], tmp.path());
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(filters.contains("||old.com^"));
    assert!(filters.contains("||new.com^"));
}

#[test]
fn test_filters_add_no_duplicate() {
    let ls = local_state_with_filters("||example.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (_, _, success) = run_cmd(&["filters", "add", "||example.com^"], tmp.path());
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    let count = filters.lines().filter(|l| l.trim() == "||example.com^").count();
    assert_eq!(count, 1);
}

#[test]
fn test_filters_remove() {
    let ls = local_state_with_filters("||a.com^\n||b.com^\n||c.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (_, _, success) = run_cmd(&["filters", "remove", "||b.com^"], tmp.path());
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(filters.contains("||a.com^"));
    assert!(!filters.contains("||b.com^"));
    assert!(filters.contains("||c.com^"));
}

#[test]
fn test_filters_remove_nonexistent() {
    let ls = local_state_with_filters("||a.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (_, stderr, success) = run_cmd(&["filters", "remove", "||nope.com^"], tmp.path());
    assert!(success);
    assert!(stderr.contains("not found"));
}

#[test]
fn test_filters_clear() {
    let ls = local_state_with_filters("||a.com^\n||b.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let (_, _, success) = run_cmd(&["filters", "clear"], tmp.path());
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(filters.is_empty());
}

#[test]
fn test_filters_add_then_list() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["filters", "add", "||example.com^"], tmp.path());
    run_cmd(&["filters", "add", "example.com##h1"], tmp.path());
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
}

#[test]
fn test_filters_add_then_remove_then_list() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["filters", "add", "||a.com^"], tmp.path());
    run_cmd(&["filters", "add", "||b.com^"], tmp.path());
    run_cmd(&["filters", "remove", "||a.com^"], tmp.path());
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0], "||b.com^");
}

#[test]
fn test_filters_clear_then_list() {
    let ls = local_state_with_filters("||a.com^\n||b.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    run_cmd(&["filters", "clear"], tmp.path());
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["custom_filters"].as_array().unwrap().len(), 0);
}

#[test]
fn test_filters_preserves_other_local_state() {
    let ls = local_state_with_filters("||old.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    run_cmd(&["filters", "add", "||new.com^"], tmp.path());
    let ls = read_local_state(tmp.path());
    // Profile info should be preserved
    assert_eq!(ls["profile"]["last_used"], "Default");
}

// ==================== --pattern flag tests ====================

#[test]
fn test_set_with_raw_pattern() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "*,*", "shields", "off", "--pattern"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["*,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_get_with_raw_pattern() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "*,*": {"setting": 2, "last_modified": "123"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (stdout, _, success) = run_cmd(&["get", "*,*", "--pattern"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "off");
}

#[test]
fn test_reset_with_raw_pattern() {
    let prefs = json!({
        "profile": {
            "content_settings": {
                "exceptions": {
                        "braveShields": {
                            "*,*": {"setting": 2, "last_modified": "123"}
                        }
                    }
                }
            }
        }
    );
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "*,*", "--pattern"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    assert!(prefs["profile"]["content_settings"]["exceptions"]["braveShields"]["*,*"].is_null());
}

#[test]
fn test_set_then_get_with_pattern() {
    let tmp = setup_fixture(&json!({}));
    run_cmd(&["set", "*,*", "shields", "off", "--pattern"], tmp.path());
    let (stdout, _, success) = run_cmd(&["get", "*,*", "--pattern"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["settings"]["shields"], "off");
}

// ==================== FILTERS LOAD command tests ====================

fn write_rules_file(dir: &Path, rules: &[&str]) -> std::path::PathBuf {
    let path = dir.join("rules.txt");
    fs::write(&path, rules.join("\n")).unwrap();
    path
}

#[test]
fn test_filters_load() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||example.com^", "example.com##h1"]);
    let (_, _, success) = run_cmd(
        &["filters", "load", rules_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(filters.contains("||example.com^"));
    assert!(filters.contains("example.com##h1"));
}

#[test]
fn test_filters_load_replaces_existing() {
    let ls = local_state_with_filters("||old.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||new.com^"]);
    let (_, _, success) = run_cmd(
        &["filters", "load", rules_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(success);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert!(!filters.contains("||old.com^"));
    assert!(filters.contains("||new.com^"));
}

#[test]
fn test_filters_load_skips_blank_lines() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "", "||b.com^", "", ""]);
    let (_, _, success) = run_cmd(
        &["filters", "load", rules_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(success);
    let (stdout, _, _) = run_cmd(&["filters", "list"], tmp.path());
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
}

#[test]
fn test_filters_load_then_list() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^", "||c.com^"]);
    run_cmd(
        &["filters", "load", rules_path.to_str().unwrap()],
        tmp.path(),
    );
    let (stdout, _, success) = run_cmd(&["filters", "list"], tmp.path());
    assert!(success);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let filters = output["custom_filters"].as_array().unwrap();
    assert_eq!(filters.len(), 3);
}

#[test]
fn test_filters_load_nonexistent_file() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(
        &["filters", "load", "/nonexistent/rules.txt"],
        tmp.path(),
    );
    assert!(!success);
}

#[test]
fn test_filters_load_preserves_other_local_state() {
    let ls = local_state_with_filters("||old.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||new.com^"]);
    run_cmd(
        &["filters", "load", rules_path.to_str().unwrap()],
        tmp.path(),
    );
    let ls = read_local_state(tmp.path());
    assert_eq!(ls["profile"]["last_used"], "Default");
}

// ==================== BISECT command tests ====================

#[test]
fn test_bisect_nonexistent_file() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd_with_stdin(
        &["filters", "bisect", "/nonexistent/rules.txt"],
        tmp.path(),
        "",
    );
    assert!(!success);
}

#[test]
fn test_bisect_empty_file() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &[]);
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "",
    );
    assert!(!success);
    assert!(stderr.contains("no filter rules found"));
}

#[test]
fn test_bisect_initial_no() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "n\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(stderr.contains("Nothing to bisect"));
    assert!(stdout.trim().is_empty());
}

#[test]
fn test_bisect_single_rule() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||bad.com^"]);
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(stderr.contains("Only one rule"));
    assert_eq!(stdout.trim(), "||bad.com^");
}

#[test]
fn test_bisect_two_rules_bad_is_second() {
    // rules: [||good.com^, ||bad.com^]
    // Initial: all loaded -> "y" (issue present)
    // candidates=[0,1], mid=1, first_half=[0], second_half=[1]
    // Except index 0, index 1 still active. Issue still present -> "y"
    // candidates=[1] -> found ||bad.com^
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||good.com^", "||bad.com^"]);
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert_eq!(stdout.trim(), "||bad.com^");
}

#[test]
fn test_bisect_two_rules_bad_is_first() {
    // rules: [||bad.com^, ||good.com^]
    // Initial: all loaded -> "y"
    // candidates=[0,1], mid=1, first_half=[0], second_half=[1]
    // Except index 0, index 1 still active. Issue gone -> "n"
    // Bad rule was in first_half -> candidates=[0] -> found ||bad.com^
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||bad.com^", "||good.com^"]);
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\nn\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert_eq!(stdout.trim(), "||bad.com^");
}

#[test]
fn test_bisect_four_rules_finds_third() {
    // rules: [a, b, c, d] -- bad rule is c (index 2)
    // Initial: "y"
    // Step 1: candidates=[0,1,2,3], mid=2, first_half=[0,1], second_half=[2,3]
    //   Except [0,1], c still active -> "y" -> candidates=[2,3]
    // Step 2: candidates=[2,3], mid=1, first_half=[2], second_half=[3]
    //   Except [2], c excepted, issue gone -> "n" -> candidates=[2]
    // Found: ||c.com^
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(
        tmp.path(),
        &["||a.com^", "||b.com^", "||c.com^", "||d.com^"],
    );
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\nn\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert_eq!(stdout.trim(), "||c.com^");
}

#[test]
fn test_bisect_eight_rules() {
    // rules: [r0..r7] -- bad rule is r5 (index 5)
    // Initial: "y"
    // Step 1: candidates=[0..8], mid=4, except [0..4], r5 active -> "y" -> [4,5,6,7]
    // Step 2: candidates=[4,5,6,7], mid=2, except [4,5], r5 excepted -> "n" -> [4,5]
    // Step 3: candidates=[4,5], mid=1, except [4], r5 active -> "y" -> [5]
    // Found: ||r5.com^
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(
        tmp.path(),
        &[
            "||r0.com^", "||r1.com^", "||r2.com^", "||r3.com^",
            "||r4.com^", "||r5.com^", "||r6.com^", "||r7.com^",
        ],
    );
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\nn\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert_eq!(stdout.trim(), "||r5.com^");
}

#[test]
fn test_bisect_restores_original_filters() {
    let ls = local_state_with_filters("||original.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    // Stdin sequence: "y" confirms issue present, "y" answers bisect step
    // (finds ||b.com^ with 2 rules), "y" confirms restore prompt.
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert_eq!(filters, "||original.com^");
}

#[test]
fn test_bisect_restores_filters_on_initial_no() {
    let ls = local_state_with_filters("||original.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||a.com^"]);
    // "n" answers initial confirmation (issue not present), "y" confirms restore
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "n\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert_eq!(filters, "||original.com^");
}

#[test]
fn test_bisect_cleans_up_state_files() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    // "y" confirms issue, "y" bisect step, "y" confirms restore (which cleans up)
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(!tmp.path().join("bisect-state.json").exists());
    assert!(!tmp.path().join("bisect-filters.txt").exists());
}

#[test]
fn test_bisect_quit() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\nq\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(stderr.contains("Quitting bisect"));
    // Stdout should be empty (no result found yet)
    assert!(stdout.trim().is_empty());
}

#[test]
fn test_bisect_quit_does_not_restore_filters() {
    // Known issue: quitting a bisect session does not restore original filters.
    // The restore code in run() is not reached after quit.
    let ls = local_state_with_filters("||original.com^");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\nq\n",
    );
    assert!(success, "stderr: {}", stderr);
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert_ne!(filters, "||original.com^", "expected filters NOT to be restored on quit");
}

#[test]
fn test_bisect_eof_treated_as_quit() {
    // When stdin reaches EOF, the prompt functions return Answer::Quit
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    let (_, _, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "",
    );
    assert!(success);
}

#[test]
fn test_bisect_resume() {
    let tmp = setup_fixture(&json!({}));

    // Create a bisect state file simulating a previously interrupted session
    // with 2 candidates remaining out of 3 original rules
    let state = json!({
        "original_filters": "",
        "channel": "release",
        "all_rules": ["||a.com^", "||b.com^", "||c.com^"],
        "candidate_indices": [1, 2],
        "step": 1,
    });
    let state_path = tmp.path().join("saved-state.json");
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Resume: candidates=[1,2], bad rule is ||c.com^ (index 2)
    // Step > 0 so initial confirmation is skipped.
    // Step: mid=1, first_half=[1], second_half=[2]
    //   Except [1], index 2 still active -> "y" -> candidates=[2]
    // Found: ||c.com^
    // Final "y" answers the restore prompt.
    let (stdout, stderr, success) = run_cmd_with_stdin(
        &[
            "filters", "bisect",
            "--resume", state_path.to_str().unwrap(),
        ],
        tmp.path(),
        "y\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(stderr.contains("Resuming bisect"));
    assert_eq!(stdout.trim(), "||c.com^");
}

#[test]
fn test_bisect_resume_channel_mismatch() {
    let tmp = setup_fixture(&json!({}));

    let state = json!({
        "original_filters": "",
        "channel": "nightly",
        "all_rules": ["||a.com^"],
        "candidate_indices": [0],
        "step": 0,
    });
    let state_path = tmp.path().join("saved-state.json");
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Default channel is "release", state has "nightly" -> should fail
    let (_, stderr, success) = run_cmd_with_stdin(
        &[
            "filters", "bisect",
            "--resume", state_path.to_str().unwrap(),
        ],
        tmp.path(),
        "",
    );
    assert!(!success);
    assert!(stderr.contains("does not match"));
}

#[test]
fn test_bisect_resume_invalid_state_file() {
    let tmp = setup_fixture(&json!({}));
    let state_path = tmp.path().join("bad-state.json");
    fs::write(&state_path, "not valid json").unwrap();

    let (_, stderr, success) = run_cmd_with_stdin(
        &[
            "filters", "bisect",
            "--resume", state_path.to_str().unwrap(),
        ],
        tmp.path(),
        "",
    );
    assert!(!success);
    assert!(stderr.contains("failed to parse bisect state"));
}

#[test]
fn test_bisect_loads_filters_into_local_state() {
    // Verify that during bisect, filters are actually written into Local State.
    // Single rule: "y" confirms issue, single rule found immediately, "y" confirms restore.
    let ls = local_state_with_filters("");
    let tmp = setup_fixture_with_local_state(&json!({}), &ls);
    let rules_path = write_rules_file(tmp.path(), &["||only.com^"]);
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    // After bisect, original (empty) filters should be restored
    let ls = read_local_state(tmp.path());
    let filters = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
    assert_eq!(filters, "");
}

#[test]
fn test_bisect_stderr_shows_step_count() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(
        tmp.path(),
        &["||a.com^", "||b.com^", "||c.com^", "||d.com^"],
    );
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\nn\n",
    );
    assert!(success, "stderr: {}", stderr);
    // Should report "Bisecting 4 rules (~2 steps)"
    assert!(stderr.contains("Bisecting 4 rules"));
}

#[test]
fn test_bisect_found_message_in_stderr() {
    let tmp = setup_fixture(&json!({}));
    let rules_path = write_rules_file(tmp.path(), &["||a.com^", "||b.com^"]);
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect", rules_path.to_str().unwrap()],
        tmp.path(),
        "y\ny\n",
    );
    assert!(success, "stderr: {}", stderr);
    assert!(stderr.contains("Found problematic rule"));
}

#[test]
fn test_bisect_no_file_without_resume() {
    let tmp = setup_fixture(&json!({}));
    let (_, stderr, success) = run_cmd_with_stdin(
        &["filters", "bisect"],
        tmp.path(),
        "",
    );
    assert!(!success);
    assert!(stderr.contains("filter rules file is required"));
}

#[test]
fn test_bisect_resume_corrupted_all_rules() {
    let tmp = setup_fixture(&json!({}));
    let state = json!({
        "original_filters": "",
        "channel": "release",
        "all_rules": ["||a.com^", 42, "||c.com^"],
        "candidate_indices": [0, 1, 2],
        "step": 1,
    });
    let state_path = tmp.path().join("bad-rules-state.json");
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    let (_, stderr, success) = run_cmd_with_stdin(
        &[
            "filters", "bisect",
            "--resume", state_path.to_str().unwrap(),
        ],
        tmp.path(),
        "",
    );
    assert!(!success);
    assert!(stderr.contains("non-string"));
}

// ==================== SCRIPTLETS command tests ====================

fn write_js_file(dir: &Path, filename: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(filename);
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_scriptlets_list_empty() {
    let tmp = setup_fixture(&json!({}));
    let (stdout, _, success) = run_cmd(&["scriptlets", "list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_scriptlets_add_and_list() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "console.log('hi')");
    let (_, _, success) = run_cmd(
        &["scriptlets", "add", "user-test.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(success);
    let (stdout, _, success) = run_cmd(&["scriptlets", "list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["name"], "user-test.js");
    assert_eq!(output[0]["size"], 17);
}

#[test]
fn test_scriptlets_get() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "console.log('hello')");
    run_cmd(
        &["scriptlets", "add", "user-test.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    let (stdout, _, success) = run_cmd(&["scriptlets", "get", "user-test.js"], tmp.path());
    assert!(success);
    assert_eq!(stdout, "console.log('hello')");
}

#[test]
fn test_scriptlets_get_nonexistent() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["scriptlets", "get", "user-nope.js"], tmp.path());
    assert!(!success);
}

#[test]
fn test_scriptlets_add_replaces_existing() {
    let tmp = setup_fixture(&json!({}));
    let js1 = write_js_file(tmp.path(), "v1.js", "// version 1");
    run_cmd(
        &["scriptlets", "add", "user-test.js", js1.to_str().unwrap()],
        tmp.path(),
    );
    let js2 = write_js_file(tmp.path(), "v2.js", "// version 2");
    run_cmd(
        &["scriptlets", "add", "user-test.js", js2.to_str().unwrap()],
        tmp.path(),
    );
    let (stdout, _, success) = run_cmd(&["scriptlets", "list"], tmp.path());
    assert!(success);
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["name"], "user-test.js");
    assert_eq!(output[0]["size"], 12);
}

#[test]
fn test_scriptlets_remove() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "// test");
    run_cmd(
        &["scriptlets", "add", "user-test.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    let (_, stderr, success) = run_cmd(&["scriptlets", "remove", "user-test.js"], tmp.path());
    assert!(success);
    assert!(stderr.contains("Removed"));
    let (stdout, _, _) = run_cmd(&["scriptlets", "list"], tmp.path());
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_scriptlets_remove_nonexistent() {
    let tmp = setup_fixture(&json!({}));
    let (_, stderr, success) = run_cmd(&["scriptlets", "remove", "user-nope.js"], tmp.path());
    assert!(success);
    assert!(stderr.contains("not found"));
}

#[test]
fn test_scriptlets_clear() {
    let tmp = setup_fixture(&json!({}));
    let js1 = write_js_file(tmp.path(), "a.js", "// a");
    let js2 = write_js_file(tmp.path(), "b.js", "// b");
    run_cmd(
        &["scriptlets", "add", "user-a.js", js1.to_str().unwrap()],
        tmp.path(),
    );
    run_cmd(
        &["scriptlets", "add", "user-b.js", js2.to_str().unwrap()],
        tmp.path(),
    );
    let (_, stderr, success) = run_cmd(&["scriptlets", "clear"], tmp.path());
    assert!(success);
    assert!(stderr.contains("Cleared"));
    let (stdout, _, _) = run_cmd(&["scriptlets", "list"], tmp.path());
    let output: Vec<Value> = serde_json::from_str(&stdout).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_scriptlets_add_invalid_name() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "// test");
    let (_, _, success) = run_cmd(
        &["scriptlets", "add", "bad-name.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(!success);
}

#[test]
fn test_scriptlets_add_nonexistent_file() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(
        &["scriptlets", "add", "user-test.js", "/nonexistent/script.js"],
        tmp.path(),
    );
    assert!(!success);
}

#[test]
fn test_scriptlets_add_path_separator_rejected() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "// test");
    let (_, stderr, success) = run_cmd(
        &["scriptlets", "add", "user-../../etc/passwd.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    assert!(!success);
    assert!(stderr.contains("path separator"));
}

#[test]
fn test_scriptlets_list_table_format() {
    let tmp = setup_fixture(&json!({}));
    let js_path = write_js_file(tmp.path(), "test.js", "console.log('hi')");
    run_cmd(
        &["scriptlets", "add", "user-test.js", js_path.to_str().unwrap()],
        tmp.path(),
    );
    let (stdout, _, success) = run_cmd(
        &["scriptlets", "list", "--format", "table"],
        tmp.path(),
    );
    assert!(success);
    assert!(stdout.contains("user-test.js"));
    assert!(stdout.contains("bytes"));
}
