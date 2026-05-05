use std::collections::BTreeMap;

use comfy_table::{Table, ContentArrangement};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Table,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "json" => Ok(OutputFormat::Json),
            "table" => Ok(OutputFormat::Table),
            _ => anyhow::bail!("unknown format '{}'. Valid: json, table", s),
        }
    }
}

/// Output for the `get` command.
pub fn print_get(
    domain: &str,
    profile: &str,
    settings: &BTreeMap<String, String>,
    format: OutputFormat,
) {
    match format {
        OutputFormat::Json => {
            let output = json!({
                "domain": domain,
                "profile": profile,
                "settings": settings,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Table => {
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Setting", "Value"]);
            for (k, v) in settings {
                table.add_row(vec![k.as_str(), v.as_str()]);
            }
            println!("Domain: {}  Profile: {}", domain, profile);
            println!("{table}");
        }
    }
}

/// A single domain's overrides for the `list` command.
#[derive(Debug)]
pub struct DomainOverrides {
    pub domain: String,
    pub overrides: BTreeMap<String, String>,
}

/// Output for the `list` command.
pub fn print_list(entries: &[DomainOverrides], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let output: Vec<Value> = entries
                .iter()
                .map(|e| {
                    json!({
                        "domain": e.domain,
                        "overrides": e.overrides,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Table => {
            if entries.is_empty() {
                println!("No per-site shield overrides found.");
                return;
            }
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Domain", "Overrides"]);
            for entry in entries {
                let overrides_str: Vec<String> = entry
                    .overrides
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                table.add_row(vec![entry.domain.as_str(), &overrides_str.join(", ")]);
            }
            println!("{table}");
        }
    }
}

/// Output for the `profiles` command.
pub fn print_profiles(profiles: &[crate::profile::ProfileInfo], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let output: Vec<Value> = profiles
                .iter()
                .map(|p| {
                    json!({
                        "dir": p.dir_name,
                        "name": p.display_name,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Table => {
            if profiles.is_empty() {
                println!("No profiles found.");
                return;
            }
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Name", "Directory"]);
            for p in profiles {
                table.add_row(vec![p.display_name.as_str(), p.dir_name.as_str()]);
            }
            println!("{table}");
        }
    }
}

/// Output for the `filters list` command.
pub fn print_filters(filters: &[String], format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let output = json!({
                "custom_filters": filters,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Table => {
            if filters.is_empty() {
                println!("No custom filter rules.");
                return;
            }
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Rule"]);
            for rule in filters {
                table.add_row(vec![rule.as_str()]);
            }
            println!("{table}");
        }
    }
}

/// Output for the `set` command.
pub fn print_set(domain: &str, setting: &str, value: &str) {
    eprintln!("Set {} = {} for {}", setting, value, domain);
}

/// Output for the `reset` command.
pub fn print_reset(domain: &str, setting: Option<&str>) {
    match setting {
        Some(s) => eprintln!("Reset {} for {}", s, domain),
        None => eprintln!("Reset all shield settings for {}", domain),
    }
}
