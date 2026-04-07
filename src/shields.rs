use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};

/// All shield setting types we support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShieldSetting {
    Shields,
    Ads,
    Fingerprinting,
    HttpsUpgrade,
    Scripts,
}

impl ShieldSetting {
    pub const ALL: &'static [ShieldSetting] = &[
        ShieldSetting::Shields,
        ShieldSetting::Ads,
        ShieldSetting::Fingerprinting,
        ShieldSetting::HttpsUpgrade,
        ShieldSetting::Scripts,
    ];

    /// CLI name for this setting.
    pub fn cli_name(&self) -> &'static str {
        match self {
            ShieldSetting::Shields => "shields",
            ShieldSetting::Ads => "ads",
            ShieldSetting::Fingerprinting => "fingerprinting",
            ShieldSetting::HttpsUpgrade => "https-upgrade",
            ShieldSetting::Scripts => "scripts",
        }
    }

    /// Parse a CLI setting name.
    pub fn from_cli(s: &str) -> Result<ShieldSetting> {
        match s {
            "shields" => Ok(ShieldSetting::Shields),
            "ads" => Ok(ShieldSetting::Ads),
            "fingerprinting" => Ok(ShieldSetting::Fingerprinting),
            "https-upgrade" => Ok(ShieldSetting::HttpsUpgrade),
            "scripts" => Ok(ShieldSetting::Scripts),
            _ => bail!(
                "unknown setting '{}'. Valid settings: shields, ads, fingerprinting, https-upgrade, scripts",
                s
            ),
        }
    }

    /// JSON keys in Preferences exceptions that this setting reads/writes.
    pub fn json_keys(&self) -> &'static [&'static str] {
        match self {
            ShieldSetting::Shields => &["braveShields"],
            ShieldSetting::Ads => &["shieldsAds", "trackers", "cosmeticFilteringV2"],
            ShieldSetting::Fingerprinting => &["fingerprintingV2"],
            ShieldSetting::HttpsUpgrade => &["httpsUpgrades"],
            ShieldSetting::Scripts => &["javascript"],
        }
    }

    /// The primary JSON key used for reading the setting value.
    pub fn primary_json_key(&self) -> &'static str {
        match self {
            ShieldSetting::Shields => "braveShields",
            ShieldSetting::Ads => "shieldsAds",
            ShieldSetting::Fingerprinting => "fingerprintingV2",
            ShieldSetting::HttpsUpgrade => "httpsUpgrades",
            ShieldSetting::Scripts => "javascript",
        }
    }

    /// Valid CLI values for this setting.
    pub fn valid_values(&self) -> &'static [&'static str] {
        match self {
            ShieldSetting::Shields => &["on", "off"],
            ShieldSetting::Ads => &["standard", "aggressive", "disabled"],
            ShieldSetting::Fingerprinting => &["standard", "aggressive", "disabled"],
            ShieldSetting::HttpsUpgrade => &["standard", "strict", "disabled"],
            ShieldSetting::Scripts => &["allow", "block"],
        }
    }
}

/// Validate that a domain contains only safe characters.
fn validate_domain(domain: &str) -> Result<()> {
    ensure!(!domain.is_empty(), "domain must not be empty");
    ensure!(
        domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'),
        "invalid domain '{}': only ASCII alphanumeric characters, dots, hyphens, and underscores are allowed",
        domain
    );
    Ok(())
}

/// Format a domain into the pattern key used in Preferences exceptions.
pub fn domain_pattern(domain: &str) -> Result<String> {
    validate_domain(domain)?;
    Ok(format!("{},*", domain))
}

/// Extract the domain from a pattern key like "example.com,*".
pub fn domain_from_pattern(pattern: &str) -> Option<&str> {
    pattern.strip_suffix(",*")
}

/// Convert a CLI value string to the stored JSON entries for a setting.
/// Returns a vec of (json_key, entry_value) pairs.
pub fn to_stored(
    setting: ShieldSetting,
    cli_value: &str,
    last_modified: &str,
) -> Result<Vec<(&'static str, Value)>> {
    match setting {
        ShieldSetting::Shields => {
            let stored = match cli_value {
                "on" => 1,
                "off" => 2,
                _ => bail!("invalid value '{}' for shields. Valid: on, off", cli_value),
            };
            Ok(vec![("braveShields", simple_entry(stored, last_modified))])
        }
        ShieldSetting::Fingerprinting => {
            let stored = match cli_value {
                "standard" => 3,
                "aggressive" => 2,
                "disabled" => 1,
                _ => bail!(
                    "invalid value '{}' for fingerprinting. Valid: standard, aggressive, disabled",
                    cli_value
                ),
            };
            Ok(vec![("fingerprintingV2", simple_entry(stored, last_modified))])
        }
        ShieldSetting::HttpsUpgrade => {
            let stored = match cli_value {
                "standard" => 3,
                "strict" => 2,
                "disabled" => 1,
                _ => bail!(
                    "invalid value '{}' for https-upgrade. Valid: standard, strict, disabled",
                    cli_value
                ),
            };
            Ok(vec![("httpsUpgrades", simple_entry(stored, last_modified))])
        }
        ShieldSetting::Scripts => {
            let stored = match cli_value {
                "allow" => 1,
                "block" => 2,
                _ => bail!("invalid value '{}' for scripts. Valid: allow, block", cli_value),
            };
            Ok(vec![("javascript", simple_entry(stored, last_modified))])
        }
        ShieldSetting::Ads => {
            let (ads_val, cosmetic_val) = match cli_value {
                "standard" => (2, 2),
                "aggressive" => (2, 1),
                "disabled" => (1, 0),
                _ => bail!(
                    "invalid value '{}' for ads. Valid: standard, aggressive, disabled",
                    cli_value
                ),
            };
            Ok(vec![
                ("shieldsAds", simple_entry(ads_val, last_modified)),
                ("trackers", simple_entry(ads_val, last_modified)),
                ("cosmeticFilteringV2", cosmetic_entry(cosmetic_val, last_modified)),
            ])
        }
    }
}

/// Convert stored JSON values back to a CLI value string.
pub fn from_stored(
    setting: ShieldSetting,
    primary_value: i64,
    cosmetic_value: Option<i64>,
) -> Result<String> {
    match setting {
        ShieldSetting::Shields => match primary_value {
            1 => Ok("on".to_string()),
            2 => Ok("off".to_string()),
            _ => bail!("unknown stored value {} for braveShields", primary_value),
        },
        ShieldSetting::Fingerprinting => match primary_value {
            3 => Ok("standard".to_string()),
            2 => Ok("aggressive".to_string()),
            1 => Ok("disabled".to_string()),
            _ => bail!("unknown stored value {} for fingerprintingV2", primary_value),
        },
        ShieldSetting::HttpsUpgrade => match primary_value {
            3 => Ok("standard".to_string()),
            2 => Ok("strict".to_string()),
            1 => Ok("disabled".to_string()),
            _ => bail!("unknown stored value {} for httpsUpgrades", primary_value),
        },
        ShieldSetting::Scripts => match primary_value {
            1 => Ok("allow".to_string()),
            2 => Ok("block".to_string()),
            _ => bail!("unknown stored value {} for javascript", primary_value),
        },
        ShieldSetting::Ads => {
            let cosmetic = cosmetic_value.unwrap_or(-1);
            match (primary_value, cosmetic) {
                (2, 2) => Ok("standard".to_string()),
                (2, 1) => Ok("aggressive".to_string()),
                (1, 0) => Ok("disabled".to_string()),
                (1, _) => Ok("disabled".to_string()),
                (2, _) => Ok("standard".to_string()),
                _ => bail!(
                    "unknown stored values shieldsAds={}, cosmeticFilteringV2={} for ads",
                    primary_value,
                    cosmetic
                ),
            }
        }
    }
}

/// Read the stored integer "setting" value from an exception entry.
pub fn read_setting_value(entry: &Value) -> Option<i64> {
    entry.get("setting").and_then(|v| v.as_i64())
}

/// Read the cosmetic filtering value from a cosmeticFilteringV2 exception entry.
/// The setting is stored as {"setting": {"cosmeticFilteringV2": N}}.
pub fn read_cosmetic_value(entry: &Value) -> Option<i64> {
    entry
        .get("setting")
        .and_then(|v| v.get("cosmeticFilteringV2"))
        .and_then(|v| v.as_i64())
}

fn simple_entry(setting: i64, last_modified: &str) -> Value {
    json!({
        "last_modified": last_modified,
        "setting": setting
    })
}

fn cosmetic_entry(cosmetic_val: i64, last_modified: &str) -> Value {
    json!({
        "last_modified": last_modified,
        "setting": {
            "cosmeticFilteringV2": cosmetic_val
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shields_to_stored() {
        let entries = to_stored(ShieldSetting::Shields, "on", "0").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "braveShields");
        assert_eq!(entries[0].1["setting"], 1);

        let entries = to_stored(ShieldSetting::Shields, "off", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 2);
    }

    #[test]
    fn test_shields_from_stored() {
        assert_eq!(from_stored(ShieldSetting::Shields, 1, None).unwrap(), "on");
        assert_eq!(from_stored(ShieldSetting::Shields, 2, None).unwrap(), "off");
    }

    #[test]
    fn test_fingerprinting_to_stored() {
        let entries = to_stored(ShieldSetting::Fingerprinting, "standard", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 3);
        let entries = to_stored(ShieldSetting::Fingerprinting, "aggressive", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 2);
        let entries = to_stored(ShieldSetting::Fingerprinting, "disabled", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 1);
    }

    #[test]
    fn test_fingerprinting_from_stored() {
        assert_eq!(from_stored(ShieldSetting::Fingerprinting, 3, None).unwrap(), "standard");
        assert_eq!(from_stored(ShieldSetting::Fingerprinting, 2, None).unwrap(), "aggressive");
        assert_eq!(from_stored(ShieldSetting::Fingerprinting, 1, None).unwrap(), "disabled");
    }

    #[test]
    fn test_https_upgrade_to_stored() {
        let entries = to_stored(ShieldSetting::HttpsUpgrade, "standard", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 3);
        let entries = to_stored(ShieldSetting::HttpsUpgrade, "strict", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 2);
        let entries = to_stored(ShieldSetting::HttpsUpgrade, "disabled", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 1);
    }

    #[test]
    fn test_https_upgrade_from_stored() {
        assert_eq!(from_stored(ShieldSetting::HttpsUpgrade, 3, None).unwrap(), "standard");
        assert_eq!(from_stored(ShieldSetting::HttpsUpgrade, 2, None).unwrap(), "strict");
        assert_eq!(from_stored(ShieldSetting::HttpsUpgrade, 1, None).unwrap(), "disabled");
    }

    #[test]
    fn test_scripts_to_stored() {
        let entries = to_stored(ShieldSetting::Scripts, "allow", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 1);
        let entries = to_stored(ShieldSetting::Scripts, "block", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 2);
    }

    #[test]
    fn test_scripts_from_stored() {
        assert_eq!(from_stored(ShieldSetting::Scripts, 1, None).unwrap(), "allow");
        assert_eq!(from_stored(ShieldSetting::Scripts, 2, None).unwrap(), "block");
    }

    #[test]
    fn test_ads_to_stored() {
        let entries = to_stored(ShieldSetting::Ads, "standard", "0").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].0, "shieldsAds");
        assert_eq!(entries[0].1["setting"], 2);
        assert_eq!(entries[1].0, "trackers");
        assert_eq!(entries[1].1["setting"], 2);
        assert_eq!(entries[2].0, "cosmeticFilteringV2");
        assert_eq!(entries[2].1["setting"]["cosmeticFilteringV2"], 2);

        let entries = to_stored(ShieldSetting::Ads, "aggressive", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 2);
        assert_eq!(entries[2].1["setting"]["cosmeticFilteringV2"], 1);

        let entries = to_stored(ShieldSetting::Ads, "disabled", "0").unwrap();
        assert_eq!(entries[0].1["setting"], 1);
        assert_eq!(entries[1].1["setting"], 1);
        assert_eq!(entries[2].1["setting"]["cosmeticFilteringV2"], 0);
    }

    #[test]
    fn test_ads_from_stored() {
        assert_eq!(from_stored(ShieldSetting::Ads, 2, Some(2)).unwrap(), "standard");
        assert_eq!(from_stored(ShieldSetting::Ads, 2, Some(1)).unwrap(), "aggressive");
        assert_eq!(from_stored(ShieldSetting::Ads, 1, Some(0)).unwrap(), "disabled");
    }

    #[test]
    fn test_domain_pattern() {
        assert_eq!(domain_pattern("example.com").unwrap(), "example.com,*");
    }

    #[test]
    fn test_domain_from_pattern() {
        assert_eq!(domain_from_pattern("example.com,*"), Some("example.com"));
        assert_eq!(domain_from_pattern("invalid"), None);
    }

    #[test]
    fn test_unknown_stored_value() {
        assert!(from_stored(ShieldSetting::Shields, 99, None).is_err());
    }

    #[test]
    fn test_invalid_cli_value() {
        assert!(to_stored(ShieldSetting::Shields, "invalid", "0").is_err());
    }

    #[test]
    fn test_validate_domain_rejects_quotes() {
        assert!(domain_pattern("foo\"bar").is_err());
        assert!(domain_pattern("foo'bar").is_err());
    }

    #[test]
    fn test_validate_domain_rejects_commas() {
        assert!(domain_pattern("foo,bar").is_err());
    }

    #[test]
    fn test_validate_domain_rejects_unicode() {
        assert!(domain_pattern("exämple.com").is_err());
        assert!(domain_pattern("\u{200b}evil.com").is_err());
    }

    #[test]
    fn test_validate_domain_rejects_empty() {
        assert!(domain_pattern("").is_err());
    }

    #[test]
    fn test_validate_domain_accepts_valid() {
        assert!(domain_pattern("example.com").is_ok());
        assert!(domain_pattern("sub.example.com").is_ok());
        assert!(domain_pattern("my-site.co.uk").is_ok());
        assert!(domain_pattern("test_site.com").is_ok());
    }
}
