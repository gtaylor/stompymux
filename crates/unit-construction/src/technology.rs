//! Chassis-wide construction technologies recorded as template flags rather than critical slots.
use anyhow::{Result, ensure};

/// A chassis technology that templates may spell by its full name or reference abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Technology {
    /// Armor where each point stops two damage and that negates armor-piercing effects. It
    /// weighs twice as much, costs a Mech one running MP, adds one to piloting and driving
    /// rolls and subtracts two from critical rolls for damage that penetrates it.
    HardenedArmor,
    /// Structure that halves internal damage and weighs twice as much.
    ReinforcedStructure,
    /// Structure that doubles internal damage and weighs half as much.
    CompositeStructure,
    /// Two-ton cockpit that adds one to piloting rolls.
    SmallCockpit,
    /// Clan heat sinks that glow in darkness; dissipation follows the chassis sink rules.
    LaserHeatSinks,
    /// Clan ECM suite that also works as an active probe.
    Watchdog,
    /// Clan missile guidance that improves on Artemis IV.
    ArtemisV,
    /// Armor that reflects part of each energy hit but spalls under area-effect blasts.
    /// Energy weapons remove half their damage in armor, rounding down; area-effect
    /// weapons such as artillery remove double.
    LaserReflectiveArmor,
}

/// How an attack's damage interacts with specialized armor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DamageClass {
    /// Ballistic, missile, physical and environmental damage.
    #[default]
    Ordinary,
    /// Lasers, PPCs, flamers and plasma weapons.
    Energy,
    /// Artillery and other blasts that fill a hex.
    AreaEffect,
}

impl DamageClass {
    /// The class of a direct hit from `weapon`.
    pub fn of_weapon(weapon: super::Weapon) -> Self {
        if weapon.is_energy() {
            Self::Energy
        } else {
            Self::Ordinary
        }
    }
}

impl Technology {
    /// Every technology handled here.
    pub const ALL: [Self; 8] = [
        Self::HardenedArmor,
        Self::ReinforcedStructure,
        Self::CompositeStructure,
        Self::SmallCockpit,
        Self::LaserHeatSinks,
        Self::Watchdog,
        Self::ArtemisV,
        Self::LaserReflectiveArmor,
    ];

    /// The reference's full flag name and abbreviation.
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::HardenedArmor => ("HardenedArmor_Tech", "HARM"),
            Self::ReinforcedStructure => ("ReinforcedInternal_Tech", "RINT"),
            Self::CompositeStructure => ("CompositeInternal_Tech", "CINT"),
            Self::SmallCockpit => ("SmallCockpit_Tech", "SMCPIT"),
            Self::LaserHeatSinks => ("LaserHS_Tech", "LHS"),
            Self::Watchdog => ("WatchDog_Tech", "WDOG"),
            Self::ArtemisV => ("ArtemisV_Tech", "AV"),
            Self::LaserReflectiveArmor => ("LaserRefArmor_Tech", "LRARM"),
        }
    }

    /// Whether a template flag spells any handled technology.
    pub fn recognizes(flag: &str) -> bool {
        Self::ALL.into_iter().any(|technology| {
            let (name, abbreviation) = technology.names();
            flag.eq_ignore_ascii_case(name) || flag.eq_ignore_ascii_case(abbreviation)
        })
    }

    /// Tabletop hardened armor: each remaining point stops two damage, so a hit removes half
    /// its damage in armor, rounding up. Damage beyond twice the remaining armor passes through
    /// at full value. Returns the armor points removed and that overflow.
    pub fn hardened_hit(amount: u32, armor: u16) -> (u16, u32) {
        let removed = amount.div_ceil(2).min(u32::from(armor)) as u16;
        (removed, amount.saturating_sub(u32::from(armor) * 2))
    }

    /// Tabletop laser-reflective armor. Energy damage is halved, rounding down with a minimum
    /// of one, so each armor point stops two energy damage; damage beyond twice the remaining
    /// armor passes through at full value. Area-effect damage is doubled against the armor,
    /// so each damage point strips two armor points and whatever the armor could not absorb
    /// passes through at its normal value. Other damage is unaffected. Returns the armor
    /// points removed and the overflow, or `None` when the armor behaves normally.
    pub fn reflective_hit(class: DamageClass, amount: u32, armor: u16) -> Option<(u16, u32)> {
        let armor = u32::from(armor);
        match class {
            DamageClass::Ordinary => None,
            DamageClass::Energy => {
                let removed = (amount / 2).max(amount.min(1)).min(armor);
                Some((removed as u16, amount.saturating_sub(armor * 2)))
            }
            DamageClass::AreaEffect => {
                let doubled = amount.saturating_mul(2);
                if doubled <= armor {
                    return Some((doubled as u16, 0));
                }
                Some((armor as u16, amount - armor.div_ceil(2)))
            }
        }
    }

    /// Critical rolls for damage that penetrates hardened armor are two lower.
    pub const HARDENED_CRITICAL_PENALTY: u8 = 2;

    /// Internal damage after reinforced or composite structure modifies it.
    pub fn structure_damage(reinforced: bool, composite: bool, amount: u16) -> u16 {
        if reinforced {
            amount.div_ceil(2)
        } else if composite {
            amount.saturating_mul(2)
        } else {
            amount
        }
    }
}

/// Critical slots laser-reflective armor claims on a Mech: ten for Inner Sphere armor and five
/// for Clan armor. Hardened armor claims none.
pub fn reflective_armor_slots(clan: bool) -> usize {
    if clan { 5 } else { 10 }
}

/// Armor-type flags; hardened and reflective armor exclude all the others.
const ARMOR_TYPES: [&str; 7] = [
    "FerroFibrous_Tech",
    "HvyFerroFibrous_Tech",
    "LtFerroFibrous_Tech",
    "StealthArmor_Tech",
    "HardenedArmor_Tech",
    "LaserRefArmor_Tech",
    "ReactiveArmor_Tech",
];

impl super::MechTemplate {
    /// Tabletop construction rules for specialized armor: laser-reflective armor fills exactly
    /// its slot count with `LaserReflective` criticals, those criticals need the armor, and
    /// hardened or reflective armor is the Mech's only armor type.
    pub fn validate_armor_slots(&self) -> Result<()> {
        let armor_types = ARMOR_TYPES
            .into_iter()
            .filter(|name| {
                self.has_special(name)
                    || Technology::ALL.into_iter().any(|technology| {
                        let (full, abbreviation) = technology.names();
                        full == *name && self.has_special(abbreviation)
                    })
            })
            .count();
        let special = self.has_technology(Technology::HardenedArmor)
            || self.has_technology(Technology::LaserReflectiveArmor);
        ensure!(
            !special || armor_types == 1,
            "Hardened and laser-reflective armor replace every other armor type"
        );
        let found = self
            .sections
            .values()
            .flat_map(|section| section.criticals.values())
            .filter(|critical| {
                matches!(
                    super::System::named(&critical.equipment),
                    Some(super::System::LaserReflective)
                )
            })
            .count();
        let expected = if self.has_technology(Technology::LaserReflectiveArmor) {
            reflective_armor_slots(self.clan_armor())
        } else {
            0
        };
        ensure!(
            found == expected,
            "Laser-reflective armor needs {expected} LaserReflective critical slots; found {found}"
        );
        Ok(())
    }
}

/// Whether a template flag spells `name`, accepting the abbreviation of a handled technology.
pub fn flag_spells_technology(flag: &str, name: &str) -> bool {
    flag.eq_ignore_ascii_case(name)
        || Technology::ALL.into_iter().any(|technology| {
            let (full, abbreviation) = technology.names();
            full.eq_ignore_ascii_case(name) && flag.eq_ignore_ascii_case(abbreviation)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_modifiers_round_up_and_saturate() {
        // Seven damage on two points: four stopped, three through at full value.
        assert_eq!(Technology::hardened_hit(7, 2), (2, 3));
        // Odd damage removes half, rounding up; nothing passes while armor remains.
        assert_eq!(Technology::hardened_hit(5, 10), (3, 0));
        assert_eq!(Technology::hardened_hit(4, 2), (2, 0));
        assert_eq!(Technology::hardened_hit(3, 0), (0, 3));
        assert_eq!(Technology::structure_damage(true, false, 1), 1);
        assert_eq!(Technology::structure_damage(false, true, 3), 6);
        assert_eq!(
            Technology::structure_damage(false, true, u16::MAX),
            u16::MAX
        );
        use DamageClass::*;
        assert_eq!(Technology::reflective_hit(Ordinary, 7, 4), None);
        // Energy damage halves, rounding down, but always removes at least one point.
        assert_eq!(Technology::reflective_hit(Energy, 7, 10), Some((3, 0)));
        assert_eq!(Technology::reflective_hit(Energy, 1, 10), Some((1, 0)));
        assert_eq!(Technology::reflective_hit(Energy, 7, 2), Some((2, 3)));
        assert_eq!(Technology::reflective_hit(Energy, 3, 0), Some((0, 3)));
        // Area-effect damage doubles until the armor is gone.
        assert_eq!(Technology::reflective_hit(AreaEffect, 4, 10), Some((8, 0)));
        assert_eq!(Technology::reflective_hit(AreaEffect, 5, 3), Some((3, 3)));
        assert_eq!(Technology::reflective_hit(AreaEffect, 2, 0), Some((0, 2)));
        assert!(Technology::recognizes("LRARM"));
        assert!(Technology::recognizes("harm"));
        assert!(Technology::recognizes("WatchDog_Tech"));
        assert!(!Technology::recognizes("Clan"));
    }

    /// Reflective armor claims ten Inner Sphere or five Clan slots; hardened armor claims none,
    /// and neither combines with another armor type.
    #[test]
    fn specialized_armor_follows_tabletop_slot_rules() {
        let jenner = |specials: &str, slots: u8| {
            let mut template =
                crate::MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml"))
                    .unwrap();
            let flags = template.attributes.entry("specials".into()).or_default();
            flags.push(' ');
            flags.push_str(specials);
            let torso = template
                .sections
                .get_mut(&crate::MechSection::LeftTorso)
                .unwrap();
            for slot in 2..2 + slots {
                torso.criticals.insert(
                    slot,
                    crate::CriticalDefinition {
                        equipment: "LaserReflective".into(),
                        data: "-".into(),
                        modes: Vec::new(),
                    },
                );
            }
            template
        };
        assert!(
            jenner("LaserRefArmor_Tech", 10)
                .validate_armor_slots()
                .is_ok()
        );
        assert!(jenner("LRARM", 10).validate_armor_slots().is_ok());
        assert!(
            jenner("LaserRefArmor_Tech Clan", 5)
                .validate_armor_slots()
                .is_ok()
        );
        for (specials, slots) in [
            ("LaserRefArmor_Tech", 0),
            ("LaserRefArmor_Tech", 9),
            ("LaserRefArmor_Tech Clan", 10),
            ("", 1),
            ("HardenedArmor_Tech", 1),
            ("HardenedArmor_Tech FerroFibrous_Tech", 0),
            ("HARM LRARM", 10),
        ] {
            assert!(
                jenner(specials, slots).validate_armor_slots().is_err(),
                "{specials} {slots}"
            );
        }
        assert!(
            jenner("HardenedArmor_Tech", 0)
                .validate_armor_slots()
                .is_ok()
        );
    }
}
