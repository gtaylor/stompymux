//! Deterministic dice for test worlds.
//!
//! Production gives every new unit, vehicle, crew recovery and map an independent dice
//! stream drawn from OS entropy, so a scenario that builds them and then rolls would differ
//! from one test process to the next. [`seed_world_dice`] replaces every stream in a world
//! with one derived from a base seed and the holder's identity.
use stompymux_rs::{Dice, ObjectId, World};

/// Base seed shared fixture builders pass to [`seed_world_dice`].
pub const FIXTURE_DICE_SEED: u64 = 0x5eed_d1ce;

/// The kinds of dice stream a world holds. Each kind derives its own seed, so a unit's
/// stream and its cockpit's recovery stream never coincide although they share an id.
#[derive(Clone, Copy)]
enum Holder {
    /// A Mech's or vehicle's own stream.
    Unit = 1,
    /// The recovery stream owned by a unit's empty cockpit.
    CrewRecovery = 2,
    /// A player's consciousness recovery stream.
    PlayerRecovery = 3,
    /// A map's fire spread stream.
    MapFire = 4,
}

/// Reseed every dice stream in `world` deterministically.
///
/// Every constructed Mech and vehicle, each one's cockpit recovery, every player recovery
/// and every map fire stream gets a fresh stream at position zero, seeded from `seed`, the
/// kind of holder and its `ObjectId`, so no two holders share a stream. A map without a fire
/// stream keeps none, because whether one exists changes how ignition behaves.
///
/// Call it after a fixture has created its units, maps and pilots and before anything rolls.
/// It replaces streams a scenario seeded on purpose, so seed particular holders afterwards.
/// Streams created later, such as a recovery prepared when a pilot is assigned afterwards,
/// still come from OS entropy; reseed them with [`seed_object_dice`] before they roll.
/// Rewriting a record clears the same runtime-only state a save and reload clears.
pub fn seed_world_dice(world: &mut World, seed: u64) {
    seed_holders(world, seed, |_| true);
}

/// Reseed only the dice streams keyed by `id`, exactly as [`seed_world_dice`] would.
///
/// That is the unit's or vehicle's own stream and its cockpit recovery, the player's
/// recovery, or the map's fire stream, whichever `id` holds. Helpers that add one holder to
/// a world a caller already set up use it so they leave the caller's other streams alone.
pub fn seed_object_dice(world: &mut World, id: ObjectId, seed: u64) {
    seed_holders(world, seed, |holder| holder == id);
}

/// Reseed every holder whose identity `select` accepts.
fn seed_holders(world: &mut World, seed: u64, select: impl Fn(ObjectId) -> bool) {
    let units: Vec<ObjectId> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .filter(|id| select(*id))
        .collect();
    for id in units {
        let unit = Dice::seeded(stream_seed(seed, Holder::Unit, id));
        let crew = Dice::seeded(stream_seed(seed, Holder::CrewRecovery, id));
        if world.btech.vehicles().contains_key(&id) {
            // A vehicle edit re-decodes and validates its whole record, so set both
            // streams in one pass.
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(unit).unwrap();
                    record["crew_recovery"]["dice"] = serde_json::to_value(crew).unwrap();
                })
                .unwrap();
            continue;
        }
        world.btech.set_unit_dice(id, unit).unwrap();
        world.btech.set_unit_crew_recovery_dice(id, crew).unwrap();
    }
    let players: Vec<ObjectId> = world
        .btech
        .recoveries()
        .keys()
        .copied()
        .filter(|id| select(*id))
        .collect();
    for player in players {
        let dice = Dice::seeded(stream_seed(seed, Holder::PlayerRecovery, player));
        world.btech.set_recovery_dice(player, dice).unwrap();
    }
    let maps: Vec<ObjectId> = world
        .btech
        .maps()
        .keys()
        .copied()
        .filter(|id| select(*id))
        .collect();
    for map in maps {
        let dice = Dice::seeded(stream_seed(seed, Holder::MapFire, map));
        world.btech.replace_map_fire_dice(map, dice).unwrap();
    }
}

/// Derive a 32-byte ChaCha key from the base seed, holder kind and identity.
fn stream_seed(seed: u64, holder: Holder, id: ObjectId) -> [u8; 32] {
    let mut state = mix(mix(mix(seed) ^ holder as u64) ^ id.0 as u64);
    let mut bytes = [0_u8; 32];
    for chunk in bytes.as_chunks_mut::<8>().0 {
        state = mix(state);
        chunk.copy_from_slice(&state.to_le_bytes());
    }
    bytes
}

/// One SplitMix64 step: a bijective, well-avalanching 64-bit mix.
fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Holders of different kinds or identities derive different keys from one base seed.
    #[test]
    fn holders_derive_distinct_keys() {
        let keys = [
            stream_seed(1, Holder::Unit, ObjectId(5)),
            stream_seed(1, Holder::CrewRecovery, ObjectId(5)),
            stream_seed(1, Holder::PlayerRecovery, ObjectId(5)),
            stream_seed(1, Holder::MapFire, ObjectId(5)),
            stream_seed(1, Holder::Unit, ObjectId(6)),
            stream_seed(2, Holder::Unit, ObjectId(5)),
        ];
        for (index, key) in keys.iter().enumerate() {
            assert!(keys[index + 1..].iter().all(|other| other != key));
        }
        assert_eq!(keys[0], stream_seed(1, Holder::Unit, ObjectId(5)));
    }
}
