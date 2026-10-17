//! Combat vehicle and VTOL mass (TechManual pp. 97-107) over intact construction or surviving
//! material: structure, engine, controls, lift equipment or rotors, turret, heat sinks, armor,
//! power amplifiers, weapons, equipment, cargo and ammunition.
use super::{
    System, SystemCritical, VehicleCriticalLocation, VehicleLoadout, VehicleMovement,
    VehiclePowerplant, VehicleSection, VehicleTemplate, Weapon,
    mass::{half_ton, targeting_computer_tons},
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
    /// Power amplifiers an ICE vehicle carries for its energy weapons (TechManual p. 106).
    pub power_amplifiers: u32,
    pub cooling: u32,
    /// The standard cargo bay `cargo_tons` describes plus every other transport bay
    /// (TechManual p. 239).
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
        // Control systems weigh 5 percent of the vehicle, rounded up to the half ton (TechManual
        // p. 102).
        let cockpit = if has_structure {
            half_ton((tons * 1024).div_ceil(20))
        } else {
            0
        };
        // Lift equipment and rotors weigh 10 percent of the vehicle, rounded up to the half ton
        // (TechManual p. 102).
        let components =
            if has_structure && (self.is_vtol() || self.movement == VehicleMovement::Hover) {
                half_ton((tons * 1024).div_ceil(10))
            } else {
                0
            };
        let turret = if self.sections.contains_key(&VehicleSection::Turret)
            && present(VehicleSection::Turret)
        {
            turret_mass(self.turret_item_mass(&loadout))
        } else {
            0
        };
        // Combat vehicles use standard structure only (TechManual p. 98); the construction
        // check refuses the other structure technologies.
        let structure_divisor = 2;
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
            if self.clan_armor() { 60 } else { 56 }
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
                .map(|part| self.system_mass(&loadout, part))
                .sum::<u32>();
        let combustion = engine.powerplant == VehiclePowerplant::Combustion;
        let power_amplifiers = if combustion {
            power_amplifier_mass(
                loadout
                    .weapons
                    .iter()
                    .filter(|mount| present(mount.criticals[0].section))
                    .map(|mount| mount.weapon),
            )
        } else {
            0
        };
        // Combat vehicles mount single heat sinks at a ton each; a fusion engine carries ten for
        // free (TechManual p. 104). Sinks take no item slot and no location.
        let sinks = u32::from(self.heat_sink_capacity());
        let cooling = sinks.saturating_sub(if combustion { 0 } else { 10 }) * 1024;
        let cargo =
            super::mass::cargo_mass(self.attributes.get(super::CARGO_TONS).map(String::as_str))?
                .checked_add(self.transport_bays()?.mass())
                .context("vehicle mass overflow")?;
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
            power_amplifiers,
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
            power_amplifiers,
            cooling,
            cargo,
            ammunition,
            ammunition_capacity,
            total,
            design_total,
        })
    }

    /// Mass of one installed vehicle system. Each entry is a whole installation in one item
    /// slot, whatever its Mech critical count. A targeting computer weighs the tonnage of the
    /// vehicle's direct-fire weapons divided by 4 for an Inner Sphere computer or 5 for a Clan
    /// one, rounded up to the full ton (TechManual p. 238).
    pub fn system_mass(
        &self,
        loadout: &VehicleLoadout,
        part: &SystemCritical<VehicleCriticalLocation>,
    ) -> u32 {
        let clan = part.is_clan(self.has_special("Clan"));
        if part.system != System::TargetingComputer {
            return self.fixed_system_mass(part.system, clan);
        }
        targeting_computer_tons(loadout.weapons.iter().map(|mount| mount.weapon), clan) * 1024
    }

    /// Mass of a vehicle system whose weight does not depend on the rest of the loadout, built
    /// from Clan technology when `clan` is set (see [`SystemCritical::is_clan`]): the Beagle
    /// and Clan active probes, the Guardian and Clan ECM suites, CASE II and the machine gun
    /// array each weigh by their own base (TechManual pp. 204-238). The targeting computer is
    /// sized by [`Self::system_mass`] and weighs nothing here.
    pub(crate) fn fixed_system_mass(&self, system: System, clan: bool) -> u32 {
        match system {
            System::Axe
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
                if clan {
                    1024
                } else {
                    1536
                }
            }
            System::ArtemisIv | System::ApolloFcs => 1024,
            System::ArtemisV => 1536,
            System::RetractableBlade => {
                512 + super::mass::half_ton((u32::from(self.tons) * 1024).div_ceil(20))
            }
            System::Lance => u32::from(self.tons.div_ceil(20)) * 1024,
            System::WreckingBall => 4096,
            System::ChainWhip | System::SmallVibroblade => 3072,
            System::Flail | System::MediumVibroblade => 5120,
            System::LargeVibroblade => 7168,
            System::Ecm => {
                if clan && !self.has_technology(super::Technology::Watchdog) {
                    1024
                } else {
                    1536
                }
            }
            // Clan CASE weighs nothing (TechManual p. 210).
            System::Case if clan => 0,
            System::Case | System::LightProbe | System::LightTag => 512,
            System::CaseIi => {
                if clan {
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
            | System::NullSignature
            | System::Talons
            | System::Spikes
            | System::TargetingComputer => 0,
            System::MachineGunArray => {
                if clan {
                    256
                } else {
                    512
                }
            }
        }
    }
}

impl VehicleTemplate {
    /// The transport bays beyond the standard cargo bay, construction data only (TechManual
    /// p. 239).
    pub fn transport_bays(&self) -> Result<super::TransportBays> {
        super::TransportBays::from_attribute(
            self.attributes
                .get(super::TRANSPORT_BAYS)
                .map(String::as_str),
        )
    }

    /// Whether the armor is Clan-made: the chassis technology base unless a mixed-technology
    /// flag names the armor's own.
    pub fn clan_armor(&self) -> bool {
        if self.has_special("ClanArmor_Tech") {
            return true;
        }
        if self.has_special("ISArmor_Tech") {
            return false;
        }
        self.has_special("Clan")
    }

    /// Whether the engine is Clan-made: the chassis technology base unless a mixed-technology
    /// flag names the engine's own.
    pub fn clan_engine(&self) -> bool {
        if self.has_special("ClanEngine_Tech") {
            return true;
        }
        if self.has_special("ISEngine_Tech") {
            return false;
        }
        self.has_special("Clan")
    }

    /// Mass of every item mounted in the turret except ammunition: the weapons, their one-shot
    /// adapters and the turret's systems (TechManual p. 106).
    pub fn turret_item_mass(&self, loadout: &VehicleLoadout) -> u32 {
        let weapons: u32 = loadout
            .weapons
            .iter()
            .filter(|mount| mount.criticals[0].section == VehicleSection::Turret)
            .map(|mount| mount.weapon.mass() + super::mass::one_shot_mass(mount))
            .sum();
        let systems: u32 = loadout
            .systems
            .iter()
            .filter(|part| part.location.section == VehicleSection::Turret)
            .map(|part| self.system_mass(loadout, part))
            .sum();
        weapons + systems
    }
}

/// Turret mechanisms weigh 10 percent of the items mounted in the turret, rounded up to the half
/// ton (TechManual pp. 98, 106).
pub fn turret_mass(items: u32) -> u32 {
    half_ton(items.div_ceil(10))
}

/// Whether an ICE vehicle needs power amplifiers for this weapon: the energy weapons that draw
/// no ammunition, plasma weapons and laser anti-missile systems (TechManual p. 106).
pub fn needs_power_amplifier(weapon: Weapon) -> bool {
    (weapon.is_energy() && weapon.profile().ammunition_per_ton == 0)
        || weapon.is_plasma()
        || matches!(weapon, Weapon::LaserAms | Weapon::ClanLaserAms)
}

/// Power amplifiers weigh 10 percent of the energy weapons they feed, rounded up to the half
/// ton (TechManual p. 106).
pub fn power_amplifier_mass(weapons: impl IntoIterator<Item = Weapon>) -> u32 {
    let energy: u32 = weapons
        .into_iter()
        .filter(|weapon| needs_power_amplifier(*weapon))
        .map(Weapon::mass)
        .sum();
    half_ton(energy.div_ceil(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Turret and amplifier masses round up to the half ton, never the quarter ton.
    #[test]
    fn turret_and_amplifier_masses_round_up_to_the_half_ton() {
        // The Zhukov's two AC/10s weigh 24 tons: a 2.5-ton turret (TechManual p. 107).
        assert_eq!(turret_mass(24 * 1024), 5 * 512);
        // Two SRM-6s weigh 6 tons: a 1-ton turret rather than 0.75.
        assert_eq!(turret_mass(6 * 1024), 1024);
        assert_eq!(turret_mass(0), 0);
        // A large laser needs half a ton of amplifiers (TechManual p. 107).
        assert_eq!(
            power_amplifier_mass([Weapon::parse("IS.LargeLaser").unwrap()]),
            512
        );
        assert_eq!(
            power_amplifier_mass([Weapon::parse("IS.AC/20").unwrap()]),
            0
        );
    }
}
