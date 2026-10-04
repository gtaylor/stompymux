//! Wizard construction-allocation reports reuse physical mass rules and share one chassis-neutral renderer.
use super::*;
use crate::{Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

/// One installed component or aggregated equipment family, in 1/1024-ton units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeightEntry {
    pub name: String,
    pub count: Option<u32>,
    pub mass: i64,
}

/// Construction allocation counts original protection and installed bins, including empty bins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeightReport {
    pub name: String,
    pub nominal_tons: u16,
    pub entries: Vec<BattleWeightEntry>,
    pub total: i64,
}

impl BattleWeightReport {
    /// Shared fixed-width cockpit layout; equipment names are literal text.
    pub fn render(&self) -> String {
        let line = "-".repeat(64);
        let mut lines = vec![
            line.clone(),
            format!("Weight totals for {}", crate::text::escape(&self.name)),
            line.clone(),
        ];
        for entry in &self.entries {
            let count = entry
                .count
                .map_or_else(String::new, |count| count.to_string());
            lines.push(format!(
                "{:<48} {:>5} {:>8.2}",
                crate::text::escape(&entry.name),
                count,
                entry.mass as f64 / 1024.0
            ));
        }
        lines.push(line.clone());
        let tons = self.total as f64 / 1024.0;
        let warning = if self.total / 1024 > i64::from(self.nominal_tons) {
            "[fg=red bold]"
        } else {
            ""
        };
        lines.push(format!(
            "[fg=green]Total: {warning}{tons:.1} tons (offset: {:.1})[reset]",
            f64::from(self.nominal_tons) - tons
        ));
        lines.push(line);
        lines.join("\r\n")
    }
}

/// Collect repeated installations in one deterministic row without coupling their mass rules.
fn add(rows: &mut BTreeMap<String, BattleWeightEntry>, name: &str, count: u32, mass: u32) {
    if mass == 0 {
        return;
    }
    let row = rows
        .entry(name.into())
        .or_insert_with(|| BattleWeightEntry {
            name: name.into(),
            count: Some(0),
            mass: 0,
        });
    *row.count.as_mut().unwrap() += count;
    row.mass += i64::from(mass);
}

/// Inspect the intact design without modifying live damage, supplies, dice or crew.
pub fn weight_report(world: &World, actor: ObjectId, id: ObjectId) -> Result<BattleWeightReport> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world.objects.get(&id).is_some_and(
            |object| object.kind == crate::Kind::Thing && !object.flags.contains(Flag::Going)
        ),
        "Unit is unavailable"
    );
    let mut entries = Vec::new();
    let mut equipment = BTreeMap::new();
    let mut component = |name: &str, mass: i64| {
        entries.push(BattleWeightEntry {
            name: name.into(),
            count: None,
            mass,
        })
    };
    let (name, nominal_tons) = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let definition = vehicle.definition();
        let mass = definition.mass()?;
        component("Engine", i64::from(mass.engine));
        component("Cockpit", i64::from(mass.cockpit));
        if mass.components > 0 {
            component("SpecialComponents", i64::from(mass.components));
        }
        if mass.turret > 0 {
            component("Turret", i64::from(mass.turret));
        }
        component("Internal Structure", i64::from(mass.structure));
        component("Armor", i64::from(mass.armor));
        component("Heat Sinks", i64::from(mass.cooling));
        if mass.cargo > 0 {
            component("CargoSpace", i64::from(mass.cargo));
        }
        let loadout = BattleVehicleLoadout::resolve(definition)?;
        for mount in &loadout.weapons {
            add(&mut equipment, mount.weapon.name(), 1, mount.weapon.mass());
        }
        for part in &loadout.systems {
            let name = &definition.sections[&part.location.section].criticals[&part.location.slot]
                .equipment;
            add(&mut equipment, name, 1, definition.system_mass(part.system));
        }
        for bin in &loadout.ammunition {
            add(
                &mut equipment,
                &format!("Ammo_{}", bin.weapon.name()),
                1,
                if bin.half_ton { 512 } else { 1024 },
            );
        }
        (definition.name.clone(), definition.tons)
    } else {
        let definition = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?
            .definition();
        let design = BattleUnit::from_template(definition.clone())?;
        let mass = design.mass()?;
        component("Engine", i64::from(mass.engine));
        component("Cockpit", i64::from(mass.cockpit));
        component("Gyro", i64::from(mass.gyro));
        component("Internal Structure", i64::from(mass.structure));
        component("Armor", i64::from(mass.armor));
        component("Heat Sinks", i64::from(design.cooling_mass()?));
        if mass.cargo > 0 {
            component("CargoSpace", i64::from(mass.cargo));
        }
        let loadout = design.loadout()?;
        for mount in &loadout.weapons {
            let mass = mount.weapon.mass() / u32::from(mount.weapon.profile().critical_slots)
                * mount.criticals.len() as u32;
            add(&mut equipment, mount.weapon.name(), 1, mass);
        }
        for part in &loadout.systems {
            let name = &definition.sections[&part.location.section].criticals[&part.location.slot]
                .equipment;
            add(
                &mut equipment,
                name,
                1,
                super::system_slot_mass(definition, part.system),
            );
        }
        for bin in &loadout.ammunition {
            add(
                &mut equipment,
                &format!("Ammo_{}", bin.weapon.name()),
                1,
                if bin.half_ton { 512 } else { 1024 },
            );
        }
        (definition.name.clone(), definition.tons)
    };
    entries.extend(equipment.into_values());
    let total = entries.iter().map(|entry| entry.mass).sum();
    Ok(BattleWeightReport {
        name,
        nominal_tons,
        entries,
        total,
    })
}

/// Publish a private wizard report atomically, preserving earlier output on a late capacity failure.
pub fn weight_action(scripts: &Scripts, actor: ObjectId, unit: ObjectId) -> Result<String> {
    let text = weight_report(&scripts.world(), actor, unit)?.render();
    scripts.atomic(|_| {
        for line in text.split("\r\n") {
            super::notify_message(scripts, BattleMessageTarget::Player(actor), line)?;
        }
        Ok(text)
    })
}

/// The reference ignores trailing arguments; the occupied physical unit supplies the design.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        weight_action(ctx.scripts, ctx.player, unit)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
