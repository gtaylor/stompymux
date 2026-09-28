//! Chassis-wide construction technologies recorded as template flags rather than critical slots.

/// A chassis technology that templates may spell by its full name or reference abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleTechnology {
    /// Armor where each point stops two damage; it weighs twice as much, costs a Mech one
    /// running MP and adds one to piloting rolls made while running.
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

    /// Tabletop hardened armor: each remaining point stops two damage, so a hit removes half
    /// its damage in armor, rounding up. Damage beyond twice the remaining armor passes through
    /// at full value. Returns the armor points removed and that overflow.
    pub(crate) fn hardened_hit(amount: u32, armor: u16) -> (u16, u32) {
        let removed = amount.div_ceil(2).min(u32::from(armor)) as u16;
        (removed, amount.saturating_sub(u32::from(armor) * 2))
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
        if !self.hardened_armor() {
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
        let (removed, overflow) = BattleTechnology::hardened_hit(u32::from(amount), armor);
        Some((removed, overflow as u16))
    }

    /// Hardened armor costs a Mech one running MP.
    pub(crate) fn hardened_speed_penalty(&self) -> f64 {
        if self.hardened_armor() { 10.75 } else { 0.0 }
    }

    /// Hardened armor adds one to piloting rolls while the Mech is running.
    pub(crate) fn hardened_piloting_modifier(&self) -> u8 {
        let running = self
            .motion()
            .is_some_and(|motion| motion.speed > self.movement_maximum_speed() * 2.0 / 3.0 + 0.1);
        u8::from(self.hardened_armor() && running)
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
        assert!(BattleTechnology::recognizes("harm"));
        assert!(BattleTechnology::recognizes("WatchDog_Tech"));
        assert!(!BattleTechnology::recognizes("Clan"));
    }
}
