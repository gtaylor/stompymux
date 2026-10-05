//! Construction-only ammunition normalization and separation from persisted live ammunition.
use crate::support;
use stompymux_rs::*;

/// A bounded ammunition template with independently supplied quantity and bin flags.
fn definition(weapon: Weapon, quantity: u16, flags: &[&str]) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let bin = template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = quantity.to_string();
    bin.modes = flags.iter().map(|s| s.to_string()).collect();
    template
}

/// Template quantities select a size and fill it once, including zero, odd capacities and overfilled bins.
#[test]
fn ammunition_template_normalization_boundaries() {
    for weapon in Weapon::ALL
        .iter()
        .copied()
        .filter(|w| w.profile().ammunition_per_ton > 0)
    {
        let full = u16::from(weapon.profile().ammunition_per_ton);
        let half = full / 2;
        for quantity in [0, 1, half, half + 1, full - 1, full, full + 1, u16::MAX] {
            for explicit_half in [false, true] {
                let flags = if explicit_half {
                    vec!["Halfton"]
                } else {
                    vec![]
                };
                let template = definition(weapon, quantity, &flags);
                let original = template.clone();
                let mut lowercase = template.clone();
                for section in lowercase.sections.values_mut() {
                    for part in section.criticals.values_mut() {
                        part.equipment = part.equipment.to_ascii_lowercase();
                    }
                }
                let lowercase_unit = Mech::from_template(lowercase).unwrap();
                let unit = Mech::from_template(template).unwrap();
                let expected_half = explicit_half || quantity <= half;
                let capacity = if expected_half { half } else { full };
                let loadout = unit.loadout().unwrap();
                assert_eq!(lowercase_unit.loadout().unwrap(), loadout);
                assert_eq!(lowercase_unit.ammunition(), unit.ammunition());
                let bin = &loadout.ammunition[0];
                assert_eq!(bin.half_ton, expected_half);
                assert_eq!(bin.rounds, capacity);
                assert_eq!(bin.capacity, capacity);
                assert_eq!(unit.ammunition(), [capacity]);
                assert_eq!(
                    original.sections[&MechSection::RightTorso].criticals[&0].data,
                    quantity.to_string()
                );
                assert_eq!(
                    unit.definition().sections[&MechSection::RightTorso].criticals[&0].data,
                    capacity.to_string()
                );
            }
        }
    }
    let unit = Mech::from_template(definition(Weapon::Lbx2, 1, &["LBX/Cluster"])).unwrap();
    assert_eq!(
        unit.loadout().unwrap().ammunition[0].mode,
        AmmunitionMode::Cluster
    );
    assert_eq!(unit.ammunition(), [22]);
    for flags in [
        vec!["Halfton", "Halfton"],
        vec!["UnknownBinFlag"],
        vec!["LBX/Cluster"],
    ] {
        assert!(Mech::from_template(definition(Weapon::MachineGun, 1, &flags)).is_err());
    }
}

/// Native/Lua construction normalize identically; callback rollback and restart never refill live bins.
#[tokio::test(flavor = "current_thread")]
async fn ammunition_template_native_lua_creation_and_empty_restart() {
    let (dir, config, mut world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("units")).unwrap();
    let source = include_str!("fixtures/btech/units/JR7-D.toml").replace(
        "item = \"Ammo_IS.SRM-4\", rounds = 25",
        "item = \"Ammo_IS.SRM-4\", rounds = 7",
    );
    std::fs::write(dir.path().join("units/partial.toml"), &source).unwrap();
    let id = world.create(&config, "Normalized Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.create({},'partial'); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("@btech unit-create #{}=partial", id.0),
    );
    assert!(text.contains("constructed"), "{text}");
    lua.eval_callback::<()>(&format!("btech.unit.create({},'partial')", id.0))
        .unwrap();
    let mut built = native.world().clone();
    let scripted = lua.world().clone();
    assert_eq!(
        built.btech.constructed_units()[&id].definition(),
        scripted.btech.constructed_units()[&id].definition()
    );
    assert_eq!(built.btech.constructed_units()[&id].ammunition(), [12]);
    assert_eq!(scripted.btech.constructed_units()[&id].ammunition(), [12]);
    let bin = built.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .ammunition[0]
        .location;
    destroy_battle_critical(&mut built, id, bin).unwrap();
    persistence::save(&config.database(), &built).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, built.btech);
    assert_eq!(restored.btech.constructed_units()[&id].ammunition(), [0]);
    assert_eq!(
        restored.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .ammunition[0]
            .rounds,
        12
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("units/partial.toml")).unwrap(),
        source
    );
}
