use crate::config::Config;
use anyhow::{Result, ensure};
use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString},
};
pub fn hash(password: &str, c: &Config) -> Result<String> {
    let params = Params::new(
        (c.security.password_hash_memlimit / 1024) as u32,
        c.security.password_hash_opslimit as u32,
        1,
        None,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string())
}
pub fn verify(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}
pub fn validate_name(name: &str, c: &Config) -> Result<()> {
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
pub fn validate_password(password: &str, c: &Config) -> Result<()> {
    ensure!(
        !password.is_empty()
            && password.len() <= c.security.player_password_length_limit
            && !password.chars().any(|ch| ch.is_control() || ch == ' ')
            && !(password.len() == 13 && password.starts_with("XX")),
        "Invalid password: use printable characters without spaces, within the configured length limit."
    );
    Ok(())
}
pub fn random_password() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b.iter().map(|v| format!("{v:02x}")).collect()
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
