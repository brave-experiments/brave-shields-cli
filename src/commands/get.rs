use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::output::{self, OutputFormat};
use crate::preferences;
use crate::shields::{self, ShieldSetting};

pub fn run(
    prefs_path: &Path,
    domain: &str,
    profile_name: &str,
    format: OutputFormat,
) -> Result<()> {
    let prefs = preferences::read_preferences(prefs_path)?;
    let pattern = shields::domain_pattern(domain)?;
    let mut settings = BTreeMap::new();

    for &setting in ShieldSetting::ALL {
        let value = read_setting_value(&prefs, setting, &pattern);
        settings.insert(setting.cli_name().to_string(), value);
    }

    output::print_get(domain, profile_name, &settings, format);
    Ok(())
}

fn read_setting_value(prefs: &serde_json::Value, setting: ShieldSetting, pattern: &str) -> String {
    let primary_key = setting.primary_json_key();
    let entries = match preferences::get_exception_entries(prefs, primary_key) {
        Some(e) => e,
        None => return "default".to_string(),
    };

    let entry = match entries.get(pattern) {
        Some(e) => e,
        None => return "default".to_string(),
    };

    let primary_value = match shields::read_setting_value(entry) {
        Some(v) => v,
        None => return "default".to_string(),
    };

    // For ads, also read cosmeticFilteringV2
    let cosmetic_value = if setting == ShieldSetting::Ads {
        preferences::get_exception_entries(prefs, "cosmeticFilteringV2")
            .and_then(|entries: &serde_json::Map<String, serde_json::Value>| entries.get(pattern))
            .and_then(|entry| shields::read_cosmetic_value(entry))
    } else {
        None
    };

    shields::from_stored(setting, primary_value, cosmetic_value)
        .unwrap_or_else(|_| "unknown".to_string())
}
