//! Player-name and authentication policy independent of credential storage.

use crate::config::Config;
use anyhow::{Result, ensure};

/// Typed marker for a completed authentication attempt with incorrect credentials.
#[derive(Debug)]
pub struct IncorrectCredentials;

impl std::fmt::Display for IncorrectCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid credentials")
    }
}

impl std::error::Error for IncorrectCredentials {}

/// Validate player-name syntax and configured denied-name patterns.
pub fn validate_name(name: &str, c: &Config) -> Result<()> {
    validate_name_syntax(name, c)?;
    for pattern in &c.names.bad {
        let re = format!(
            "(?i)^{}$",
            regex::escape(pattern)
                .replace("\\*", ".*")
                .replace("\\?", ".")
        );
        ensure!(
            !regex::Regex::new(&re)?.is_match(name),
            "That name is not available."
        );
    }
    Ok(())
}

/// Validate syntax at the interactive username step.
pub fn validate_name_syntax(name: &str, c: &Config) -> Result<()> {
    ensure!(
        name.len() >= 2 && name.len() <= c.names.maximum_length,
        "New usernames must be between 2 and the configured maximum length."
    );
    ensure!(
        name.as_bytes()[0].is_ascii_alphabetic()
            && name.bytes().all(|b| b.is_ascii_alphanumeric()
                || b"`$_-.,\'".contains(&b)
                || (b == b' ' && c.mux.player_name_spaces)),
        "New usernames must start with a letter and use valid player-name characters."
    );
    Ok(())
}
