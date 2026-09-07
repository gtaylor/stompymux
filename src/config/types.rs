//! Structured values shared by the typed configuration sections.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, net::IpAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Audience permitted to receive Lua callback error details.
pub enum ErrorReporting {
    Off,
    Wizards,
    All,
}

pub use crate::flags::Flag;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Legacy log decoration flags retained for compatibility.
pub enum LogOption {
    Flags,
    Location,
    Timestamp,
}
/// Red, green and blue components in the inclusive range 0–255.
pub type Rgb = [u8; 3];
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Address/mask policy entry; declaration order is retained by the compiled site policy.
pub struct SiteRule {
    pub address: IpAddr,
    pub mask: IpAddr,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(transparent)]
/// Permission tokens normalized from either a string or a string array.
pub struct Permissions(pub Vec<String>);
impl<'de> Deserialize<'de> for Permissions {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            One(String),
            Many(Vec<String>),
        }
        let words = match Input::deserialize(d)? {
            Input::One(s) => vec![s],
            Input::Many(v) => v,
        };
        Ok(Self(
            words
                .iter()
                .flat_map(|s| s.split_whitespace().map(str::to_owned))
                .collect(),
        ))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
/// Nonnegative dbref represented as a TOML object-map key.
pub struct BootstrapId(pub i64);
impl TryFrom<String> for BootstrapId {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err("bootstrap keys must be nonnegative dbrefs".into());
        }
        s.parse::<i64>().map(Self).map_err(|e| e.to_string())
    }
}
impl From<BootstrapId> for String {
    fn from(id: BootstrapId) -> Self {
        id.0.to_string()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Object kinds supported by foundational bootstrap.
pub enum BootstrapKind {
    Room,
    Player,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Declarative object created only when initializing an empty world.
pub struct BootstrapObject {
    pub r#type: BootstrapKind,
    pub name: String,
    #[serde(default)]
    pub wizard: bool,
}
/// Build the legacy foundational objects with stable identities.
pub fn default_bootstrap_objects() -> BTreeMap<BootstrapId, BootstrapObject> {
    [
        (0, BootstrapKind::Room, "Limbo"),
        (1, BootstrapKind::Player, "GOD"),
        (2, BootstrapKind::Player, "Wizard"),
        (3, BootstrapKind::Room, "Used Mech Store"),
        (4, BootstrapKind::Room, "Starter Room"),
        (5, BootstrapKind::Room, "Afterlife"),
    ]
    .into_iter()
    .map(|(id, kind, name)| {
        (
            BootstrapId(id),
            BootstrapObject {
                r#type: kind,
                name: name.into(),
                wizard: kind == BootstrapKind::Player,
            },
        )
    })
    .collect()
}
