//! BattleMech anatomy over stable section identities, separate from simulation readiness.
use super::{MechSection, MechTemplate};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Limb arrangement determines section names, leg roles and critical capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechChassis {
    Biped,
    Quad,
}

impl MechChassis {
    /// Quad ground and non-FASA jump turns use twice the ordinary angular rate.
    pub fn turn_multiplier(self) -> f64 {
        if self == Self::Quad { 2.0 } else { 1.0 }
    }

    /// Decode the asset movement class without assuming its live simulation is available.
    pub fn parse(name: &str) -> Result<Self> {
        if name.eq_ignore_ascii_case("Biped") {
            return Ok(Self::Biped);
        }
        if name.eq_ignore_ascii_case("Quad") {
            return Ok(Self::Quad);
        }
        anyhow::bail!("Unsupported BattleMech chassis {name}")
    }

    /// Configured skill identity shared by checks and piloting experience awards.
    pub fn piloting_skill(self, extended: bool) -> &'static str {
        if !extended {
            return "Piloting-Battlemech";
        }
        match self {
            Self::Biped => "Piloting-Biped",
            Self::Quad => "Piloting-Quad",
        }
    }

    /// Inherent control advantage, before damage cancels a quad's intact-leg bonus.
    pub fn piloting_modifier(self) -> i16 {
        if self == Self::Quad { -2 } else { 0 }
    }

    /// Load-bearing limbs in stable section order; a quad's front attachments are legs.
    pub fn legs(self) -> &'static [MechSection] {
        use MechSection::*;
        match self {
            Self::Biped => &[LeftLeg, RightLeg],
            Self::Quad => &[LeftArm, RightArm, LeftLeg, RightLeg],
        }
    }

    /// Whether the attachment is a leg for this chassis.
    pub fn is_leg(self, section: MechSection) -> bool {
        self.legs().contains(&section)
    }

    /// Critical capacity of one section, independent of installed equipment or damage.
    pub fn critical_slots(self, section: MechSection) -> u8 {
        if section == MechSection::Head || self.is_leg(section) {
            return 6;
        }
        12
    }

    /// Asset spelling for a stable section identity under this chassis arrangement.
    pub fn section_name(self, section: MechSection) -> &'static str {
        use MechSection::*;
        if self == Self::Biped {
            return section.name();
        }
        match section {
            LeftArm => "Front_Left_Leg",
            RightArm => "Front_Right_Leg",
            LeftLeg => "Rear_Left_Leg",
            RightLeg => "Rear_Right_Leg",
            _ => section.name(),
        }
    }

    /// Decode player-facing section names or compact labels for this chassis.
    pub fn parse_location(self, value: &str) -> Result<MechSection> {
        let normalized = value.to_ascii_lowercase().replace(['_', ' '], "");
        let short = match self {
            Self::Biped => ["la", "ra", "lt", "rt", "ct", "ll", "rl", "h"],
            Self::Quad => ["fll", "frl", "lt", "rt", "ct", "rll", "rrl", "h"],
        };
        MechSection::ALL
            .into_iter()
            .zip(short)
            .find(|(section, short)| {
                normalized == *short
                    || normalized
                        == self
                            .section_name(*section)
                            .to_ascii_lowercase()
                            .replace('_', "")
                    || (*section == MechSection::Head && normalized == "hd")
                    || (self == Self::Quad
                        && match section {
                            MechSection::LeftArm => normalized == "flleg",
                            MechSection::RightArm => normalized == "frleg",
                            MechSection::LeftLeg => normalized == "rlleg",
                            MechSection::RightLeg => normalized == "rrleg",
                            _ => false,
                        })
            })
            .map(|(section, _)| section)
            .with_context(|| format!("Invalid section {value} for {self:?}"))
    }

    /// Decode only headings belonging to this anatomy, rejecting mixed limb arrangements.
    pub fn parse_section(self, name: &str) -> Result<MechSection> {
        MechSection::ALL
            .into_iter()
            .find(|section| self.section_name(*section).eq_ignore_ascii_case(name))
            .with_context(|| format!("unsupported section {name} for {self:?}"))
    }
}

impl MechTemplate {
    /// Derive anatomy from the retained asset fields without storing a duplicate class value.
    pub fn chassis(&self) -> Result<MechChassis> {
        MechChassis::parse(
            self.attributes
                .get("move_type")
                .context("Missing movement type")?,
        )
    }
}
