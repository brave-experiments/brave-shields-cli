use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::output::OutputFormat;
use crate::platform::Channel;
use crate::preferences;
use crate::scriptlets;

pub fn run_list(brave_dir: &Path, format: OutputFormat) -> Result<()> {
    let entries = scriptlets::read_scriptlets(brave_dir)?;
    match format {
        OutputFormat::Json => {
            let output: Vec<serde_json::Value> = entries
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "size": s.content.len(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Table => {
            if entries.is_empty() {
                println!("No custom scriptlets.");
                return Ok(());
            }
            let mut table = comfy_table::Table::new();
            table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
            table.set_header(vec!["Name", "Size"]);
            for s in &entries {
                table.add_row(vec![
                    s.name.as_str(),
                    &format!("{} bytes", s.content.len()),
                ]);
            }
            println!("{table}");
        }
    }
    Ok(())
}

pub fn run_get(brave_dir: &Path, name: &str) -> Result<()> {
    let entries = scriptlets::read_scriptlets(brave_dir)?;
    match entries.iter().find(|s| s.name == name) {
        Some(s) => {
            print!("{}", s.content);
            Ok(())
        }
        None => anyhow::bail!("scriptlet '{}' not found", name),
    }
}

pub fn run_add(
    brave_dir: &Path,
    name: &str,
    js_file: &str,
    channel: Channel,
    force: bool,
) -> Result<()> {
    let content = fs::read_to_string(js_file)
        .with_context(|| format!("failed to read {}", js_file))?;

    preferences::check_brave_not_running(channel, force)?;
    scriptlets::add_scriptlet(brave_dir, name, &content)?;
    eprintln!("Added scriptlet: {}", name);
    Ok(())
}

pub fn run_remove(
    brave_dir: &Path,
    name: &str,
    channel: Channel,
    force: bool,
) -> Result<()> {
    preferences::check_brave_not_running(channel, force)?;
    if scriptlets::remove_scriptlet(brave_dir, name)? {
        eprintln!("Removed scriptlet: {}", name);
    } else {
        eprintln!("Scriptlet not found: {}", name);
    }
    Ok(())
}

pub fn run_clear(brave_dir: &Path, channel: Channel, force: bool) -> Result<()> {
    preferences::check_brave_not_running(channel, force)?;
    scriptlets::clear_scriptlets(brave_dir)?;
    eprintln!("Cleared all custom scriptlets");
    Ok(())
}
