//! Fixtures, rule sets, and dice seeds shared by the four `btech_motion_*` scenario files.
//!
//! Each motion suite compiles this module beside one scenario file and uses only part of it,
//! so unused helpers are expected in any single suite.
#![allow(dead_code)]

use crate::support;
use stompymux_rs::{
    BattleMapAsset, BattleMovementRules, BattleTemplate, Kind, ObjectId, advance_battle_units,
    assign_battle_pilot, create_battle_map, create_battle_unit, place_battle_unit,
    start_battle_unit,
};

pub(crate) const RULES: BattleMovementRules = BattleMovementRules {
    fasa_turning: false,
    slowdown: 2,
    ..stompymux_rs::BattleMovementRules::STANDARD
};

/// Seed for the main fixture unit's own stream. The fixture seeds every other stream it
/// creates through [`support::seed_world_dice`] and then gives this unit the fixed stream its
/// scenarios were written against.
pub(crate) const FIXTURE_UNIT_SEED: u8 = 0;

/// A running, piloted Jenner in the middle of a uniform test battlefield.
pub(crate) async fn fixture(
    terrain: char,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let row = format!("{terrain}0").repeat(12);
    let source = format!("12 12\n{}", format!("{row}\n").repeat(12));
    fixture_source(&source).await
}

/// Build a battlefield from a supplied grid so crossing tests can place narrow obstacles.
pub(crate) async fn fixture_source(
    source: &str,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    fixture_assets(
        source,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .await
}

/// Supply equipment as well as terrain for environmental heat scenarios.
pub(crate) async fn fixture_assets(
    source: &str,
    template: BattleTemplate,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Movement field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "motion.map",
        BattleMapAsset::from_cells(source).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, id, template).unwrap();
    place_battle_unit(&mut world, id, map, 5, 5).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    shot_seed(&mut world, id, FIXTURE_UNIT_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// A visible, acquired neighbor for target-selection scenarios.
pub(crate) async fn lock_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Lock target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    (dir, config, world, id, target)
}

/// Conventional aim configuration with no optional rule overrides.
pub(crate) fn optical_aim_rules() -> stompymux_rs::BattleAimRules {
    stompymux_rs::BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

/// Current conventional direct-shot rules, excluding unsupported game variants.
pub(crate) fn shot_rules() -> stompymux_rs::BattleShotRules {
    stompymux_rs::BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: stompymux_rs::BattleStackingRules::STANDARD,
        glancing: stompymux_rs::BattleGlancingMode::Disabled,
        stagger: stompymux_rs::BattleStaggerMode::Retain,
        aim: optical_aim_rules(),
        hit: stompymux_rs::BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: true,
        extended_piloting: true,
        target_toughness: false,
    }
}

/// Match host-selected hit routing when comparing direct predictions with native/Lua actions.
pub(crate) fn configured_shot_rules(
    config: &stompymux_rs::Config,
) -> stompymux_rs::BattleShotRules {
    let mut rules = shot_rules();
    rules.range_damage = config.battletech.moddamagewithrange != 0;
    rules.hit = stompymux_rs::BattleFallRules::configured(config).hit;
    rules
}

/// Put an acquired target ahead of a connected pilot, ready for ordinary direct fire.
pub(crate) async fn shot_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, id, target) = lock_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    (dir, config, world, id, target)
}

/// Reference incidental misses draw ignition and clearing checks even on grass.
/// The low ignition branch performs one additional roll before checking terrain.
pub(crate) fn expected_grass_miss_rolls(mut roll: impl FnMut() -> u8) {
    let ignition = roll();
    roll();
    if ignition <= 3 {
        roll();
    }
}

/// Seed a unit's persisted stream for a reproducible shot scenario.
pub(crate) fn shot_seed(world: &mut stompymux_rs::World, id: ObjectId, seed: u8) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] =
                serde_json::to_value(stompymux_rs::BattleDice::seeded([seed; 32])).unwrap();
        })
        .unwrap();
}

/// Set laser skill against a seven-attribute total, yielding gunnery target 11 minus level.
pub(crate) fn shot_skill(world: &mut stompymux_rs::World, level: u8) {
    stompymux_rs::set_battle_character(
        world,
        ObjectId(1),
        stompymux_rs::BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(world, ObjectId(1), support::FIXTURE_DICE_SEED);
    stompymux_rs::set_battle_character_value(
        world,
        ObjectId(1),
        "Gunnery-Laser",
        stompymux_rs::BattleCharacterValue {
            value: level,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
}

pub(crate) fn fall_rules() -> stompymux_rs::BattleFallRules {
    stompymux_rs::BattleFallRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: stompymux_rs::BattleStackingRules::STANDARD,
        stagger: stompymux_rs::BattleStaggerMode::Retain,
        hit: shot_rules().hit,
        extended_piloting: true,
        toughness: false,
    }
}

/// A conscious pilot in a prone, undamaged-enough Jenner with base piloting six.
pub(crate) async fn stand_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let (dir, config, mut world, id) = fixture('.').await;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() >= 7)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    shot_skill(&mut world, 0);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    stompymux_rs::set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        stompymux_rs::BattleCharacterValue {
            value: 5,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    (dir, config, world, id)
}

/// Choose one TAC slot deterministically without depending on a runtime entropy source.
pub(crate) fn single_critical_seed(candidates: u16, selected: u16) -> u8 {
    (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            dice.two_d6(); // Material entry.
            matches!(dice.two_d6(), 8 | 9) && dice.die(candidates).unwrap() == selected
        })
        .expect("a seed selecting one requested critical")
}

/// Give the connected pilot a guaranteed ordinary balance success, retaining real dice consumption.
pub(crate) fn balance_skill(world: &mut stompymux_rs::World) {
    shot_skill(world, 0);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    stompymux_rs::set_battle_character_value(
        world,
        ObjectId(1),
        "Piloting-Biped",
        stompymux_rs::BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
}

/// A selected hit used to isolate consequences from hit-location selection.
pub(crate) fn balance_hit(
    section: stompymux_rs::BattleSection,
    tac: bool,
) -> stompymux_rs::BattleHit {
    stompymux_rs::BattleHit {
        section,
        rear_armor: false,
        through_armor_critical: tac,
        crew_stun: false,
    }
}

/// A well-armored running biped whose pilot can pass ordinary stagger checks.
pub(crate) async fn stagger_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (dir, config, mut world, id) = fixture_assets(
        &source,
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .await;
    balance_skill(&mut world);
    (dir, config, world, id)
}

/// Explicit native stagger configuration used by boundary tests.
pub(crate) fn stagger_rules(
    mode: stompymux_rs::BattleStaggerMode,
) -> stompymux_rs::BattleStaggerRules {
    stompymux_rs::BattleStaggerRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        mode,
        interval: 5,
        tonnage: true,
        hit: fall_rules().hit,
        extended_piloting: true,
    }
}

/// Deliver a selected ordinary hit through the same stagger accounting used by weapon shots.
pub(crate) fn stagger_hit(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    section: stompymux_rs::BattleSection,
    damage: u16,
    mode: stompymux_rs::BattleStaggerMode,
) {
    let _impact = stompymux_rs::resolve_battle_tactical_impact(
        world,
        id,
        balance_hit(section, false),
        damage,
        stompymux_rs::BattleFallRules {
            stagger: mode,
            ..fall_rules()
        },
    )
    .unwrap();
}

/// Keep water-fall assertions separate from damage to the newly flooded leg.
pub(crate) fn water_fall_seed(material: bool) -> u8 {
    (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            if material {
                dice.two_d6();
            }
            dice.two_d6();
            dice.d6() == 1 && dice.two_d6() == 7
        })
        .unwrap()
}

/// Place a conventional biped at a requested water depth with deterministic safe fall dice.
pub(crate) async fn water_fixture(
    depth: u8,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
) {
    let source = format!(
        "12 12\n{}",
        format!("{}\n", format!("~{depth}").repeat(12)).repeat(12)
    );
    let (dir, config, mut world, id) = fixture_source(&source).await;
    balance_skill(&mut world);
    shot_seed(&mut world, id, water_fall_seed(false));
    (dir, config, world, id)
}

/// Supported rules for the thermal consequence pass after heat accounting.
pub(crate) fn overheat_rules() -> stompymux_rs::BattleOverheatRules {
    let rules = fall_rules();
    stompymux_rs::BattleOverheatRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: stompymux_rs::BattleStackingRules::STANDARD,
        hit: rules.hit,
        extended_piloting: rules.extended_piloting,
        stagger: rules.stagger,
    }
}

/// Set a reproducible sampled heat and due-clock boundary without advancing any dice.
pub(crate) fn overheat_due(world: &mut stompymux_rs::World, id: ObjectId, heat: f64, injury: bool) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["heat"] = serde_json::json!({"stored":heat + 10.0,"excess":heat});
            record["overheat_clock"] =
                serde_json::json!({"elapsed":30,"phase":0,"injury_due":injury});
        })
        .unwrap();
}

/// Install a Computer skill against intuition three and learning two.
pub(crate) fn computer_skill(world: &mut stompymux_rs::World, level: u8) {
    shot_skill(world, 0);
    stompymux_rs::set_battle_character_value(
        world,
        ObjectId(1),
        "Computer",
        stompymux_rs::BattleCharacterValue {
            value: level,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
}

/// A close-range kick fixture retains normal scanner acquisition and pilot skill initialization.
pub(crate) async fn kick_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 0);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    place_battle_unit(&mut world, target, map, 5, 5).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    (dir, config, world, id, target)
}

/// Explicit physical rules keep direct tests independent of local configuration defaults.
pub(crate) fn kick_rules() -> stompymux_rs::BattlePhysicalRules {
    stompymux_rs::BattlePhysicalRules {
        use_pilot_skill: false,
        fasa_turning: false,
        extended_movement: false,
        hit_arc_mode: 0,
        glancing: stompymux_rs::BattleGlancingMode::Disabled,
        fall: fall_rules(),
    }
}

/// A charge collision fixture isolates current velocity, accumulated travel and deterministic control skills.
pub(crate) fn prepare_test_charge(world: &mut stompymux_rs::World, id: ObjectId) {
    use stompymux_rs::*;
    set_battle_character_value(
        world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 5,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["motion"]["speed"] = 21.5.into();
            record["motion"]["desired_speed"] = 21.5.into();
        })
        .unwrap();
}

/// Independent executions may cross a wall-clock second; compare all other XP and combat facts.
pub(crate) fn without_xp_timestamps(value: &impl serde::Serialize) -> serde_json::Value {
    fn clear(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (name, value) in fields {
                    if name == "last_used" {
                        *value = 0.into();
                    } else {
                        clear(value);
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(clear),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(value).unwrap();
    clear(&mut value);
    value
}
