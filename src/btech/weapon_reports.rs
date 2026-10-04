//! Weapon specifications and damage diagnostics share catalogue facts and live equipment state.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

/// Physical/component condition, independent of ammunition supply, power and recycling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleEquipmentCondition {
    Empty,
    Operational,
    Damaged,
    Disabled,
    Broken,
    Destroyed,
    Jammed,
    Shorted,
    AmmoJam,
}

impl BattleEquipmentCondition {
    /// Stable cockpit label shared by critical and whole-weapon reports.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::Operational => "Operational",
            Self::Damaged => "Damaged",
            Self::Disabled => "Disabled",
            Self::Broken => "Broken",
            Self::Destroyed => "Destroyed",
            Self::Jammed => "Jammed",
            Self::Shorted => "Shorted",
            Self::AmmoJam => "Ammojam",
        }
    }
}

/// One installed weapon's durable condition and the effects already used by firing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeaponDiagnostic {
    pub index: usize,
    pub weapon: BattleWeapon,
    pub section: String,
    pub condition: BattleEquipmentCondition,
    pub damaged_slots: u8,
    pub destroyed_slots: u8,
    pub disabled_slots: u8,
    pub effects: BattleWeaponDamageEffects,
    pub preferred_ammunition_section: Option<String>,
}

/// Return stable mount numbers even for broken weapons; readiness is a separate report.
pub fn weapon_diagnostics(world: &World, id: ObjectId) -> Result<Vec<BattleWeaponDiagnostic>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit
            .loadout()?
            .weapons
            .iter()
            .enumerate()
            .map(|(index, mount)| {
                let destroyed = mount
                    .criticals
                    .iter()
                    .filter(|slot| unit.critical_destroyed(**slot))
                    .count() as u8;
                let disabled = mount
                    .criticals
                    .iter()
                    .filter(|slot| {
                        !unit.critical_destroyed(**slot) && unit.critical_unavailable(**slot)
                    })
                    .count() as u8;
                let condition = if destroyed > 0 {
                    BattleEquipmentCondition::Destroyed
                } else if disabled > 0 || unit.powered_down_weapons.contains(&index) {
                    BattleEquipmentCondition::Disabled
                } else if let Some(failure) = unit.weapon_failures().get(&index) {
                    failure.condition()
                } else if unit.jammed_weapons.contains(&index) {
                    BattleEquipmentCondition::AmmoJam
                } else {
                    BattleEquipmentCondition::Operational
                };
                Ok(BattleWeaponDiagnostic {
                    index,
                    weapon: mount.weapon,
                    section: mount.criticals[0].section.name().into(),
                    condition,
                    damaged_slots: 0,
                    destroyed_slots: destroyed,
                    disabled_slots: disabled,
                    effects: Default::default(),
                    preferred_ammunition_section: unit
                        .ammunition_section(index)
                        .map(|section| section.name().into()),
                })
            })
            .collect();
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.loadout()?
        .weapons
        .iter()
        .enumerate()
        .map(|(index, mount)| {
            let destroyed = mount
                .criticals
                .iter()
                .filter(|slot| unit.critical_destroyed(**slot))
                .count() as u8;
            let disabled = mount
                .criticals
                .iter()
                .filter(|slot| {
                    !unit.critical_destroyed(**slot) && unit.critical_unavailable(**slot)
                })
                .count() as u8;
            let damaged = unit
                .weapon_damage()
                .iter()
                .filter(|damage| mount.criticals.contains(&damage.location))
                .count() as u8;
            let condition = if destroyed > 0 {
                BattleEquipmentCondition::Destroyed
            } else if disabled > 0 || unit.powered_down_weapons.contains(&index) {
                BattleEquipmentCondition::Disabled
            } else if let Some(failure) = unit.weapon_failures.get(&index) {
                failure.condition()
            } else if unit.jammed_weapons.contains(&index)
                || unit.weapon_damage_jams.contains(&index)
            {
                BattleEquipmentCondition::AmmoJam
            } else if damaged > 0 {
                BattleEquipmentCondition::Damaged
            } else {
                BattleEquipmentCondition::Operational
            };
            Ok(BattleWeaponDiagnostic {
                index,
                weapon: mount.weapon,
                section: unit
                    .chassis()
                    .section_name(mount.criticals[0].section)
                    .into(),
                condition,
                damaged_slots: damaged,
                destroyed_slots: destroyed,
                disabled_slots: disabled,
                effects: unit.weapon_damage_effects(index)?,
                preferred_ammunition_section: unit
                    .ammunition_section(index)
                    .map(|section| unit.chassis().section_name(section).into()),
            })
        })
        .collect()
}

/// Immutable statistics for one distinct installed catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWeaponSpecification {
    pub weapon: BattleWeapon,
    /// Ammunition family for this profile; MMLs expose both SRM and LRM rows.
    pub ammunition: super::BattleAmmunitionMode,
    pub heat: u8,
    pub damage: u8,
    pub minimum_range: u8,
    pub short_range: u8,
    pub medium_range: u8,
    pub long_range: u16,
    pub extended_range: Option<u16>,
    pub recycle_seconds: u8,
}

/// Distinct weapon types retain first-installation order, including destroyed installations.
pub fn weapon_specifications(
    world: &World,
    id: ObjectId,
    extended: bool,
) -> Result<Vec<BattleWeaponSpecification>> {
    let mut seen = BTreeSet::new();
    let weapons: Vec<_> = crate::btech::with_unit!(
        world
            .btech
            .unit(id)
            .context("Unit construction state is unavailable")?,
        |unit| {
            unit.loadout()?
                .weapons
                .iter()
                .map(|mount| mount.weapon)
                .collect()
        }
    );
    Ok(weapons
        .into_iter()
        .filter(|weapon| seen.insert(weapon.name()))
        .flat_map(|weapon| {
            let modes: &[super::BattleAmmunitionMode] = if weapon.is_mml() {
                &[
                    super::BattleAmmunitionMode::Normal,
                    super::BattleAmmunitionMode::MmlLrm,
                ]
            } else {
                &[super::BattleAmmunitionMode::Normal]
            };
            modes
                .iter()
                .copied()
                .map(move |ammunition| (weapon, ammunition))
        })
        .map(|(weapon, ammunition)| {
            let profile = weapon.profile_for_ammunition(ammunition);
            BattleWeaponSpecification {
                weapon,
                ammunition,
                heat: profile.heat,
                damage: profile.damage,
                minimum_range: profile.minimum_range,
                short_range: profile.short_range,
                medium_range: profile.medium_range,
                long_range: weapon.effective_range_for_ammunition(false, ammunition),
                extended_range: extended
                    .then(|| weapon.effective_range_for_ammunition(true, ammunition)),
                recycle_seconds: world.btech.weapon_settings.recycle_seconds(weapon),
            }
        })
        .collect())
}

/// Render damage without treating shutdown, recycle or an empty bin as equipment destruction.
pub fn weapon_diagnostic_text(world: &World, id: ObjectId) -> Result<String> {
    let mut lines = vec![
        "WEAPON SYSTEMS STATUS".into(),
        "[##] Weapon | Location | Status".into(),
    ];
    for row in weapon_diagnostics(world, id)? {
        lines.push(format!(
            "[{:2}] {} | {} | {}",
            row.index,
            row.weapon.name(),
            row.section,
            row.condition.label().to_ascii_uppercase()
        ));
        let effect = row.effects;
        if effect.moderate > 0 {
            lines.push(format!("  General damage: +{} to hit.", effect.moderate));
        }
        if effect.damage > 0 {
            lines.push(format!(
                "  Focus misalignment: -{} damage; +{} to hit beyond {} hexes.",
                effect.damage,
                effect.ranging,
                row.weapon.profile().short_range
            ));
        } else if effect.ranging > 0 {
            lines.push(format!(
                "  Ranging system damage: +{} to hit beyond {} hexes.",
                effect.ranging,
                row.weapon.profile().short_range
            ));
        }
        if effect.heat > 0 {
            lines.push(format!(
                "  Charging crystal damage: +{} heat; explodes on {} or less.",
                effect.heat,
                effect.explosion + 1
            ));
        }
        if effect.feed_locked {
            lines.push(format!(
                "  Ammo feed damage: can't switch ammo; explodes on {} or less.",
                effect.explosion + 1
            ));
        }
        if effect.jam > 0 {
            lines.push(format!(
                "  Barrel damage: jams on {} or less.",
                effect.jam + 1
            ));
        }
        if row.damaged_slots > 0 && effect == BattleWeaponDamageEffects::default() {
            lines.push("  Damaged, but fully operational.".into());
        }
        if let Some(section) = row.preferred_ammunition_section {
            lines.push(format!("  Preferred ammo source: {section}"));
        }
        if row.damaged_slots + row.destroyed_slots + row.disabled_slots > 0 {
            lines.push(format!(
                "  Slot status: Damaged: {}. Destroyed: {}. Disabled: {}",
                row.damaged_slots, row.destroyed_slots, row.disabled_slots
            ));
        }
    }
    Ok(lines.join("\r\n"))
}

/// Render one catalogue row per installed type with the configured extended-range column.
pub fn weapon_specification_text(world: &World, id: ObjectId, extended: bool) -> Result<String> {
    let rows = weapon_specifications(world, id, extended)?;
    if rows.is_empty() {
        return Ok("You have no weapons!".into());
    }
    let (name, reference) = if let Some(unit) = world.btech.vehicles().get(&id) {
        (&unit.definition().name, &unit.definition().reference)
    } else {
        let definition = world.btech.constructed_units()[&id].definition();
        (&definition.name, &definition.reference)
    };
    let title = if name == reference {
        name.to_owned()
    } else {
        format!("{name}: {reference}")
    };
    let header = if extended {
        "[fg=green]Weapon Name             Heat  Damage  Range: Min Short Med Long Ext VRT[reset]"
    } else {
        "[fg=green]Weapon Name             Heat  Damage  Range: Min  Short  Med  Long  VRT[reset]"
    };
    let rows = rows.into_iter().map(|row| {
        let family = if !row.weapon.is_mml() {
            ""
        } else if row.ammunition.is_mml_lrm() {
            " (LRM)"
        } else {
            " (SRM)"
        };
        let name = format!("{}{family}", row.weapon.name());
        let line = if let Some(extreme) = row.extended_range {
            format!(
                "{name:<24} {:>2}     {:>2}           {:>2}  {:>2}    {:>2}  {:>3}  {:>3} {:>2}",
                row.heat,
                row.damage,
                row.minimum_range,
                row.short_range,
                row.medium_range,
                row.long_range,
                extreme,
                row.recycle_seconds
            )
        } else {
            format!(
                "{name:<24} {:>2}     {:>2}           {:>2}    {:>2}     {:>2}  {:>3}   {:>2}",
                row.heat,
                row.damage,
                row.minimum_range,
                row.short_range,
                row.medium_range,
                row.long_range,
                row.recycle_seconds
            )
        };
        crate::text::escape(&line)
    });
    Ok(super::menu::information(
        &crate::text::escape(&format!("Weapons statistics for {title}")),
        header,
        rows,
    ))
}

/// Resolve report equipment through cockpit admission without granting pilot controls.
pub(super) fn cockpit(
    ctx: &crate::CommandContext<'_>,
    conscious: bool,
    require_pilot: bool,
) -> Result<ObjectId> {
    let world = ctx.scripts.world.borrow();
    let id = world
        .objects
        .get(&ctx.player)
        .and_then(|player| player.location)
        .context("Enter a unit first")?;
    if conscious {
        ensure!(!world.btech.unconscious(ctx.player), "You are unconscious");
    }
    ensure!(
        super::scanner::scanner_unit(&world, id).is_some(),
        "Unit construction state is unavailable"
    );
    if require_pilot {
        super::power::controlled(&world, id, ctx.player)?;
    }
    Ok(id)
}

/// Native weapon damage diagnostics accept passengers and shutdown units, preserving report semantics.
pub(crate) fn status_command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    report((|| {
        let id = cockpit(ctx, true, false)?;
        let world = ctx.scripts.world.borrow();
        ensure!(
            super::scanner::scanner_unit(&world, id)
                .and_then(|unit| unit.position)
                .is_some(),
            "Unit is not on a map"
        );
        weapon_diagnostic_text(&world, id)
    })())
}

/// Native specifications ignore trailing arguments, as does the catalogue report.
pub(crate) fn specs_command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = cockpit(ctx, false, false)?;
        weapon_specification_text(
            &ctx.scripts.world.borrow(),
            id,
            ctx.config.battletech.erange != 0,
        )
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Styled(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Keep read-only command failures separate from literal report output.
pub(super) fn report(result: Result<String>) -> Result<crate::CommandAction> {
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Literal(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}
