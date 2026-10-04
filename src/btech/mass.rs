//! Current conventional biped mass derived from durable construction and damage facts.
use super::{
    Engine, Mech, MechSection, System, Technology, armor_mass, cargo_space_mass, engine_mass,
    half_ton, one_shot_mass, power_amplifier_mass, rated_output, structure_mass, system_slot_mass,
};
use anyhow::{Context, Result};
use serde::Serialize;

/// Mass components in 1/1024-ton units. Gyro accounting is signed for a destroyed center torso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Mass {
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

impl Mech {
    /// Recalculate physical mass without caching derived values or substituting nominal tonnage.
    /// Damaged weapon criticals retain mass until their section is lost; ammunition uses live rounds.
    pub fn mass(&self) -> Result<Mass> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let survives = |section| self.sections()[&section].internal > 0;
        let rating = rated_output(definition.tons, definition.max_speed)?;
        // C derives the engine mass family from technology flags alone; reference
        // builds without engine criticals still carry a nominal engine mass.
        let engine_family = || {
            Engine::resolve(&loadout, definition.clan_engine()).unwrap_or_else(|_| {
                Engine::display_from_flags(
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
        let engine = if survives(MechSection::CenterTorso) {
            installed_engine
        } else {
            0
        };
        let gyro = if survives(MechSection::CenterTorso) {
            rating.div_ceil(100) as i32 * 1024
        } else {
            -1024
        };
        let gyro = self.gyro().mass(gyro);
        let cockpit = if survives(MechSection::Head) {
            if definition.has_technology(Technology::SmallCockpit) {
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
        let structure_divisor = if definition.has_technology(Technology::ReinforcedStructure) {
            1
        } else if material_count(System::EndoSteel) >= material_slots(definition.clan_structure())
            || definition.has_technology(Technology::CompositeStructure)
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
        let armor_denominator =
            if material_count(System::FerroFibrous) >= material_slots(definition.clan_armor()) {
                if definition.clan_armor() { 60 } else { 56 }
            } else if material_count(System::HeavyFerroFibrous) >= 21 {
                62
            } else if material_count(System::LightFerroFibrous) >= 7 {
                53
            } else if definition.has_technology(Technology::HardenedArmor) {
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
        if loadout
            .systems
            .iter()
            .any(|part| part.system == System::Supercharger && survives(part.location.section))
        {
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
        let cargo = cargo_space_mass(
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
        Ok(Mass {
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
            .saturating_sub(u16::from(self.system_hits(System::HeatSink)));
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
                .filter(|slot| slot.system == System::HeatSink && survives(slot.location.section))
                .count() as u32
                * slot_mass
        })
    }
}
