//! Construction mass of an intact BattleMech by TechManual rounding: every component weighed
//! as a whole item and rounded the way the book rounds it, so the total can be compared with
//! the Mech's tonnage (TM pp. 46-57). Live mass with damage is the server's concern.
use super::{
    Engine, Gyro, MechLoadout, MechTemplate, System, Technology, armor_mass, cargo_space_mass,
    engine_mass, half_ton, one_shot_mass, power_amplifier_mass, rated_output, structure_mass,
    system_mass,
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;

/// One weighed item or family of identical items, in 1/1024-ton units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MassItem {
    /// Template spelling of the weapon, system or ammunition bin.
    pub name: String,
    /// Installed items: weapons and bins count each one, systems count critical slots.
    pub count: u32,
    pub mass: u32,
}

/// Construction mass of an intact BattleMech, in 1/1024-ton units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MechMass {
    pub engine: u32,
    pub gyro: u32,
    pub cockpit: u32,
    pub structure: u32,
    pub armor: u32,
    /// Heat sinks beyond the engine's ten weight-free sinks.
    pub heat_sinks: u32,
    /// Authored cargo-space installation.
    pub cargo: u32,
    /// Weapons, systems, jump jets, ammunition, supercharger and power amplifiers by name.
    pub items: Vec<MassItem>,
    pub total: u32,
}

/// Armor technology a template declares, with its protection per ton in fiftieths of a
/// standard point (TM p. 56; Tactical Operations: Advanced Units & Equipment armor table).
pub fn armor_denominator(template: &MechTemplate) -> u32 {
    if template.has_special("FerroFibrous_Tech") {
        return if template.clan_armor() { 60 } else { 56 };
    }
    if template.has_special("HvyFerroFibrous_Tech") {
        return 62;
    }
    if template.has_special("LtFerroFibrous_Tech") {
        return 53;
    }
    if template.has_technology(Technology::HardenedArmor) {
        return 25;
    }
    50
}

/// Internal structure mass divisor a template declares: standard structure weighs 10% of
/// the Mech, endo steel and composite 5% and reinforced 20% (TM p. 47; Tactical Operations:
/// Advanced Units & Equipment structure table).
pub fn structure_divisor(template: &MechTemplate) -> u32 {
    if template.has_technology(Technology::ReinforcedStructure) {
        return 1;
    }
    if template.has_special("EndoSteel_Tech")
        || template.has_technology(Technology::CompositeStructure)
    {
        return 4;
    }
    2
}

/// Heat sinks a template declares, counted in sinks rather than dissipation: a double sink
/// dissipates two points of heat (TM p. 53).
pub fn heat_sink_count(template: &MechTemplate) -> u16 {
    if template.has_double_heat_sinks() {
        template.heat_sinks / 2
    } else {
        template.heat_sinks
    }
}

impl MechTemplate {
    /// Construction mass of the intact Mech with `loadout`, by book rounding: the engine,
    /// gyro, structure, armor and supercharger rounded up to the half ton, every weapon and
    /// system at its whole catalogue mass, and heat sinks beyond the engine's free ten at a ton
    /// each (TM pp. 47-57).
    pub fn construction_mass(&self, loadout: &MechLoadout) -> Result<MechMass> {
        let rating = rated_output(self.tons, self.max_speed)?;
        let ice = self.has_special("ICEEngine_Tech");
        let engine = if ice {
            // Internal combustion engines weigh twice a standard fusion engine.
            half_ton(engine_mass(rating) * 2)
        } else {
            let family = Engine::resolve(loadout, self.clan_engine())?;
            half_ton(family.unrounded_mass(engine_mass(rating)))
        };
        let gyro = Gyro::from_definition(self).mass(rating.div_ceil(100) as i32 * 1024) as u32;
        let cockpit = if self.has_technology(Technology::SmallCockpit) {
            2 * 1024
        } else {
            3 * 1024
        };
        let structure = structure_mass(self.tons, 1, 1, structure_divisor(self))?;
        let protection: u32 = self
            .sections
            .values()
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        let armor = armor_mass(protection, armor_denominator(self));
        // Fusion engines carry ten heat sinks for free; combustion engines carry none.
        let free = if ice { 0 } else { 10 };
        let heat_sinks = u32::from(heat_sink_count(self)).saturating_sub(free) * 1024;
        let cargo = cargo_space_mass(
            self.attributes.get("cargo_space").map(String::as_str),
            self.has_special("Carrier_Tech"),
            self.has_special("CargoTech"),
        )?;

        let mut items: BTreeMap<String, MassItem> = BTreeMap::new();
        let mut add = |name: &str, count: u32, mass: u32| {
            if mass == 0 {
                return;
            }
            let item = items.entry(name.to_owned()).or_insert_with(|| MassItem {
                name: name.to_owned(),
                count: 0,
                mass: 0,
            });
            item.count = item.count.saturating_add(count);
            item.mass = item.mass.saturating_add(mass);
        };
        for mount in &loadout.weapons {
            add(
                mount.weapon.name(),
                1,
                mount.weapon.mass() + one_shot_mass(mount),
            );
        }
        let mut systems: BTreeMap<(System, String), usize> = BTreeMap::new();
        for part in &loadout.systems {
            if part.system == System::HeatSink {
                continue;
            }
            let name =
                &self.sections[&part.location.section].criticals[&part.location.slot].equipment;
            *systems.entry((part.system, name.clone())).or_default() += 1;
        }
        for ((system, name), slots) in systems {
            add(&name, slots as u32, system_mass(self, system, slots));
        }
        for bin in &loadout.ammunition {
            add(
                &format!("Ammo_{}", bin.weapon.name()),
                1,
                if bin.half_ton { 512 } else { 1024 },
            );
        }
        if loadout
            .systems
            .iter()
            .any(|part| part.system == System::Supercharger)
        {
            // A supercharger weighs a tenth of the engine, rounded up to the half ton
            // (Tactical Operations: Advanced Units & Equipment p. 223).
            add("SuperCharger", 0, half_ton(engine.div_ceil(10)));
        }
        if ice {
            add(
                "PowerAmplifiers",
                0,
                power_amplifier_mass(&loadout.weapons, |_| true),
            );
        }
        let items: Vec<MassItem> = items.into_values().collect();
        let total = [engine, gyro, cockpit, structure, armor, heat_sinks, cargo]
            .into_iter()
            .chain(items.iter().map(|item| item.mass))
            .try_fold(0u32, u32::checked_add)
            .context("Mech mass overflow")?;
        Ok(MechMass {
            engine,
            gyro,
            cockpit,
            structure,
            armor,
            heat_sinks,
            cargo,
            items,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Jenner JR7-D weighs exactly its 35 tons by book rounding (TM record sheet).
    #[test]
    fn jenner_weighs_its_tonnage() {
        let template =
            MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml")).unwrap();
        let loadout = MechLoadout::resolve(&template).unwrap();
        let mass = template.construction_mass(&loadout).unwrap();
        // 245 standard fusion engine: 12 tons; gyro 3; cockpit 3; structure 3.5; 64 points
        // of standard armor: 4 tons.
        assert_eq!(
            (
                mass.engine,
                mass.gyro,
                mass.cockpit,
                mass.structure,
                mass.armor
            ),
            (12 * 1024, 3 * 1024, 3 * 1024, 3584, 4 * 1024)
        );
        assert_eq!(mass.heat_sinks, 0);
        assert_eq!(mass.total, 35 * 1024);
    }
}
