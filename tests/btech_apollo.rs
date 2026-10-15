//! MRM "Apollo" fire control: launcher links, construction, Battle Value, the negated MRM
//! to-hit penalty and damage that leaves the launcher a plain MRM.
use crate::support;
use stompymux_rs::*;

/// A Jenner whose left torso carries an MRM-10, a ton of its ammunition and, when `apollo`
/// is set, an Apollo system linked to the launcher.
fn jenner(apollo: bool) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let torso = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    torso.criticals.clear();
    let slot = |equipment: &str, data: &str| CriticalDefinition {
        equipment: equipment.into(),
        data: data.into(),
        modes: vec![],
    };
    torso.criticals.insert(0, slot("IS.MRM-10", "-"));
    torso.criticals.insert(1, slot("IS.MRM-10", "-"));
    torso.criticals.insert(2, slot("Ammo_IS.MRM-10", "24"));
    if apollo {
        torso.criticals.insert(3, slot("ApolloFCS", "1"));
    }
    support::templates::book_legal_keeping(&mut template, Some(MechSection::LeftTorso));
    template
}

/// The Apollo system serves the MRM in its location, raises the launcher's BV by 15 percent
/// (Tactical Operations: Advanced Units & Equipment p. 194, note L), and every MRM needs one
/// once any has one (ibid. p. 142).
#[test]
fn apollo_links_and_battle_value() {
    let guided = Mech::from_template(jenner(true)).unwrap();
    let index = guided
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Mrm10)
        .unwrap();
    assert!(guided.apollo_operational(index).unwrap());
    assert!(!guided.artemis_operational(index).unwrap());
    // A system without a link would guide nothing, so construction refuses it.
    let mut unassigned = jenner(true);
    unassigned
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .get_mut(&3)
        .unwrap()
        .data = "-".into();
    let error = Mech::from_template(unassigned).unwrap_err().to_string();
    assert!(error.contains("has no launcher link"), "{error}");
    // Against the same Mech with the system's ton spent on leg armor, which only the
    // defensive rating counts; it fires a plain MRM.
    let mut idle = jenner(true);
    idle.sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .remove(&3);
    for leg in [MechSection::LeftLeg, MechSection::RightLeg] {
        idle.sections.get_mut(&leg).unwrap().armor += 8;
    }
    let idle = Mech::from_template(idle).unwrap();
    assert!(!idle.apollo_operational(index).unwrap());
    let launcher = f64::from(Weapon::Mrm10.battle_value());
    let gained = guided.battle_value().unwrap().offensive - idle.battle_value().unwrap().offensive;
    assert!(gained > 0.0 && gained <= launcher * 0.15 * 2.0, "{gained}");
    assert!(Mech::from_template(jenner(false)).is_ok());
    let mut unlinked = jenner(true);
    let torso = unlinked.sections.get_mut(&MechSection::LeftTorso).unwrap();
    let apollo = torso.criticals.remove(&3).unwrap();
    unlinked
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(5, apollo);
    let error = Mech::from_template(unlinked).unwrap_err().to_string();
    assert!(
        error.contains("links slot 1, which is not the first slot of an MRM launcher"),
        "{error}"
    );
}

/// A working Apollo system negates the MRM's +1 to hit; once destroyed the launcher fires
/// as a plain MRM again (Tactical Operations: Advanced Units & Equipment p. 143).
#[tokio::test]
async fn apollo_negates_the_mrm_to_hit_penalty_until_destroyed() {
    let source = jenner(true).to_document().unwrap();
    let target = include_str!("fixtures/btech/units/JR7-D.toml");
    let (_dir, config, world) = support::isolated_world().await;
    let (mut world, shooter, target, _) = support::btech_firing::supply_fixture_on(
        world, &config, &source, None, target, false, None,
    );
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Mrm10)
        .unwrap();
    let accuracy = |world: &World| {
        battle_aim_modifiers(world, shooter, target, index, 4, AimRules::STANDARD)
            .unwrap()
            .weapon_accuracy
    };
    assert_eq!(accuracy(&world), 0);
    let apollo = CriticalLocation {
        section: MechSection::LeftTorso,
        slot: 3,
    };
    destroy_battle_critical(&mut world, shooter, apollo).unwrap();
    assert!(
        !world.btech.constructed_units()[&shooter]
            .apollo_operational(index)
            .unwrap()
    );
    assert_eq!(accuracy(&world), 1);
}
