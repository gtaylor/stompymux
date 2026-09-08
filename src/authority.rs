//! Persistent world authority checks, separate from flags and lock evaluation.

use crate::{
    flags::Flag,
    world::{Kind, ObjectId, World},
};

/// Whether the actor has Wizard command authority; GOD always qualifies.
pub fn is_wizard(world: &World, actor: ObjectId) -> bool {
    actor == ObjectId(1)
        || world
            .objects
            .get(&actor)
            .is_some_and(|object| object.flags.contains(Flag::Wizard))
}

/// Whether the actor has legacy administrative control of the target.
///
/// Control is narrower than Wizard authority: Wizards cannot control GOD or
/// another Wizard. This policy does not evaluate operation-specific locks.
pub fn controls(world: &World, actor: ObjectId, target: ObjectId) -> bool {
    let Some(target) = world
        .objects
        .get(&target)
        .filter(|object| object.kind != Kind::Garbage)
    else {
        return false;
    };
    let wizard = world
        .objects
        .get(&actor)
        .is_some_and(|object| object.flags.contains(Flag::Wizard));
    actor == ObjectId(1)
        || wizard
            && (actor == target.id
                || target.id != ObjectId(1) && !target.flags.contains(Flag::Wizard))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_authority_and_control_remain_distinct() {
        let config = crate::config::Config::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        )
        .unwrap();
        let mut world = World {
            next_id: 2,
            ..Default::default()
        };
        let wizard = world.create(&config, "Wizard".into(), Kind::Player);
        let peer = world.create(&config, "Peer".into(), Kind::Player);
        let ordinary = world.create(&config, "Ordinary".into(), Kind::Thing);
        world
            .objects
            .get_mut(&wizard)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world
            .objects
            .get_mut(&peer)
            .unwrap()
            .flags
            .insert(Flag::Wizard);

        assert!(is_wizard(&world, wizard));
        assert!(controls(&world, wizard, ordinary));
        assert!(!controls(&world, wizard, peer));
        assert!(!controls(&world, wizard, ObjectId(1)));
        assert!(controls(&world, ObjectId(1), peer));
    }
}
