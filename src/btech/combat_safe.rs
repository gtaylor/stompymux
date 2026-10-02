//! Shared scenario immunity for damage, with source-map and target-unit policy kept distinct.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Read the operator-imposed immunity independently of construction and cockpit power.
pub fn battle_combat_safe(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit.combat_safe);
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?
        .combat_safe)
}

/// Trusted scenario edit; the caller owns administrative authority and transaction publication.
pub fn set_battle_combat_safe(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Going)),
        "Unit must be a live thing"
    );
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.combat_safe = enabled;
        return Ok(());
    }
    world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit is not constructed")?
        .combat_safe = enabled;
    Ok(())
}

/// Damage uses the source map; unattributed environmental entries originate at the target.
pub(super) fn protects(world: &World, attacker: Option<ObjectId>, target: ObjectId) -> bool {
    if battle_combat_safe(world, target).unwrap_or(false) {
        return true;
    }
    let source = attacker.unwrap_or(target);
    let position = world
        .btech
        .vehicles()
        .get(&source)
        .and_then(|unit| unit.position())
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&source)
                .and_then(|unit| unit.position())
        });
    position
        .and_then(|position| world.btech.maps().get(&position.map))
        .is_some_and(|map| map.building.is_complex())
}

/// Suppressed incoming damage informs only a distinct attacking cockpit.
pub(super) fn notice(attacker: Option<ObjectId>, target: ObjectId) -> Option<super::BattleNotice> {
    attacker
        .filter(|id| *id != target)
        .map(|unit| super::BattleNotice {
            unit,
            text: "Your efforts only scratch the paint!".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Damage attribution uses the source map even when the target occupies a different battlefield.
    #[test]
    fn source_map_and_target_flag_are_independent_for_both_anatomies() {
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let sources = [
            include_str!("../../game/mechs/JR7-D.toml"),
            include_str!("../../game/mechs/Demolisher.toml"),
        ];
        for source in sources {
            for target in sources {
                let mut world = World::default();
                let mut maps = Vec::new();
                let mut units = Vec::new();
                for template in [source, target] {
                    let map = world.create(&config, "Map".into(), Kind::Room);
                    crate::create_battle_map(
                        &mut world,
                        map,
                        "field",
                        crate::BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
                    )
                    .unwrap();
                    let id = world.create(&config, "Unit".into(), Kind::Thing);
                    crate::BattleUnitTemplate::parse("unit", template)
                        .unwrap()
                        .create(&mut world, id)
                        .unwrap();
                    crate::place_battle_unit(&mut world, id, map, 0, 0).unwrap();
                    maps.push(map);
                    units.push(id);
                }
                for flags in [(2, 0), (0, 2), (8, 0), (0, 8)] {
                    for (map, flags) in maps.iter().zip([flags.0, flags.1]) {
                        let mut building = world.btech.maps()[map].building;
                        building.flags = flags;
                        crate::set_building_state(&mut world, *map, building).unwrap();
                    }
                    assert_eq!(protects(&world, Some(units[0]), units[1]), flags.0 == 2);
                    assert_eq!(protects(&world, None, units[1]), flags.1 == 2);
                    set_battle_combat_safe(&mut world, units[1], true).unwrap();
                    assert!(protects(&world, Some(units[0]), units[1]));
                    set_battle_combat_safe(&mut world, units[1], false).unwrap();
                }
            }
        }
    }
}
