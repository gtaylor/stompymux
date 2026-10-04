//! Construction mass arithmetic in 1/1024-ton units: engines, structure, armor, systems and
//! cargo space, shared by Mech and vehicle mass calculations.
use super::{MechSection, System, WeaponMount};
use anyhow::{Context, Result, ensure};

/// Installed system mass per surviving Mech critical, independent of current critical damage.
pub fn system_slot_mass(definition: &super::MechTemplate, system: System) -> u32 {
    match system {
        System::Case | System::LightProbe => 512,
        System::C3i => 1280,
        // The Clan active probe fills one slot for a ton; the Beagle two for a ton and a half.
        System::BeagleProbe => {
            if definition.has_special("Clan") {
                1024
            } else {
                768
            }
        }
        System::BloodhoundProbe => 2048 / 3,
        System::TargetingComputer
        | System::Masc
        | System::C3Master
        | System::C3Slave
        | System::Tag
        | System::AngelEcm
        | System::Axe
        | System::Mace
        | System::DualSaw
        | System::Claw => 1024,
        // An Artemis V controller weighs a ton and a half.
        System::ArtemisIv => {
            if definition.has_technology(super::Technology::ArtemisV) {
                1536
            } else {
                1024
            }
        }
        // A Watchdog CEWS weighs a ton and a half across two ECM slots.
        System::Ecm => {
            if definition.has_technology(super::Technology::Watchdog) {
                768
            } else if definition.has_special("Clan") {
                1024
            } else {
                768
            }
        }
        // Inner Sphere CASE II weighs a ton per slot; the Clan version weighs half as much.
        System::CaseIi => {
            if definition.has_special("Clan") {
                512
            } else {
                1024
            }
        }
        System::Sword => {
            u32::from(definition.tons.div_ceil(10)) * 512 / u32::from(definition.tons.div_ceil(15))
        }
        // Half a ton plus a twentieth of the Mech rounded up to the half ton, spread over one
        // slot plus one per twenty tons.
        System::RetractableBlade => {
            let slots = u32::from(definition.tons.div_ceil(20)) + 1;
            (512 + half_ton((u32::from(definition.tons) * 1024).div_ceil(20))) / slots
        }
        System::Lance => 1024,
        System::Flail => 5 * 1024 / 4,
        System::WreckingBall => 4 * 1024 / 5,
        System::ChainWhip => 3 * 1024 / 2,
        System::SmallVibroblade => 3 * 1024,
        System::MediumVibroblade => 5 * 1024 / 2,
        System::LargeVibroblade => 7 * 1024 / 4,
        System::JumpJet => match definition.tons {
            0..=55 => 512,
            56..=85 => 1024,
            _ => 2048,
        },
        _ => 0,
    }
}

/// A one-shot launcher weighs half a ton more than the launcher it adapts; rocket launchers are
/// built as one-shot weapons and their catalogue mass already counts it.
pub fn one_shot_mass<L>(mount: &super::WeaponMount<L>) -> u32 {
    if mount.one_shot && !mount.weapon.is_rocket() {
        512
    } else {
        0
    }
}

/// Power amplifiers a combustion-engined Mech needs for its surviving energy weapons: a tenth of
/// their mass, rounded up to the half ton.
pub fn power_amplifier_mass(
    weapons: &[WeaponMount],
    survives: impl Fn(MechSection) -> bool,
) -> u32 {
    let energy: u32 = weapons
        .iter()
        .filter(|mount| {
            (mount.weapon.is_energy() && mount.weapon.profile().ammunition_per_ton == 0)
                || matches!(
                    mount.weapon,
                    super::Weapon::PlasmaRifle
                        | super::Weapon::LaserAms
                        | super::Weapon::ClanLaserAms
                )
        })
        .filter(|mount| survives(mount.criticals[0].section))
        .map(|mount| mount.weapon.mass())
        .sum();
    half_ton(energy.div_ceil(10))
}

/// Armor mass for `protection` points of a type worth `denominator` fiftieths of a standard
/// point each: converted to tons without truncating, then rounded up to the next half ton.
pub fn armor_mass(protection: u32, denominator: u32) -> u32 {
    half_ton((protection * 50 * 1024).div_ceil(denominator * 16))
}

/// Shared structure accounting preserves the surviving proportion before half-ton rounding.
/// Wide intermediate arithmetic supports large authored protection values without overflow.
pub fn structure_mass(tons: u16, current: u32, original: u32, divisor: u32) -> Result<u32> {
    let mass = u64::from(tons) * 1024 * u64::from(current)
        / 5
        / u64::from(original.max(1))
        / u64::from(divisor);
    Ok(half_ton(u32::try_from(mass)?))
}

/// Half-ton rounding tolerates a single fixed-point unit above an exact boundary.
pub fn half_ton(value: u32) -> u32 {
    let remainder = value % 512;
    if remainder <= 1 {
        value - remainder
    } else {
        value + 512 - remainder
    }
}

/// Standard fusion-engine catalog in half tons, for ratings 10 through 500 in steps of five.
pub fn engine_mass(rating: u32) -> u32 {
    const HALF_TONS: [u16; 99] = [
        1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 6, 7, 7, 8, 8, 8, 9, 9, 10, 10, 11,
        11, 12, 12, 12, 14, 14, 15, 15, 16, 17, 17, 18, 19, 20, 20, 21, 22, 23, 24, 25, 26, 27, 28,
        29, 31, 32, 33, 35, 36, 38, 39, 41, 43, 45, 47, 49, 51, 54, 57, 59, 63, 66, 69, 73, 77, 82,
        87, 92, 98, 105, 113, 122, 133, 145, 159, 175, 194, 215, 239, 267, 300, 337, 380, 429, 486,
        551, 626, 712, 811, 925,
    ];
    if !(10..=500).contains(&rating) || !rating.is_multiple_of(5) {
        return 0;
    }
    u32::from(HALF_TONS[((rating - 10) / 5) as usize]) * 512
}

/// Construction mass for authored cargo space, shared by Mechs and vehicles.
/// This measures the cargo installation; loose stock is accounted for separately.
pub fn cargo_space_mass(space: Option<&str>, carrier: bool, cargo: bool) -> Result<u32> {
    let space = space
        .map(|value| value.parse::<u32>().context("Invalid cargo space"))
        .transpose()?
        .unwrap_or(0);
    let divisor = if carrier {
        1000.0
    } else if cargo {
        100.0
    } else {
        500.0
    };
    let mass = (space as f32 / divisor * 1024.0).trunc();
    ensure!(
        f64::from(mass) <= f64::from(u32::MAX),
        "Cargo space mass overflow"
    );
    Ok(mass as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_rounding_and_engine_catalog_boundaries() {
        for (value, expected) in [
            (0, 0),
            (1, 0),
            (2, 512),
            (511, 512),
            (512, 512),
            (513, 512),
            (514, 1024),
        ] {
            assert_eq!(half_ton(value), expected);
        }
        for (rating, half_tons) in [
            (0, 0),
            (5, 0),
            (10, 1),
            (11, 0),
            (100, 6),
            (245, 24),
            (300, 38),
            (400, 105),
            (500, 925),
            (505, 0),
        ] {
            assert_eq!(engine_mass(rating), half_tons * 512);
        }
    }

    /// Armor converts points to tons before rounding, so a fraction of a point still costs
    /// the next half ton.
    #[test]
    fn armor_mass_rounds_up_without_truncating_points() {
        // 144 Inner Sphere ferro-fibrous points weigh 8.04 tons: 8.5 after rounding.
        assert_eq!(armor_mass(144, 56), 8 * 1024 + 512);
        // 154 Clan ferro-fibrous points weigh 8.02 tons.
        assert_eq!(armor_mass(154, 60), 8 * 1024 + 512);
        assert_eq!(armor_mass(160, 50), 10 * 1024);
        assert_eq!(armor_mass(161, 50), 10 * 1024 + 512);
        // Hardened armor carries eight points per ton.
        assert_eq!(armor_mass(80, 25), 10 * 1024);
    }

    /// CASE II weighs a ton per Inner Sphere slot and half a ton per Clan slot.
    #[test]
    fn case_ii_slot_mass_depends_on_technology_base() {
        let mut definition =
            crate::MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml"))
                .unwrap();
        assert_eq!(system_slot_mass(&definition, System::CaseIi), 1024);
        assert_eq!(system_slot_mass(&definition, System::Case), 512);
        let specials = definition.attributes.entry("specials".into()).or_default();
        specials.push_str(" Clan");
        assert_eq!(system_slot_mass(&definition, System::CaseIi), 512);
        assert_eq!(System::parse("CASE-II").unwrap(), System::CaseIi);
        assert!(System::CaseIi.is_noncritical());
    }
}
