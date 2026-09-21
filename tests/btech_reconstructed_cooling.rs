//! Cooling reconstruction adopts engine allocation only on critical changes and retains future losses.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Set one trusted administrative field under normal authorization and rollback.
fn set(scripts: &Scripts, config: &Config, id: ObjectId, field: &str, value: &str) {
    set_battle_unit_field_action(scripts, config, ObjectId(1), id, field, value).unwrap();
}

#[tokio::test]
async fn single_sink_reconstruction_preserves_samples_and_applies_later_damage() {
    for (source, external) in firing::templates().into_iter().take(2).zip([1, 5]) {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let original = world.btech.constructed_units()[&id].cooling_capacity();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set(&scripts, &config, id, "heat", "7");
        set(&scripts, &config, id, "dissheat", "3");
        set(&scripts, &config, id, "hsengoverride", "1");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            original
        );
        set(&scripts, &config, id, "mechdamage", "A:2/1");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            original
        );
        set(&scripts, &config, id, "mechdamage", "C:7/0");
        let first = external + 1;
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            first
        );
        set(&scripts, &config, id, "hsengoverride", "2");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            first
        );
        set(&scripts, &config, id, "mechdamage", "C:7/1");
        {
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.cooling_capacity(), external + 2);
            assert_eq!(
                unit.sampled_heat_rates(),
                BattleHeatRates {
                    production: 7.0,
                    dissipation: 3.0
                }
            );
            assert_eq!(unit.heat_rates(&world).dissipation, f64::from(external + 2));
        }
        let sink = scripts.world().btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .systems
            .into_iter()
            .find(|part| part.system == BattleSystem::HeatSink)
            .unwrap()
            .location;
        destroy_battle_critical(&mut scripts.world_mut(), id, sink).unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            external + 1
        );
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
        set(&scripts, &config, id, "hsengoverride", "-1");
        let before = scripts.world().btech.clone();
        assert!(
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "")
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        set(&scripts, &config, id, "hsengoverride", "0");
        set(&scripts, &config, id, "mechdamage", "");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            original
        );
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn double_sinks_count_complete_installations_and_two_points_per_internal_sink() {
    for clan in [false, true] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let mut definition = BattleTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap();
        definition.heat_sinks = 20;
        definition.attributes.insert(
            "specials".into(),
            if clan { "Clan DoubleHS" } else { "DoubleHS" }.into(),
        );
        for section in definition.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
        for slot in 6..if clan { 8 } else { 9 } {
            definition
                .sections
                .get_mut(&BattleSection::RightArm)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "HeatSink".into(),
                        data: "-".into(),
                        modes: Vec::new(),
                        brand: None,
                    },
                );
        }
        let id = world.create(&config, "Cooling".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, id, definition).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set(&scripts, &config, id, "hsengoverride", "1");
        set(&scripts, &config, id, "mechdamage", "C:7/0");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            4
        );
        set(&scripts, &config, id, "mechdamage", "C:1/6");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            2
        );
        set(&scripts, &config, id, "mechdamage", "");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            4
        );
        set(&scripts, &config, id, "hsengoverride", "2147483647");
        set(&scripts, &config, id, "mechdamage", "C:7/0");
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].cooling_capacity(),
            22
        );
        scripts.world().validate(&config).unwrap();
    }
}
