mod chromium_time;
mod commands;
mod local_state;
mod output;
mod platform;
mod preferences;
mod profile;
mod shields;

use anyhow::Result;
use clap::{Parser, Subcommand};

use output::OutputFormat;
use platform::Channel;

#[derive(Parser)]
#[command(name = "brave-shields-cli", about = "Read and write Brave Shields per-site preferences")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Profile display name or directory name (default: last used)
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Brave channel: release, beta, nightly, dev [env: BRAVE_SHIELDS_CLI_CHANNEL=]
    #[arg(long, global = true, env = "BRAVE_SHIELDS_CLI_CHANNEL")]
    channel: Option<Channel>,

    /// Override Brave data directory (takes precedence over --channel)
    #[arg(long = "brave-dir", global = true)]
    brave_dir: Option<String>,

    /// Output format: json or table
    #[arg(long, global = true, default_value = "json")]
    format: String,
}

#[derive(Subcommand)]
enum Command {
    /// Show shield settings for a domain
    Get {
        /// Domain to query (e.g. example.com)
        domain: String,
    },
    /// Set a shield setting for a domain
    Set {
        /// Domain to modify (e.g. example.com)
        domain: String,
        /// Setting name (shields, ads, fingerprinting, https-upgrade, scripts)
        setting: String,
        /// Value to set (depends on setting)
        value: String,
    },
    /// List all per-site shield overrides
    List,
    /// List available browser profiles
    Profiles,
    /// Reset shield settings for a domain
    Reset {
        /// Domain to reset (e.g. example.com)
        domain: String,
        /// Specific setting to reset (omit to reset all)
        setting: Option<String>,
    },
    /// Manage custom adblock filter rules
    Filters {
        #[command(subcommand)]
        action: FiltersAction,
    },
}

#[derive(Subcommand)]
enum FiltersAction {
    /// List all custom filter rules
    List,
    /// Add a custom filter rule
    Add {
        /// Filter rule (e.g. "||example.com^" or "example.com##h1")
        rule: String,
    },
    /// Remove a custom filter rule
    Remove {
        /// Filter rule to remove (must match exactly)
        rule: String,
    },
    /// Remove all custom filter rules
    Clear,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let format = OutputFormat::from_str(&cli.format)?;

    let channel = cli.channel.unwrap_or_default();
    let brave_dir = platform::resolve_brave_dir(cli.brave_dir.as_deref(), channel)?;

    // Commands that don't need a resolved profile
    if let Command::Profiles = cli.command {
        commands::profiles::run(&brave_dir, format)?;
        return Ok(());
    }
    if let Command::Filters { ref action } = cli.command {
        match action {
            FiltersAction::List => commands::filters::run_list(&brave_dir, format)?,
            FiltersAction::Add { rule } => commands::filters::run_add(&brave_dir, rule, channel)?,
            FiltersAction::Remove { rule } => {
                commands::filters::run_remove(&brave_dir, rule, channel)?
            }
            FiltersAction::Clear => commands::filters::run_clear(&brave_dir, channel)?,
        }
        return Ok(());
    }

    let profile_info = profile::resolve_profile(&brave_dir, cli.profile.as_deref())?;
    let prefs_path = profile::preferences_path(&brave_dir, &profile_info);

    match cli.command {
        Command::Get { domain } => {
            commands::get::run(&prefs_path, &domain, &profile_info.display_name, format)?;
        }
        Command::Set {
            domain,
            setting,
            value,
        } => {
            commands::set::run(&prefs_path, &domain, &setting, &value, channel)?;
        }
        Command::List => {
            commands::list::run(&prefs_path, format)?;
        }
        Command::Reset { domain, setting } => {
            commands::reset::run(&prefs_path, &domain, setting.as_deref(), channel)?;
        }
        Command::Profiles | Command::Filters { .. } => unreachable!(),
    }

    Ok(())
}
