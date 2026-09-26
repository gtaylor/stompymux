//! Unit-owned contact display choices, independent of player category preferences.
use crate::{Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const CONTACTS: [&str; 4] = [
    "0 - Very verbose",
    "1 - Short form, the usual one",
    "2 - Short form, the usual one, but do not see buildings",
    "3 - Shorter form",
];
const AUTOMATIC: [&str; 7] = [
    "0 - See enemies and friends, long text, color",
    "1 - See enemies and friends, short text, color",
    "2 - See enemies only, long text, color",
    "3 - See enemies only, short text, color",
    "4 - See enemies and friends, short text, no color",
    "5 - See enemies only, short text, no color",
    "6 - Disabled",
];

/// Independently editable display modes, stored with the unit rather than its pilot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleBriefSettings {
    pub contacts: u8,
    pub automatic: u8,
}

impl Default for BattleBriefSettings {
    fn default() -> Self {
        Self {
            contacts: 1,
            automatic: 0,
        }
    }
}

impl BattleBriefSettings {
    /// Frame rendered rows; shortest mode deliberately emits no header, footer or empty-list text.
    pub fn frame_contacts(self, rows: &[String]) -> String {
        if self.contacts == 3 {
            return rows.join("\r\n");
        }
        let mut output = Vec::with_capacity(rows.len() + 2);
        output.push("Line of Sight Contacts:");
        output.extend(rows.iter().map(String::as_str));
        output.push("End Contact List");
        output.join("\r\n")
    }

    /// Reject unsupported modes before mutation or persistence.
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.contacts <= 3 && self.automatic <= 6,
            "Number out of range!"
        );
        Ok(())
    }

    /// Only standard short contact mode includes structures implicitly.
    pub fn includes_buildings(self) -> bool {
        self.contacts == 1
    }

    /// Routine notices may be disabled or restricted to hostile units.
    pub fn announces(self, friendly: bool) -> bool {
        self.automatic != 6 && !(friendly && matches!(self.automatic, 2 | 3 | 5))
    }
}

impl super::BattleUnit {
    /// The unit's durable display choices.
    pub fn brief_settings(&self) -> BattleBriefSettings {
        self.brief
    }
}

impl super::BattleVehicle {
    /// Saved cockpit display and automatic-contact notification choices.
    pub fn brief_settings(&self) -> BattleBriefSettings {
        self.brief
    }
}

/// Shared native/Lua result; query does not notify the cockpit or change state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleBriefReport {
    pub settings: BattleBriefSettings,
    pub changed: bool,
    pub text: String,
}

/// Query or edit one mode and publish an edit to occupants atomically.
/// A conscious cockpit occupant may use this even when engines are stopped.
pub fn brief(
    scripts: &Scripts,
    unit: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<BattleBriefReport> {
    let before = scripts.world.borrow().clone();
    let effects = scripts.effects.checkpoint();
    let result = (|| {
        display_access(&scripts.world.borrow(), unit, pilot, true)?;
        let mut settings = {
            let world = scripts.world.borrow();
            world.btech.vehicles().get(&unit).map_or_else(
                || world.btech.constructed_units()[&unit].brief,
                |vehicle| vehicle.brief,
            )
        };
        let arguments = arguments.trim();
        if arguments.is_empty() {
            return Ok(BattleBriefReport {
                settings,
                changed: false,
                text: format!(
                    "Brief status for #{}:\r\n    (A)utocontacts: {}\r\n    (C)ontacts:     {}",
                    unit.0,
                    AUTOMATIC[usize::from(settings.automatic)],
                    CONTACTS[usize::from(settings.contacts)]
                ),
            });
        }
        let category = arguments.chars().next().unwrap();
        let value = arguments[category.len_utf8()..].trim();
        ensure!(!value.is_empty(), "Argument missing!");
        let value: i32 = value.parse().context("Invalid number!")?;
        let text = match category.to_ascii_uppercase() {
            'C' => {
                ensure!((0..=3).contains(&value), "Number out of range!");
                settings.contacts = value as u8;
                format!("Contact brevity set to {}.", CONTACTS[value as usize])
            }
            'A' => {
                ensure!((0..=6).contains(&value), "Number out of range!");
                settings.automatic = value as u8;
                format!("Autocontact brevity set to {}.", AUTOMATIC[value as usize])
            }
            _ => anyhow::bail!("Usage: brief [A 0-6 | C 0-3]"),
        };
        settings.validate()?;
        {
            let mut world = scripts.world.borrow_mut();
            if let Some(vehicle) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&unit) {
                vehicle.brief = settings;
            } else {
                Arc::make_mut(&mut world.btech.constructed)
                    .get_mut(&unit)
                    .unwrap()
                    .brief = settings;
            }
        }
        super::notify_unit_text(scripts, unit, &text)?;
        Ok(BattleBriefReport {
            settings,
            changed: true,
            text,
        })
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(effects);
    }
    result
}

/// Native adapter uses the invoking pilot's current cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        brief(ctx.scripts, unit, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(report) if report.changed => crate::CommandAction::Continue,
        Ok(report) => crate::CommandAction::Report(crate::CommandReport::Reply(report.text)),
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Display controls are shared by live cockpit occupants, including passengers.
pub(super) fn cockpit_access(world: &World, unit: ObjectId, viewer: ObjectId) -> Result<()> {
    display_access(world, unit, viewer, false)
}

/// Admit vehicle occupants only for display operations that support their state.
pub(super) fn display_access(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
    vehicles: bool,
) -> Result<()> {
    ensure!(!world.btech.unconscious(viewer), "You are unconscious");
    ensure!(
        (world.btech.constructed_units().contains_key(&unit)
            || (vehicles && world.btech.vehicles().contains_key(&unit)))
            && world
                .objects
                .get(&unit)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    ensure!(
        world
            .objects
            .get(&viewer)
            .is_some_and(|player| player.kind == Kind::Player
                && player.location == Some(unit)
                && !player.flags.contains(Flag::Going)),
        "Enter the unit first"
    );
    Ok(())
}

/// Read-only reports may use a registered station's selection with its parent's physical state.
/// Ordinary cockpit occupants retain passenger access; this does not admit mutable display controls.
pub(super) fn display_source(
    world: &World,
    owner: ObjectId,
    viewer: ObjectId,
) -> Result<super::fire_target::TargetSource> {
    let station = if world.btech.gunner_stations().contains_key(&owner) {
        Some(owner)
    } else {
        world
            .objects
            .get(&viewer)
            .and_then(|player| player.location)
            .filter(|id| world.btech.gunner_stations().contains_key(id))
    };
    if let Some(station) = station {
        let context = super::gunner_context(world, station, viewer)?;
        ensure!(
            owner == station || owner == context.parent,
            "Station does not control this unit"
        );
        super::power::control_health(world, viewer)?;
        return context.target_source(world);
    }
    display_access(world, owner, viewer, true)?;
    Ok(owner.into())
}
