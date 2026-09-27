//! Read-only cockpit status assembled from the unit's owned state and existing derived rules.
use super::fire_target::TargetSource;
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

mod armor;
mod layout;
mod technology;
mod weapons;

/// Render adversarial protection through the same standard armor silhouettes as cockpit status.
pub(super) fn scan_armor(world: &World, id: ObjectId) -> Result<String> {
    armor::scan(world, id)
}

/// Shared visible condition rows for adversarial scans and reports.
pub(super) fn scan_conditions(world: &World, id: ObjectId) -> Result<Vec<String>> {
    layout::scan_conditions(world, id)
}

/// Share absolute turret and signed-offset presentation with cockpit status.
pub(super) fn turret_line(turret: f64, heading: f64, stationary: bool) -> String {
    layout::turret_line(turret, heading, stationary)
}

/// Display policies shared by cockpit and observer reports.
#[derive(Clone, Copy)]
pub(crate) struct StatusRules {
    pub speed: super::SpeedPolicy,
    pub new_charge: bool,
}

impl Default for StatusRules {
    fn default() -> Self {
        Self {
            speed: super::SpeedPolicy::STANDARD,
            new_charge: false,
        }
    }
}

impl StatusRules {
    /// Read host switches without retaining another copy of simulation state.
    pub(crate) fn configured(config: &crate::Config) -> Self {
        Self {
            speed: super::SpeedPolicy::configured(config),
            new_charge: config.battletech.newcharge != 0,
        }
    }
}

/// Select independent display sections; the short display takes precedence.
#[derive(Default)]
struct Sections {
    armor: bool,
    info: bool,
    weapons: bool,
    heat: bool,
    short: bool,
    export: bool,
    reference: bool,
}

impl Sections {
    /// Accept compact selectors or full section names without treating arguments as object ids.
    fn parse(options: &str) -> Self {
        if options.trim().is_empty() {
            return Self {
                armor: true,
                info: true,
                weapons: true,
                heat: true,
                short: false,
                export: false,
                reference: false,
            };
        }
        let mut selected = Self::default();
        for token in options.split_whitespace() {
            let token = token.to_ascii_lowercase();
            let codes = match token.as_str() {
                "armor" => "a",
                "info" => "i",
                "weapons" => "w",
                "heat" => "h",
                "short" => "s",
                _ => token.as_str(),
            };
            for code in codes.chars() {
                match code {
                    'a' => selected.armor = true,
                    'i' => selected.info = true,
                    'w' => selected.weapons = true,
                    'h' => selected.heat = true,
                    's' => selected.short = true,
                    'n' => selected.export = true,
                    'r' => {
                        selected.armor = true;
                        selected.info = true;
                        selected.weapons = true;
                        selected.heat = true;
                        selected.reference = true;
                    }
                    _ => {}
                }
            }
        }
        selected
    }
}

/// Inspect a live constructed unit as styled text without changing contacts, timers, heat or dice.
/// Literal fields are escaped; the native report and Lua notification paths can render the result directly.
pub fn unit_status(world: &World, id: ObjectId, options: &str) -> Result<String> {
    unit_status_configured(world, id, options, StatusRules::default())
}

/// Render cockpit state using the host's configured towing assistance.
pub(crate) fn unit_status_configured(
    world: &World,
    id: ObjectId,
    options: &str,
    rules: StatusRules,
) -> Result<String> {
    render_source(world, id.into(), options, rules)
}

/// Resolve occupant authority before rendering the unit's status.
pub(crate) fn for_operator(
    world: &World,
    owner: ObjectId,
    viewer: ObjectId,
    options: &str,
    rules: StatusRules,
) -> Result<String> {
    let source = super::brief::display_source(world, owner, viewer)?;
    render_source(world, source, options, rules)
}

/// Render physical facts and targeting for one unit.
fn render_source(
    world: &World,
    source: TargetSource,
    options: &str,
    rules: StatusRules,
) -> Result<String> {
    let id = source.unit;
    let selected = Sections::parse(options);
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| !o.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    if selected.export && !selected.short {
        if let Some(unit) = world.btech.vehicles().get(&id) {
            return super::status_export::render_vehicle(unit, selected.weapons);
        }
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Enter a constructed unit first")?;
        return super::status_export::render(unit, selected.weapons);
    }
    layout::render(world, source, &selected, rules)
}

/// Occupants inspect their local cockpit; explicit remote ids are not command arguments.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world.borrow();
        ensure!(!world.btech.unconscious(ctx.player), "You are unconscious");
        let id = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        for_operator(
            &world,
            id,
            ctx.player,
            &input.args,
            StatusRules::configured(ctx.config),
        )
    })();
    Ok(CommandAction::Report(match result {
        Ok(text) => CommandReport::Styled(text),
        Err(error) => CommandReport::Reply(format!("{error:#}")),
    }))
}
