//! Gyro construction families: the critical slots and mass each technology flag implies.
use super::MechTemplate;
use serde::Serialize;

/// Gyro families with implemented construction and damage behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Gyro {
    Standard,
    Hardened,
    Xl,
    Compact,
}

impl Gyro {
    /// Resolve the explicit construction family; ambiguous technology is rejected at construction.
    pub fn from_definition(definition: &MechTemplate) -> Self {
        if definition.has_special("XLGYRO") || definition.has_special("XLGyro_Tech") {
            return Self::Xl;
        }
        if definition.has_special("HDGYRO") || definition.has_special("HDGyro_Tech") {
            return Self::Hardened;
        }
        if definition.has_special("CGYRO") || definition.has_special("CompactGyro_Tech") {
            return Self::Compact;
        }
        Self::Standard
    }

    /// Number of critical slots in a complete installation.
    pub fn critical_slots(self) -> usize {
        match self {
            Self::Standard | Self::Hardened => 4,
            Self::Xl => 6,
            Self::Compact => 2,
        }
    }

    /// Scale the rounded standard mass, including the destroyed-center accounting value.
    pub fn mass(self, standard: i32) -> i32 {
        match self {
            Self::Standard => standard,
            Self::Hardened => standard * 2,
            Self::Xl => standard / 2,
            Self::Compact => standard * 3 / 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_follow_specials_and_scale_slots_and_mass() {
        let mut template =
            MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml")).unwrap();
        assert_eq!(Gyro::from_definition(&template), Gyro::Standard);
        for (flag, family, slots, mass) in [
            ("XLGYRO", Gyro::Xl, 6, 1024),
            ("HDGyro_Tech", Gyro::Hardened, 4, 4096),
            ("CGYRO", Gyro::Compact, 2, 3072),
        ] {
            template.attributes.insert("specials".into(), flag.into());
            assert_eq!(Gyro::from_definition(&template), family);
            assert_eq!(family.critical_slots(), slots);
            assert_eq!(family.mass(2048), mass);
        }
    }
}
