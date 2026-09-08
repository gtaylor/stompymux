//! Deterministic, nonrecursive command-source discovery shared with catalog inspection.
use crate::{
    flags::Flag,
    world::{Kind, ObjectId, World},
};
use std::collections::BTreeSet;

/// An eligible object and its position in local command fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSource {
    pub object: ObjectId,
    pub stage: &'static str,
    /// Direct-object calls honor NO_COMMAND; list traversal does not in C.
    pub no_command_gated: bool,
}

impl CommandSource {
    /// Zone-derived sources are searched only when direct local handlers did not handle input.
    pub fn fallback(self) -> bool {
        self.stage.ends_with("fallback")
    }

    /// Recheck the C eligibility applicable to this source's discovery route.
    pub fn eligible(self, world: &World) -> bool {
        world.objects.get(&self.object).is_some_and(|o| {
            o.kind != Kind::Garbage
                && !o.flags.contains(Flag::Halted)
                && (!self.no_command_gated || !o.flags.contains(Flag::NoCommand))
        })
    }
}

/// Runtime eligibility, rechecked before each invocation.
pub fn eligible(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|o| {
        o.kind != Kind::Garbage
            && ![Flag::Going, Flag::Halted, Flag::NoCommand]
                .into_iter()
                .any(|f| o.flags.contains(f))
    })
}

/// Capture direct sources once; callbacks cannot grow the current command's search.
pub fn sources(world: &World, player: ObjectId) -> Vec<CommandSource> {
    let Some(actor) = world.objects.get(&player) else {
        return Vec::new();
    };
    let location = actor.location;
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |object, stage| {
        if seen.insert(object) && eligible(world, object) {
            result.push(CommandSource {
                object,
                stage,
                no_command_gated: true,
            });
        }
    };
    add(player, "caller");
    if let Some(location) = location {
        for o in world
            .objects
            .values()
            .filter(|o| o.location == Some(location) && o.kind != Kind::Exit)
        {
            add(o.id, "nearby");
        }
        add(location, "location");
    }
    for o in world
        .objects
        .values()
        .filter(|o| o.location == Some(player) && o.kind != Kind::Exit)
    {
        add(o.id, "inventory");
    }
    let zone = location
        .and_then(|id| world.objects.get(&id))
        .and_then(|o| o.zone);
    if let Some(zone) = zone.and_then(|id| world.objects.get(&id)) {
        if zone.kind == Kind::Room {
            for o in world
                .objects
                .values()
                .filter(|o| o.location == Some(zone.id) && o.kind != Kind::Exit)
            {
                add(o.id, "location-zone fallback");
            }
        } else {
            add(zone.id, "location-zone fallback");
        }
    }
    if let Some(zone) = actor.zone.filter(|id| Some(*id) != zone) {
        add(zone, "player-zone fallback");
    }
    result
}

/// Preserve C list traversal duplicates and route-specific NO_COMMAND behavior.
pub fn dispatch_sources(world: &World, player: ObjectId) -> Vec<CommandSource> {
    let Some(actor) = world.objects.get(&player) else {
        return Vec::new();
    };
    let location = actor.location;
    let mut result = Vec::new();
    let mut add = |object, stage, no_command_gated| {
        let source = CommandSource {
            object,
            stage,
            no_command_gated,
        };
        if source.eligible(world) {
            result.push(source);
        }
    };
    add(player, "caller", true);
    if let Some(location) = location {
        for object in world
            .objects
            .values()
            .filter(|o| o.location == Some(location) && o.kind != Kind::Exit)
        {
            add(object.id, "nearby", false);
        }
        add(location, "location", true);
    }
    for object in world
        .objects
        .values()
        .filter(|o| o.location == Some(player) && o.kind != Kind::Exit)
    {
        add(object.id, "inventory", false);
    }
    let location_zone = location
        .and_then(|id| world.objects.get(&id))
        .and_then(|o| o.zone);
    if let Some(zone) = location_zone.and_then(|id| world.objects.get(&id)) {
        if zone.kind == Kind::Room {
            if location != actor.zone {
                for object in world
                    .objects
                    .values()
                    .filter(|o| o.location == Some(zone.id) && o.kind != Kind::Exit)
                {
                    add(object.id, "location-zone fallback", false);
                }
            }
        } else {
            add(zone.id, "location-zone fallback", true);
        }
    }
    if let Some(zone) = actor.zone.filter(|id| Some(*id) != location_zone) {
        add(zone, "player-zone fallback", true);
    }
    result
}
