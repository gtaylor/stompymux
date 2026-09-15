//! C-style access edits and shared role, type and runtime prerequisite evaluation.
use crate::{
    flags,
    world::{Kind, ObjectId, World},
};
use anyhow::{Result, bail, ensure};

/// Effective permission bits shared by native commands, Lua declarations and lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Permissions(u16);
/// Catalog order and minimum C token abbreviations.
pub const CATALOG: [(&str, usize, Permissions); 9] = [
    ("god", 2, Permissions::GOD),
    ("wizard", 3, Permissions::WIZARD),
    ("no_suspect", 5, Permissions::NO_SUSPECT),
    ("queue_enabled", 6, Permissions::QUEUE),
    ("disabled", 4, Permissions::DISABLED),
    ("need_location", 6, Permissions::LOCATION),
    ("need_contents", 6, Permissions::CONTENTS),
    ("need_player", 6, Permissions::PLAYER),
    ("dark", 4, Permissions::DARK),
];
/// Runtime conditions are sampled for each selected handler, never persisted.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    /// Current runtime control, independent of world snapshots.
    pub queue_enabled: bool,
}
impl Permissions {
    /// No role restriction.
    pub const EVERYONE: Self = Self(0);
    /// GOD alternative.
    pub const GOD: Self = Self(1);
    /// Wizard alternative; GOD also qualifies.
    pub const WIZARD: Self = Self(2);
    /// Reject non-Wizard executors holding SUSPECT.
    pub const NO_SUSPECT: Self = Self(4);
    /// Require the live queue-enabled control.
    pub const QUEUE: Self = Self(8);
    /// Deny all invocations, including GOD.
    pub const DISABLED: Self = Self(16);
    /// Require an object type with a location slot.
    pub const LOCATION: Self = Self(32);
    /// Require an object type supporting contents.
    pub const CONTENTS: Self = Self(64);
    /// Require the Player type.
    pub const PLAYER: Self = Self(128);
    /// Hide command metadata from discovery without blocking execution.
    pub const DARK: Self = Self(256);
    /// Test one bit or a group of bits.
    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
    /// Return role alternatives without behavioral prerequisites.
    pub fn roles(self) -> Self {
        Self(self.0 & 3)
    }
    /// Decode the intentionally small Lua declaration vocabulary.
    pub fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "everyone" => Self::EVERYONE,
            "wizard" => Self::WIZARD,
            "god" => Self::GOD,
            _ => bail!("invalid permission {name:?}"),
        })
    }
    /// Roles are alternatives; disabling a command denies even GOD.
    pub fn allows(self, world: &World, player: ObjectId) -> bool {
        if self.contains(Self::DISABLED) {
            return false;
        }
        if player == ObjectId(1) {
            return true;
        }
        let wizard = crate::authority::is_wizard(world, player);
        if self.roles() != Self::EVERYONE && !(wizard && self.contains(Self::WIZARD)) {
            return false;
        }
        wizard
            || !self.contains(Self::NO_SUSPECT)
            || !world
                .objects
                .get(&player)
                .is_some_and(|o| o.flags.contains(flags::Flag::Suspect))
    }
    /// Command prerequisites use type capabilities, including empty containers.
    pub fn denial(self, world: &World, player: ObjectId, context: Context) -> Option<&'static str> {
        let kind = world.objects.get(&player).map(|o| o.kind);
        let location = matches!(kind, Some(Kind::Player | Kind::Thing | Kind::Garbage));
        let contents = location || kind == Some(Kind::Room);
        if self.contains(Self::LOCATION) && !location
            || self.contains(Self::CONTENTS) && !contents
            || self.contains(Self::PLAYER) && kind != Some(Kind::Player)
        {
            return Some("Command incompatible with invoker type.");
        }
        if self.contains(Self::QUEUE) && !context.queue_enabled {
            return Some("Sorry, queueing and triggering are not allowed now.");
        }
        if !self.allows(world, player) {
            return Some("Permission denied.");
        }
        None
    }
    /// Render effective bits in catalog order, including defaults.
    pub fn name(self) -> String {
        let names = CATALOG
            .iter()
            .filter(|(_, _, bit)| self.contains(*bit))
            .map(|(name, _, _)| *name)
            .collect::<Vec<_>>();
        if names.is_empty() {
            "everyone".into()
        } else {
            names.join(" ")
        }
    }
    /// Apply validated edits in order.
    pub fn edit(&mut self, edits: &[Edit]) {
        for edit in edits {
            if edit.set {
                self.0 |= edit.bit.0;
            } else {
                self.0 &= !edit.bit.0;
            }
        }
    }
}
impl std::ops::BitOr for Permissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
/// A single set/clear operation in a configuration declaration.
#[derive(Clone, Debug)]
pub struct Edit {
    bit: Permissions,
    set: bool,
}
impl Edit {
    /// Parse exactly the C configurable vocabulary and minimum abbreviations.
    pub fn parse(word: &str) -> Result<Self> {
        let word = word.to_ascii_lowercase();
        let (set, name) = word
            .strip_prefix('!')
            .map_or((true, word.as_str()), |s| (false, s));
        let (_, _, bit) = CATALOG
            .iter()
            .find(|(token, min, _)| name.len() >= *min && token.starts_with(name))
            .ok_or_else(|| anyhow::anyhow!("unknown access token {word:?}"))?;
        Ok(Self { bit: *bit, set })
    }
}
/// Ordered configuration declaration with source context.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Command/optional switch or list topic spelling.
    pub target: String,
    /// True when targeting a list topic.
    pub list: bool,
    /// Ordered set/clear operations.
    pub edits: Vec<Edit>,
    /// File and configuration path used in errors.
    pub origin: String,
}
/// Capture source order before deserializing dynamic maps into sorted maps.
pub fn rules(
    value: Option<&toml::Value>,
    origins: &std::collections::BTreeMap<String, std::path::PathBuf>,
) -> Result<Vec<Rule>> {
    let mut rules = Vec::new();
    if let Some(table) = value.and_then(toml::Value::as_table) {
        for (category, entries) in table {
            if !matches!(category.as_str(), "commands" | "lists") {
                continue;
            }
            for (target, entry) in entries.as_table().unwrap() {
                let path = format!("access.{category}.{target}");
                let origin = format!(
                    "{}: {path}",
                    origins
                        .get(&path)
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
                let tokens: crate::config::Permissions = entry.clone().try_into()?;
                let edits = tokens
                    .0
                    .iter()
                    .map(|word| Edit::parse(word))
                    .collect::<Result<Vec<_>>>()
                    .map_err(|e| anyhow::anyhow!("{origin}: {e}"))?;
                ensure!(
                    !target.is_empty() && !target.chars().any(char::is_whitespace),
                    "{origin}: invalid access target"
                );
                rules.push(Rule {
                    target: target.to_ascii_lowercase(),
                    list: category == "lists",
                    edits,
                    origin,
                });
            }
        }
    }
    Ok(rules)
}
/// C command prerequisites layered over each registration's declared role defaults.
pub fn native_defaults(name: &str, roles: Permissions) -> Permissions {
    use Permissions as P;
    let extra = match name {
        "@clone" | "@create" => P::CONTENTS,
        "@emit" | "@femit" | "@fpose" | "@fsay" | "@oemit" | "enter" | "get" | "give" | "leave"
        | "goto" | "look" | "pose" | "say" => P::LOCATION,
        "drop" => P::CONTENTS | P::LOCATION,
        "@force" | "@wait" => P::QUEUE,
        ";" | "\\" => P::LOCATION | P::DARK,
        _ => P::EVERYONE,
    };
    roles | extra
}
/// Native switch defaults; domain IC and ownership checks remain in their services.
pub fn switch_default(command: &str, switch: &str) -> Permissions {
    if matches!(command, "@boot" | "@wall" | "@btech") || command == "@examine" && switch == "debug"
    {
        Permissions::WIZARD
    } else {
        Permissions::EVERYONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Roles, edits and type capabilities follow the C access/name/type tables.
    #[test]
    fn c_access_edits_and_prerequisites() {
        let mut world = World::default();
        let config = crate::config::Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        world.next_id = 2;
        let player = world.create(&config, "Test".into(), Kind::Player);
        world
            .objects
            .get_mut(&player)
            .unwrap()
            .flags
            .insert(flags::Flag::Wizard);
        let mut mask = Permissions::WIZARD;
        mask.edit(&[Edit::parse("GO").unwrap()]);
        assert!(mask.allows(&world, ObjectId(2)));
        mask.edit(&[Edit::parse("!wiz").unwrap()]);
        assert!(!mask.allows(&world, ObjectId(2)) && mask.allows(&world, ObjectId(1)));
        mask.edit(&[Edit::parse("disA").unwrap()]);
        assert!(!mask.allows(&world, ObjectId(1)));
        for (name, min, bit) in CATALOG {
            let mut mask = Permissions::EVERYONE;
            mask.edit(&[Edit::parse(&name[..min]).unwrap()]);
            assert_eq!(mask, bit);
            mask.edit(&[Edit::parse(&format!("!{name}")).unwrap()]);
            assert_eq!(mask, Permissions::EVERYONE);
            assert!(Edit::parse(&name[..min - 1]).is_err());
        }
        for token in [
            "everyone",
            "public",
            "!",
            "no_ic",
            "wizard!",
            "need_",
            "wizard,god",
        ] {
            assert!(Edit::parse(token).is_err());
        }
        for (kind, location, contents) in [
            (Kind::Player, true, true),
            (Kind::Thing, true, true),
            (Kind::Room, false, true),
            (Kind::Exit, false, false),
            (Kind::Garbage, true, true),
        ] {
            world.objects.get_mut(&ObjectId(2)).unwrap().kind = kind;
            for (mask, allowed) in [
                (Permissions::LOCATION, location),
                (Permissions::CONTENTS, contents),
                (Permissions::PLAYER, kind == Kind::Player),
            ] {
                assert_eq!(
                    mask.denial(
                        &world,
                        ObjectId(2),
                        Context {
                            queue_enabled: true
                        }
                    )
                    .is_none(),
                    allowed
                );
            }
        }
        let object = world.objects.get_mut(&ObjectId(2)).unwrap();
        object.kind = Kind::Player;
        object.flags.insert(flags::Flag::Suspect);
        assert!(Permissions::NO_SUSPECT.allows(&world, ObjectId(2)));
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(flags::Flag::Wizard);
        assert!(!Permissions::NO_SUSPECT.allows(&world, ObjectId(2)));
        assert_eq!(
            Permissions::QUEUE.denial(
                &world,
                ObjectId(1),
                Context {
                    queue_enabled: false
                }
            ),
            Some("Sorry, queueing and triggering are not allowed now.")
        );
        assert!(Permissions::DARK.allows(&world, ObjectId(2)));
    }
}
