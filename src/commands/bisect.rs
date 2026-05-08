use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write as _};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::commands::filters;
use crate::local_state;
use crate::platform::Channel;

fn channel_to_str(channel: Channel) -> &'static str {
    match channel {
        Channel::Release => "release",
        Channel::Beta => "beta",
        Channel::Nightly => "nightly",
        Channel::Dev => "dev",
    }
}

fn channel_from_str(s: &str) -> Result<Channel> {
    match s {
        "release" => Ok(Channel::Release),
        "beta" => Ok(Channel::Beta),
        "nightly" => Ok(Channel::Nightly),
        "dev" => Ok(Channel::Dev),
        _ => anyhow::bail!("unknown channel '{}'", s),
    }
}

struct BisectState {
    original_filters: String,
    channel: String,
    all_rules: Vec<String>,
    candidate_indices: Vec<usize>,
    step: u32,
}

impl BisectState {
    fn to_json(&self) -> Value {
        json!({
            "original_filters": self.original_filters,
            "channel": self.channel,
            "all_rules": self.all_rules,
            "candidate_indices": self.candidate_indices,
            "step": self.step,
        })
    }

    fn from_json(v: &Value) -> Result<Self> {
        let state = BisectState {
            original_filters: v["original_filters"]
                .as_str()
                .context("missing original_filters")?
                .to_string(),
            channel: v["channel"]
                .as_str()
                .context("missing channel")?
                .to_string(),
            all_rules: v["all_rules"]
                .as_array()
                .context("missing all_rules")?
                .iter()
                .map(|r| {
                    r.as_str()
                        .map(|s| s.to_string())
                        .context("all_rules contains non-string value")
                })
                .collect::<Result<Vec<_>>>()?,
            candidate_indices: v["candidate_indices"]
                .as_array()
                .context("missing candidate_indices")?
                .iter()
                .map(|i| {
                    i.as_u64()
                        .map(|n| n as usize)
                        .context("candidate_indices contains non-integer value")
                })
                .collect::<Result<Vec<_>>>()?,
            step: v["step"].as_u64().context("missing step")? as u32,
        };
        for &idx in &state.candidate_indices {
            anyhow::ensure!(
                idx < state.all_rules.len(),
                "candidate index {} out of range ({})",
                idx,
                state.all_rules.len()
            );
        }
        Ok(state)
    }
}

enum Answer {
    Yes,
    No,
    Quit,
}

/// Return value from bisect_loop indicating how it exited.
enum BisectOutcome {
    /// Found the problematic rule, or issue wasn't present. Restore and clean up.
    Done,
    /// User quit mid-bisect. Leave state file, don't restore filters.
    Suspended,
}

// Note: bisect does not call check_brave_not_running before writing filters.
// This is intentional: the interactive loop explicitly asks the user to restart
// the browser between steps, so the tool writes while Brave may be running and
// relies on the user to restart for changes to take effect.
pub fn run(
    brave_dir: &Path,
    file_path: Option<&str>,
    resume_path: Option<&str>,
    output_dir: Option<&str>,
    channel: Channel,
) -> Result<()> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = interrupted.clone();
    ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::SeqCst);
    })?;

    let out_dir = match output_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::current_dir()?,
    };
    let state_path = out_dir.join("bisect-state.json");
    let working_file = out_dir.join("bisect-filters.txt");

    let mut state = match resume_path {
        Some(path) => {
            let content = fs::read_to_string(path)
                .with_context(|| format!("failed to read bisect state from {}", path))?;
            let v: Value = serde_json::from_str(&content)
                .context("failed to parse bisect state file")?;
            let state = BisectState::from_json(&v)
                .context("invalid bisect state file")?;
            let saved_channel = channel_from_str(&state.channel)?;
            anyhow::ensure!(
                channel_to_str(saved_channel) == channel_to_str(channel),
                "state file channel ({}) does not match current channel ({})",
                state.channel,
                channel_to_str(channel)
            );
            anyhow::ensure!(
                !state.candidate_indices.is_empty(),
                "bisect state has no candidates"
            );
            eprintln!(
                "Resuming bisect: {} candidates remaining (step {})",
                state.candidate_indices.len(),
                state.step
            );
            state
        }
        None => {
            let file_path = file_path.context(
                "a filter rules file is required (or use --resume to continue a previous session)"
            )?;
            let rules = read_rules_from_file(file_path)?;
            anyhow::ensure!(!rules.is_empty(), "no filter rules found in {}", file_path);

            let local_state = local_state::read_local_state(brave_dir)?;
            let original = local_state
                .pointer("/brave/ad_block/custom_filters")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            BisectState {
                original_filters: original,
                channel: channel_to_str(channel).to_string(),
                all_rules: rules,
                candidate_indices: Vec::new(), // filled below
                step: 0,
            }
        }
    };

    // For new sessions, populate candidate_indices (skipping resume which already has them)
    if state.candidate_indices.is_empty() && state.step == 0 {
        state.candidate_indices = (0..state.all_rules.len()).collect();
        save_state(&state_path, &state)?;
    }

    let steps = if state.candidate_indices.len() <= 1 {
        1
    } else {
        (state.candidate_indices.len() as f64).log2().ceil() as u32
    };
    eprintln!(
        "Bisecting {} rules (~{} steps)",
        state.candidate_indices.len(),
        steps
    );
    eprintln!("Working file: {}", working_file.display());

    let outcome = bisect_loop(brave_dir, &mut state, &state_path, &working_file, &interrupted)?;

    match outcome {
        BisectOutcome::Done => {
            eprintln!("Quit the browser before restoring original filters.");
            match prompt("Ready to restore? [y]es / [n]o / [q]uit: ", &interrupted)? {
                Answer::Yes => {
                    eprintln!("Restoring original custom filters...");
                    let ls_path = brave_dir.join("Local State");
                    if let Err(e) = filters::write_all_filters(&ls_path, &state.original_filters) {
                        eprintln!("Warning: failed to restore filters: {}", e);
                    }
                    let _ = fs::remove_file(&state_path);
                    let _ = fs::remove_file(&working_file);
                }
                Answer::No | Answer::Quit => {
                    eprintln!("Skipping restore. Original filters are saved in the state file.");
                    save_state(&state_path, &state)?;
                    eprintln!("State saved to {}", state_path.display());
                }
            }
        }
        BisectOutcome::Suspended => {
            // Leave state and working file for --resume
        }
    }

    Ok(())
}

/// Convert a filter rule into its exception form.
/// - Network rules (`||example.com^`) get `@@` prefix
/// - Cosmetic rules (`example.com##.ad`) get `##` replaced with `#@#`
/// - Rules already in exception form are returned as-is
/// Note: some rule types ($removeparam, $redirect, etc.) may not be fully
/// neutralized by this approach. This is a known limitation.
fn make_exception(rule: &str) -> String {
    if rule.starts_with("@@") || rule.starts_with("#@#") {
        return rule.to_string();
    }
    // Cosmetic filter: contains ## (but not scriptlet ###+js)
    if let Some(pos) = rule.find("##") {
        if !rule[pos..].starts_with("###+") {
            let (domain, selector) = rule.split_at(pos);
            return format!("{}#@#{}", domain, &selector[2..]);
        }
    }
    format!("@@{}", rule)
}

/// Write the working filter file and load it into Local State.
fn write_and_load_filters(
    working_file: &Path,
    ls_path: &Path,
    all_rules: &[String],
    excepted_indices: &[usize],
) -> Result<()> {
    let excepted: HashSet<usize> = excepted_indices.iter().copied().collect();
    let mut lines = Vec::with_capacity(all_rules.len());
    for (i, rule) in all_rules.iter().enumerate() {
        if excepted.contains(&i) {
            lines.push(make_exception(rule));
        } else {
            lines.push(rule.clone());
        }
    }
    let content = lines.join("\n");

    fs::write(working_file, &content)
        .with_context(|| format!("failed to write {}", working_file.display()))?;

    filters::write_all_filters(ls_path, &content)
}

fn bisect_loop(
    brave_dir: &Path,
    state: &mut BisectState,
    state_path: &Path,
    working_file: &Path,
    interrupted: &AtomicBool,
) -> Result<BisectOutcome> {
    let ls_path = brave_dir.join("Local State");

    // On a fresh session, load all rules and confirm the issue is present.
    // On resume (step > 0), skip this since the user already confirmed.
    if state.step == 0 {
        eprintln!("Loading all {} rules as custom filters...", state.all_rules.len());
        write_and_load_filters(working_file, &ls_path, &state.all_rules, &[])?;
        eprintln!("Custom filters loaded. Restart the browser and confirm the issue is present.");

        let initial = prompt("Is the issue present? [y]es / [n]o / [q]uit: ", interrupted)?;
        match initial {
            Answer::Yes => {}
            Answer::No => {
                eprintln!("Issue is not present with these filters. Nothing to bisect.");
                return Ok(BisectOutcome::Done);
            }
            Answer::Quit => {
                save_state(state_path, state)?;
                eprintln!("State saved to {}", state_path.display());
                return Ok(BisectOutcome::Suspended);
            }
        }
    }

    if state.candidate_indices.len() == 1 {
        let rule = &state.all_rules[state.candidate_indices[0]];
        eprintln!("Only one rule; it must be: {}", rule);
        println!("{}", rule);
        return Ok(BisectOutcome::Done);
    }

    loop {
        if interrupted.load(Ordering::SeqCst) {
            eprintln!("\nInterrupted.");
            save_state(state_path, state)?;
            eprintln!("State saved to {}", state_path.display());
            return Ok(BisectOutcome::Suspended);
        }

        state.step += 1;
        let mid = state.candidate_indices.len() / 2;
        let first_half: Vec<usize> = state.candidate_indices[..mid].to_vec();
        let second_half: Vec<usize> = state.candidate_indices[mid..].to_vec();

        eprintln!(
            "\nStep {} | Excepting out {} of {} candidate rules (keeping {} active)",
            state.step,
            first_half.len(),
            state.candidate_indices.len(),
            second_half.len(),
        );

        write_and_load_filters(working_file, &ls_path, &state.all_rules, &first_half)?;
        eprintln!(
            "Custom filters updated (see {}). Restart the browser and check if the issue persists.",
            working_file.display()
        );

        let answer = prompt("Issue still present? [y]es / [n]o / [q]uit: ", interrupted)?;

        match answer {
            Answer::Yes => {
                // Issue still present; the excepted rules (first half) are innocent.
                // The bad rule is in the second half (still active).
                state.candidate_indices = second_half;
            }
            Answer::No => {
                // Issue gone; the excepted rules (first half) contain the bad one.
                state.candidate_indices = first_half;
            }
            Answer::Quit => {
                eprintln!("Quitting bisect.");
                save_state(state_path, state)?;
                eprintln!("State saved to {}", state_path.display());
                return Ok(BisectOutcome::Suspended);
            }
        }

        save_state(state_path, state)?;

        if state.candidate_indices.len() == 1 {
            let rule = &state.all_rules[state.candidate_indices[0]];
            eprintln!("\nFound problematic rule:");
            println!("{}", rule);
            return Ok(BisectOutcome::Done);
        }
    }
}

fn read_rules_from_file(path: &str) -> Result<Vec<String>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let rules: Vec<String> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('!')
        })
        .map(|line| line.to_string())
        .collect();
    Ok(rules)
}

fn save_state(path: &Path, state: &BisectState) -> Result<()> {
    let content = serde_json::to_string_pretty(&state.to_json())?;
    fs::write(path, content)
        .with_context(|| format!("failed to save bisect state to {}", path.display()))
}

fn prompt(message: &str, interrupted: &AtomicBool) -> Result<Answer> {
    loop {
        eprint!("{}", message);
        io::stderr().flush()?;

        let stdin = io::stdin();
        let line = stdin.lock().lines().next();

        if interrupted.load(Ordering::SeqCst) {
            return Ok(Answer::Quit);
        }

        let line = match line {
            Some(Ok(l)) => l,
            Some(Err(e)) => return Err(e.into()),
            None => return Ok(Answer::Quit),
        };

        match line.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(Answer::Yes),
            "n" | "no" => return Ok(Answer::No),
            "q" | "quit" => return Ok(Answer::Quit),
            _ => eprintln!("Please answer y, n, or q."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_rules_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rules.txt");
        fs::write(&path, "||a.com^\n\n||b.com^\n!comment\n\n").unwrap();
        let rules = read_rules_from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(rules, vec!["||a.com^", "||b.com^"]);
    }

    #[test]
    fn test_read_rules_skips_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rules.txt");
        fs::write(&path, "!title\n||a.com^\n! another comment\n||b.com^").unwrap();
        let rules = read_rules_from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(rules, vec!["||a.com^", "||b.com^"]);
    }

    #[test]
    fn test_read_rules_empty_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rules.txt");
        fs::write(&path, "\n\n").unwrap();
        let rules = read_rules_from_file(path.to_str().unwrap()).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_read_rules_only_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rules.txt");
        fs::write(&path, "! comment 1\n! comment 2\n").unwrap();
        let rules = read_rules_from_file(path.to_str().unwrap()).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_save_and_load_state() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        let state = BisectState {
            original_filters: "||original.com^".to_string(),
            channel: "release".to_string(),
            all_rules: vec!["||a.com^".to_string(), "||b.com^".to_string()],
            candidate_indices: vec![0, 1],
            step: 0,
        };
        save_state(&path, &state).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();
        let loaded = BisectState::from_json(&v).unwrap();
        assert_eq!(loaded.original_filters, "||original.com^");
        assert_eq!(loaded.all_rules.len(), 2);
        assert_eq!(loaded.candidate_indices, vec![0, 1]);
        assert_eq!(loaded.step, 0);
    }

    #[test]
    fn test_save_state_preserves_step() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        let state = BisectState {
            original_filters: "".to_string(),
            channel: "nightly".to_string(),
            all_rules: vec!["||a.com^".to_string()],
            candidate_indices: vec![0],
            step: 5,
        };
        save_state(&path, &state).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();
        let loaded = BisectState::from_json(&v).unwrap();
        assert_eq!(loaded.step, 5);
        assert_eq!(loaded.channel, "nightly");
    }

    #[test]
    fn test_write_and_load_filters_no_exceptions() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(&ls_path, "{}").unwrap();

        let rules = vec!["||a.com^".to_string(), "||b.com^".to_string()];
        write_and_load_filters(&working, &ls_path, &rules, &[]).unwrap();

        let content = fs::read_to_string(&working).unwrap();
        assert_eq!(content, "||a.com^\n||b.com^");
    }

    #[test]
    fn test_write_and_load_filters_with_exceptions() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(&ls_path, "{}").unwrap();

        let rules = vec![
            "||a.com^".to_string(),
            "||b.com^".to_string(),
            "||c.com^".to_string(),
            "||d.com^".to_string(),
        ];
        write_and_load_filters(&working, &ls_path, &rules, &[0, 1]).unwrap();

        let content = fs::read_to_string(&working).unwrap();
        assert_eq!(content, "@@||a.com^\n@@||b.com^\n||c.com^\n||d.com^");

        // Verify it was also loaded into Local State
        let ls: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ls_path).unwrap()).unwrap();
        let loaded = ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap();
        assert_eq!(loaded, content);
    }

    #[test]
    fn test_write_and_load_filters_does_not_double_prefix_exceptions() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(&ls_path, "{}").unwrap();

        let rules = vec![
            "||a.com^".to_string(),
            "@@||b.com^".to_string(), // already an exception rule
            "||c.com^".to_string(),
        ];
        // Exception out indices 0 and 1
        write_and_load_filters(&working, &ls_path, &rules, &[0, 1]).unwrap();

        let content = fs::read_to_string(&working).unwrap();
        // Index 0 gets @@, index 1 already has @@ so stays as-is
        assert_eq!(content, "@@||a.com^\n@@||b.com^\n||c.com^");
    }

    #[test]
    fn test_write_and_load_filters_all_excepted() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(&ls_path, "{}").unwrap();

        let rules = vec!["||a.com^".to_string(), "||b.com^".to_string()];
        write_and_load_filters(&working, &ls_path, &rules, &[0, 1]).unwrap();

        let content = fs::read_to_string(&working).unwrap();
        assert_eq!(content, "@@||a.com^\n@@||b.com^");
    }

    #[test]
    fn test_write_and_load_filters_single_exception() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(&ls_path, "{}").unwrap();

        let rules = vec![
            "||a.com^".to_string(),
            "||b.com^".to_string(),
            "||c.com^".to_string(),
        ];
        write_and_load_filters(&working, &ls_path, &rules, &[1]).unwrap();

        let content = fs::read_to_string(&working).unwrap();
        assert_eq!(content, "||a.com^\n@@||b.com^\n||c.com^");
    }

    #[test]
    fn test_write_and_load_preserves_other_local_state() {
        let tmp = tempfile::tempdir().unwrap();
        let working = tmp.path().join("bisect-filters.txt");
        let ls_path = tmp.path().join("Local State");
        fs::write(
            &ls_path,
            serde_json::to_string(&serde_json::json!({
                "profile": {"last_used": "Default"},
                "brave": {"ad_block": {"custom_filters": "||old.com^"}}
            }))
            .unwrap(),
        )
        .unwrap();

        let rules = vec!["||new.com^".to_string()];
        write_and_load_filters(&working, &ls_path, &rules, &[]).unwrap();

        let ls: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ls_path).unwrap()).unwrap();
        assert_eq!(ls["profile"]["last_used"], "Default");
        assert_eq!(
            ls["brave"]["ad_block"]["custom_filters"].as_str().unwrap(),
            "||new.com^"
        );
    }

    #[test]
    fn test_bisect_split_halves_8_rules() {
        let candidates: Vec<usize> = (0..8).collect();
        let mid = candidates.len() / 2;
        assert_eq!(&candidates[..mid], &[0, 1, 2, 3]);
        assert_eq!(&candidates[mid..], &[4, 5, 6, 7]);
    }

    #[test]
    fn test_bisect_split_halves_odd() {
        let candidates: Vec<usize> = (0..5).collect();
        let mid = candidates.len() / 2;
        // First half is smaller
        assert_eq!(&candidates[..mid], &[0, 1]);
        assert_eq!(&candidates[mid..], &[2, 3, 4]);
    }

    #[test]
    fn test_bisect_split_halves_2() {
        let candidates: Vec<usize> = vec![3, 7];
        let mid = candidates.len() / 2;
        assert_eq!(&candidates[..mid], &[3]);
        assert_eq!(&candidates[mid..], &[7]);
    }

    #[test]
    fn test_read_rules_nonexistent_file() {
        let result = read_rules_from_file("/nonexistent/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_make_exception_network_rule() {
        assert_eq!(make_exception("||example.com^"), "@@||example.com^");
    }

    #[test]
    fn test_make_exception_cosmetic_rule() {
        assert_eq!(
            make_exception("example.com##.ad-banner"),
            "example.com#@#.ad-banner"
        );
    }

    #[test]
    fn test_make_exception_already_exception_network() {
        assert_eq!(make_exception("@@||example.com^"), "@@||example.com^");
    }

    #[test]
    fn test_make_exception_already_exception_cosmetic() {
        assert_eq!(
            make_exception("#@#.ad-banner"),
            "#@#.ad-banner"
        );
    }

    #[test]
    fn test_make_exception_cosmetic_no_domain() {
        // ##.ad is a global cosmetic rule (no domain), still uses #@# exception
        assert_eq!(make_exception("##.ad"), "#@#.ad");
    }

    #[test]
    fn test_make_exception_scriptlet() {
        // Scriptlet injection: ###+js(...) should get @@ prefix, not #@#
        assert_eq!(
            make_exception("example.com###+js(abort-on-property-read, foo)"),
            "@@example.com###+js(abort-on-property-read, foo)"
        );
    }

    #[test]
    fn test_make_exception_cosmetic_with_subdomain() {
        assert_eq!(
            make_exception("sub.example.com##div.ad"),
            "sub.example.com#@#div.ad"
        );
    }

    #[test]
    fn test_from_json_rejects_non_integer_indices() {
        let v = serde_json::json!({
            "original_filters": "",
            "channel": "release",
            "all_rules": ["||a.com^"],
            "candidate_indices": [0, "bad", 2],
            "step": 0,
        });
        let result = BisectState::from_json(&v);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_rejects_out_of_bounds_indices() {
        let v = serde_json::json!({
            "original_filters": "",
            "channel": "release",
            "all_rules": ["||a.com^"],
            "candidate_indices": [0, 99],
            "step": 0,
        });
        let result = BisectState::from_json(&v);
        assert!(result.is_err());
        assert!(format!("{}", result.err().unwrap()).contains("out of range"));
    }

    #[test]
    fn test_from_json_rejects_non_string_rules() {
        let v = serde_json::json!({
            "original_filters": "",
            "channel": "release",
            "all_rules": ["||a.com^", 42],
            "candidate_indices": [0],
            "step": 0,
        });
        let result = BisectState::from_json(&v);
        assert!(result.is_err());
        assert!(format!("{}", result.err().unwrap()).contains("non-string"));
    }

    #[test]
    fn test_from_json_rejects_missing_step() {
        let v = serde_json::json!({
            "original_filters": "",
            "channel": "release",
            "all_rules": ["||a.com^"],
            "candidate_indices": [0],
        });
        let result = BisectState::from_json(&v);
        assert!(result.is_err());
    }
}
