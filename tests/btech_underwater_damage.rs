//! Damage to 'Mechs under water: hull-breach rolls on every damaging hit (Total Warfare p. 121),
//! falls from above the surface taking two halved stages (p. 57), and a destroyed life support
//! system drowning its MechWarrior at the end of each turn (p. 127).
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A Jenner placed in a one-hex field of the given cells, with its pilot aboard.
async fn jenner_in(cells: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Lake".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "lake",
        MapAsset::from_cells(&format!("1 1\n{cells}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Ordinary tactical fall rules.
fn rules() -> FallRules {
    FallRules::STANDARD
}

/// Every damaging hit on an underwater location rolls 2D6 and breaches it on 10 or more; a
/// unit out of the water rolls nothing.
#[tokio::test]
async fn underwater_hits_roll_for_a_hull_breach() {
    let (_dir, _config, wet, id) = jenner_in("~2").await;
    let (_dir, _config, dry, _) = jenner_in(".0").await;
    let hit = Hit {
        section: MechSection::LeftArm,
        rear_armor: false,
        through_armor_critical: false,
    };
    let mut exercised = [false, false];
    for seed in 0..=63 {
        let mut dice = Dice::seeded([seed; 32]);
        dice.two_d6();
        let roll = dice.two_d6();
        // One point scratches the arm's armor, so only the roll can breach it.
        let mut world = wet.clone();
        world
            .btech
            .set_unit_dice(id, Dice::seeded([seed; 32]))
            .unwrap();
        resolve_battle_tactical_impact(&mut world, id, hit, 1, rules()).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        let breached = unit.flooded_sections().contains(&MechSection::LeftArm);
        assert_eq!(breached, roll >= 10, "seed {seed} roll {roll}");
        assert_eq!(unit.sections()[&MechSection::LeftArm].armor, 3);
        exercised[usize::from(breached)] = true;
        let mut world = dry.clone();
        world
            .btech
            .set_unit_dice(id, Dice::seeded([seed; 32]))
            .unwrap();
        resolve_battle_tactical_impact(&mut world, id, hit, 1, rules()).unwrap();
        // Only the damage-entry roll was made.
        let mut entry = Dice::seeded([seed; 32]);
        entry.two_d6();
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [entry.d6()]);
    }
    assert_eq!(exercised, [true, true]);
}

/// A fall into water from above the surface takes half damage for striking the surface and,
/// separately, half for reaching the bottom; a fall that starts below the surface takes one
/// halved amount (Total Warfare p. 57).
#[tokio::test]
async fn water_falls_from_above_the_surface_strike_twice() {
    let (_dir, _config, base, id) = jenner_in("~2").await;
    // Thirty-five tons fall for 4 a multiplier. From two levels above the surface of Depth 2
    // water: 4 x 3 / 2 for the surface, then 4 x 3 / 2 for the bottom.
    let mut world = base.clone();
    let report = resolve_battle_fall(&mut world, id, 5, rules()).unwrap();
    assert_eq!(report.damage, 12);
    assert_eq!(
        report
            .groups
            .iter()
            .map(|group| group.damage)
            .collect::<Vec<_>>(),
        [5, 1, 5, 1]
    );
    // Falling where it stands, on the bottom: 4 x 1 / 2.
    let mut world = base.clone();
    let report = resolve_battle_fall(&mut world, id, 1, rules()).unwrap();
    assert_eq!(report.damage, 2);
}

/// Without life support a submerged MechWarrior takes 1 damage at the end of every turn; on
/// land, or mid-turn, nothing happens (Total Warfare p. 127).
#[tokio::test]
async fn submerged_mechwarriors_without_life_support_take_damage_each_turn() {
    for (cells, injured) in [("~2", true), (".0", false)] {
        let (_dir, config, mut world, id) = jenner_in(cells).await;
        let life_support = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|part| part.system == System::LifeSupport)
            .unwrap()
            .location;
        destroy_battle_critical(&mut world, id, life_support).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["turn_clock"] = 1.into();
        world.btech = serde_json::from_value(saved).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_life_support_action(&scripts, &config).unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].pilot_injuries(),
            0,
            "{cells} mid-turn"
        );
        let mut saved = serde_json::to_value(&scripts.world().btech).unwrap();
        saved["turn_clock"] = 0.into();
        scripts.world_mut().btech = serde_json::from_value(saved).unwrap();
        advance_battle_life_support_action(&scripts, &config).unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].pilot_injuries(),
            u8::from(injured),
            "{cells}"
        );
    }
}
