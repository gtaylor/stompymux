//! Shared supported-chassis firing fixtures for sighting and anatomical targeting.
use crate::support;
use stompymux_rs::*;

/// Representative supported chassis, including fixed ground platforms and landed rotorcraft.
pub fn templates() -> Vec<String> {
    let tracked = include_str!("../../game/mechs/Demolisher");
    vec![
        include_str!("../../game/mechs/JR7-D").into(),
        include_str!("../../game/mechs/GOL-1H").into(),
        tracked.into(),
        tracked.replace("{ Track }", "{ Wheel }"),
        tracked.replace("{ Track }", "{ Hover }"),
        tracked
            .replace("{ Track }", "{ None }")
            .replace("{ 53.75 }", "{ 0 }"),
        include_str!("../../game/mechs/Kestrel").into(),
    ]
}

/// Edit only explicitly selected saved test state, retaining the normal validation path.
pub fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut state = serde_json::to_value(&world.btech).unwrap();
    change(&mut state[key][id.0.to_string()]);
    world.btech = serde_json::from_value(state).unwrap();
}

/// A configurable recipient supports both ground and airborne target admission cases.
pub async fn fixture_with_target(
    source: &str,
    weapon: Option<BattleWeapon>,
    target_source: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_with_computer(source, weapon, target_source, false).await
}

/// Optional authored computer linkage exercises aim and per-packet guidance with the same loadout.
pub async fn fixture_with_computer(
    source: &str,
    weapon: Option<BattleWeapon>,
    target_source: &str,
    computer: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_with_supply(source, weapon, target_source, computer, None).await
}

/// Supply an injected weapon with an optional authored ammunition type.
pub async fn fixture_with_supply(
    source: &str,
    weapon: Option<BattleWeapon>,
    target_source: &str,
    computer: bool,
    ammunition_flag: Option<&str>,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, world) = support::isolated_world().await;
    let (world, shooter, target, index) = supply_fixture_on(
        world,
        &config,
        source,
        weapon,
        target_source,
        computer,
        ammunition_flag,
    );
    (dir, config, world, shooter, target, index)
}

/// Construct the sighting lane, shooter and target on a supplied base world.
pub fn supply_fixture_on(
    mut world: World,
    config: &Config,
    source: &str,
    weapon: Option<BattleWeapon>,
    target_source: &str,
    computer: bool,
    ammunition_flag: Option<&str>,
) -> (World, ObjectId, ObjectId, usize) {
    let map = world.create(config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        BattleMapAsset::parse(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap(),
    )
    .unwrap();
    let mut template = BattleUnitTemplate::parse(source).unwrap();
    if let Some(weapon) = weapon {
        let (section, count) = match &mut template {
            BattleUnitTemplate::Mech(definition) => (
                definition
                    .sections
                    .get_mut(&BattleSection::LeftTorso)
                    .unwrap(),
                weapon.profile().critical_slots,
            ),
            BattleUnitTemplate::Vehicle(definition) => (
                definition
                    .sections
                    .get_mut(&BattleVehicleSection::Front)
                    .unwrap(),
                1,
            ),
        };
        section.criticals.clear();
        for slot in 0..count {
            section.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    modes: if computer {
                        vec!["OnTC".into()]
                    } else {
                        vec![]
                    },
                    brand: None,
                },
            );
        }
        if let Some(flag) = ammunition_flag.filter(|_| weapon.profile().ammunition_per_ton > 0) {
            section.criticals.insert(
                count + u8::from(computer),
                CriticalDefinition {
                    equipment: format!("Ammo_{}", weapon.name()),
                    data: weapon.profile().ammunition_per_ton.to_string(),
                    modes: if flag.is_empty() {
                        vec![]
                    } else {
                        vec![flag.into()]
                    },
                    brand: None,
                },
            );
        }
        if computer {
            section.criticals.insert(
                count,
                CriticalDefinition {
                    equipment: "TargetingComputer".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
        }
    }
    let shooter = world.create(config, "Sighter".into(), Kind::Thing);
    let target = world.create(config, "Target".into(), Kind::Thing);
    for id in [shooter, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    template.create(&mut world, shooter).unwrap();
    BattleUnitTemplate::parse(target_source)
        .unwrap()
        .create(&mut world, target)
        .unwrap();
    place_battle_unit(&mut world, shooter, map, 0, 11).unwrap();
    place_battle_unit(&mut world, target, map, 0, 10).unwrap();
    for id in [shooter, target] {
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
        });
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
    let index = if let Some(weapon) = weapon {
        if let Some(unit) = world.btech.vehicles().get(&shooter) {
            unit.loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap()
        } else {
            world.btech.constructed_units()[&shooter]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap()
        }
    } else {
        0
    };
    world.validate(config).unwrap();
    (world, shooter, target, index)
}
