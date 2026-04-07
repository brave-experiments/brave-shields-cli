use std::path::Path;

use anyhow::Result;

use crate::chromium_time;
use crate::output;
use crate::platform::Channel;
use crate::preferences;
use crate::shields::{self, ShieldSetting};

pub fn run(prefs_path: &Path, domain: &str, setting_name: &str, value: &str, channel: Channel) -> Result<()> {
    let setting = ShieldSetting::from_cli(setting_name)?;

    // Validate the value
    if !setting.valid_values().contains(&value) {
        anyhow::bail!(
            "invalid value '{}' for {}. Valid values: {}",
            value,
            setting_name,
            setting.valid_values().join(", ")
        );
    }

    let pattern = shields::domain_pattern(domain)?;
    let timestamp = chromium_time::now()?;
    let entries = shields::to_stored(setting, value, &timestamp)?;

    preferences::warn_if_brave_running(channel);
    preferences::locked_read_modify_write(prefs_path, |prefs| {
        for (json_key, entry_value) in &entries {
            let exception_entries = preferences::get_exception_entries_mut(prefs, json_key);
            exception_entries.insert(pattern.clone(), entry_value.clone());
        }
    })?;
    output::print_set(domain, setting_name, value);
    Ok(())
}
