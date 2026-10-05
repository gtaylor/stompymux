//! Live effects of chassis-wide technologies on a constructed unit's armor and structure.
use super::{DamageClass, Mech, MechSection, Technology};

impl Mech {
    /// Whether this Mech's armor is hardened.
    pub(crate) fn hardened_armor(&self) -> bool {
        self.definition().has_technology(Technology::HardenedArmor)
    }

    /// Hardened armor points removed from a location and the full-value overflow, or `None`
    /// for ordinary armor. Rear hits on torsos use rear armor.
    pub(crate) fn hardened_hit(
        &self,
        section: MechSection,
        rear: bool,
        amount: u16,
    ) -> Option<(u16, u16)> {
        self.armor_hit(section, rear, amount, DamageClass::Ordinary)
    }

    /// Armor points a hit of `class` removes from a location with hardened or reflective
    /// armor, and the damage that passes on to the structure, or `None` when the location's
    /// armor absorbs the hit point for point. Rear hits on torsos use rear armor.
    pub(crate) fn armor_hit(
        &self,
        section: MechSection,
        rear: bool,
        amount: u16,
        class: DamageClass,
    ) -> Option<(u16, u16)> {
        let hardened = self.hardened_armor();
        let reflective = self
            .definition()
            .has_technology(Technology::LaserReflectiveArmor);
        if !hardened && !reflective {
            return None;
        }
        let state = &self.sections()[&section];
        let torso = matches!(
            section,
            MechSection::LeftTorso | MechSection::CenterTorso | MechSection::RightTorso
        );
        let armor = if rear && torso {
            state.rear
        } else {
            state.armor
        };
        if hardened {
            let (removed, overflow) = Technology::hardened_hit(u32::from(amount), armor);
            return Some((removed, overflow as u16));
        }
        Technology::reflective_hit(class, u32::from(amount), armor)
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
        Technology::structure_damage(
            definition.has_technology(Technology::ReinforcedStructure),
            definition.has_technology(Technology::CompositeStructure),
            amount,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::btech::{CriticalDefinition, Mech, MechSection, MechTemplate};

    /// The twenty-damage piloting check counts each hardened point lost once, plus overflow.
    #[test]
    fn hardened_hits_count_armor_points_toward_piloting_checks() {
        let mut template = MechTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
        )
        .unwrap();
        let specials = template.attributes.entry("specials".into()).or_default();
        specials.push_str(" HARM");
        let unit = Mech::from_template(template).unwrap();
        let left_arm = MechSection::LeftArm;
        // Six damage removes three of four points: three count.
        assert_eq!(unit.hardened_hit(left_arm, false, 6), Some((3, 0)));
        // Twelve damage removes all four and four more pass through: eight count.
        assert_eq!(unit.hardened_hit(left_arm, false, 12), Some((4, 4)));
        // Rear torso hits use rear armor.
        let torso = MechSection::CenterTorso;
        assert_eq!(unit.hardened_hit(torso, true, 10), Some((3, 4)));
        assert_eq!(unit.hardened_piloting_modifier(), 1);
    }

    /// Unit construction enforces the template's specialized armor slot rules.
    #[test]
    fn construction_rejects_misallocated_reflective_armor() {
        let jenner = |slots: u8| {
            let mut template = MechTemplate::parse(
                "JR7-D",
                include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap();
            let flags = template.attributes.entry("specials".into()).or_default();
            flags.push_str(" LaserRefArmor_Tech");
            let torso = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
            for slot in 2..2 + slots {
                torso.criticals.insert(
                    slot,
                    CriticalDefinition {
                        equipment: "LaserReflective".into(),
                        data: "-".into(),
                        modes: Vec::new(),
                    },
                );
            }
            template
        };
        assert!(Mech::from_template(jenner(9)).is_err());
        assert!(Mech::from_template(jenner(10)).is_ok());
    }

    /// A small cockpit is a construction choice; the bare technology flag cannot stand in
    /// for it on a unit whose head keeps standard life support.
    #[test]
    fn small_cockpits_construct_only_from_their_construction_choice() {
        let jenner = include_str!("../../tests/fixtures/btech/units/JR7-D.toml");
        let small = jenner
            .replace(
                "{ at = 4, item = \"HeatSink\" }",
                "{ at = 5, item = \"HeatSink\" }",
            )
            .replacen(
                "\n[sections.",
                "\n[construction]\ncockpit = \"small\"\n\n[sections.",
                1,
            );
        let template = MechTemplate::parse("JR7-D", &small).unwrap();
        assert!(Mech::from_template(template).is_ok());

        let mut flagged = MechTemplate::parse("JR7-D", jenner).unwrap();
        flagged
            .attributes
            .insert("specials".into(), "SmallCockpit_Tech".into());
        assert!(Mech::from_template(flagged).is_err());
    }
}
