//! Unit-test helpers that keep edited Mech designs within TechManual construction, so a test
//! that adds equipment to exercise one rule still fields a Mech of its own tonnage.
use super::{MechLoadout, MechTemplate, Weapon};

/// Bring an edited design to its tonnage: while it is overweight, shed the lightest weapon
/// outside `keep` (ammunition included); spare tonnage then becomes cargo space, which weighs a
/// ton per 500 units (TechManual pp. 46, 57).
pub(crate) fn balance(template: &mut MechTemplate, keep: &[&str]) {
    let tons = u32::from(template.tons) * 1024;
    let mass = |template: &MechTemplate| {
        let loadout = MechLoadout::resolve(template).unwrap();
        template.construction_mass(&loadout).unwrap().total
    };
    while mass(template) > tons {
        let lightest = template
            .sections
            .iter()
            .flat_map(|(&section, layout)| {
                layout
                    .criticals
                    .values()
                    .map(move |critical| (section, critical.equipment.clone()))
            })
            .filter(|(_, equipment)| {
                let weapon = equipment.trim_start_matches("Ammo_");
                Weapon::parse(weapon).is_ok() && !keep.contains(&weapon)
            })
            .min_by_key(|(_, equipment)| {
                Weapon::parse(equipment.trim_start_matches("Ammo_"))
                    .map(Weapon::mass)
                    .unwrap_or(0)
            })
            .expect("a weapon to shed");
        template
            .sections
            .get_mut(&lightest.0)
            .unwrap()
            .criticals
            .retain(|_, critical| critical.equipment != lightest.1);
    }
    let spare = tons - mass(template);
    if spare >= 512 {
        template
            .attributes
            .insert("cargo_space".into(), (spare * 500 / 1024).to_string());
    }
}
