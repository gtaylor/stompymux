//! Chassis-wide construction technologies recorded as template flags rather than critical slots.

/// A chassis technology that templates may spell by its full name or reference abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleTechnology {
    /// Armor that halves incoming armor damage and weighs twice as much.
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
}

impl BattleTechnology {
    /// Every technology handled here.
    pub const ALL: [Self; 7] = [
        Self::HardenedArmor,
        Self::ReinforcedStructure,
        Self::CompositeStructure,
        Self::SmallCockpit,
        Self::LaserHeatSinks,
        Self::Watchdog,
        Self::ArtemisV,
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
        }
    }

    /// Whether a template flag spells any handled technology.
    pub(crate) fn recognizes(flag: &str) -> bool {
        Self::ALL.into_iter().any(|technology| {
            let (name, abbreviation) = technology.names();
            flag.eq_ignore_ascii_case(name) || flag.eq_ignore_ascii_case(abbreviation)
        })
    }

    /// Armor damage after hardened armor absorbs two points per point of protection.
    pub(crate) fn armor_damage(hardened: bool, amount: u16) -> u16 {
        if hardened { amount.div_ceil(2) } else { amount }
    }

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

/// Whether a template flag spells `name`, accepting the abbreviation of a handled technology.
pub(crate) fn spells(flag: &str, name: &str) -> bool {
    flag.eq_ignore_ascii_case(name)
        || BattleTechnology::ALL.into_iter().any(|technology| {
            let (full, abbreviation) = technology.names();
            full.eq_ignore_ascii_case(name) && flag.eq_ignore_ascii_case(abbreviation)
        })
}

impl super::BattleUnit {
    /// Armor damage this Mech actually applies to its armor.
    pub(crate) fn armor_damage(&self, amount: u16) -> u16 {
        BattleTechnology::armor_damage(
            self.definition()
                .has_technology(BattleTechnology::HardenedArmor),
            amount,
        )
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
        assert_eq!(BattleTechnology::armor_damage(true, 5), 3);
        assert_eq!(BattleTechnology::armor_damage(false, 5), 5);
        assert_eq!(BattleTechnology::structure_damage(true, false, 1), 1);
        assert_eq!(BattleTechnology::structure_damage(false, true, 3), 6);
        assert_eq!(
            BattleTechnology::structure_damage(false, true, u16::MAX),
            u16::MAX
        );
        assert!(BattleTechnology::recognizes("harm"));
        assert!(BattleTechnology::recognizes("WatchDog_Tech"));
        assert!(!BattleTechnology::recognizes("Clan"));
    }
}
