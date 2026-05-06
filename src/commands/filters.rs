use std::path::Path;

use anyhow::{Context, Result};

use crate::local_state;
use crate::output::{self, OutputFormat};
use crate::platform::Channel;
use crate::preferences;

pub fn run_list(brave_dir: &Path, format: OutputFormat) -> Result<()> {
    let local_state = local_state::read_local_state(brave_dir)?;
    let filters = local_state::get_custom_filters(&local_state);
    output::print_filters(&filters, format);
    Ok(())
}

pub fn run_add(brave_dir: &Path, filter: &str, channel: Channel, force: bool) -> Result<()> {
    let filter = filter.trim();
    anyhow::ensure!(!filter.is_empty(), "filter rule must not be empty");

    let ls_path = brave_dir.join("Local State");
    preferences::check_brave_not_running(channel, force)?;

    local_state::locked_read_modify_write(&ls_path, |state| {
        let filters_str = state
            .pointer("/brave/ad_block/custom_filters")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check for duplicate
        let already_exists = filters_str
            .lines()
            .any(|line| line.trim() == filter);
        if already_exists {
            return;
        }

        let new_filters = if filters_str.is_empty() {
            filter.to_string()
        } else {
            format!("{}\n{}", filters_str, filter)
        };

        // Ensure brave.ad_block exists
        if !state.get("brave").is_some_and(|v| v.is_object()) {
            state["brave"] = serde_json::json!({});
        }
        if !state["brave"].get("ad_block").is_some_and(|v| v.is_object()) {
            state["brave"]["ad_block"] = serde_json::json!({});
        }
        state["brave"]["ad_block"]["custom_filters"] = serde_json::Value::String(new_filters);
    })
    .context("failed to add custom filter")?;

    eprintln!("Added custom filter: {}", filter);
    Ok(())
}

pub fn run_remove(brave_dir: &Path, filter: &str, channel: Channel, force: bool) -> Result<()> {
    let filter = filter.trim();
    anyhow::ensure!(!filter.is_empty(), "filter rule must not be empty");

    let ls_path = brave_dir.join("Local State");
    preferences::check_brave_not_running(channel, force)?;

    let mut found = false;
    local_state::locked_read_modify_write(&ls_path, |state| {
        let filters_str = state
            .pointer("/brave/ad_block/custom_filters")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let new_filters: Vec<&str> = filters_str
            .lines()
            .filter(|line| {
                if line.trim() == filter {
                    found = true;
                    false
                } else {
                    true
                }
            })
            .collect();

        if found {
            state["brave"]["ad_block"]["custom_filters"] =
                serde_json::Value::String(new_filters.join("\n"));
        }
    })
    .context("failed to remove custom filter")?;

    if found {
        eprintln!("Removed custom filter: {}", filter);
    } else {
        eprintln!("Filter not found: {}", filter);
    }
    Ok(())
}

pub fn run_clear(brave_dir: &Path, channel: Channel, force: bool) -> Result<()> {
    let ls_path = brave_dir.join("Local State");
    preferences::check_brave_not_running(channel, force)?;

    local_state::locked_read_modify_write(&ls_path, |state| {
        if state.pointer("/brave/ad_block/custom_filters").is_some() {
            state["brave"]["ad_block"]["custom_filters"] =
                serde_json::Value::String(String::new());
        }
    })
    .context("failed to clear custom filters")?;

    eprintln!("Cleared all custom filters");
    Ok(())
}
