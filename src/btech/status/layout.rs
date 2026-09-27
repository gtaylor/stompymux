//! Fixed cockpit columns assembled from shared read-only unit projections.
use super::Sections;
use crate::btech::{self, fire_target::TargetSource, *};
use crate::{ObjectId, World, text};
use anyhow::{Context, Result};

/// Chassis adapters provide facts; all column and selector decisions live here.
struct Facts<'a> {
    name: &'a str,
    reference: &'a str,
    tons: u16,
    movement: Option<BattleVehicleMovement>,
    pilot: Option<ObjectId>,
    injuries: u16,
    motion: Option<BattleMotion>,
    position: Option<BattlePosition>,
    z: i32,
    speed_limit: f64,
    jump: u16,
    sinks: u16,
    production: f64,
    dissipation: f64,
    excess: f64,
    vertical: f64,
    turret: Option<f64>,
}

/// Resolve shared display inputs without advancing clocks or recomputing sampled heat.
fn facts(
    world: &World,
    id: ObjectId,
    reference: bool,
    policy: btech::SpeedPolicy,
) -> Result<Facts<'_>> {
    let scan = btech::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let tile = scan.position.and_then(|p| {
        world
            .btech
            .maps()
            .get(&p.map)?
            .hex(p.x.into(), p.y.into())
            .ok()
    });
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let def = unit.definition();
        let rates = unit.sampled_heat_rates();
        let gravity = unit
            .position()
            .and_then(|p| world.btech.maps().get(&p.map))
            .map_or(100, |m| m.gravity);
        return Ok(Facts {
            name: if reference {
                &def.name
            } else {
                world
                    .btech
                    .unit_configuration
                    .get(&id)
                    .and_then(|configuration| configuration.display_name.as_deref())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| unit.display_name.effective(&def.name))
            },
            reference: &def.reference,
            tons: def.tons,
            movement: None,
            pilot: unit.pilot(),
            injuries: unit
                .character_pilot_status()
                .map_or(unit.pilot_injuries().into(), |s| s.injuries),
            motion: unit.motion(),
            position: unit.position(),
            z: tile.map_or(0, |t| unit.altitude(t) as i32),
            speed_limit: btech::motion_controls::throttle_configured(world, id, policy)?,
            jump: unit.jump_capacity(gravity)?.movement_points,
            sinks: unit.cooling_capacity(),
            production: rates.production,
            dissipation: rates.dissipation,
            excess: unit.heat().excess,
            vertical: 0.0,
            turret: None,
        });
    }
    let unit = &world.btech.vehicles()[&id];
    let def = unit.definition();
    Ok(Facts {
        name: if reference {
            &def.name
        } else {
            world
                .btech
                .unit_configuration
                .get(&id)
                .and_then(|configuration| configuration.display_name.as_deref())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| unit.display_name.effective(&def.name))
        },
        reference: &def.reference,
        tons: def.tons,
        movement: Some(def.movement),
        pilot: unit.pilot(),
        injuries: unit
            .character_pilot_status()
            .map_or(unit.pilot_injuries().into(), |s| s.injuries),
        motion: unit.motion(),
        position: unit.position(),
        z: tile.map_or(0, |t| unit.altitude(t) as i32),
        speed_limit: btech::motion_controls::throttle_configured(world, id, policy)?,
        jump: 0,
        sinks: unit.cooling_capacity()?,
        production: 0.0,
        dissipation: 0.0,
        excess: 0.0,
        vertical: unit.vtol_flight().map_or(0.0, |f| f.vertical_speed),
        turret: unit.turret_heading(),
    })
}

/// Escape player text only after truncating/padding its visible column.
fn column(value: &str, width: usize) -> String {
    let visible = value.chars().take(width).collect::<String>();
    text::escape(&format!("{visible:<width$}"))
}

/// Render the selected blocks in cockpit order.
pub(super) fn render(
    world: &World,
    source: TargetSource,
    selected: &Sections,
    rules: super::StatusRules,
) -> Result<String> {
    let id = source.unit;
    let f = facts(world, id, selected.reference, rules.speed)?;
    let m = f.motion;
    let (speed, wanted, heading, desired) = (
        m.map_or(0.0, |m| m.speed),
        m.map_or(0.0, |m| m.desired_speed),
        m.map_or(0.0, |m| m.heading) as i32,
        m.map_or(0.0, |m| m.desired_heading) as i32,
    );
    let (x, y) = f.position.map_or((0, 0), |p| (p.x, p.y));
    let mut lines = Vec::new();
    if selected.short {
        let specific = if f.movement.is_none() {
            format!(
                " HT: {:3}/{:3}/{:<3} ",
                (f.production * 10.0) as i32,
                u32::from(f.sinks) * 10,
                (f.dissipation * 10.0) as i32
            )
        } else if f.movement == Some(BattleVehicleMovement::Vtol) {
            format!(" VSPD: {:3.1} ", f.vertical)
        } else if let Some(turret) = f.turret {
            format!(" TUR: {:3} ", turret as i32)
        } else {
            String::new()
        };
        lines.push(format!("LOC: {x:3},{y:3},{:3}  HD: {heading:3}/{desired:3}  SP: {speed:3.1}/{wanted:3.1} {specific} ST:{}",f.z,short_flags(world,id)?));
        targeting(&mut lines, world, source, rules.new_charge)?;
        return Ok(lines.join("\r\n"));
    }
    if !selected.heat || selected.armor || selected.info || selected.weapons {
        header(&mut lines, world, id, &f)?;
    }
    if selected.armor {
        lines.push(super::armor::render(world, id)?);
        lines.push(" ".into());
    }
    if selected.info {
        if f.movement.is_none() {
            lines.push(format!("X, Y, Z:{x:3},{y:3},{:3}  Excess Heat:  {:3} deg C.  Heat Production:  {:3} deg C.",f.z,(f.excess*10.0) as i32,(f.production*10.0) as i32));
            lines.push(format!("Speed:      [fg=green bold]{:3}[reset] KPH  Heading:      [fg=green bold]{heading:3}[reset] deg     Heat Sinks:       {:3}",speed as i32,f.sinks));
            lines.push(format!("Des. Speed: {:3} KPH  Des. Heading: {desired:3} deg     Heat Dissipation: {:3} deg C.",wanted as i32,(f.dissipation*10.0) as i32));
            let unit = &world.btech.constructed_units()[&id];
            if unit.lateral().active != BattleLateralMode::None {
                lines.push(format!(
                    "You are moving laterally {}",
                    unit.lateral().active.description()
                ));
            }
        } else {
            lines.push(format!(
                "X, Y, Z:{x:3},{y:3},{:3}  Heat Sinks:          {:3}       ",
                f.z, f.sinks
            ));
            if f.movement == Some(BattleVehicleMovement::Vtol) {
                lines.push(format!("Speed:      [fg=green bold]{:3}[reset] KPH  Vertical Speed:      [fg=green bold]{:3}[reset] KPH   Des. Speed {:3} KPH",speed as i32,f.vertical as i32,wanted as i32));
                let fuel = btech::vtol_fuel_status(world, id)?;
                let percent = if fuel.capacity > 0 {
                    fuel.remaining.max(0) as f64 * 100.0 / fuel.capacity as f64
                } else {
                    0.0
                };
                lines.push(format!("Heading:    [fg=green bold]{heading:3}[reset] deg  Des. Heading:        {desired:3} deg   Fuel: {} ({percent:.2} %)",fuel.remaining.max(0)));
            } else if f.movement != Some(BattleVehicleMovement::Stationary) {
                lines.push(format!("Speed:      [fg=green bold]{:3}[reset] KPH  Heading:      [fg=green bold]{heading:3}[reset] deg",speed as i32));
                lines.push(format!(
                    "Des. Speed: {:3} KPH  Des. Heading: {desired:3} deg",
                    wanted as i32
                ));
            }
            if let Some(turret) = f.turret {
                lines.push(turret_line(
                    turret,
                    f64::from(heading),
                    f.movement == Some(BattleVehicleMovement::Stationary),
                ));
            }
        }
    }
    if f.movement.is_none() && (selected.info || selected.heat) {
        lines.push(heat_bar(f.production, f.dissipation));
    }
    if selected.info {
        lines.push("  ".into());
        targeting(&mut lines, world, source, rules.new_charge)?;
        if let Some(target) = world.btech.tows().get(&id) {
            lines.push(format!("Towing {}.", display_id(world, *target)));
        }
    }
    if selected.weapons {
        lines.push(super::weapons::render(world, id)?);
    }
    Ok(lines.join("\r\n"))
}

/// Standard identification and condition lines precede every non-heat block.
fn header(lines: &mut Vec<String>, world: &World, id: ObjectId, f: &Facts<'_>) -> Result<()> {
    let scan = btech::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let label = scan.label().unwrap_or_else(|| "??".into());
    match f.movement {
        None => {
            lines.push(format!(
                "Mech Name: {}  ID:[{}]   Mech Reference: {}",
                column(f.name, 18),
                text::escape(&label),
                text::escape(f.reference)
            ));
            lines.push(format!(
                "Tonnage:   {:3}     MaxSpeed: {:3}       JumpRange: {}",
                f.tons, f.speed_limit as i32, f.jump
            ));
        }
        Some(BattleVehicleMovement::Stationary) => lines.push(format!(
            "Name: {}  ID:[{}]   Reference: {}",
            column(f.name, 15),
            text::escape(&label),
            text::escape(f.reference)
        )),
        Some(movement) => {
            let movement = match movement {
                BattleVehicleMovement::Tracked => "Tracked",
                BattleVehicleMovement::Wheeled => "Wheeled",
                BattleVehicleMovement::Hover => "Hover",
                BattleVehicleMovement::Vtol => "VTOL",
                BattleVehicleMovement::Stationary => unreachable!(),
            };
            lines.push(format!(
                "Vehicle Name: {}  ID:[{}]   Vehicle Reference: {}",
                column(f.name, 15),
                text::escape(&label),
                text::escape(f.reference)
            ));
            lines.push(format!(
                "Tonnage:   {:3}      FlankSpeed: {:3}       Movement Type: {movement}",
                f.tons, f.speed_limit as i32
            ));
        }
    }
    if f.movement != Some(BattleVehicleMovement::Stationary) {
        lines.push(if let Some(pilot) = f.pilot {
            format!(
                "Pilot Name: {} Pilot Injury: {}",
                column(
                    &world
                        .objects
                        .get(&pilot)
                        .context("Pilot is unavailable")?
                        .name,
                    28
                ),
                f.injuries
            )
        } else {
            "Pilot: NONE".into()
        });
    }
    conditions(lines, world, id)?;
    if let Some(u) = world.btech.constructed_units().get(&id) {
        if let Some(flight) = u.flight() {
            let end = flight
                .path()
                .sample(1.0, flight.path().movement_points())?
                .point
                .containing_hex()?;
            let mut line = format!("JUMPING --> {:3},{:3}", end.x, end.y);
            if let Some(target) = flight.dfa_target() {
                line.push_str(&format!(
                    "  Death From Above Target: {}",
                    display_id(world, target)
                ));
            }
            lines.push(line);
        } else if u.posture() != BattlePosture::Prone
            && u.power() == BattlePower::Running
            && let Some(target) = u.charge().target
        {
            lines.push(format!("CHARGING --> {}", display_id(world, target)));
        }
    }
    Ok(())
}

/// Display identification is presentation-only and never acquires a contact.
fn display_id(world: &World, id: ObjectId) -> String {
    btech::scanner::scanner_unit(world, id).map_or_else(
        || "unknown".into(),
        |s| {
            format!(
                "{} [{}]",
                text::escape(s.name),
                text::escape(&s.label().unwrap_or_else(|| "??".into()))
            )
        },
    )
}

/// Show the source's selected target and its visibility.
fn targeting(
    lines: &mut Vec<String>,
    world: &World,
    source: TargetSource,
    new_charge: bool,
) -> Result<()> {
    match source.selection(world) {
        Some(BattleTargetSelection::Unit(lock)) => {
            if !btech::visibility::unit_unblocked(world, source.unit, lock.target)? {
                lines.push("Target: NOT in line of sight!".into());
            } else if let Ok(range) = btech::geometry::unit_range(world, source.unit, lock.target) {
                lines.push(format!(
                    "Target: {}\t   Range: {:.1} hexes   Bearing: {} deg",
                    display_id(world, lock.target),
                    range.spatial,
                    range.bearing.unwrap_or(0.0) as i32
                ));
                let observer = btech::scanner::scanner_unit(world, source.unit)
                    .context("Observer is unavailable")?;
                let arc = observer.facing.contact_arc(
                    observer.heading.unwrap_or(0.0),
                    range.bearing.unwrap_or(0.0),
                )?;
                let arc = match arc {
                    BattleContactArc::Front => "Forward",
                    BattleContactArc::Rear => "Rear",
                    BattleContactArc::Left => {
                        if observer.vehicle {
                            "Left Side"
                        } else {
                            "Left Arm"
                        }
                    }
                    BattleContactArc::Right => {
                        if observer.vehicle {
                            "Right Side"
                        } else {
                            "Right Arm"
                        }
                    }
                };
                let turret = if let Some(u) = world.btech.vehicles().get(&source.unit) {
                    u.loadout()?.weapons.iter().any(|mount| {
                        mount.criticals[0].section == BattleVehicleSection::Turret
                            && mount
                                .bears_on(
                                    observer.heading.unwrap_or(0.0),
                                    range.bearing.unwrap_or(0.0),
                                    u.turret_heading(),
                                )
                                .unwrap_or(false)
                    })
                } else {
                    false
                };
                let arc = if turret { "Turret" } else { arc };
                let aim = btech::aimed_target::aimed_section(world, source.unit)?
                    .filter(|a| a.matches(world, lock.target));
                let location = match aim {
                    Some(BattleAimSelection::Mech(s)) => s.name().replace('_', " "),
                    Some(BattleAimSelection::GroundVehicle(s) | BattleAimSelection::Vtol(s)) => {
                        s.name().replace('_', " ")
                    }
                    None => "None".into(),
                };
                lines.push(format!(
                    "Target in {arc} Weapons Arc\t   Aimed Shot Location: {location}"
                ));
            }
        }
        Some(BattleTargetSelection::Hex(lock)) => lines.push(format!(
            "Target: {}{} {}",
            match lock.mode {
                BattleHexTargetMode::Building => "Building at ",
                BattleHexTargetMode::UnitAtHex => "",
                _ => "Hex ",
            },
            lock.hex.x,
            lock.hex.y
        )),
        None => {}
    }
    let safety = if let Some(u) = world.btech.constructed_units().get(&source.unit) {
        u.mw_safety()
    } else {
        let u = &world.btech.vehicles()[&source.unit];
        u.mw_safety()
    };
    if !safety {
        lines.push("Weapon Safeties are [fg=red bold]OFF[reset].".into());
    }
    if new_charge
        && let Some(unit) = world.btech.constructed_units().get(&source.unit)
        && let Some(target) = unit.charge().target
        && btech::scanner::scanner_unit(world, target).is_some()
    {
        let timer = unit.charge().elapsed / 2;
        if btech::visibility::unit_unblocked(world, source.unit, target)? {
            lines.push(format!(
                "ChargeTarget: {}\t  ChargeTimer: {timer}",
                display_id(world, target)
            ));
        } else {
            lines.push(format!(
                "ChargeTarget: NOT in line of sight!\t Timer: {timer}"
            ));
        }
    }
    Ok(())
}

/// Forty heat bands follow the current dissipation point, with a bounded cool-side scale.
fn heat_bar(production: f64, dissipation: f64) -> String {
    let heat = production.max(0.0) as usize;
    let sinks = dissipation.max(0.0) as usize;
    let start = sinks.saturating_sub(27);
    let mut bar = String::from("Temp:[fg=black bold]");
    bar.push(if start > 0 { '<' } else { ' ' });
    for i in start..sinks + 40 {
        if i == sinks {
            bar.push_str("[fg=green bold]|[reset][fg=green]");
        }
        if i == sinks + 7 {
            bar.push_str("[bold]");
        }
        if i == sinks + 13 {
            bar.push_str("[reset][fg=yellow bold]|[reset][fg=yellow]");
        }
        if i == sinks + 16 {
            bar.push_str("[bold]");
        }
        if i == sinks + 18 {
            bar.push_str("[reset][fg=red bold]|[reset][fg=red]");
        }
        if i == sinks + 24 {
            bar.push_str("[bold]");
        }
        bar.push(if i < heat { ':' } else { '.' });
    }
    bar.push_str("[fg=white bold]|[reset]");
    bar
}

/// Condition banners use existing authoritative systems rather than stored display flags.
fn conditions(lines: &mut Vec<String>, world: &World, id: ObjectId) -> Result<()> {
    condition_lines(lines, world, id, true)
}

/// Scans expose visible conditions without cockpit-only equipment and control details.
pub(super) fn scan_conditions(world: &World, id: ObjectId) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    condition_lines(&mut lines, world, id, false)?;
    Ok(lines
        .into_iter()
        .map(|line| format!("      {line}"))
        .collect())
}

/// Both views use the same owned condition predicates; only disclosure differs.
fn condition_lines(
    lines: &mut Vec<String>,
    world: &World,
    id: ObjectId,
    owned: bool,
) -> Result<()> {
    let scan = btech::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let mut flags: Vec<(&str, &str)> = Vec::new();
    if btech::battle_combat_safe(world, id)? {
        flags.push(("COMBAT SAFE", "blue bold"));
    }
    if let Some(u) = world.btech.constructed_units().get(&id) {
        for (enabled, label, color) in [
            (u.fortified, "FORTIFIED", "green bold"),
            (u.weapons_hold, "WEAPONS HOLD", "red bold"),
            (u.posture() == BattlePosture::Prone, "FALLEN", "red bold"),
            (u.hull_down().active, "HULLDOWN", "green bold"),
            (u.stagger().action_level() > 0, "STAGGERING", "red bold"),
            (
                owned && u.stealth().enabled,
                "STEALTH ARMOR ACTIVE",
                "green bold",
            ),
            (
                owned && u.null_signature().enabled,
                "NULL SIGNATURE SYSTEM ACTIVE",
                "green bold",
            ),
        ] {
            if enabled {
                flags.push((label, color));
            }
        }
    } else {
        let u = &world.btech.vehicles()[&id];
        if owned && u.turret_jammed() {
            lines.push("     TURRET JAMMED".into());
        } else if owned && u.turret_locked() {
            lines.push("     TURRET LOCKED".into());
        }
        if owned
            && u.vtol_flight()
                .is_some_and(|f| f.phase == BattleVtolFlightPhase::Landed)
        {
            lines.push("LANDED".into());
        }
        for (enabled, label, color) in [
            (u.fortified, "FORTIFIED", "green bold"),
            (u.weapons_hold, "WEAPONS HOLD", "red bold"),
            (
                if u.definition().is_vtol() {
                    u.rotor_destroyed()
                        && u.vtol_flight()
                            .is_some_and(|f| f.phase == BattleVtolFlightPhase::Landed)
                } else {
                    u.immobilized()
                },
                match u.definition().movement {
                    BattleVehicleMovement::Tracked => "TRACK DESTROYED",
                    BattleVehicleMovement::Wheeled => "AXLE DESTROYED",
                    BattleVehicleMovement::Hover => "LIFT FAN DESTROYED",
                    BattleVehicleMovement::Vtol => "ROTOR DESTROYED",
                    BattleVehicleMovement::Stationary => "",
                },
                "red bold",
            ),
            (u.dig_state().dug_in, "DUG IN", "green bold"),
            (u.dig_state().digging, "DIGGING IN", "green"),
            (
                owned && u.automatic_turret(),
                "TURRET AUTO-TURN ENGAGED",
                "green bold",
            ),
        ] {
            if enabled && !label.is_empty() {
                flags.push((label, color));
            }
        }
    }
    if owned && scan.signature.hidden {
        flags.push(("HIDDEN", "green bold"));
    }
    let (lamp, electronics, inferno) = if let Some(u) = world.btech.constructed_units().get(&id) {
        (u.searchlight(), u.electronics(), u.inferno_remaining() > 0)
    } else {
        let u = &world.btech.vehicles()[&id];
        (
            u.searchlight(),
            u.electronics(),
            u.inferno_remaining() > 0 || !u.burning_sections().is_empty(),
        )
    };
    for (enabled, label, color) in [
        (lamp.destroyed, "SEARCHLIGHT DESTROYED", "red bold"),
        (lamp.on && !lamp.destroyed, "SEARCHLIGHT ON", "green bold"),
        (
            !lamp.on && btech::unit_illuminated(world, id),
            "ILLUMINATED",
            "green bold",
        ),
        (inferno, "ON FIRE", "red bold"),
        (
            owned && electronics.field.protected,
            "PROTECTED BY ECM",
            "green bold",
        ),
        (
            owned && electronics.field.angel_protected,
            "PROTECTED BY ANGEL ECM",
            "green bold",
        ),
        (
            owned && electronics.field.disturbed,
            "AFFECTED BY ECM",
            "yellow bold",
        ),
        (
            owned && electronics.field.angel_disturbed,
            "AFFECTED BY ANGEL ECM",
            "yellow bold",
        ),
        (
            owned && electronics.field.countered,
            "COUNTERED BY ECCM",
            "yellow bold",
        ),
    ] {
        if enabled {
            flags.push((label, color));
        }
    }
    if !owned && scan.signature.hidden {
        flags.push(("HIDDEN", "green bold"));
    }
    for (kind, label) in [
        (BattleBeaconKind::Narc, "NARC POD ATTACHED"),
        (BattleBeaconKind::Homing, "INARC HOMING POD ATTACHED"),
        (BattleBeaconKind::Haywire, "INARC HAYWIRE POD ATTACHED"),
        (BattleBeaconKind::Ecm, "INARC ECM POD ATTACHED"),
    ] {
        if owned && btech::narc::has_beacon(world, id, kind) {
            flags.push((label, "yellow bold"));
        }
    }
    for (label, color) in flags {
        lines.push(format!("[fg={color}]{label}[reset]"));
    }
    let self_destruct = if let Some(unit) = world.btech.constructed_units().get(&id) {
        unit.self_destruct()
    } else {
        world.btech.vehicles()[&id].self_destruct()
    };
    if owned && let Some(countdown) = self_destruct {
        lines.push(format!(
            "[fg=red bold]Self-destruction: {}s remaining[reset]",
            countdown.remaining
        ));
    }
    if scan.destroyed {
        lines.push("DESTROYED".into());
    }
    if scan.power != BattlePower::Running {
        lines.push("SHUTDOWN".into());
    }
    if owned && matches!(scan.facing.torso, BattleTorso::Right | BattleTorso::Both) {
        lines.push("Torso is 60 degrees right".into());
    }
    if owned && matches!(scan.facing.torso, BattleTorso::Left | BattleTorso::Both) {
        lines.push("Torso is 60 degrees left".into());
    }
    Ok(())
}

/// Compact status lists active conditions in a stable order, without blank contact columns.
fn short_flags(world: &World, id: ObjectId) -> Result<String> {
    let s = btech::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let mut flags = String::new();
    if s.destroyed {
        flags.push('D');
    }
    match s.power {
        BattlePower::Starting { .. } => flags.push('s'),
        BattlePower::Off => flags.push('S'),
        BattlePower::Running => {}
    }
    let (lamp, electronics, inferno) = if let Some(u) = world.btech.constructed_units().get(&id) {
        if u.stand_timer().is_some() {
            flags.push('f');
        } else if u.posture() == BattlePosture::Prone {
            flags.push('F');
        }
        if u.hull_down().pending.is_some() {
            flags.push('h');
        } else if u.hull_down().active {
            flags.push('H');
        }
        if world.btech.towed_by(id).is_some() {
            flags.push('T');
        } else if world.btech.tows().contains_key(&id) {
            flags.push('t');
        }
        if u.flight().is_some() {
            flags.push('J');
        }
        if u.heat().excess != 0.0 {
            flags.push('+');
        }
        (u.searchlight(), u.electronics(), u.inferno_remaining() > 0)
    } else {
        let u = &world.btech.vehicles()[&id];
        if u.immobilized() {
            flags.push('F');
        }
        if world.btech.towed_by(id).is_some() {
            flags.push('T');
        } else if world.btech.tows().contains_key(&id) {
            flags.push('t');
        }
        if !u.burning_sections().is_empty() {
            flags.push('B');
        }
        (u.searchlight(), u.electronics(), u.inferno_remaining() > 0)
    };
    if inferno {
        flags.push('I');
    }
    if lamp.on && !lamp.destroyed {
        flags.push('L');
    }
    if btech::unit_illuminated(world, id) {
        flags.push('l');
    }
    if world
        .btech
        .constructed_units()
        .get(&id)
        .is_some_and(|u| u.carried_club().is_some())
    {
        flags.push('C');
    }
    if btech::narc::has_beacon(world, id, BattleBeaconKind::Narc)
        || btech::narc::has_beacon(world, id, BattleBeaconKind::Homing)
    {
        flags.push('n');
    }
    let modes = [electronics.guardian, electronics.angel];
    if modes.contains(&BattleElectronicMode::Eccm) {
        flags.push('P');
    }
    if modes.contains(&BattleElectronicMode::Ecm) {
        flags.push('E');
    }
    if electronics.field.protected || electronics.field.angel_protected {
        flags.push('p');
    }
    if electronics.field.blocks_outgoing_guidance() {
        flags.push('e');
    }
    Ok(flags)
}

/// Turret bearings are absolute, with a signed offset only for moving chassis.
pub(super) fn turret_line(turret: f64, heading: f64, stationary: bool) -> String {
    let mut offset = (turret - heading).rem_euclid(360.0).trunc() as i32;
    if offset > 180 {
        offset -= 360;
    }
    let suffix = if offset != 0 && !stationary {
        format!(" ({offset} offset from heading)")
    } else {
        String::new()
    };
    format!(
        "      Turret Facing: {} degrees{suffix}",
        turret.trunc().rem_euclid(360.0) as i32
    )
}
