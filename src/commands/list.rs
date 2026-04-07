use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::output::{self, DomainOverrides, OutputFormat};
use crate::preferences;
use crate::shields::{self, ShieldSetting};

pub fn run(prefs_path: &Path, format: OutputFormat) -> Result<()> {
    let prefs = preferences::read_preferences(prefs_path)?;

    // Collect all overrides keyed by domain
    let mut domain_map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for &setting in ShieldSetting::ALL {
        collect_setting_overrides(&prefs, setting, &mut domain_map);
    }

    let entries: Vec<DomainOverrides> = domain_map
        .into_iter()
        .map(|(domain, overrides)| DomainOverrides { domain, overrides })
        .collect();

    output::print_list(&entries, format);
    Ok(())
}

fn collect_setting_overrides(
    prefs: &serde_json::Value,
    setting: ShieldSetting,
    domain_map: &mut BTreeMap<String, BTreeMap<String, String>>,
) {
    let primary_key = setting.primary_json_key();
    let entries = match preferences::get_exception_entries(prefs, primary_key) {
        Some(e) => e,
        None => return,
    };

    for (pattern, entry) in entries {
        let domain = match shields::domain_from_pattern(pattern) {
            Some(d) => d.to_string(),
            None => continue,
        };

        let primary_value = match shields::read_setting_value(entry) {
            Some(v) => v,
            None => continue,
        };

        let cosmetic_value = if setting == ShieldSetting::Ads {
            preferences::get_exception_entries(prefs, "cosmeticFilteringV2")
                .and_then(|entries: &serde_json::Map<String, serde_json::Value>| entries.get(pattern))
                .and_then(|entry| shields::read_cosmetic_value(entry))
        } else {
            None
        };

        if let Ok(cli_value) = shields::from_stored(setting, primary_value, cosmetic_value) {
            domain_map
                .entry(domain)
                .or_default()
                .insert(setting.cli_name().to_string(), cli_value);
        }
    }
}
