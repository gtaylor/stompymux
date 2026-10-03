//! Chassis-wide construction technologies recorded as template flags rather than critical slots.
use anyhow::{Result, ensure};

/// A chassis technology that templates may spell by its full name or reference abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleTechnology {
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
pub enum BattleDamageClass {
    /// Ballistic, missile, physical and environmental damage.
    #[default]
    Ordinary,
    /// Lasers, PPCs, flamers and plasma weapons.
    Energy,
    /// Artillery and other blasts that fill a hex.
    AreaEffect,
}

impl BattleDamageClass {
    /// The class of a direct hit from `weapon`.
    pub fn of_weapon(weapon: super::BattleWeapon) -> Self {
        if weapon.is_energy() {
            Self::Energy
        } else {
            Self::Ordinary
        }
    }
}

impl BattleTechnology {
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
    pub(crate) fn recognizes(flag: &str) -> bool {
        Self::ALL.into_iter().any(|technology| {
            let (name, abbreviation) = technology.names();
            flag.eq_ignore_ascii_case(name) || flag.eq_ignore_ascii_case(abbreviation)
        })
    }

    /// Tabletop hardened armor: each remaining point stops two damage, so a hit removes half
    /// its damage in armor, rounding up. Damage beyond twice the remaining armor passes through
    /// at full value. Returns the armor points removed and that overflow.
    pub(crate) fn hardened_hit(amount: u32, armor: u16) -> (u16, u32) {
        let removed = amount.div_ceil(2).min(u32::from(armor)) as u16;
        (removed, amount.saturating_sub(u32::from(armor) * 2))
    }

    /// Tabletop laser-reflective armor. Energy damage is halved, rounding down with a minimum
    /// of one, so each armor point stops two energy damage; damage beyond twice the remaining
    /// armor passes through at full value. Area-effect damage is doubled against the armor,
    /// so each damage point strips two armor points and whatever the armor could not absorb
    /// passes through at its normal value. Other damage is unaffected. Returns the armor
    /// points removed and the overflow, or `None` when the armor behaves normally.
    pub(crate) fn reflective_hit(
        class: BattleDamageClass,
        amount: u32,
        armor: u16,
    ) -> Option<(u16, u32)> {
        let armor = u32::from(armor);
        match class {
            BattleDamageClass::Ordinary => None,
            BattleDamageClass::Energy => {
                let removed = (amount / 2).max(amount.min(1)).min(armor);
                Some((removed as u16, amount.saturating_sub(armor * 2)))
            }
            BattleDamageClass::AreaEffect => {
                let doubled = amount.saturating_mul(2);
                if doubled <= armor {
                    return Some((doubled as u16, 0));
                }
                Some((armor as u16, amount - armor.div_ceil(2)))
            }
        }
    }

    /// Critical rolls for damage that penetrates hardened armor are two lower.
    pub(crate) const HARDENED_CRITICAL_PENALTY: u8 = 2;

    /// Internal damage after reinforced or composite structure modifies it.
    pub(crate) fn structure_damage(reinforced: bool, composite: bool, amount: u16) -> u16 {
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

impl super::BattleTemplate {
    /// Tabletop construction rules for specialized armor: laser-reflective armor fills exactly
    /// its slot count with `LaserReflective` criticals, those criticals need the armor, and
    /// hardened or reflective armor is the Mech's only armor type.
    pub(crate) fn validate_armor_slots(&self) -> Result<()> {
        let armor_types = ARMOR_TYPES
            .into_iter()
            .filter(|name| {
                self.has_special(name)
                    || BattleTechnology::ALL.into_iter().any(|technology| {
                        let (full, abbreviation) = technology.names();
                        full == *name && self.has_special(abbreviation)
                    })
            })
            .count();
        let special = self.has_technology(BattleTechnology::HardenedArmor)
            || self.has_technology(BattleTechnology::LaserReflectiveArmor);
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
                    super::BattleSystem::parse(&critical.equipment),
                    Ok(super::BattleSystem::LaserReflective)
                )
            })
            .count();
        let expected = if self.has_technology(BattleTechnology::LaserReflectiveArmor) {
            reflective_armor_slots(self.has_special("Clan"))
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
pub(crate) fn spells(flag: &str, name: &str) -> bool {
    flag.eq_ignore_ascii_case(name)
        || BattleTechnology::ALL.into_iter().any(|technology| {
            let (full, abbreviation) = technology.names();
            full.eq_ignore_ascii_case(name) && flag.eq_ignore_ascii_case(abbreviation)
        })
}

impl super::BattleUnit {
    /// Whether this Mech's armor is hardened.
    pub(crate) fn hardened_armor(&self) -> bool {
        self.definition()
            .has_technology(BattleTechnology::HardenedArmor)
    }

    /// Hardened armor points removed from a location and the full-value overflow, or `None`
    /// for ordinary armor. Rear hits on torsos use rear armor.
    pub(crate) fn hardened_hit(
        &self,
        section: super::BattleSection,
        rear: bool,
        amount: u16,
    ) -> Option<(u16, u16)> {
        self.armor_hit(section, rear, amount, BattleDamageClass::Ordinary)
    }

    /// Armor points a hit of `class` removes from a location with hardened or reflective
    /// armor, and the damage that passes on to the structure, or `None` when the location's
    /// armor absorbs the hit point for point. Rear hits on torsos use rear armor.
    pub(crate) fn armor_hit(
        &self,
        section: super::BattleSection,
        rear: bool,
        amount: u16,
        class: BattleDamageClass,
    ) -> Option<(u16, u16)> {
        let hardened = self.hardened_armor();
        let reflective = self
            .definition()
            .has_technology(BattleTechnology::LaserReflectiveArmor);
        if !hardened && !reflective {
            return None;
        }
        let state = &self.sections()[&section];
        let torso = matches!(
            section,
            super::BattleSection::LeftTorso
                | super::BattleSection::CenterTorso
                | super::BattleSection::RightTorso
        );
        let armor = if rear && torso {
            state.rear
        } else {
            state.armor
        };
        if hardened {
            let (removed, overflow) = BattleTechnology::hardened_hit(u32::from(amount), armor);
            return Some((removed, overflow as u16));
        }
        BattleTechnology::reflective_hit(class, u32::from(amount), armor)
            .map(|(removed, overflow)| (removed, overflow.min(u32::from(u16::MAX)) as u16))
    }

    /// Hardened armor costs a Mech one running MP.
    pub(crate) fn hardened_speed_penalty(&self) -> f64 {
        if self.hardened_armor() { 10.75 } else { 0.0 }
    }

    /// Hardened armor adds one to every piloting roll.
    pub(crate) fn hardened_piloting_modifier(&self) -> u8 {
        u8::from(self.hardened_armor())
    }

    /// Internal damage this Mech actually applies to its structure.
    pub(crate) fn structure_damage(&self, amount: u16) -> u16 {
        let definition = self.definition();
        BattleTechnology::structure_damage(
            definition.has_technology(BattleTechnology::ReinforcedStructure),
            definition.has_technology(BattleTechnology::CompositeStructure),
            amount,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_modifiers_round_up_and_saturate() {
        // Seven damage on two points: four stopped, three through at full value.
        assert_eq!(BattleTechnology::hardened_hit(7, 2), (2, 3));
        // Odd damage removes half, rounding up; nothing passes while armor remains.
        assert_eq!(BattleTechnology::hardened_hit(5, 10), (3, 0));
        assert_eq!(BattleTechnology::hardened_hit(4, 2), (2, 0));
        assert_eq!(BattleTechnology::hardened_hit(3, 0), (0, 3));
        assert_eq!(BattleTechnology::structure_damage(true, false, 1), 1);
        assert_eq!(BattleTechnology::structure_damage(false, true, 3), 6);
        assert_eq!(
            BattleTechnology::structure_damage(false, true, u16::MAX),
            u16::MAX
        );
        use BattleDamageClass::*;
        assert_eq!(BattleTechnology::reflective_hit(Ordinary, 7, 4), None);
        // Energy damage halves, rounding down, but always removes at least one point.
        assert_eq!(
            BattleTechnology::reflective_hit(Energy, 7, 10),
            Some((3, 0))
        );
        assert_eq!(
            BattleTechnology::reflective_hit(Energy, 1, 10),
            Some((1, 0))
        );
        assert_eq!(BattleTechnology::reflective_hit(Energy, 7, 2), Some((2, 3)));
        assert_eq!(BattleTechnology::reflective_hit(Energy, 3, 0), Some((0, 3)));
        // Area-effect damage doubles until the armor is gone.
        assert_eq!(
            BattleTechnology::reflective_hit(AreaEffect, 4, 10),
            Some((8, 0))
        );
        assert_eq!(
            BattleTechnology::reflective_hit(AreaEffect, 5, 3),
            Some((3, 3))
        );
        assert_eq!(
            BattleTechnology::reflective_hit(AreaEffect, 2, 0),
            Some((0, 2))
        );
        assert!(BattleTechnology::recognizes("LRARM"));
        assert!(BattleTechnology::recognizes("harm"));
        assert!(BattleTechnology::recognizes("WatchDog_Tech"));
        assert!(!BattleTechnology::recognizes("Clan"));
    }

    /// The twenty-damage piloting check counts each hardened point lost once, plus overflow.
    #[test]
    fn hardened_hits_count_armor_points_toward_piloting_checks() {
        let mut template = crate::btech::BattleTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml"),
        )
        .unwrap();
        let specials = template.attributes.entry("specials".into()).or_default();
        specials.push_str(" HARM");
        let unit = crate::btech::BattleUnit::from_template(template).unwrap();
        let left_arm = crate::btech::BattleSection::LeftArm;
        // Six damage removes three of four points: three count.
        assert_eq!(unit.hardened_hit(left_arm, false, 6), Some((3, 0)));
        // Twelve damage removes all four and four more pass through: eight count.
        assert_eq!(unit.hardened_hit(left_arm, false, 12), Some((4, 4)));
        // Rear torso hits use rear armor.
        let torso = crate::btech::BattleSection::CenterTorso;
        assert_eq!(unit.hardened_hit(torso, true, 10), Some((3, 4)));
        assert_eq!(unit.hardened_piloting_modifier(), 1);
    }

    /// Reflective armor claims ten Inner Sphere or five Clan slots; hardened armor claims none,
    /// and neither combines with another armor type.
    #[test]
    fn specialized_armor_follows_tabletop_slot_rules() {
        let jenner = |specials: &str, slots: u8| {
            let mut template = crate::btech::BattleTemplate::parse(
                "JR7-D",
                include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml"),
            )
            .unwrap();
            let flags = template.attributes.entry("specials".into()).or_default();
            flags.push(' ');
            flags.push_str(specials);
            let torso = template
                .sections
                .get_mut(&crate::btech::BattleSection::LeftTorso)
                .unwrap();
            for slot in 2..2 + slots {
                torso.criticals.insert(
                    slot,
                    crate::btech::CriticalDefinition {
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
        assert!(crate::btech::BattleUnit::from_template(jenner("LaserRefArmor_Tech", 9)).is_err());
        assert!(crate::btech::BattleUnit::from_template(jenner("LaserRefArmor_Tech", 10)).is_ok());
    }
}
