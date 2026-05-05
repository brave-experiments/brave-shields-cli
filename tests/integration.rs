use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

/// Build the binary once and return its path.
fn binary_path() -> std::path::PathBuf {
    let output = Command::new("cargo")
        .args(["build", "--quiet"])
        .output()
        .expect("failed to build");
    assert!(output.status.success(), "cargo build failed: {}", String::from_utf8_lossy(&output.stderr));
    std::path::PathBuf::from("target/debug/brave-shields-cli")
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
        .args(["--brave-dir", brave_dir.to_str().unwrap()])
        .output()
        .expect("failed to run binary");
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
        "account_values": {
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
    });
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
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_shields_on() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "on"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 1);
}

#[test]
fn test_set_fingerprinting_disabled() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "disabled"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 1);
}

#[test]
fn test_set_fingerprinting_aggressive() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "aggressive"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_fingerprinting_standard() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "fingerprinting", "standard"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["fingerprintingV2"]["example.com,*"];
    assert_eq!(entry["setting"], 3);
}

#[test]
fn test_set_https_strict() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "https-upgrade", "strict"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["httpsUpgrades"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_scripts_block() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "scripts", "block"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["javascript"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_ads_standard() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "ads", "standard"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
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
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
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
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
    assert_eq!(exceptions["shieldsAds"]["example.com,*"]["setting"], 1);
    assert_eq!(exceptions["trackers"]["example.com,*"]["setting"], 1);
    assert_eq!(exceptions["cosmeticFilteringV2"]["example.com,*"]["setting"]["cosmeticFilteringV2"], 0);
}

#[test]
fn test_set_overwrites_existing() {
    let prefs = json!({
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    assert_eq!(entry["setting"], 2);
}

#[test]
fn test_set_generates_last_modified() {
    let tmp = setup_fixture(&json!({}));
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "on"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let entry = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"]["example.com,*"];
    let lm = entry["last_modified"].as_str().unwrap();
    assert!(!lm.is_empty());
    assert!(lm.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_set_preserves_other_domains() {
    let prefs = json!({
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["set", "example.com", "shields", "off"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let shields = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"];
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
        "account_values": {
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
    });
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
        "account_values": {
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
    });
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
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
    assert!(exceptions["braveShields"]["example.com,*"].is_null());
    assert!(exceptions["fingerprintingV2"]["example.com,*"].is_null());
}

#[test]
fn test_reset_single_setting() {
    let prefs = json!({
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com", "fingerprinting"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
    // fingerprinting should be removed
    assert!(exceptions["fingerprintingV2"]["example.com,*"].is_null());
    // shields should be preserved
    assert_eq!(exceptions["braveShields"]["example.com,*"]["setting"], 2);
}

#[test]
fn test_reset_ads_removes_all_three() {
    let prefs = json!({
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com", "ads"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let exceptions = &prefs["account_values"]["profile"]["content_settings"]["exceptions"];
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
        "account_values": {
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
    });
    let tmp = setup_fixture(&prefs);
    let (_, _, success) = run_cmd(&["reset", "example.com"], tmp.path());
    assert!(success);
    let prefs = read_prefs(tmp.path());
    let shields = &prefs["account_values"]["profile"]["content_settings"]["exceptions"]["braveShields"];
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
