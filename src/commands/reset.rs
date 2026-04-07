use std::path::Path;

use anyhow::Result;

use crate::output;
use crate::platform::Channel;
use crate::preferences;
use crate::shields::{self, ShieldSetting};

pub fn run(prefs_path: &Path, domain: &str, setting_name: Option<&str>, channel: Channel) -> Result<()> {
    let pattern = shields::domain_pattern(domain)?;

    let settings_to_reset: Vec<ShieldSetting> = match setting_name {
        Some(name) => vec![ShieldSetting::from_cli(name)?],
        None => ShieldSetting::ALL.to_vec(),
    };

    preferences::warn_if_brave_running(channel);
    preferences::locked_read_modify_write(prefs_path, |prefs| {
        for setting in &settings_to_reset {
            for json_key in setting.json_keys() {
                if let Some(entries) = preferences::get_exception_entries(prefs, json_key) {
                    if entries.contains_key(&pattern) {
                        let entries_mut =
                            preferences::get_exception_entries_mut(prefs, json_key);
                        entries_mut.remove(&pattern);
                    }
                }
            }
        }
    })?;
    output::print_reset(domain, setting_name);
    Ok(())
}
