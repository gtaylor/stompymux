//! Shared vehicle and rotorcraft mass over intact construction or surviving material.
use super::{
    System, VehicleLoadout, VehicleMovement, VehiclePowerplant, VehicleSection, VehicleTemplate,
    mass::half_ton,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Component masses in 1/1024-ton units. Neither total certifies a legal construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VehicleMass {
    pub engine: u32,
    pub cockpit: u32,
    pub components: u32,
    pub turret: u32,
    pub structure: u32,
    pub armor: u32,
    /// Weapons and systems, excluding separately reported cooling and ammunition.
    pub equipment: u32,
    pub cooling: u32,
    pub cargo: u32,
    /// Physical mass of listed or currently loaded rounds in surviving sections.
    pub ammunition: u32,
    /// Surviving installed bin mass, including empty bins; half-ton flags determine bin size.
    pub ammunition_capacity: u32,
    /// Physical mass using the supplied ammunition inventory.
    pub total: u32,
    /// Component total counting full surviving bins instead of loaded rounds.
    pub design_total: u32,
}

/// Surviving material a vehicle mass calculation reads. A template reports its intact
/// construction; a live vehicle reports current structure, protection and loaded rounds.
pub trait VehicleMaterial {
    /// Internal structure points remaining in a hull face, turret or rotor.
    fn internal(&self, section: VehicleSection) -> u16;

    /// Front and rear armor points remaining in a section.
    fn protection(&self, section: VehicleSection) -> u32;

    /// Rounds loaded in the ammunition bin at `bin`, in resolved loadout order. `listed` is
    /// the bin's capacity as the template records it.
    fn rounds(&self, bin: usize, listed: u16) -> u16;
}

impl VehicleMaterial for VehicleTemplate {
    fn internal(&self, section: VehicleSection) -> u16 {
        self.sections[&section].internal
    }

    fn protection(&self, section: VehicleSection) -> u32 {
        let layout = &self.sections[&section];
        u32::from(layout.armor) + u32::from(layout.rear)
    }

    fn rounds(&self, _bin: usize, listed: u16) -> u16 {
        listed
    }
}

impl VehicleTemplate {
    /// Resolve equipment and calculate intact mass, retaining zero engine mass for uncatalogued ratings.
    /// The engine query exposes missing catalogue entries separately from material accounting.
    pub fn mass(&self) -> Result<VehicleMass> {
        self.mass_of(self)
    }

    /// Share construction arithmetic while reading intact or live material through `material`.
    /// Broken equipment retains mass until its section is lost; crew loss does not remove material.
    pub fn mass_of(&self, material: &impl VehicleMaterial) -> Result<VehicleMass> {
        let engine = self.engine()?;
        for section in [
            VehicleSection::Left,
            VehicleSection::Right,
            VehicleSection::Front,
            VehicleSection::Rear,
        ] {
            ensure!(
                self.sections
                    .get(&section)
                    .is_some_and(|layout| layout.internal > 0),
                "Vehicle hull face {} requires positive internals",
                section.name()
            );
        }
        let loadout = VehicleLoadout::resolve(self)?;
        let tons = u32::from(self.tons);
        let internal = |section| material.internal(section);
        let present = |section| internal(section) > 0;
        let has_structure = self.sections.keys().any(|&section| present(section));
        let engine_mass = if has_structure {
            engine.installed_mass
        } else {
            0
        };
        let cockpit = if has_structure {
            quarter_ton(tons * 1024 / 20)
        } else {
            0
        };
        let components =
            if has_structure && (self.is_vtol() || self.movement == VehicleMovement::Hover) {
                half_ton(tons * 1024 / 10)
            } else {
                0
            };
        let turret_weapons: u32 = loadout
            .weapons
            .iter()
            .filter(|mount| {
                mount.criticals[0].section == VehicleSection::Turret
                    && present(VehicleSection::Turret)
            })
            .map(|mount| mount.weapon.mass())
            .sum();
        let turret = quarter_ton(turret_weapons / 10);
        let structure_divisor = if self.has_technology(super::Technology::ReinforcedStructure) {
            1
        } else if self.has_special("EndoSteel_Tech")
            || self.has_technology(super::Technology::CompositeStructure)
        {
            4
        } else {
            2
        };
        let original: u32 = self
            .sections
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        let current: u32 = self
            .sections
            .keys()
            .map(|&section| u32::from(internal(section)))
            .sum();
        let structure =
            super::mass::structure_mass(self.tons, current, original, structure_divisor)?;
        let protection: u32 = self
            .sections
            .iter()
            .filter(|(_, section)| section.internal > 0)
            .map(|(&section, _)| material.protection(section))
            .sum();
        // Fiftieths of a standard armor point that one point of this armor is worth.
        let denominator = if self.has_special("FerroFibrous_Tech") {
            if self.has_special("Clan") { 60 } else { 56 }
        } else if self.has_special("HvyFerroFibrous_Tech") {
            62
        } else if self.has_special("LtFerroFibrous_Tech") {
            53
        } else if self.has_technology(super::Technology::HardenedArmor) {
            25
        } else {
            50
        };
        let armor = super::mass::armor_mass(protection, denominator);
        let equipment = loadout
            .weapons
            .iter()
            .filter(|mount| present(mount.criticals[0].section))
            .map(|mount| mount.weapon.mass() + super::mass::one_shot_mass(mount))
            .sum::<u32>()
            + loadout
                .systems
                .iter()
                .filter(|part| present(part.location.section))
                .map(|part| self.system_mass(part.system))
                .sum::<u32>();
        let combustion = engine.powerplant == VehiclePowerplant::Combustion;
        let sinks = u32::from(self.heat_sink_capacity());
        let efficiency = if self.has_special("Clan") || self.has_special("DoubleHS") {
            2
        } else {
            1
        };
        // Vehicle sinks occupy one slot; installed sink criticals do not add a second cooling charge.
        let cooling = (sinks / efficiency).saturating_sub(if combustion { 0 } else { 10 }) * 1024;
        let cargo = super::mass::cargo_space_mass(
            self.attributes.get("cargo_space").map(String::as_str),
            self.has_special("Carrier_Tech"),
            self.has_special("CargoTech"),
        )?;
        let ammunition = loadout
            .ammunition
            .iter()
            .enumerate()
            .filter(|(_, bin)| present(bin.location.section))
            .map(|(index, bin)| {
                let rounds = material.rounds(index, bin.rounds);
                u32::from(rounds) * 1024
                    / u32::from(
                        bin.weapon
                            .profile_for_ammunition(bin.mode)
                            .ammunition_per_ton,
                    )
            })
            .sum();
        let ammunition_capacity = loadout
            .ammunition
            .iter()
            .filter(|bin| present(bin.location.section))
            .map(|bin| if bin.half_ton { 512 } else { 1024 })
            .sum();
        let base = [
            engine_mass,
            cockpit,
            components,
            turret,
            structure,
            armor,
            equipment,
            cooling,
            cargo,
        ]
        .into_iter()
        .try_fold(0u32, |total, mass| total.checked_add(mass))
        .context("vehicle mass overflow")?;
        let total = base
            .checked_add(ammunition)
            .context("vehicle mass overflow")?
            .max(1);
        let design_total = base
            .checked_add(ammunition_capacity)
            .context("vehicle mass overflow")?
            .max(1);
        Ok(VehicleMass {
            engine: engine_mass,
            cockpit,
            components,
            turret,
            structure,
            armor,
            equipment,
            cooling,
            cargo,
            ammunition,
            ammunition_capacity,
            total,
            design_total,
        })
    }

    /// Whole vehicle systems have their own mass, independent of Mech multi-critical installation sizes.
    pub fn system_mass(&self, system: System) -> u32 {
        match system {
            System::TargetingComputer
            | System::Axe
            | System::Claw
            | System::Mace
            | System::DualSaw
            | System::Masc
            | System::C3Slave
            | System::Tag => 1024,
            System::C3i => 2560,
            System::AngelEcm | System::BloodhoundProbe => 2048,
            System::C3Master => 5120,
            System::Sword => {
                u32::from(self.tons.div_ceil(10)) * 512 / u32::from(self.tons.div_ceil(15))
            }
            // A Clan active probe weighs a ton; the Beagle a ton and a half.
            System::BeagleProbe => {
                if self.has_special("Clan") {
                    1024
                } else {
                    1536
                }
            }
            System::ArtemisIv => {
                if self.has_technology(super::Technology::ArtemisV) {
                    1536
                } else {
                    1024
                }
            }
            System::RetractableBlade => {
                512 + super::mass::half_ton((u32::from(self.tons) * 1024).div_ceil(20))
            }
            System::Lance => u32::from(self.tons.div_ceil(20)) * 1024,
            System::WreckingBall => 4096,
            System::ChainWhip | System::SmallVibroblade => 3072,
            System::Flail | System::MediumVibroblade => 5120,
            System::LargeVibroblade => 7168,
            System::Ecm => {
                if self.has_special("Clan") && !self.has_technology(super::Technology::Watchdog) {
                    1024
                } else {
                    1536
                }
            }
            System::Case | System::LightProbe => 512,
            System::CaseIi => {
                if self.has_special("Clan") {
                    512
                } else {
                    1024
                }
            }
            System::JumpJet => match self.tons {
                0..=55 => 512,
                56..=85 => 1024,
                _ => 2048,
            },
            System::ShoulderOrHip
            | System::UpperActuator
            | System::LowerActuator
            | System::HandOrFootActuator
            | System::Engine
            | System::Gyro
            | System::Cockpit
            | System::LifeSupport
            | System::Sensors
            | System::HeatSink
            | System::FuelTank
            | System::FerroFibrous
            | System::EndoSteel
            | System::TripleStrengthMyomer
            | System::Supercharger
            | System::HeavyFerroFibrous
            | System::LightFerroFibrous
            | System::StealthArmor
            | System::LaserReflective
            | System::NullSignature => 0,
        }
    }
}

/// Round controls and turret components upward to quarter tons, tolerating one fixed-point unit.
fn quarter_ton(value: u32) -> u32 {
    let remainder = value % 256;
    if remainder <= 1 {
        value - remainder
    } else {
        value + 256 - remainder
    }
}
