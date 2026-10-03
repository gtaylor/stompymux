//! Current conventional biped mass derived from durable construction and damage facts.
use super::{BattleSection, BattleSystem, BattleUnit};
use anyhow::{Context, Result};
use serde::Serialize;

/// Mass components in 1/1024-ton units. Gyro accounting is signed for a destroyed center torso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleMass {
    pub engine: u32,
    pub cockpit: u32,
    pub gyro: i32,
    pub structure: u32,
    pub armor: u32,
    pub equipment: u32,
    pub ammunition: u32,
    /// Authored cargo-space installation, independent of loose stock.
    pub cargo: u32,
    pub total: u32,
}

impl BattleUnit {
    /// Recalculate physical mass without caching derived values or substituting nominal tonnage.
    /// Damaged weapon criticals retain mass until their section is lost; ammunition uses live rounds.
    pub fn mass(&self) -> Result<BattleMass> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let survives = |section| self.sections()[&section].internal > 0;
        let rating = super::engine::rated_output(definition.tons, definition.max_speed)?;
        // C derives the engine mass family from technology flags alone; reference
        // builds without engine criticals still carry a nominal engine mass.
        let engine_family = || {
            super::BattleEngine::resolve(&loadout, definition.clan_engine()).unwrap_or_else(|_| {
                super::BattleEngine::display_from_flags(
                    definition.has_special("LightEngine_Tech"),
                    definition.has_special("CompactEngine_Tech"),
                    definition.has_special("XXL_Tech"),
                    definition.has_special("XLEngine_Tech"),
                )
            })
        };
        let ice = definition.has_special("ICEEngine_Tech");
        // Internal combustion engines weigh twice a standard fusion engine.
        let installed_engine = if ice {
            half_ton(engine_mass(rating) * 2)
        } else {
            half_ton(engine_family().unrounded_mass(engine_mass(rating)))
        };
        let engine = if survives(BattleSection::CenterTorso) {
            installed_engine
        } else {
            0
        };
        let gyro = if survives(BattleSection::CenterTorso) {
            rating.div_ceil(100) as i32 * 1024
        } else {
            -1024
        };
        let gyro = self.gyro().mass(gyro);
        let cockpit = if survives(BattleSection::Head) {
            if definition.has_technology(super::BattleTechnology::SmallCockpit) {
                2 * 1024
            } else {
                3 * 1024
            }
        } else {
            0
        };
        let original: u32 = definition
            .sections
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        let current: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        // Construction technology counts installed material, including damaged or lost slots.
        let material_count = |system| {
            loadout
                .systems
                .iter()
                .filter(|part| part.system == system)
                .count()
        };
        let material_slots = |clan| if clan { 7 } else { 14 };
        let structure_divisor =
            if definition.has_technology(super::BattleTechnology::ReinforcedStructure) {
                1
            } else if material_count(BattleSystem::EndoSteel)
                >= material_slots(definition.clan_structure())
                || definition.has_technology(super::BattleTechnology::CompositeStructure)
            {
                4
            } else {
                2
            };
        let structure = structure_mass(definition.tons, current, original, structure_divisor)?;
        let protection: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        let armor_denominator = if material_count(BattleSystem::FerroFibrous)
            >= material_slots(definition.clan_armor())
        {
            if definition.clan_armor() { 60 } else { 56 }
        } else if material_count(BattleSystem::HeavyFerroFibrous) >= 21 {
            62
        } else if material_count(BattleSystem::LightFerroFibrous) >= 7 {
            53
        } else if definition.has_technology(super::BattleTechnology::HardenedArmor) {
            // Hardened armor provides eight points per ton instead of sixteen.
            25
        } else {
            50
        };
        let armor = armor_mass(protection, armor_denominator);
        let mut equipment = loadout
            .systems
            .iter()
            .filter(|part| survives(part.location.section))
            .map(|part| system_slot_mass(definition, part.system))
            .sum::<u32>();
        for mount in &loadout.weapons {
            let slot_mass = mount.weapon.mass() / u32::from(mount.weapon.profile().critical_slots);
            equipment += slot_mass
                * mount
                    .criticals
                    .iter()
                    .filter(|slot| survives(slot.section))
                    .count() as u32;
            if survives(mount.criticals[0].section) {
                equipment += one_shot_mass(mount);
            }
        }
        let sink_mass = self.cooling_mass()?;
        equipment += sink_mass;
        if ice {
            equipment += power_amplifier_mass(&loadout.weapons, survives);
        }
        if loadout.systems.iter().any(|part| {
            part.system == BattleSystem::Supercharger && survives(part.location.section)
        }) {
            // A supercharger weighs a tenth of the engine, rounded up to the half ton.
            equipment += half_ton(installed_engine.div_ceil(10));
        }
        let ammunition = loadout
            .ammunition
            .iter()
            .zip(self.ammunition())
            .filter(|(bin, _)| survives(bin.location.section))
            .map(|(bin, &rounds)| {
                u32::from(rounds) * 1024
                    / u32::from(
                        bin.weapon
                            .profile_for_ammunition(bin.mode)
                            .ammunition_per_ton,
                    )
            })
            .sum();
        let cargo = super::load::cargo_space_mass(
            definition.attributes.get("cargo_space").map(String::as_str),
            definition.has_special("Carrier_Tech"),
            definition.has_special("CargoTech"),
        )?;
        let total = (i64::from(engine)
            + i64::from(cockpit)
            + i64::from(gyro)
            + i64::from(structure)
            + i64::from(armor)
            + i64::from(equipment)
            + i64::from(ammunition)
            + i64::from(cargo))
        .max(1);
        let total = u32::try_from(total).context("Mech mass overflow")?;
        Ok(BattleMass {
            engine,
            cockpit,
            gyro,
            structure,
            armor,
            equipment,
            ammunition,
            cargo,
            total,
        })
    }

    /// Current external cooling mass, shared by movement accounting and allocation reports.
    pub(super) fn cooling_mass(&self) -> Result<u32> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let survives = |section| self.sections()[&section].internal > 0;
        let sinks = definition
            .heat_sinks
            .saturating_sub(u16::from(self.system_hits(BattleSystem::HeatSink)));
        let sink_slots = definition.heat_sink_slots() as u32;
        let sink_efficiency = if definition.has_double_heat_sinks() {
            2
        } else {
            1
        };
        // Keep per-critical integer rounding, including 341 units per IS double-sink slot.
        let slot_mass = 1024 / sink_slots;
        // Fusion engines carry ten heat sinks for free; combustion engines carry none.
        let free = if definition.has_special("ICEEngine_Tech") {
            0
        } else {
            10
        };
        Ok(if sinks > 0 {
            (u32::from(sinks) * sink_slots / sink_efficiency).saturating_sub(free * sink_slots)
                * slot_mass
        } else {
            loadout
                .systems
                .iter()
                .filter(|slot| {
                    slot.system == BattleSystem::HeatSink && survives(slot.location.section)
                })
                .count() as u32
                * slot_mass
        })
    }
}

/// Installed system mass per surviving Mech critical, independent of current critical damage.
pub(super) fn system_slot_mass(definition: &super::BattleTemplate, system: BattleSystem) -> u32 {
    match system {
        BattleSystem::Case | BattleSystem::LightProbe => 512,
        BattleSystem::C3i => 1280,
        // The Clan active probe fills one slot for a ton; the Beagle two for a ton and a half.
        BattleSystem::BeagleProbe => {
            if definition.has_special("Clan") {
                1024
            } else {
                768
            }
        }
        BattleSystem::BloodhoundProbe => 2048 / 3,
        BattleSystem::TargetingComputer
        | BattleSystem::Masc
        | BattleSystem::C3Master
        | BattleSystem::C3Slave
        | BattleSystem::Tag
        | BattleSystem::AngelEcm
        | BattleSystem::Axe
        | BattleSystem::Mace
        | BattleSystem::DualSaw
        | BattleSystem::Claw => 1024,
        // An Artemis V controller weighs a ton and a half.
        BattleSystem::ArtemisIv => {
            if definition.has_technology(super::BattleTechnology::ArtemisV) {
                1536
            } else {
                1024
            }
        }
        // A Watchdog CEWS weighs a ton and a half across two ECM slots.
        BattleSystem::Ecm => {
            if definition.has_technology(super::BattleTechnology::Watchdog) {
                768
            } else if definition.has_special("Clan") {
                1024
            } else {
                768
            }
        }
        // Inner Sphere CASE II weighs a ton per slot; the Clan version weighs half as much.
        BattleSystem::CaseIi => {
            if definition.has_special("Clan") {
                512
            } else {
                1024
            }
        }
        BattleSystem::Sword => {
            u32::from(definition.tons.div_ceil(10)) * 512 / u32::from(definition.tons.div_ceil(15))
        }
        // Half a ton plus a twentieth of the Mech rounded up to the half ton, spread over one
        // slot plus one per twenty tons.
        BattleSystem::RetractableBlade => {
            let slots = u32::from(definition.tons.div_ceil(20)) + 1;
            (512 + half_ton((u32::from(definition.tons) * 1024).div_ceil(20))) / slots
        }
        BattleSystem::Lance => 1024,
        BattleSystem::Flail => 5 * 1024 / 4,
        BattleSystem::WreckingBall => 4 * 1024 / 5,
        BattleSystem::ChainWhip => 3 * 1024 / 2,
        BattleSystem::SmallVibroblade => 3 * 1024,
        BattleSystem::MediumVibroblade => 5 * 1024 / 2,
        BattleSystem::LargeVibroblade => 7 * 1024 / 4,
        BattleSystem::JumpJet => match definition.tons {
            0..=55 => 512,
            56..=85 => 1024,
            _ => 2048,
        },
        _ => 0,
    }
}

/// A one-shot launcher weighs half a ton more than the launcher it adapts; rocket launchers are
/// built as one-shot weapons and their catalogue mass already counts it.
pub(super) fn one_shot_mass<L>(mount: &super::WeaponMount<L>) -> u32 {
    if mount.one_shot && !mount.weapon.is_rocket() {
        512
    } else {
        0
    }
}

/// Power amplifiers a combustion-engined Mech needs for its surviving energy weapons: a tenth of
/// their mass, rounded up to the half ton.
fn power_amplifier_mass(
    weapons: &[super::WeaponMount],
    survives: impl Fn(BattleSection) -> bool,
) -> u32 {
    let energy: u32 = weapons
        .iter()
        .filter(|mount| {
            (mount.weapon.is_energy() && mount.weapon.profile().ammunition_per_ton == 0)
                || matches!(
                    mount.weapon,
                    super::BattleWeapon::PlasmaRifle
                        | super::BattleWeapon::LaserAms
                        | super::BattleWeapon::ClanLaserAms
                )
        })
        .filter(|mount| survives(mount.criticals[0].section))
        .map(|mount| mount.weapon.mass())
        .sum();
    half_ton(energy.div_ceil(10))
}

/// Armor mass for `protection` points of a type worth `denominator` fiftieths of a standard
/// point each: converted to tons without truncating, then rounded up to the next half ton.
pub(super) fn armor_mass(protection: u32, denominator: u32) -> u32 {
    half_ton((protection * 50 * 1024).div_ceil(denominator * 16))
}

/// Shared structure accounting preserves the surviving proportion before half-ton rounding.
/// Wide intermediate arithmetic supports large authored protection values without overflow.
pub(super) fn structure_mass(tons: u16, current: u32, original: u32, divisor: u32) -> Result<u32> {
    let mass = u64::from(tons) * 1024 * u64::from(current)
        / 5
        / u64::from(original.max(1))
        / u64::from(divisor);
    Ok(half_ton(u32::try_from(mass)?))
}

/// Half-ton rounding tolerates a single fixed-point unit above an exact boundary.
pub(super) fn half_ton(value: u32) -> u32 {
    let remainder = value % 512;
    if remainder <= 1 {
        value - remainder
    } else {
        value + 512 - remainder
    }
}

/// Standard fusion-engine catalog in half tons, for ratings 10 through 500 in steps of five.
pub(super) fn engine_mass(rating: u32) -> u32 {
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
        let mut definition = super::super::BattleTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml"),
        )
        .unwrap();
        assert_eq!(system_slot_mass(&definition, BattleSystem::CaseIi), 1024);
        assert_eq!(system_slot_mass(&definition, BattleSystem::Case), 512);
        let specials = definition.attributes.entry("specials".into()).or_default();
        specials.push_str(" Clan");
        assert_eq!(system_slot_mass(&definition, BattleSystem::CaseIi), 512);
        assert_eq!(
            BattleSystem::parse("CASE-II").unwrap(),
            BattleSystem::CaseIi
        );
        assert!(BattleSystem::CaseIi.is_noncritical());
    }
}
