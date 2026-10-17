//! Live effects of chassis-wide technologies on a constructed unit's armor and structure.
use super::{DamageClass, Mech, MechSection, Technology};
use serde::{Deserialize, Serialize};

/// One of a location's armor facings: its front armor, or a torso's rear armor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArmorFacing {
    pub section: MechSection,
    pub rear: bool,
}

impl ArmorFacing {
    /// A location's front armor.
    pub fn front(section: MechSection) -> Self {
        Self {
            section,
            rear: false,
        }
    }

    /// A torso's rear armor.
    pub fn rear(section: MechSection) -> Self {
        Self {
            section,
            rear: true,
        }
    }

    /// The facing a hit on `section` strikes: rear-arc hits strike a torso's rear armor, and
    /// every other location has only front armor.
    pub fn struck(section: MechSection, rear: bool) -> Self {
        Self {
            section,
            rear: rear
                && matches!(
                    section,
                    MechSection::LeftTorso | MechSection::CenterTorso | MechSection::RightTorso
                ),
        }
    }
}

/// What one hit does to a location's hardened or laser-reflective armor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArmorHit {
    /// Armor points destroyed.
    pub removed: u16,
    /// Damage passing on to the internal structure.
    pub overflow: u16,
    /// For hardened armor, whether the facing's outermost point is left with one slash.
    pub dented: Option<bool>,
}

impl Mech {
    /// Whether this Mech's armor is hardened.
    pub(crate) fn hardened_armor(&self) -> bool {
        self.definition().has_technology(Technology::HardenedArmor)
    }

    /// Whether this Mech's armor is laser-reflective.
    pub(crate) fn reflective_armor(&self) -> bool {
        self.definition()
            .has_technology(Technology::LaserReflectiveArmor)
    }

    /// What an ordinary hit does to a location's hardened armor, or `None` for other armor.
    /// Rear hits on torsos use rear armor.
    pub(crate) fn hardened_hit(
        &self,
        section: MechSection,
        rear: bool,
        amount: u16,
    ) -> Option<ArmorHit> {
        self.armor_hit(section, rear, amount, DamageClass::Ordinary)
    }

    /// What a hit of `class` does to a location with hardened or reflective armor, or `None`
    /// when the location's armor absorbs the hit point for point (Tactical Operations:
    /// Advanced Units & Equipment p. 93). Hardened armor takes two damage per point, keeping
    /// a half-damaged point between hits, and armor-piercing rounds as standard rounds on
    /// standard armor. Rear hits on torsos use rear armor.
    pub(crate) fn armor_hit(
        &self,
        section: MechSection,
        rear: bool,
        amount: u16,
        class: DamageClass,
    ) -> Option<ArmorHit> {
        let facing = ArmorFacing::struck(section, rear);
        let state = &self.sections()[&section];
        let armor = if facing.rear { state.rear } else { state.armor };
        if self.hardened_armor() {
            if class == DamageClass::ArmorPiercing {
                return None;
            }
            let hit = Technology::hardened_hit(
                u32::from(amount),
                armor,
                self.hardened_dents.contains(&facing),
            );
            return Some(ArmorHit {
                removed: hit.removed,
                overflow: hit.overflow.min(u32::from(u16::MAX)) as u16,
                dented: Some(hit.dented),
            });
        }
        if !self.reflective_armor() {
            return None;
        }
        Technology::reflective_hit(class, u32::from(amount), armor).map(|(removed, overflow)| {
            ArmorHit {
                removed,
                overflow: overflow.min(u32::from(u16::MAX)) as u16,
                dented: None,
            }
        })
    }

    /// Record whether a hardened facing's outermost point is left half-damaged by a hit.
    pub(crate) fn mark_hardened_dent(&mut self, facing: ArmorFacing, dented: bool) {
        if dented {
            self.hardened_dents.insert(facing);
        } else {
            self.hardened_dents.remove(&facing);
        }
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
        // Hardened armor's doubled weight comes out of the weapons.
        crate::btech::test_designs::balance(&mut template, &[]);
        let mut unit = Mech::from_template(template).unwrap();
        let left_arm = MechSection::LeftArm;
        let hit = |removed, overflow, dented| {
            Some(super::ArmorHit {
                removed,
                overflow,
                dented: Some(dented),
            })
        };
        // Six damage removes three of four points: three count.
        assert_eq!(unit.hardened_hit(left_arm, false, 6), hit(3, 0, false));
        // Twelve damage removes all four and four more pass through: eight count.
        assert_eq!(unit.hardened_hit(left_arm, false, 12), hit(4, 4, false));
        // Rear torso hits use rear armor.
        let torso = MechSection::CenterTorso;
        assert_eq!(unit.hardened_hit(torso, true, 10), hit(3, 4, false));
        // One damage only slashes a point, and the slash belongs to the facing struck.
        assert_eq!(unit.hardened_hit(left_arm, false, 1), hit(0, 0, true));
        unit.mark_hardened_dent(super::ArmorFacing::front(left_arm), true);
        assert_eq!(unit.hardened_hit(left_arm, false, 1), hit(1, 0, false));
        assert_eq!(unit.hardened_hit(left_arm, true, 1), hit(1, 0, false));
        assert_eq!(unit.hardened_hit(torso, false, 1), hit(0, 0, true));
        // Armor-piercing rounds meet hardened armor as standard rounds on standard armor.
        assert_eq!(
            unit.armor_hit(left_arm, false, 5, crate::btech::DamageClass::ArmorPiercing),
            None
        );
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
                "[construction]\n",
                "[construction]\ncockpit = \"small\"\n",
                1,
            );
        let mut template = MechTemplate::parse("JR7-D", &small).unwrap();
        // The ton the small cockpit saves becomes cargo space.
        crate::btech::test_designs::balance(&mut template, &[]);
        assert!(Mech::from_template(template).is_ok());

        let mut flagged = MechTemplate::parse("JR7-D", jenner).unwrap();
        flagged
            .attributes
            .insert("specials".into(), "SmallCockpit_Tech".into());
        assert!(Mech::from_template(flagged).is_err());
    }
}
