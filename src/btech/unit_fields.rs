//! Named unit inspection projects existing services through one chassis-independent field catalogue.
use super::unit_identity::IdentityField;
use super::*;
use crate::{Config, Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// One inspected field; absent values distinguish unavailable projections from a numeric zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleUnitField {
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Detached wizard inspection, retaining full field names independently of display width.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleUnitFieldReport {
    pub unit: ObjectId,
    pub columns: usize,
    pub fields: Vec<BattleUnitField>,
    pub text: String,
}

/// Public field order; carried-unit, repair and excluded chassis fields are deliberately absent.
const FIELDS: &[&str] = &[
    "displayname",
    "mapindex",
    "id",
    "mechname",
    "maxspeed",
    "unit_era",
    "unit_tro",
    "templatesp",
    "pilotnum",
    "xpmod",
    "pilotdam",
    "speed",
    "units_killed",
    "damage_taken",
    "damage_inflicted",
    "shots_fired",
    "shots_hit",
    "shots_missed",
    "basewalkspeed",
    "baserunspeed",
    "heading",
    "status",
    "status2",
    "critstatus",
    "critstatus2",
    "tankcritstatus",
    "target",
    "team",
    "tons",
    "towing",
    "heat",
    "disabled_hs",
    "overheat",
    "dissheat",
    "hsengoverride",
    "heatsinks",
    "last_startup",
    "C3iNetworkSize",
    "realweight",
    "StaggerDamage",
    "MechPrefs",
    "mechtype",
    "mechmovetype",
    "mechdamage",
    "centdist",
    "centbearing",
    "sensors",
    "mechref",
    "fuel",
    "fuel_orig",
    "cocoon",
    "numseen",
    "fx",
    "fy",
    "fz",
    "x",
    "y",
    "z",
    "targcomp",
    "lrsrange",
    "radiorange",
    "scanrange",
    "tacrange",
    "radiotype",
    "bv",
    "cargospace",
    "turret0",
    "turret1",
    "turret2",
    "unusablearcs",
    "maxjumpspeed",
    "jumpheading",
    "jumplength",
];

/// Wizard authority is independent of pilot occupancy or powered state.
fn admission(world: &World, actor: ObjectId, id: ObjectId) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing
        && !object.flags.contains(Flag::Going)), "Unit is unavailable");
    ensure!(
        super::scanner::scanner_unit(world, id).is_some(),
        "Unit construction state is unavailable"
    );
    Ok(())
}

/// Read a field from existing authoritative services; projections never consume random state.
fn value(world: &World, config: &Config, id: ObjectId, field: &str) -> Result<Option<String>> {
    let scanner = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let mech = world.btech.constructed_units().get(&id);
    let vehicle = world.btech.vehicles().get(&id);
    let (definition, tons, reference, template_speed, pilot, injuries, experience) =
        if let Some(unit) = mech {
            (
                unit.definition().name.as_str(),
                unit.definition().tons,
                unit.definition().reference.as_str(),
                unit.template_speed(),
                unit.pilot(),
                unit.pilot_injuries(),
                unit.experience_settings(),
            )
        } else {
            let unit = vehicle.unwrap();
            (
                unit.definition().name.as_str(),
                unit.definition().tons,
                unit.definition().reference.as_str(),
                unit.template_speed(),
                unit.pilot(),
                unit.pilot_injuries(),
                unit.experience_settings(),
            )
        };
    let position = mech
        .and_then(|unit| unit.position)
        .or_else(|| vehicle.and_then(BattleVehicle::retained_position));
    let altitude = position.and_then(|position| {
        let tile = world
            .btech
            .maps()
            .get(&position.map)?
            .base_hex(i64::from(position.x), i64::from(position.y))
            .ok()?;
        Some(mech.map_or_else(
            || vehicle.unwrap().altitude(tile),
            |unit| unit.altitude(tile),
        ))
    });
    let float = |value: f64| Some(format!("{value:.2}"));
    let integer = |value: i64| Some(value.to_string());
    Ok(match field {
        "mapindex" => integer(scanner.position.map_or(-1, |p| p.map.0)),
        "id" => scanner
            .label
            .or_else(|| mech.and_then(|unit| unit.battlefield_label.clone()))
            .or_else(|| vehicle.and_then(|unit| unit.battlefield_label.clone())),
        "unit_era" | "unit_tro" => {
            let attributes = mech.map_or_else(
                || &vehicle.unwrap().definition().attributes,
                |unit| &unit.definition().attributes,
            );
            Some(super::unit_identity::metadata(attributes, field).into())
        }
        "displayname" => Some(super::display_name::display_name(world, id)?.into()),
        "bv" => float(
            super::battle_value::configured(world, id, super::SpeedPolicy::configured(config))?
                .total,
        ),
        "MechPrefs" => Some(super::field_bits::format(i64::from(
            super::preference_fields::read(world, id)?,
        ))),
        "mechdamage" => Some(super::unit_damage_field(world, id)?),
        "critstatus" => Some(super::field_bits::format(i64::from(
            super::status_fields::primary_criticals(world, id)?,
        ))),
        "status" => Some(super::field_bits::format(i64::from(
            super::status_fields::primary_status(world, id)?,
        ))),
        "status2" => Some(super::field_bits::format(i64::from(
            super::status_fields::secondary_status(world, id)?,
        ))),
        "tankcritstatus" => Some(super::field_bits::format(i64::from(
            super::status_fields::vehicle_criticals(world, id)?,
        ))),
        "critstatus2" => Some(super::field_bits::format(i64::from(
            super::status_fields::secondary_criticals(world, id)?,
        ))),
        "StaggerDamage" => integer(mech.map_or(0, |unit| i64::from(unit.stagger().action_damage))),
        // Weapon admission derives physical arcs.
        "unusablearcs" => integer(0),
        "hsengoverride" => integer(i64::from(super::engine_sink_override::read(
            mech.map_or_else(
                || &vehicle.unwrap().definition().attributes,
                |unit| &unit.definition().attributes,
            ),
        )?)),
        "mechname" => Some(definition.into()),
        "mechref" => Some(reference.into()),
        "maxspeed" => float(mech.map_or_else(
            || vehicle.unwrap().maximum_speed(),
            |u| u.mobility().maximum_speed,
        )),
        "templatesp" => float(template_speed),
        "units_killed" => integer(i64::from(super::kill_counters::read(world, id)?)),
        "damage_taken" => integer(i64::from(super::damage_counters::read(world, id)?.taken)),
        "damage_inflicted" => integer(i64::from(
            super::damage_counters::read(world, id)?.inflicted,
        )),
        "shots_fired" => integer(i64::from(super::shot_counters::read(world, id)?.fired)),
        "shots_hit" => integer(i64::from(super::shot_counters::read(world, id)?.hit)),
        "shots_missed" => integer(i64::from(super::shot_counters::read(world, id)?.missed)),
        "basewalkspeed" => integer(i64::from(
            super::base_movement_fields::read(world, id)?.walk,
        )),
        "baserunspeed" => integer(i64::from(super::base_movement_fields::read(world, id)?.run)),
        "pilotnum" => integer(pilot.map_or(-1, |pilot| pilot.0)),
        "last_startup" => {
            integer(mech.map_or_else(|| vehicle.unwrap().last_startup, |unit| unit.last_startup))
        }
        "pilotdam" => integer(i64::from(injuries)),
        "xpmod" => float(experience.multiplier),
        "speed" => float(scanner.speed),
        "heading" => scanner.heading.map(|heading| (heading as i16).to_string()),
        "target" => integer(scanner.selected.map_or(-1, |target| target.0)),
        "team" => integer(i64::from(scanner.signature.team)),
        "towing" => integer(world.btech.tows().get(&id).map_or(-1, |target| target.0)),
        "tons" => integer(i64::from(tons)),
        "realweight" => integer(i64::from(if let Some(unit) = mech {
            unit.effective_mass()?
        } else {
            vehicle.unwrap().effective_mass()?
        })),
        "fx" => scanner.point.and_then(|point| float(point.x * 322.5)),
        "fy" => scanner.point.and_then(|point| float(point.y * 322.5)),
        "fz" => altitude.and_then(|height| float(height * 64.5)),
        "x" => position.map(|p| p.x.to_string()),
        "y" => position.map(|p| p.y.to_string()),
        "z" => altitude.map(|z| (z as i32).to_string()),
        "heat" => float(mech.map_or(0.0, |unit| unit.sampled_heat_rates().production)),
        "dissheat" => float(mech.map_or(0.0, |unit| unit.sampled_heat_rates().dissipation)),
        "overheat" => float(mech.map_or(0.0, |unit| unit.heat().excess)),
        "disabled_hs" => integer(i64::from(
            mech.map_or(0, |unit| unit.heat_cutoff().disabled),
        )),
        "heatsinks" => integer(i64::from(if let Some(unit) = mech {
            unit.cooling_capacity()
        } else {
            vehicle.unwrap().cooling_capacity()?
        })),
        "centdist" | "centbearing" => {
            if let (Some(position), Some(point)) = (position, scanner.point) {
                let (range, bearing) = super::find_center::measurement(
                    point,
                    BattleHexCoordinate {
                        x: i32::from(position.x),
                        y: i32::from(position.y),
                    },
                )?;
                if field == "centdist" {
                    float(range)
                } else {
                    integer(i64::from(bearing))
                }
            } else {
                None
            }
        }
        "sensors" => Some(
            mech.map_or_else(
                || vehicle.unwrap().sensor_selection(),
                BattleUnit::sensor_selection,
            )
            .field_text(),
        ),
        "cargospace" => integer(i64::from(super::load::cargo_capacity(world, id))),
        "C3iNetworkSize" => integer(
            super::command_network::members(world, id)?
                .into_iter()
                .filter(|peer| *peer != id)
                .count() as i64,
        ),
        "jumpheading" => integer(i64::from(mech.map_or(0, |unit| unit.last_jump.heading))),
        "jumplength" => integer(i64::from(mech.map_or(0, |unit| unit.last_jump.length))),
        "numseen" => integer(super::contacts::enemy_count(world, id)? as i64),
        "turret0" | "turret1" | "turret2" => {
            let slot = usize::from(field.as_bytes()[6] - b'0');
            integer(super::cockpit_links::links(world, id).0[slot].0)
        }
        "targcomp" => integer(i64::from(super::targeting_mode::mode(world, id))),
        "lrsrange" | "scanrange" | "tacrange" => {
            let ranges = mech.map_or_else(
                || vehicle.unwrap().sensor_ranges(),
                BattleUnit::sensor_ranges,
            );
            integer(i64::from(match field {
                "lrsrange" => ranges.long_range,
                "scanrange" => ranges.scan,
                _ => ranges.tactical,
            }))
        }
        "radiorange" | "radiotype" => {
            let radio = super::unit_radio_capabilities(world, id)?;
            integer(if field == "radiorange" {
                i64::from(radio.range)
            } else {
                i64::from(radio.configuration())
            })
        }
        "fuel" | "fuel_orig" => vehicle.and_then(BattleVehicle::vtol_fuel).and_then(|fuel| {
            integer(if field == "fuel" {
                fuel.remaining()
            } else {
                i64::from(fuel.capacity())
            })
        }),
        "cocoon" => integer(
            match super::orbital_drop_state::current(world, id).map(|drop| drop.protection()) {
                Some(BattleDropProtection::Cocoon { integrity }) => i64::from(integrity),
                Some(BattleDropProtection::JumpJets) => -1,
                _ => 0,
            },
        ),
        "maxjumpspeed" => float(super::jump_thrust::speed(world, id)?),
        "mechtype" => Some(
            if mech.is_some() {
                "Mech"
            } else if vehicle.unwrap().definition().is_vtol() {
                "VTOL"
            } else {
                "Vehicle"
            }
            .into(),
        ),
        "mechmovetype" => Some(
            if let Some(unit) = mech {
                if unit.chassis() == BattleMechChassis::Quad {
                    "Quad"
                } else {
                    "Biped"
                }
            } else {
                match vehicle.unwrap().definition().movement {
                    BattleVehicleMovement::Tracked => "Track",
                    BattleVehicleMovement::Wheeled => "Wheel",
                    BattleVehicleMovement::Hover => "Hover",
                    BattleVehicleMovement::Stationary => "None",
                    BattleVehicleMovement::Vtol => "VTOL",
                }
            }
            .into(),
        ),
        _ => None,
    })
}

/// Capture filtered fields and publish their literal values using the shared wizard layout.
pub fn view_unit_fields_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    arguments: &str,
) -> Result<BattleUnitFieldReport> {
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let world = scripts.world();
        admission(&world, actor, id)?;
        let (columns, filter) = super::field_report::options(arguments);
        let fields = FIELDS
            .iter()
            .copied()
            .filter(|name| name.to_ascii_lowercase().starts_with(&filter))
            .map(|name| {
                Ok(BattleUnitField {
                    name,
                    value: value(&world, config, id, name)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let text = super::field_report::render(
            &world.objects[&id].name,
            "MECH",
            columns,
            fields
                .iter()
                .map(|field| (field.name, field.value.as_deref())),
        );
        let report = BattleUnitFieldReport {
            unit: id,
            columns,
            fields,
            text,
        };
        drop(world);
        for line in report.text.lines() {
            super::notify_message(
                scripts,
                BattleMessageTarget::Player(actor),
                &crate::text::escape(line),
            )?;
        }
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })();
    if result.is_err() {
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native inspection selects the wizard's current unit without requiring a cockpit assignment.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|object| object.location)
            .context("Player has no location")?;
        view_unit_fields_action(ctx.scripts, ctx.config, ctx.player, id, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Apply a named administrative edit through the existing unit services and validate the candidate.
/// The field catalogue is independent of cockpit control and allows off-map administration.
pub fn set_unit_field_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    id: ObjectId,
    field: &str,
    value: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        admission(&before, actor, id)?;
        ensure!(
            ![
                "mapindex",
                "towing",
                "disabled_hs",
                "heatsinks",
                "C3iNetworkSize",
                "StaggerDamage",
                "cocoon",
                "unusablearcs",
                "id",
                "centdist",
                "centbearing",
                "sensors",
                "bv",
                "numseen"
            ]
            .iter()
            .any(|name| field.eq_ignore_ascii_case(name)),
            "Field {field} is read-only"
        );
        match field.to_ascii_lowercase().as_str() {
            "status" | "critstatus" | "mechtype" => {
                bail!("Field {field} is inspectable but direct writes are not supported.");
            }
            "mechdamage" => super::damage_application::set(
                &mut scripts.world_mut(),
                id,
                value,
                config.battletech.tsm_tow_bonus != 0,
            )?,
            "jumpheading" | "jumplength" => super::jump_fields::set(
                &mut scripts.world_mut(),
                id,
                field.eq_ignore_ascii_case("jumpheading"),
                value,
            )?,
            "mechmovetype" => super::construction_fields::set_movement(
                &mut scripts.world_mut(),
                id,
                value,
                config.battletech.tsm_tow_bonus != 0,
            )?,
            "tons" => super::construction_fields::set_tonnage(
                &mut scripts.world_mut(),
                id,
                value,
                config.battletech.tsm_tow_bonus != 0,
            )?,
            "realweight" => super::live_mass::set(
                &mut scripts.world_mut(),
                id,
                value,
                config.battletech.tsm_tow_bonus != 0,
            )?,
            "tankcritstatus" => {
                let bits = super::field_bits::parse(value.trim())? as u32;
                super::status_edits::vehicle_criticals(&mut scripts.world_mut(), id, bits)?;
            }
            "critstatus2" => {
                let bits = super::field_bits::parse(value.trim())? as u32;
                super::status_edits::secondary_criticals(&mut scripts.world_mut(), id, bits)?;
            }
            "status2" => {
                let bits = super::field_bits::parse(value.trim())? as u32;
                super::status_edits::secondary(&mut scripts.world_mut(), id, bits)?;
            }
            "pilotdam" => {
                let notice = super::pilot_injury::set_administrative_injuries(
                    &mut scripts.world_mut(),
                    id,
                    value,
                )?;
                if let Some(notice) = notice {
                    super::notify_unit(scripts, notice)?;
                }
            }
            "maxjumpspeed" => super::jump_thrust::set(&mut scripts.world_mut(), id, value)?,
            "maxspeed" => {
                let speed = super::propulsion::parse(value)?;
                let mut world = scripts.world_mut();
                if let Some(unit) =
                    std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id)
                {
                    unit.propulsion.set(speed);
                    ensure!(
                        unit.mobility().maximum_speed == speed,
                        "Material damage prevents that maximum speed"
                    );
                } else {
                    let unit = std::sync::Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&id)
                        .unwrap();
                    unit.propulsion.set(speed);
                    ensure!(
                        unit.maximum_speed() == speed,
                        "Material damage or chassis prevents that maximum speed"
                    );
                }
            }
            "templatesp" => {
                let speed = super::template_speed::parse(value)?;
                let mut world = scripts.world_mut();
                if let Some(unit) =
                    std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id)
                {
                    unit.set_template_speed(speed);
                } else {
                    std::sync::Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&id)
                        .unwrap()
                        .set_template_speed(speed);
                }
            }
            "fx" | "fy" | "fz" => super::scenario_position::set_precise_field_action(
                scripts,
                id,
                &field.to_ascii_lowercase(),
                value,
            )?,
            "x" | "y" | "z" => super::scenario_position::set_field_action(
                scripts,
                config,
                actor,
                id,
                &field.to_ascii_lowercase(),
                value,
            )?,
            "heading" | "speed" => super::motion_fields::set(
                &mut scripts.world_mut(),
                id,
                &field.to_ascii_lowercase(),
                value,
            )?,
            "fuel_orig" => super::vtol_fuel::set_original_capacity(
                &mut scripts.world_mut(),
                config,
                id,
                value,
            )?,
            "cargospace" => super::load::set_cargo_capacity(
                &mut scripts.world_mut(),
                id,
                value,
                config.battletech.tsm_tow_bonus != 0,
            )?,
            "mechprefs" => {
                let bits = super::field_bits::parse(value.trim())? as u32;
                super::preference_fields::set(&mut scripts.world_mut(), id, bits)?;
            }
            "last_startup" => {
                let timestamp = value
                    .trim()
                    .parse::<i64>()
                    .context("Expected a signed Unix timestamp")?;
                let mut world = scripts.world_mut();
                if let Some(unit) =
                    std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id)
                {
                    unit.last_startup = timestamp;
                } else {
                    std::sync::Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&id)
                        .unwrap()
                        .last_startup = timestamp;
                }
            }
            "units_killed" => {
                super::kill_counters::set(&mut scripts.world_mut(), id, value)?;
            }
            "damage_taken" | "damage_inflicted" => {
                super::damage_counters::set(
                    &mut scripts.world_mut(),
                    id,
                    &field.to_ascii_lowercase(),
                    value,
                )?;
            }
            "shots_fired" | "shots_hit" | "shots_missed" => {
                super::shot_counters::set(
                    &mut scripts.world_mut(),
                    id,
                    &field.to_ascii_lowercase(),
                    value,
                )?;
            }
            "basewalkspeed" | "baserunspeed" => {
                super::base_movement_fields::set(
                    &mut scripts.world_mut(),
                    id,
                    field.eq_ignore_ascii_case("basewalkspeed"),
                    value,
                )?;
            }
            "hsengoverride" => {
                let value = super::engine_sink_override::parse(value)?;
                let mut world = scripts.world_mut();
                if let Some(unit) =
                    std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id)
                {
                    unit.set_engine_sink_override(value);
                } else {
                    std::sync::Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&id)
                        .unwrap()
                        .set_engine_sink_override(value);
                }
            }
            "pilotnum" | "target" => {
                let reference = value
                    .trim()
                    .parse::<i64>()
                    .context("Expected a numeric object reference")?;
                ensure!(reference >= -1, "Use -1 to clear an object reference");
                let reference = (reference != -1).then_some(ObjectId(reference));
                if field.eq_ignore_ascii_case("pilotnum") {
                    super::crew::set_administrative_pilot(&mut scripts.world_mut(), id, reference)?;
                } else {
                    super::targeting::set_administrative_target(
                        &mut scripts.world_mut(),
                        id,
                        reference,
                    )?;
                }
            }
            "displayname" => super::display_name::set(&mut scripts.world_mut(), id, value)?,
            "mechname" | "mechref" | "unit_era" | "unit_tro" => {
                let identity = match field.to_ascii_lowercase().as_str() {
                    "mechname" => IdentityField::Name,
                    "mechref" => IdentityField::Reference,
                    "unit_era" => IdentityField::Era,
                    _ => IdentityField::Tro,
                };
                super::unit_identity::set(&mut scripts.world_mut(), id, identity, value)?;
            }
            "targcomp" | "tacrange" | "lrsrange" | "scanrange" | "radiorange" | "radiotype" => {
                let sensor_hits = before
                    .btech
                    .constructed_units()
                    .get(&id)
                    .map_or(0, |unit| unit.system_hits(BattleSystem::Sensors));
                let mut world = scripts.world_mut();
                let hardware = if let Some(unit) =
                    std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id)
                {
                    &mut unit.hardware
                } else {
                    &mut std::sync::Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&id)
                        .unwrap()
                        .hardware
                };
                hardware.set(&field.to_ascii_lowercase(), value, sensor_hits)?;
            }
            "heat" | "dissheat" | "overheat" => {
                std::sync::Arc::make_mut(&mut scripts.world_mut().btech.constructed)
                    .get_mut(&id)
                    .context("This unit does not use Mech thermal state")?
                    .set_thermal_field(&field.to_ascii_lowercase(), value)?;
            }
            "turret0" | "turret1" | "turret2" => {
                let slot = usize::from(field.as_bytes()[6] - b'0');
                let destination = ObjectId(
                    value
                        .trim()
                        .parse::<i64>()
                        .context("Expected a numeric object reference")?,
                );
                super::cockpit_links::set(&mut scripts.world_mut(), id, slot, destination)?;
            }
            "team" => {
                let mut signature = super::scanner::scanner_unit(&before, id).unwrap().signature;
                signature.team = value
                    .trim()
                    .parse::<i32>()
                    .context("Expected a signed 32-bit integer")?;
                super::set_sensor_signature(&mut scripts.world_mut(), id, signature)?;
            }
            "fuel" => {
                let amount = value
                    .trim()
                    .parse::<u32>()
                    .context("Expected a nonnegative fuel amount")?;
                super::set_vtol_fuel(&mut scripts.world_mut(), config, actor, id, amount)?;
            }
            "xpmod" => {
                let mut settings = before
                    .btech
                    .constructed_units()
                    .get(&id)
                    .map(BattleUnit::experience_settings)
                    .or_else(|| {
                        before
                            .btech
                            .vehicles()
                            .get(&id)
                            .map(BattleVehicle::experience_settings)
                    })
                    .unwrap();
                let multiplier = value
                    .trim()
                    .parse::<f32>()
                    .context("Expected a finite floating-point value")?;
                settings.multiplier = f64::from(multiplier);
                super::set_unit_experience(&mut scripts.world_mut(), id, settings)?;
            }
            _ => bail!("Error: No matching field for this BTech type was found."),
        }
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native named edits retain the full value following the first field-name token.
pub(crate) fn set_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let (field, value) = super::field_report::assignment(&input.args)?;
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|object| object.location)
            .context("Player has no location")?;
        set_unit_field_action(ctx.scripts, ctx.config, ctx.player, id, field, value)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
