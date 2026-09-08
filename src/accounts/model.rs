//! Persistent account credentials and login history.

use serde::{Deserialize, Serialize};

/// Persistent authentication state associated with a player object.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub hash: Option<String>,
    pub alias: Option<String>,
    pub last_login: Option<i64>,
    pub last_site: Option<String>,
    pub successes: i64,
    pub failures: i64,
    pub unreported_failures: i64,
    pub history: Vec<Login>,
}

/// One retained authentication attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Login {
    pub success: bool,
    pub at: i64,
    pub host: String,
}
