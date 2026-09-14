use std::collections::HashSet;
use std::fs;
use std::io;

use crate::rules::{self, RuleName};

/// Filename githist-lint looks for in the current directory when no
/// config is given explicitly. Not a CLI flag (yet): the config belongs
/// to the repo it lints, the same way `.gitignore` does, so it lives
/// next to the checkout rather than being passed around.
pub const DEFAULT_CONFIG_FILE: &str = ".githist-lint.toml";

/// The rule thresholds and enable/disable state used by `rules::check`.
/// `Config::default()` matches the hardcoded behavior this project had
/// before a config file existed.
pub struct Config {
    pub max_subject_len: usize,
    pub max_body_line_len: usize,
    disabled: HashSet<RuleName>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_subject_len: 72,
            max_body_line_len: 100,
            disabled: HashSet::new(),
        }
    }
}

impl Config {
    pub fn is_enabled(&self, rule: RuleName) -> bool {
        !self.disabled.contains(&rule)
    }

    /// Loads `DEFAULT_CONFIG_FILE` from the current directory if it
    /// exists, otherwise falls back to defaults. A missing file is not
    /// an error; a present-but-invalid one is, so typos get caught
    /// instead of silently doing nothing.
    pub fn load_default() -> io::Result<Config> {
        match fs::read_to_string(DEFAULT_CONFIG_FILE) {
            Ok(contents) => Config::parse(&contents),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e),
        }
    }

    /// Parses a small subset of TOML: comments, blank lines, and dotted
    /// `rule-name.field = value` assignments. Every line that isn't a
    /// comment or blank must be one of those assignments; anything else
    /// (a section header, a quoted string, an unknown rule or field) is
    /// rejected rather than ignored, since a silently-ignored typo in a
    /// threshold defeats the point of having one.
    fn parse(contents: &str) -> io::Result<Config> {
        let mut config = Config::default();

        for (line_no, raw_line) in contents.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| {
                invalid(line_no, "expected `rule-name.field = value`")
            })?;
            let (rule_str, field) = key.trim().rsplit_once('.').ok_or_else(|| {
                invalid(line_no, "expected a dotted key like `subject-too-long.max`")
            })?;
            let value = value.trim();

            let rule = rules::RULE_NAMES
                .iter()
                .find(|name| name.as_str() == rule_str)
                .copied()
                .ok_or_else(|| invalid(line_no, &format!("unknown rule \"{rule_str}\"")))?;

            match field {
                "enabled" => {
                    let enabled = match value {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(invalid(
                                line_no,
                                &format!("expected true or false, got \"{other}\""),
                            ))
                        }
                    };
                    if enabled {
                        config.disabled.remove(&rule);
                    } else {
                        config.disabled.insert(rule);
                    }
                }
                "max" => {
                    let max: usize = value.parse().map_err(|_| {
                        invalid(line_no, &format!("expected a whole number, got \"{value}\""))
                    })?;
                    match rule {
                        RuleName::SubjectTooLong => config.max_subject_len = max,
                        RuleName::BodyLineTooLong => config.max_body_line_len = max,
                        _ => {
                            return Err(invalid(
                                line_no,
                                &format!("rule \"{rule_str}\" has no \"max\" setting"),
                            ))
                        }
                    }
                }
                other => {
                    return Err(invalid(line_no, &format!("unknown field \"{other}\"")))
                }
            }
        }

        Ok(config)
    }
}

fn invalid(line_no: usize, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{DEFAULT_CONFIG_FILE}:{}: {reason}", line_no + 1),
    )
}
