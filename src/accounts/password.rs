//! Compatible password hashing, verification, and credential generation.

use crate::config::Config;
use anyhow::{Result, ensure};
use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::phc::PasswordHash,
};

/// Hash a password using the configured Argon2id cost.
pub fn hash(password: &str, c: &Config) -> Result<String> {
    let params = Params::new(
        (c.security.password_hash_memlimit / 1024) as u32,
        c.security.password_hash_opslimit as u32,
        1,
        None,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string())
}

/// Verify a password, treating malformed hashes as failed credentials.
pub fn verify(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

/// Distinguish a wrong password from malformed hashes and backend failures.
pub fn verify_checked(password: &str, hash: &str) -> Result<bool> {
    let hash = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    match Argon2::default().verify_password(password.as_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false),
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}

/// Validate the password grammar accepted by player creation and changes.
pub fn validate(password: &str, c: &Config) -> Result<()> {
    ensure!(
        !password.is_empty()
            && password.len() <= c.security.player_password_length_limit
            && !password.chars().any(|ch| ch.is_control() || ch == ' ')
            && !(password.len() == 13 && password.starts_with("XX")),
        "Invalid password: use printable characters without spaces, within the configured length limit."
    );
    Ok(())
}

/// Generate a random printable credential for administrative player creation.
pub fn random() -> String {
    use rand::TryRng;
    let mut bytes = [0u8; 16];
    rand::rngs::SysRng
        .try_fill_bytes(&mut bytes)
        .expect("OS randomness unavailable");
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn verifies_existing_argon2id_hash() {
        let hash = "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ$CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc";
        assert!(super::verify("password", hash));
        assert!(!super::verify("incorrect", hash));
    }
}
