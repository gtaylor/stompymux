//! Trusted macro operations with validation before any world mutation.
use super::{MacroEntry, MacroSet, MacroSetId, PlayerMacros, SLOT_COUNT, alias, text};
use crate::world::{Kind, ObjectId, World};

/// Domain failure code and diagnostic, translated by the Lua boundary.
#[derive(Debug)]
pub(crate) struct Error(pub &'static str, pub String);

type Result<T> = std::result::Result<T, Error>;

/// Report malformed input independently of the scripting boundary.
fn invalid(message: impl ToString) -> Error {
    Error("mux.arg.invalid", message.to_string())
}

/// Resolve a handle without interpreting a mutable set number as identity.
pub(crate) fn index(macros: &PlayerMacros, id: MacroSetId) -> Result<usize> {
    macros
        .sets
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| Error("mux.macro.invalid", "macro set no longer exists".into()))
}

/// Validate owners and attachment targets, including offline players.
pub(crate) fn object(world: &World, id: ObjectId, player: bool) -> Result<()> {
    match world.objects.get(&id) {
        Some(o) if o.kind != Kind::Garbage && (!player || o.kind == Kind::Player) => Ok(()),
        _ => Err(Error(
            "mux.object.invalid",
            if player {
                "expected a live player"
            } else {
                "expected a live object"
            }
            .into(),
        )),
    }
}

/// Create an unattached, private, unlocked set.
pub(crate) fn create(
    world: &mut World,
    owner: ObjectId,
    description: String,
) -> Result<MacroSetId> {
    object(world, owner, false)?;
    text(&description).map_err(invalid)?;
    let set = MacroSet {
        id: Default::default(),
        origin: Default::default(),
        owner,
        modes: Default::default(),
        description,
        entries: vec![],
    };
    let id = set.id;
    world.macros.sets.push(set);
    Ok(id)
}

/// Change ownership without applying player command authority rules.
pub(crate) fn set_owner(world: &mut World, id: MacroSetId, owner: ObjectId) -> Result<()> {
    object(world, owner, false)?;
    let i = index(&world.macros, id)?;
    world.macros.sets[i].owner = owner;
    Ok(())
}

/// Validated definition operation; updates retain spelling and durable row origin.
pub(crate) fn define(
    macros: &mut PlayerMacros,
    id: MacroSetId,
    name: String,
    expansion: String,
    update: bool,
) -> Result<()> {
    let i = index(macros, id)?;
    alias(&name).map_err(invalid)?;
    if name.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(invalid("alias cannot contain whitespace"));
    }
    text(&expansion).map_err(invalid)?;
    if expansion.is_empty() {
        return Err(invalid("expansion must not be empty"));
    }
    let set = &mut macros.sets[i];
    let found = set
        .entries
        .iter()
        .position(|e| e.alias.eq_ignore_ascii_case(&name));
    if update {
        let position = found
            .ok_or_else(|| Error("mux.macro.not_found", "macro alias does not exist".into()))?;
        set.entries[position].expansion = expansion;
    } else {
        if found.is_some() {
            return Err(Error(
                "mux.macro.exists",
                "macro alias already exists".into(),
            ));
        }
        set.entries.push(MacroEntry {
            origin: Default::default(),
            alias: name,
            expansion,
        });
        set.entries.sort_by_key(|e| e.alias.to_ascii_lowercase());
    }
    Ok(())
}

/// Delete one definition by case-insensitive alias.
pub(crate) fn delete(macros: &mut PlayerMacros, id: MacroSetId, name: &str) -> Result<()> {
    let i = index(macros, id)?;
    let set = &mut macros.sets[i];
    let position = set
        .entries
        .iter()
        .position(|e| e.alias.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error("mux.macro.not_found", "macro alias does not exist".into()))?;
    set.entries.remove(position);
    Ok(())
}

/// Attach to the first free slot, preserving the editing selection.
pub(crate) fn attach(world: &mut World, player: ObjectId, id: MacroSetId) -> Result<usize> {
    object(world, player, true)?;
    let i = index(&world.macros, id)?;
    let slot = world
        .macros
        .players
        .get(&player)
        .map_or(Some(0), |s| s.slots.iter().position(Option::is_none))
        .ok_or_else(|| {
            Error(
                "mux.macro.slots_full",
                "player has no free macro slots".into(),
            )
        })?;
    world.macros.players.entry(player).or_default().slots[slot] = Some(i);
    Ok(slot)
}

/// Detach a slot without moving any other slot.
pub(crate) fn detach(world: &mut World, player: ObjectId, slot: usize) -> Result<bool> {
    object(world, player, true)?;
    if slot >= SLOT_COUNT {
        return Err(invalid("slot must be between 0 and 4"));
    }
    let Some(slots) = world.macros.players.get_mut(&player) else {
        return Ok(false);
    };
    let removed = slots.slots[slot].take().is_some();
    if slots.current == Some(slot) {
        slots.current = None;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Snapshot identities survive clones, but serialized worlds receive fresh identities.
    #[test]
    fn identity_is_runtime_only_and_failed_definitions_are_atomic() {
        let set = MacroSet {
            id: Default::default(),
            origin: Default::default(),
            owner: ObjectId(1),
            modes: Default::default(),
            description: "test".into(),
            entries: vec![],
        };
        let id = set.id;
        assert_eq!(set.clone().id, id);
        let encoded = serde_json::to_value(&set).unwrap();
        assert!(encoded.get("id").is_none());
        let decoded: MacroSet = serde_json::from_value(encoded).unwrap();
        assert_ne!(decoded.id, id);
        let mut macros = PlayerMacros {
            sets: vec![set],
            players: Default::default(),
        };
        define(&mut macros, id, "Hi".into(), "look".into(), false).unwrap();
        let before = serde_json::to_value(&macros).unwrap();
        assert_eq!(
            define(&mut macros, id, "hI".into(), "bad".into(), false)
                .unwrap_err()
                .0,
            "mux.macro.exists"
        );
        assert_eq!(
            define(&mut macros, id, "Hi".into(), "".into(), true)
                .unwrap_err()
                .0,
            "mux.arg.invalid"
        );
        assert_eq!(serde_json::to_value(&macros).unwrap(), before);
        macros.remove(0);
        assert_eq!(index(&macros, id).unwrap_err().0, "mux.macro.invalid");
    }
}
