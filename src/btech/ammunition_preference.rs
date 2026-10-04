//! Preferred ammunition sections share admission and feed priority across unit anatomies.
use super::{Notice, SectionState, WeaponMount, WeaponReadiness};
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Preferred, mount-local and other sections retain stable slot order within each rank.
pub(super) fn priority<S: Eq>(preferred: Option<S>, mount: S, bin: S) -> u8 {
    if preferred.as_ref() == Some(&bin) {
        return 0;
    }
    if mount == bin {
        return 1;
    }
    2
}

/// Only weapons fed by ammunition bins carry a preferred feed section.
fn supports(weapon: super::Weapon) -> bool {
    weapon.profile().ammunition_per_ton > 0
}

/// Saved preferences may survive section damage but must refer to authored anatomy and ammo weapons.
pub(super) fn validate<S: Copy, L>(
    preferences: &BTreeMap<usize, S>,
    weapons: &[WeaponMount<L>],
    exists: impl Fn(S) -> bool,
) -> Result<()> {
    ensure!(
        preferences.iter().all(|(&index, &section)| exists(section)
            && weapons.get(index).is_some_and(|m| supports(m.weapon))),
        "Invalid preferred ammunition section"
    );
    Ok(())
}

impl super::Mech {
    /// Pilot preference is independent of bin availability and remains set after depletion.
    pub fn ammunition_section(&self, index: usize) -> Option<super::MechSection> {
        self.ammunition_sections.get(&index).copied()
    }
}
impl super::Vehicle {
    /// Pilot preference is independent of bin availability and remains set after depletion.
    pub fn ammunition_section(&self, index: usize) -> Option<super::VehicleSection> {
        self.ammunition_sections.get(&index).copied()
    }
}

/// Apply the same equipment and section admission without requiring ammunition or reactor power.
fn admit<S: Ord>(
    state: &WeaponReadiness,
    sections: &BTreeMap<S, SectionState>,
    section: Option<&S>,
) -> Result<()> {
    ensure!(state.intact, "That weapon has been destroyed");
    ensure!(
        state.recycle_remaining == 0,
        "That weapon is still recycling"
    );
    ensure!(supports(state.weapon), "Energy weapons do not use ammo");
    if let Some(section) = section {
        ensure!(
            sections.get(section).is_some_and(|s| s.internal > 0),
            "That section is absent or destroyed"
        );
    }
    Ok(())
}

/// Change one weapon's preference; chassis adapters supply anatomy and reuse all other rules.
pub fn set_battle_ammunition_section(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    section: Option<&str>,
) -> Result<Notice> {
    let (name, already) = if let Some(unit) = world.btech.vehicles().get(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
        ensure!(unit.position().is_some(), "Unit must be on a map");
        let section = section
            .map(super::VehicleSection::parse_location)
            .transpose()?;
        admit(
            &unit.weapon_readiness(index)?,
            unit.sections(),
            section.as_ref(),
        )?;
        let already = unit.ammunition_section(index) == section;
        let name = section.map(|s| s.name());
        set(
            &mut world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .ammunition_sections,
            index,
            section,
        );
        (name, already)
    } else {
        super::power::controlled_unit(world, id, pilot)?;
        let unit = &world.btech.constructed_units()[&id];
        ensure!(unit.position().is_some(), "Unit must be on a map");
        let section = section
            .map(|s| unit.chassis().parse_location(s))
            .transpose()?;
        admit(
            &unit.weapon_readiness(index)?,
            unit.sections(),
            section.as_ref(),
        )?;
        let already = unit.ammunition_section(index) == section;
        let name = section.map(|s| unit.chassis().section_name(s));
        set(
            &mut world
                .btech
                .constructed
                .get_mut(&id)
                .unwrap()
                .ammunition_sections,
            index,
            section,
        );
        (name, already)
    };
    Ok(Notice {
        unit: id,
        text: name.map_or_else(
            || format!("Preferred ammo source reset for weapon #{index}"),
            |name| {
                format!(
                    "Preferred ammo source {}set to {name} for weapon #{index}",
                    if already { "already " } else { "" }
                )
            },
        ),
    })
}

/// Keep cleared preferences absent from snapshots.
fn set<S>(preferences: &mut BTreeMap<usize, S>, index: usize, section: Option<S>) {
    if let Some(section) = section {
        preferences.insert(index, section);
    } else {
        preferences.remove(&index);
    }
}

/// Cockpit and Lua controls call the same domain operation.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|before| {
        let mut args = input.args.split_whitespace();
        let index = args
            .next()
            .context("Usage: usebin <weapon> <section|->")?
            .parse()
            .context("Invalid weapon number")?;
        let section = args.next().context("Supply an ammunition section")?;
        let id = before
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notice = set_battle_ammunition_section(
            &mut ctx.scripts.world.borrow_mut(),
            id,
            ctx.player,
            index,
            (!section.starts_with('-')).then_some(section),
        )?;
        super::notify_unit(ctx.scripts, notice)
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
