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
    /// Four-ton cockpit in the center torso, with life support in the side torsos and a third
    /// sensor package; it adds one to piloting rolls and outlives the head (Tactical
    /// Operations: Advanced Units & Equipment pp. 112-113).
    TorsoMountedCockpit,
    /// Clan heat sinks that glow in darkness; dissipation follows the chassis sink rules.
    LaserHeatSinks,
    /// Clan ECM suite that also works as an active probe.
    Watchdog,
    /// Armor that reflects part of each energy hit but is brittle. Energy weapons remove half
    /// their damage in armor, rounding down; area-effect weapons such as artillery and mines,
    /// falls, physical attacks and moving through building walls remove double.
    LaserReflectiveArmor,
}

/// How an attack's damage interacts with specialized armor (Tactical Operations: Advanced
/// Units & Equipment p. 93).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DamageClass {
    /// Ballistic and missile weapon fire and environmental damage.
    #[default]
    Ordinary,
    /// Lasers, PPCs, flamers and plasma weapons.
    Energy,
    /// Artillery, minefields and other blasts that fill a hex.
    AreaEffect,
    /// Falls, physical attacks and moving through building walls, which laser-reflective
    /// armor takes doubled.
    Impact,
    /// Armor-piercing autocannon rounds, which hardened armor takes point for point as a
    /// standard round on standard armor.
    ArmorPiercing,
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
        Self::TorsoMountedCockpit,
        Self::LaserHeatSinks,
        Self::Watchdog,
        Self::LaserReflectiveArmor,
    ];

    /// The reference's full flag name and abbreviation.
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::HardenedArmor => ("HardenedArmor_Tech", "HARM"),
            Self::ReinforcedStructure => ("ReinforcedInternal_Tech", "RINT"),
            Self::CompositeStructure => ("CompositeInternal_Tech", "CINT"),
            Self::SmallCockpit => ("SmallCockpit_Tech", "SMCPIT"),
            Self::TorsoMountedCockpit => ("TorsoCockpit_Tech", "TCPIT"),
            Self::LaserHeatSinks => ("LaserHS_Tech", "LHS"),
            Self::Watchdog => ("WatchDog_Tech", "WDOG"),
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

    /// Tabletop hardened armor (Tactical Operations: Advanced Units & Equipment p. 93): each
    /// point takes two damage, the first slashing it and the second destroying it. `dented`
    /// says the outermost remaining point already carries one slash. A hit removes the points
    /// it completes and may leave the next one slashed; damage beyond what the remaining
    /// points can take passes through at full value.
    pub fn hardened_hit(amount: u32, armor: u16, dented: bool) -> HardenedHit {
        let dented = dented && armor > 0;
        let capacity = u32::from(armor) * 2 - u32::from(dented);
        if amount >= capacity {
            return HardenedHit {
                removed: armor,
                overflow: amount - capacity,
                dented: false,
            };
        }
        // Count the earlier slash as damage already marked against the outermost point.
        let marked = amount + u32::from(dented);
        HardenedHit {
            removed: (marked / 2) as u16,
            overflow: 0,
            dented: marked % 2 == 1,
        }
    }

    /// Tabletop laser-reflective armor (Tactical Operations: Advanced Units & Equipment p. 93).
    /// Energy damage is halved, rounding down with a minimum of one, so each armor point stops
    /// two energy damage; damage beyond twice the remaining armor passes through at full value.
    /// Area-effect and impact damage are doubled against the armor, so each damage point
    /// strips two armor points and whatever the armor could not absorb passes through at its
    /// normal value. Other damage is unaffected. Returns the armor points removed and the
    /// overflow, or `None` when the armor behaves normally.
    pub fn reflective_hit(class: DamageClass, amount: u32, armor: u16) -> Option<(u16, u32)> {
        let armor = u32::from(armor);
        match class {
            DamageClass::Ordinary | DamageClass::ArmorPiercing => None,
            DamageClass::Energy => {
                let removed = (amount / 2).max(amount.min(1)).min(armor);
                Some((removed as u16, amount.saturating_sub(armor * 2)))
            }
            DamageClass::AreaEffect | DamageClass::Impact => {
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

    /// Armor-piercing critical rolls against laser-reflective armor are two higher (Tactical
    /// Operations: Advanced Units & Equipment p. 93).
    pub const REFLECTIVE_ARMOR_PIERCING_BONUS: u8 = 2;

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

/// What one hit does to a location's hardened armor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardenedHit {
    /// Armor points destroyed.
    pub removed: u16,
    /// Damage passing on to the internal structure at full value.
    pub overflow: u32,
    /// Whether the outermost remaining point is left with one slash.
    pub dented: bool,
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
    fn damage_modifiers_keep_half_points_and_saturate() {
        let hit = |removed, overflow, dented| HardenedHit {
            removed,
            overflow,
            dented,
        };
        // Seven damage on two points: four stopped, three through at full value.
        assert_eq!(Technology::hardened_hit(7, 2, false), hit(2, 3, false));
        // Odd damage completes half its points and slashes the next; nothing passes while
        // armor remains.
        assert_eq!(Technology::hardened_hit(5, 10, false), hit(2, 0, true));
        assert_eq!(Technology::hardened_hit(4, 2, false), hit(2, 0, false));
        assert_eq!(Technology::hardened_hit(3, 0, false), hit(0, 3, false));
        // A single point of damage only slashes a point; the next one destroys it.
        assert_eq!(Technology::hardened_hit(1, 4, false), hit(0, 0, true));
        assert_eq!(Technology::hardened_hit(1, 4, true), hit(1, 0, false));
        assert_eq!(Technology::hardened_hit(4, 4, true), hit(2, 0, true));
        // A slashed last point stops one more damage before the rest passes through.
        assert_eq!(Technology::hardened_hit(3, 1, true), hit(1, 2, false));
        assert_eq!(Technology::hardened_hit(2, 0, true), hit(0, 2, false));
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
        // Area-effect and impact damage double until the armor is gone.
        assert_eq!(Technology::reflective_hit(AreaEffect, 4, 10), Some((8, 0)));
        assert_eq!(Technology::reflective_hit(AreaEffect, 5, 3), Some((3, 3)));
        assert_eq!(Technology::reflective_hit(AreaEffect, 2, 0), Some((0, 2)));
        assert_eq!(Technology::reflective_hit(Impact, 5, 9), Some((9, 0)));
        assert_eq!(Technology::reflective_hit(ArmorPiercing, 5, 9), None);
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
