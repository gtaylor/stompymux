//! Detailed unit scans with hardware limits and separate ordinary/observer information disclosure.
use super::{BattlePower, BattleSystem, BattleUnit};
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Installed or administratively assigned view ranges, including sensor critical losses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleSensorRanges {
    /// Tactical display radius in hexes.
    pub tactical: u8,
    /// Long-range display radius in hexes.
    pub long_range: u8,
    /// Detailed unit inspection radius in hexes.
    pub scan: u8,
}

impl BattleUnit {
    /// Nonzero template ranges override technology-base defaults independently.
    /// Template zero selects defaults; runtime zero remains zero. Subsequent sensor hits degrade ranges.
    pub fn sensor_ranges(&self) -> BattleSensorRanges {
        configured_ranges(
            &self.definition().attributes,
            self.definition().has_special("Clan"),
            self.system_hits(BattleSystem::Sensors),
            self.hardware,
        )
    }
}

impl super::BattleVehicle {
    /// Vehicle computer ranges use template overrides; vehicle sensor damage affects gunnery.
    pub fn sensor_ranges(&self) -> BattleSensorRanges {
        configured_ranges(
            &self.definition().attributes,
            self.definition().has_special("Clan"),
            0,
            self.hardware,
        )
    }
}

/// Derive shared computer defaults before applying Mech-specific sensor critical degradation.
fn configured_ranges(
    attributes: &std::collections::BTreeMap<String, String>,
    clan: bool,
    hits: u8,
    hardware: super::hardware_settings::HardwareSettings,
) -> BattleSensorRanges {
    let base = if clan { 35 } else { 25 };
    let range = |field: &str, default, explicit: Option<super::hardware_settings::RangeSetting>| {
        if let Some(value) = explicit {
            return value.at_hits(hits);
        }
        let configured = attributes
            .get(field)
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|v| *v != 0)
            .unwrap_or(default);
        match hits {
            0 => configured,
            1 => configured / 2,
            _ => 0,
        }
    };
    BattleSensorRanges {
        tactical: range("tac_range", base, hardware.tactical),
        long_range: range("lrs_range", base * 2, hardware.long_range),
        scan: range("scan_range", base, hardware.scan),
    }
}

/// Select independent unit report sections without exposing cockpit-only status options.
pub(super) fn options(text: &str) -> Result<(bool, bool, bool)> {
    if text.trim().is_empty() {
        return Ok((true, true, true));
    }
    let mut selected = (false, false, false);
    for word in text.split_whitespace() {
        let lower = word.to_ascii_lowercase();
        let codes = match lower.as_str() {
            "armor" => "a",
            "info" => "i",
            "weapons" => "w",
            _ => &lower,
        };
        for c in codes.chars() {
            match c {
                'a' => selected.0 = true,
                'i' => selected.1 = true,
                'w' => selected.2 = true,
                _ => bail!("Usage: scan [target] [A|I|W|AIW] or scan x y [B|H]"),
            }
        }
    }
    Ok(selected)
}

/// Inspect an acquired visible unit, showing exact internals only to administrator-assigned observers.
/// Ordinary range uses the truncated spatial distance; observer mode bypasses range, not visibility
/// or disabled scanner hardware. The query consumes no dice, contacts or equipment state.
pub fn scan_unit(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    selection: &str,
) -> Result<String> {
    scan_unit_configured(
        world,
        observer,
        pilot,
        target,
        selection,
        super::status::StatusRules::default(),
    )
}

/// Observer detail uses the same configured cockpit report as native and Lua status.
fn scan_unit_configured(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    selection: &str,
    status_rules: super::status::StatusRules,
) -> Result<String> {
    let (armor, info, weapons) = options(selection)?;
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let mut text = unit_summary(world, observer, pilot, target, true)?;
    let source = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    if source.observer {
        let mut selected = String::new();
        if armor {
            selected.push('a');
        }
        if info {
            selected.push('i');
        }
        if weapons {
            selected.push('w');
        }
        text.push('\n');
        text.push_str(&super::unit_status_configured(
            world,
            target,
            &selected,
            status_rules,
        )?);
        return Ok(text);
    }
    if armor {
        text.push('\n');
        text.push_str(&super::status::scan_armor(world, target)?);
    }
    if info {
        if let Some(unit) = world.btech.constructed_units().get(&target) {
            if matches!(
                unit.facing().torso,
                super::BattleTorso::Right | super::BattleTorso::Both
            ) {
                text.push_str("\nTorso is 60 degrees right");
            }
            if matches!(
                unit.facing().torso,
                super::BattleTorso::Left | super::BattleTorso::Both
            ) {
                text.push_str("\nTorso is 60 degrees left");
            }
        }
        if let Some(towed) = world.btech.tows().get(&target) {
            let seen = super::visible_contact(world, target, *towed)?;
            let name = seen
                .as_ref()
                .map_or("something", |contact| contact.name.as_str());
            let mut label = super::scanner::scanner_unit(world, *towed)
                .and_then(|unit| unit.label())
                .unwrap_or_else(|| "??".into());
            if seen.as_ref().is_some_and(|contact| contact.friendly) {
                label.make_ascii_lowercase();
            }
            text.push_str(&crate::text::escape(&format!("\nTowing {name} [{label}].")));
        }
        text.push_str("\n ");
    }
    if weapons {
        text.push('\n');
        text.push_str(&super::scan_weapons::render(world, target)?);
    }
    Ok(text)
}

/// Brief visible-unit report shared by detailed scans and the silent report command.
/// Direct reports require working hardware but do not impose the detailed scan radius.
pub fn report_unit(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
) -> Result<String> {
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    unit_summary(world, observer, pilot, target, false)
}

/// Validate inspection once and render the common identity, motion and position summary.
fn unit_summary(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    enforce_range: bool,
) -> Result<String> {
    let (source, maximum) = scanner(world, observer, pilot)?;
    let view = super::visible_contact(world, observer, target)?
        .context("Target is not in line of sight!")?;
    ensure!(
        super::visibility::unit_unblocked(world, observer, target)?,
        "That target isn't seen well enough by the scanners for scanning!"
    );
    ensure!(
        !enforce_range || source.observer || view.range.spatial.trunc() <= f64::from(maximum),
        "Target is out of scanner range."
    );
    super::scan_summary::render(world, observer, &view)
}

/// Inspect a unit and stage its scan warning within the enclosing host transaction.
/// The target identifies the scanner from its own contacts. Observer scans and
/// shutdown targets stay silent; failed queries publish no warning.
pub fn scan_unit_action(
    scripts: &crate::Scripts,
    observer: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    selection: &str,
) -> Result<String> {
    let observer = super::combat_operator::for_owner(&scripts.world(), observer, pilot)?
        .source
        .unit;
    scripts.atomic(|_| {
        let (text, notice) = {
            let world = scripts.world.borrow();
            let text = scan_unit_configured(
                &world,
                observer,
                pilot,
                target,
                selection,
                super::status::StatusRules::configured(&crate::lua::configuration(&scripts.lua)),
            )?;
            let source =
                super::scanner::scanner_unit(&world, observer).context("Scanner is unavailable")?;
            let scanned =
                super::scanner::scanner_unit(&world, target).context("Target is unavailable")?;
            let notice = if source.observer || scanned.power != BattlePower::Running {
                None
            } else {
                let seen = super::visible_contact(&world, target, observer)?;
                let mut label = source.label().context("Scanner is not placed")?;
                if seen.as_ref().is_some_and(|view| view.friendly) {
                    label.make_ascii_lowercase();
                }
                let name = seen.as_ref().map_or("something", |view| view.name.as_str());
                Some(super::BattleNotice {
                    unit: target,
                    text: format!("You are being scanned by {name} [{label}]"),
                })
            };
            (text, notice)
        };
        if let Some(notice) = notice {
            super::notify_unit(scripts, notice)?;
        }
        Ok(text)
    })
}

/// Scan the first acquired visible occupant of a coordinate, in saved map membership order.
/// Empty or unacquired occupants produce the same empty-hex reply. Coordinate
/// inspection never acquires a contact, selects a weapon target or consumes dice.
pub fn scan_hex_unit_action(
    scripts: &crate::Scripts,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: super::BattleHexCoordinate,
    selection: &str,
) -> Result<String> {
    options(selection)?;
    let observer = super::combat_operator::for_owner(&scripts.world(), observer, pilot)?
        .source
        .unit;
    let target = {
        let world = scripts.world.borrow();
        let map = check_coordinate(&world, observer, pilot, coordinate, true)?;
        occupant(&world, observer, map, coordinate)?
    };
    let Some(target) = target else {
        return Ok("You see nobody in the hex!".into());
    };
    scan_unit_action(scripts, observer, pilot, target, selection)
}

/// Select the first acquired visible occupant, without rolling or inspecting unit internals.
pub(super) fn occupant(
    world: &World,
    observer: ObjectId,
    map: ObjectId,
    coordinate: super::BattleHexCoordinate,
) -> Result<Option<ObjectId>> {
    for candidate in super::map_slots::hex_occupants(world, map, coordinate)? {
        if candidate == observer {
            continue;
        }
        if super::visible_contact(world, observer, candidate)?.is_some() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Apply the shared cockpit and working scanner admission for unit and terrain queries.
fn scanner(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
) -> Result<(super::scanner::ScannerUnit<'_>, u8)> {
    super::combat_operator::controlled(world, observer, pilot)?;
    let source = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    ensure!(
        source.power == BattlePower::Running && !source.destroyed,
        "Start the unit first"
    );
    let maximum = world.btech.vehicles().get(&observer).map_or_else(
        || {
            world.btech.constructed_units()[&observer]
                .sensor_ranges()
                .scan
        },
        |vehicle| vehicle.sensor_ranges().scan,
    );
    ensure!(maximum > 0, "Your system seems to be inoperational.");
    Ok((source, maximum))
}

/// Validate coordinate scan admission; explicit structure mode retains its hardware range limit.
pub(super) fn check_coordinate(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: super::BattleHexCoordinate,
    observer_range: bool,
) -> Result<ObjectId> {
    let (unit, maximum) = scanner(world, observer, pilot)?;
    let map = unit.position.context("Scanner is not placed")?.map;
    let (_, distance) = super::los::unit_hex_los(world, observer, coordinate)?;
    ensure!(
        (observer_range && unit.observer) || distance.trunc() <= f64::from(maximum),
        "Those coordinates are out of scanner range."
    );
    ensure!(
        super::hex_visible(world, observer, coordinate)?,
        "Coordinates are not in line of sight!"
    );
    Ok(map)
}

/// Native unit-scan selection supports explicit target or the selected unit lock.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world.borrow();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let source = super::combat_operator::for_owner(&world, unit, ctx.player)?.source;
        let args: Vec<_> = input.args.split_whitespace().collect();
        if let [x, y, mode] = args.as_slice() {
            ensure!(
                mode.eq_ignore_ascii_case("b") || mode.eq_ignore_ascii_case("h"),
                "Invalid 3rd argument!"
            );
            let coordinate = super::BattleHexCoordinate {
                x: x.parse().context("Invalid coordinates!")?,
                y: y.parse().context("Invalid coordinates!")?,
            };
            drop(world);
            if mode.eq_ignore_ascii_case("h") {
                super::scan_hex_action(ctx.scripts, ctx.config, unit, ctx.player, coordinate)?;
            } else {
                super::scan_building_action(ctx.scripts, ctx.config, unit, ctx.player, coordinate)?;
            }
            return Ok(String::new());
        }
        if let [x, y] = args.as_slice()
            && let Ok(x) = x.parse::<i32>()
        {
            let coordinate = super::BattleHexCoordinate {
                x,
                y: y.parse().context("Invalid coordinates!")?,
            };
            drop(world);
            return scan_hex_unit_action(ctx.scripts, unit, ctx.player, coordinate, "");
        }
        let selected_option = match args.as_slice() {
            [] => Some(""),
            [one]
                if ["a", "i", "w", "aiw", "armor", "info", "weapons"]
                    .contains(&one.to_ascii_lowercase().as_str()) =>
            {
                Some(*one)
            }
            _ => None,
        };
        if let Some(option) = selected_option {
            drop(world);
            return Ok(
                match super::scan_selected_action(
                    ctx.scripts,
                    ctx.config,
                    unit,
                    ctx.player,
                    option,
                )? {
                    super::BattleSelectedScan::Unit(text) => text,
                    _ => String::new(),
                },
            );
        }
        let (target, option) = match args.as_slice() {
            [one] => (super::radio_targeted::target(&world, source.unit, one)?, ""),
            [one, option] => (
                super::radio_targeted::target(&world, source.unit, one)?,
                *option,
            ),
            _ => bail!("Usage: scan [target] [A|I|W|AIW] or scan x y [B|H]"),
        };
        drop(world);
        scan_unit_action(ctx.scripts, unit, ctx.player, target, option)
    })();
    if result.as_ref().is_ok_and(|text| text.is_empty()) {
        return Ok(crate::CommandAction::Continue);
    }
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Styled(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}
