//! Durable optical observations with explicit acquisition, retention and loss transitions.
use super::{
    BattleScanTarget, BattleSensorMode, BattleSensorScan, BattleSensorScanReport, BattleUnit,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Sensor roles that could observe the target at its most recent contact update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleContact {
    /// Whether the last retained observation identified the target through clear terrain.
    #[serde(default)]
    pub identified: bool,
    pub primary: bool,
    pub secondary: bool,
}

/// Count acquired enemy contacts without rerunning detection or changing saved observations.
pub(super) fn enemy_count(world: &World, id: ObjectId) -> Result<usize> {
    let observer = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    Ok(observer
        .contacts
        .iter()
        .filter(|(target, contact)| {
            (contact.primary || contact.secondary)
                && super::scanner::scanner_unit(world, **target)
                    .is_some_and(|unit| unit.signature.team != observer.signature.team)
        })
        .count())
}

/// Live sensor roles returned by a read-only contact display.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleContactSensors {
    pub primary: bool,
    pub secondary: bool,
}

/// Change in ownership of an optical contact; retained observations do not reroll acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleContactTransition {
    Unseen,
    Acquired,
    Retained,
    Lost,
}

/// Scenario facts until skills, teams, target illumination and map sensor-disable bits are owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleContactRules {
    pub target: BattleScanTarget,
    pub perception: i16,
    pub visual_disabled: bool,
    pub amplification_disabled: bool,
    /// False refreshes existing observations without trying to acquire unseen targets.
    pub acquire: bool,
}

/// A committed candidate observation and the acquisition attempts, when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleContactUpdate {
    pub transition: BattleContactTransition,
    pub contact: Option<BattleContact>,
    pub scan: Option<BattleSensorScanReport>,
}

impl BattleUnit {
    /// Last observed contacts, which must be refreshed before being used as current visibility.
    pub fn contacts(&self) -> &BTreeMap<ObjectId, BattleContact> {
        &self.contacts
    }
}

impl super::BattleVehicle {
    /// Last observed contacts; refresh their eligibility before using them as current visibility.
    pub fn contacts(&self) -> &BTreeMap<ObjectId, BattleContact> {
        &self.contacts
    }
}

/// Refresh one target using the observer's saved active modes and atomically owned dice.
pub fn update_optical_contact(
    world: &mut World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleContactRules,
) -> Result<BattleContactUpdate> {
    ensure!(observer != target, "A unit cannot acquire itself");
    let (power, pair, known) = if let Some(vehicle) = world.btech.vehicles().get(&observer) {
        (
            vehicle.power(),
            vehicle.sensor_selection().active,
            vehicle.contacts.contains_key(&target),
        )
    } else {
        let unit = world
            .btech
            .constructed_units()
            .get(&observer)
            .context("Unit construction state is unavailable")?;
        (
            unit.power(),
            unit.sensor_selection().active,
            unit.contacts.contains_key(&target),
        )
    };
    ensure!(power == super::BattlePower::Running, "Start the unit first");
    let query = |sensor| {
        super::map_optical_contact(
            world,
            observer,
            target,
            sensor,
            rules.target.lit,
            match sensor {
                BattleSensorMode::Infrared
                | BattleSensorMode::Seismic
                | BattleSensorMode::Electromagnetic
                | BattleSensorMode::Radar
                | BattleSensorMode::BeagleProbe
                | BattleSensorMode::LightProbe
                | BattleSensorMode::BloodhoundProbe => false,
                BattleSensorMode::Visual => rules.visual_disabled,
                BattleSensorMode::LightAmplification => rules.amplification_disabled,
            },
        )
    };
    let primary = query(pair.primary)?.eligible;
    let secondary = if pair.primary == pair.secondary {
        primary
    } else {
        query(pair.secondary)?.eligible
    };
    let mut contact = (primary || secondary).then_some(BattleContact {
        primary,
        secondary,
        identified: !super::unit_terrain_los(world, observer, target)?.blocked,
    });
    let mut scan = None;
    if !known {
        if contact.is_some() && rules.acquire {
            let attempt = super::scan_optical_target(
                world,
                observer,
                target,
                BattleSensorScan {
                    primary: pair.primary,
                    secondary: pair.secondary,
                    target: rules.target,
                    perception: rules.perception,
                    visual_disabled: rules.visual_disabled,
                    amplification_disabled: rules.amplification_disabled,
                },
            )?;
            if attempt.detected_by.is_none() {
                contact = None;
            }
            scan = Some(attempt);
        } else {
            contact = None;
        }
    }
    let transition = match (known, contact.is_some()) {
        (false, false) => BattleContactTransition::Unseen,
        (false, true) => BattleContactTransition::Acquired,
        (true, true) => BattleContactTransition::Retained,
        (true, false) => BattleContactTransition::Lost,
    };
    if contact.is_none() {
        for station in Arc::make_mut(&mut world.btech.gunner_stations).values_mut() {
            if station.parent == observer && station.target == target {
                station.set_target_selection(None);
            }
        }
    }
    let contacts = if world.btech.vehicles().contains_key(&observer) {
        let unit = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&observer)
            .expect("validated observer");
        if contact.is_none() && unit.target_lock().is_some_and(|lock| lock.target == target) {
            unit.target_lock = None;
        }
        &mut unit.contacts
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&observer)
            .expect("validated observer");
        if contact.is_none() && unit.target_lock().is_some_and(|lock| lock.target == target) {
            unit.target_lock = None;
        }
        &mut unit.contacts
    };
    if let Some(contact) = contact {
        contacts.insert(target, contact);
    } else {
        contacts.remove(&target);
    }
    Ok(BattleContactUpdate {
        transition,
        contact,
        scan,
    })
}

/// Map membership changes invalidate both outgoing and incoming observations.
pub(super) fn forget_unit(world: &mut World, id: ObjectId) {
    forget_state(&mut world.btech, id);
}

/// Forget a battlefield identity without changing game-object ownership.
pub(super) fn forget_state(state: &mut super::BtechState, id: ObjectId) {
    invalidate_observations(state, id, false);
}

/// Scenario relocation invalidates incoming locks while preserving the moved unit's selection.
pub(super) fn relocate_observations(world: &mut World, id: ObjectId) -> Vec<super::BattleNotice> {
    let notices = super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|observer| *observer != id)
        .filter(|observer| {
            super::scanner::scanner_unit(world, *observer)
                .is_some_and(|unit| unit.selected == Some(id))
        })
        .map(|unit| super::BattleNotice {
            unit,
            text: "Weapon system reports the lock has been lost.".into(),
        })
        .collect();
    invalidate_observations(&mut world.btech, id, true);
    notices
}

/// Share acquisition invalidation while allowing same-map edits to keep outgoing selections.
fn invalidate_observations(state: &mut super::BtechState, id: ObjectId, preserve_selection: bool) {
    for station in Arc::make_mut(&mut state.gunner_stations).values_mut() {
        if (station.parent == id && !preserve_selection)
            || (station.parent != id && station.target == id)
        {
            station.set_target_selection(None);
        }
    }
    for (observer, vehicle) in Arc::make_mut(&mut state.vehicles).iter_mut() {
        if *observer == id {
            vehicle.contacts.clear();
            if !preserve_selection {
                vehicle.target_lock = None;
            }
        } else {
            vehicle.contacts.remove(&id);
            if vehicle.target_lock().is_some_and(|lock| lock.target == id) {
                vehicle.target_lock = None;
            }
        }
    }
    for (observer, unit) in Arc::make_mut(&mut state.constructed).iter_mut() {
        if *observer == id {
            unit.contacts.clear();
            if !preserve_selection {
                unit.target_lock = None;
            }
        } else {
            unit.contacts.remove(&id);
            if unit.target_lock().is_some_and(|lock| lock.target == id) {
                unit.target_lock = None;
            }
        }
    }
}

/// Current, eligible information for an acquired contact; excludes hidden future dice and damage internals.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleContactView {
    /// Battlefield label, lowercase only for identified friendly contacts.
    pub label: String,
    pub coordinate: super::BattleHexCoordinate,
    pub elevation: i32,
    /// Plain compact row shared by native C1/C2/C3 and Lua.
    pub short_text: String,
    /// Plain multiline C0 report, shared with Lua.
    pub verbose_text: String,
    /// Current clear-terrain identification, independent of retained acquisition.
    pub identified: bool,
    /// General direction relative to the observer torso, not a mount firing guarantee.
    pub weapon_arc: super::BattleContactArc,
    /// Current eligibility of each sensor role for this already acquired target.
    pub sensors: BattleContactSensors,
    /// Five visible status columns; terrain-obscured identities receive blanks.
    pub status: String,
    pub target: ObjectId,
    pub name: String,
    pub friendly: bool,
    pub range: super::BattleRange,
    pub heading: f64,
    pub speed: f64,
}

impl BattleContactView {
    /// Style the compact row with selection priority; all row content remains literal.
    pub fn styled_short_text(&self, selected: bool) -> String {
        let color = if selected {
            Some("red")
        } else if !self.friendly {
            Some("yellow")
        } else {
            None
        };
        styled_row(&self.short_text, color)
    }

    /// Describe a visible unit using current condition and observer-relative contact geometry.
    fn verbose_text(&self, world: &World, vehicle_observer: bool) -> Result<String> {
        let mech = world.btech.constructed_units().get(&self.target);
        let vehicle = world.btech.vehicles().get(&self.target);
        let tons = mech.map_or_else(
            || vehicle.expect("validated target").definition().tons,
            |unit| unit.definition().tons,
        );
        let heat = mech.map_or(0.0, |unit| unit.heat().excess);
        let facts =
            super::scanner::scanner_unit(world, self.target).context("Contact disappeared")?;
        let arc = self.weapon_arc.description(vehicle_observer);
        let mut lines = vec![
            format!(
                "[{}] {:<17}  Tonnage: {}",
                self.label,
                crate::text::plain(&self.name),
                tons
            ),
            format!(
                "      Range: {:.1} hex\tBearing: {} degrees",
                self.range.spatial,
                self.range
                    .bearing
                    .unwrap_or(180.0)
                    .round()
                    .rem_euclid(360.0) as u16
            ),
            format!(
                "      Speed: {:.1} KPH\tHeading: {} degrees",
                self.speed,
                self.heading.trunc().rem_euclid(360.0) as u16
            ),
            format!(
                "      X, Y: {:>3}, {:>3} \tHeat: {:.0} deg C.",
                self.coordinate.x, self.coordinate.y, heat
            ),
            format!("      Movement Type: {}", movement_type(world, self.target)),
            format!("      Mech is in {arc} Arc"),
        ];
        if facts.destroyed {
            lines.push("      Mech Destroyed".into());
        }
        if facts.power != super::BattlePower::Running {
            lines.push("      Mech Shutdown".into());
        }
        if mech.is_some_and(|unit| unit.posture() == super::BattlePosture::Prone) {
            lines.push("      Mech has Fallen!".into());
        }
        if mech.is_some_and(|unit| unit.hull_down().pending.is_some()) {
            lines.push("      Mech is changing hull-down mode".into());
        }
        if mech.is_some_and(|unit| unit.hull_down().active) {
            lines.push("      Mech is hull-down".into());
        }
        if let Some(flight) = mech.and_then(|unit| unit.flight()) {
            lines.push(format!(
                "      Mech is Jumping!\tJump Heading: {}",
                flight.path().heading()?
            ));
        }
        lines.push(" ".into());
        Ok(lines.join("\r\n"))
    }

    /// Render compact contact data with the target movement marker and no embedded styles.
    fn compact_text(&self, movement: &str) -> String {
        let name: String = crate::text::plain(&self.name).chars().take(12).collect();
        format!(
            "{}{}{}[{}]{} {:<12} x:{:>3} y:{:>3} z:{:>3} r:{:>4.1} b:{:>3} s:{:>5.1} h:{:>3} S:{}",
            if self.sensors.primary { 'P' } else { ' ' },
            if self.sensors.secondary { 'S' } else { ' ' },
            self.weapon_arc.symbol(),
            self.label,
            movement.chars().next().unwrap_or('U'),
            name,
            self.coordinate.x,
            self.coordinate.y,
            self.elevation,
            self.range.spatial,
            self.range
                .bearing
                .unwrap_or(180.0)
                .round()
                .rem_euclid(360.0) as u16,
            self.speed,
            self.heading.trunc().rem_euclid(360.0) as u16,
            self.status
        )
    }
}

/// Validate the observer once, whether reading one contact or the complete display.
fn contact_observer(world: &World, observer: ObjectId) -> Result<super::scanner::ScannerUnit<'_>> {
    let unit =
        super::scanner::scanner_unit(world, observer).context("Enter a constructed unit first")?;
    ensure!(
        unit.power == super::BattlePower::Running,
        "Start the unit first"
    );
    unit.position.context("Unit is not on a battlefield")?;
    Ok(unit)
}

/// Read-only facts needed by autonomous observation and the ordinary contact display.
///
/// This deliberately stops before labels, names, arcs, condition strings, and
/// compact/verbose rendering.  `visible_contacts` still adds those fields for
/// callers that request the human-facing view, while autopilot can consume the
/// same authority and sensor checks without constructing presentation text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BattleContactFacts {
    pub(crate) target: ObjectId,
    pub(crate) position: super::BattlePosition,
    pub(crate) identified: bool,
    pub(crate) friendly: bool,
    pub(crate) sensors: BattleContactSensors,
    pub(crate) known_destroyed: bool,
    pub(crate) range: super::BattleRange,
}

/// Build an acquired contact or a clairvoyant observation using current mixed-class geometry.
fn contact_facts_with_unit(
    world: &World,
    observer: ObjectId,
    unit: &super::scanner::ScannerUnit<'_>,
    target: ObjectId,
    illumination: Option<&super::searchlight::IlluminationContext<'_>>,
) -> Result<Option<BattleContactFacts>> {
    if observer == target || (!unit.visibility.clairvoyant && !unit.contacts.contains_key(&target))
    {
        return Ok(None);
    }
    let Some(other) = super::scanner::scanner_unit(world, target) else {
        return Ok(None);
    };
    let position = unit.position.expect("validated observer placement");
    if !other.position.is_some_and(|p| p.map == position.map)
        || world
            .objects
            .get(&target)
            .is_none_or(|object| object.flags.contains(crate::Flag::Going))
    {
        return Ok(None);
    }
    let pair = unit.pair;
    // Geometry is immutable during this read. Trace it once for both sensor
    // roles and identification; do not retain it across movement or a shot.
    let geometry = if unit.visibility.clairvoyant {
        None
    } else {
        super::los::unit_terrain_geometry(world, observer, target).ok()
    };
    let eligible = |sensor| {
        super::sensors::map_optical_contact_prepared(
            world,
            observer,
            target,
            sensor,
            other.signature.illuminated,
            false,
            geometry,
            illumination,
        )
        .is_ok_and(|report| report.eligible)
    };
    let primary = !unit.visibility.clairvoyant && eligible(pair.primary);
    let secondary = if pair.primary == pair.secondary {
        primary
    } else {
        !unit.visibility.clairvoyant && eligible(pair.secondary)
    };
    if !unit.visibility.clairvoyant && !primary && !secondary {
        return Ok(None);
    }
    let identified = unit.visibility.clairvoyant
        || match geometry {
            Some((terrain, _)) => !terrain.blocked,
            None => !super::unit_terrain_los(world, observer, target)?.blocked,
        };
    let range = match geometry {
        Some((_, range)) => range,
        None => super::unit_range(world, observer, target)?,
    };
    let friendly = identified && unit.signature.team == other.signature.team;
    let position = other.position.context("Contact has no position")?;
    Ok(Some(BattleContactFacts {
        target,
        position,
        identified,
        friendly,
        sensors: BattleContactSensors { primary, secondary },
        // contact_status hides all condition columns behind the same clear
        // terrain rule.  The target's authoritative destruction bit plus the
        // already-computed identification result is the equivalent fact,
        // without allocating the five-column status string.
        known_destroyed: identified && other.destroyed,
        range,
    }))
}

/// Read the shared facts for one observer/target pair after observer admission.
pub(crate) fn contact_facts(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<Option<BattleContactFacts>> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Contacts,
    );
    let unit = contact_observer(world, observer)?;
    contact_facts_with_unit(world, observer, &unit, target, None)
}

/// Build an acquired contact or a clairvoyant observation using current mixed-class geometry.
fn contact_view(
    world: &World,
    observer: ObjectId,
    unit: &super::scanner::ScannerUnit<'_>,
    target: ObjectId,
) -> Result<Option<BattleContactView>> {
    let Some(facts) = contact_facts_with_unit(world, observer, unit, target, None)? else {
        return Ok(None);
    };
    let other = super::scanner::scanner_unit(world, target).context("Contact disappeared")?;
    let heading = unit.heading.context("Observer has no motion state")?;
    let bearing = facts.range.bearing.unwrap_or(180.0);
    let mut label = other.label().context("Contact has no battlefield label")?;
    if facts.friendly {
        label.make_ascii_lowercase();
    }
    let mut view = BattleContactView {
        label,
        coordinate: super::BattleHexCoordinate {
            x: i32::from(facts.position.x),
            y: i32::from(facts.position.y),
        },
        elevation: super::unit_elevation(world, target)?.context("Contact has no elevation")?,
        short_text: String::new(),
        verbose_text: String::new(),
        identified: facts.identified,
        weapon_arc: unit.facing.contact_arc(
            if unit.vehicle {
                heading.trunc()
            } else {
                heading
            },
            if unit.vehicle {
                bearing.round()
            } else {
                bearing
            },
        )?,
        sensors: facts.sensors,
        status: super::contact_status::contact_status(world, observer, target)?,
        target,
        name: if facts.identified {
            other.name.to_owned()
        } else {
            "something".into()
        },
        friendly: facts.friendly,
        range: facts.range,
        heading: other.travel_heading.context("Contact has no motion")?,
        speed: other.speed,
    };
    view.short_text = view.compact_text(movement_type(world, target));
    view.verbose_text = view.verbose_text(world, unit.vehicle)?;
    Ok(Some(view))
}

/// Contact movement labels retain the game's spelling and stationary fallback.
pub(super) fn movement_type(world: &World, target: ObjectId) -> &'static str {
    let Some(vehicle) = world.btech.vehicles().get(&target) else {
        return "BIPED";
    };
    match vehicle.definition().movement {
        super::BattleVehicleMovement::Tracked => "TRACKED",
        super::BattleVehicleMovement::Wheeled => "WHEELED",
        super::BattleVehicleMovement::Hover => "HOVER",
        super::BattleVehicleMovement::Stationary => "Unknown",
        super::BattleVehicleMovement::Vtol => "VTOL",
    }
}

/// Inspect one visible contact without scanning, sorting or allocating other contacts.
/// Clairvoyants can inspect unacquired or invisible units; invalid observers return an error.
pub fn visible_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<Option<BattleContactView>> {
    contact_view(world, observer, &contact_observer(world, observer)?, target)
}

/// List eligible acquired contacts, or all same-map units for a clairvoyant observer.
/// This read-only display never attempts acquisition or updates the saved observation.
pub fn visible_contacts(world: &World, observer: ObjectId) -> Result<Vec<BattleContactView>> {
    let unit = contact_observer(world, observer)?;
    let mut views = Vec::new();
    let targets: Vec<_> = if unit.visibility.clairvoyant {
        super::map_slots::all_unit_order(world, unit.position.unwrap().map)?
    } else {
        unit.contacts.keys().copied().collect()
    };
    for target in targets {
        if let Some(view) = contact_view(world, observer, &unit, target)? {
            views.push(view);
        }
    }
    views.sort_by(|a, b| {
        a.range
            .spatial
            .total_cmp(&b.range.spatial)
            .then(a.target.cmp(&b.target))
    });
    Ok(views)
}

/// Add only trusted row styling after escaping all data-derived text.
pub(super) fn styled_row(text: &str, color: Option<&str>) -> String {
    let text = crate::text::escape(text);
    color.map_or_else(
        || text.clone(),
        |color| format!("[fg={color} bold]{text}[reset]"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BattleMapAsset, BattlePower, BattleSensorSignature, BattleUnitTemplate, Config, Kind,
        ObjectId, World, create_battle_map, place_battle_unit, set_battle_sensor_signature,
    };
    use std::sync::Arc;

    fn facts_fixture(blocked: bool) -> (World, ObjectId, ObjectId) {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let map = world.create(&config, "Contact facts map".into(), Kind::Room);
        let middle = if blocked { ".3" } else { ".0" };
        create_battle_map(
            &mut world,
            map,
            "contact.facts",
            BattleMapAsset::parse(&format!("1 3\n.0\n{middle}\n.0\n")).unwrap(),
        )
        .unwrap();
        let observer = world.create(&config, "Contact facts observer".into(), Kind::Thing);
        let target = world.create(&config, "Contact facts target".into(), Kind::Thing);
        for (id, y) in [(observer, 0), (target, 2)] {
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            BattleUnitTemplate::parse(
                &include_str!("../../tests/fixtures/btech/mechs/JR7-D")
                    .replace("FlipArms", "FlipArms Searchlight"),
            )
            .unwrap()
            .create(&mut world, id)
            .unwrap();
            place_battle_unit(&mut world, id, map, 0, y).unwrap();
            Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap()
                .power = BattlePower::Running;
        }
        set_battle_sensor_signature(
            &mut world,
            target,
            BattleSensorSignature {
                team: 1,
                ..Default::default()
            },
        )
        .unwrap();
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&observer)
            .unwrap()
            .contacts
            .insert(
                target,
                BattleContact {
                    identified: true,
                    primary: true,
                    secondary: false,
                },
            );
        (world, observer, target)
    }

    fn assert_parity(world: &World, observer: ObjectId, target: ObjectId) {
        let facts = contact_facts(world, observer, target)
            .unwrap()
            .expect("the acquired contact should be available");
        let view = visible_contact(world, observer, target)
            .unwrap()
            .expect("the acquired contact should have a display view");
        assert_eq!(facts.target, view.target);
        assert_eq!(facts.position.x, view.coordinate.x as u16);
        assert_eq!(facts.position.y, view.coordinate.y as u16);
        assert_eq!(facts.identified, view.identified);
        assert_eq!(facts.friendly, view.friendly);
        assert_eq!(facts.sensors, view.sensors);
        assert_eq!(facts.range, view.range);
        assert_eq!(
            facts.known_destroyed,
            view.status.as_bytes().get(1) == Some(&b'D')
        );
    }

    #[test]
    fn observation_lighting_context_matches_live_lights_and_invalidates_by_borrow() {
        let (mut world, observer, target) = facts_fixture(false);
        for step in 0..5 {
            let unit = Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&observer)
                .unwrap();
            unit.motion.as_mut().unwrap().heading = 180.0;
            unit.inferno_remaining = if step == 1 { 10 } else { 0 };
            unit.searchlight.on = step % 2 == 0;
            unit.searchlight.destroyed = step == 2;
            unit.sensor_signature.illuminated = step == 3;
            let context = super::super::searchlight::IlluminationContext::new(&world);
            if step == 0 {
                assert!(context.illuminated(target));
            }
            for id in [observer, target, observer, target] {
                assert_eq!(
                    context.illuminated(id),
                    super::super::unit_illuminated(&world, id)
                );
            }
        }
    }
    #[test]
    fn batch_observation_matches_scalar_across_chassis_sensors_and_live_changes() {
        use crate::btech::{BattleSensorMode as Sensor, BattleSensorPair};
        let config = Config::load("tests/fixtures/game").unwrap();
        let templates = [
            include_str!("../../game/mechs/JR7-D"),
            include_str!("../../game/mechs/Demolisher"),
            include_str!("../../game/mechs/Flatbed_Truck"),
            include_str!("../../game/mechs/Fulcrum"),
        ];
        for template in templates {
            let (mut world, _, target) = facts_fixture(false);
            let map = world.btech.constructed_units()[&target]
                .position()
                .unwrap()
                .map;
            let observer = world.create(&config, "Batch observer".into(), Kind::Thing);
            BattleUnitTemplate::parse(template)
                .unwrap()
                .create(&mut world, observer)
                .unwrap();
            place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
            let contact = BattleContact {
                identified: true,
                primary: true,
                secondary: true,
            };
            if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&observer) {
                unit.power = BattlePower::Running;
                unit.contacts.insert(target, contact);
            } else {
                let unit = Arc::make_mut(&mut world.btech.vehicles)
                    .get_mut(&observer)
                    .unwrap();
                unit.power = BattlePower::Running;
                unit.contacts.insert(target, contact);
            }
            for primary in [
                Sensor::Visual,
                Sensor::LightAmplification,
                Sensor::Infrared,
                Sensor::Radar,
                Sensor::Electromagnetic,
                Sensor::Seismic,
            ] {
                let pair = BattleSensorPair {
                    primary,
                    secondary: Sensor::Visual,
                };
                if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&observer) {
                    unit.sensor_selection.active = pair;
                } else {
                    Arc::make_mut(&mut world.btech.vehicles)
                        .get_mut(&observer)
                        .unwrap()
                        .sensor_selection
                        .active = pair;
                }
                for light in 0..=2 {
                    for visibility in [0, 10, 60] {
                        let map = Arc::make_mut(&mut world.btech.maps).get_mut(&map).unwrap();
                        map.light = light;
                        map.visibility = visibility;
                        let expected: Vec<_> = contact_facts(&world, observer, target)
                            .ok()
                            .flatten()
                            .into_iter()
                            .collect();
                        let before = serde_json::to_value(&world.btech).unwrap();
                        assert_eq!(acquired_contact_facts(&world, observer).unwrap(), expected);
                        assert_eq!(
                            before,
                            serde_json::to_value(&world.btech).unwrap(),
                            "observation must not mutate state or dice"
                        );
                    }
                }
            }
            // Every call owns a fresh immutable context; the pure topology cache remains warm.
            for step in 0..6 {
                match step {
                    0 => {
                        Arc::make_mut(&mut world.btech.constructed)
                            .get_mut(&target)
                            .unwrap()
                            .sensor_signature
                            .illuminated = true
                    }
                    1 => {
                        Arc::make_mut(&mut world.btech.constructed)
                            .get_mut(&target)
                            .unwrap()
                            .visibility
                            .invisible = true
                    }
                    2 => {
                        Arc::make_mut(&mut world.btech.constructed)
                            .get_mut(&target)
                            .unwrap()
                            .power = BattlePower::Off;
                        place_battle_unit(&mut world, target, map, 0, 1).unwrap();
                        Arc::make_mut(&mut world.btech.constructed)
                            .get_mut(&target)
                            .unwrap()
                            .power = BattlePower::Running;
                    }
                    3 => {
                        Arc::make_mut(&mut world.btech.constructed)
                            .get_mut(&target)
                            .unwrap()
                            .visibility
                            .invisible = false
                    }
                    4 => {
                        Arc::make_mut(&mut world.btech.maps)
                            .get_mut(&map)
                            .unwrap()
                            .light = 0;
                    }
                    _ => {
                        world
                            .objects
                            .get_mut(&target)
                            .unwrap()
                            .flags
                            .insert(crate::Flag::Going);
                    }
                }
                let expected: Vec<_> = contact_facts(&world, observer, target)
                    .ok()
                    .flatten()
                    .into_iter()
                    .collect();
                assert_eq!(acquired_contact_facts(&world, observer).unwrap(), expected);
            }
        }
        let (mut world, observer, target) = facts_fixture(true);
        assert!(acquired_contact_facts(&world, observer).unwrap().is_empty());
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&observer)
            .unwrap();
        unit.visibility.clairvoyant = true;
        unit.contacts.clear();
        assert!(contact_facts(&world, observer, target).unwrap().is_some());
        assert!(
            acquired_contact_facts(&world, observer).unwrap().is_empty(),
            "clairvoyance cannot add unacquired autopilot contacts"
        );
    }

    #[test]
    fn acquired_contact_facts_match_display_for_visibility_cases() {
        let (world, observer, target) = facts_fixture(false);
        assert_parity(&world, observer, target);

        let (world, observer, target) = facts_fixture(true);
        assert!(contact_facts(&world, observer, target).unwrap().is_none());
        assert!(visible_contact(&world, observer, target).unwrap().is_none());

        let (mut world, observer, target) = facts_fixture(false);
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&target)
            .unwrap()
            .visibility
            .invisible = true;
        assert!(contact_facts(&world, observer, target).unwrap().is_none());
        assert!(visible_contact(&world, observer, target).unwrap().is_none());

        let (mut world, observer, target) = facts_fixture(false);
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&observer)
            .unwrap()
            .visibility
            .clairvoyant = true;
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&target)
            .unwrap()
            .visibility
            .invisible = true;
        assert_parity(&world, observer, target);
    }
}

/// Read acquired facts in stable unit order, resolving observer state once.
/// Unlike the contact display, clairvoyance does not enumerate unacquired enemies.
pub(crate) fn acquired_contact_facts(
    world: &World,
    observer: ObjectId,
) -> Result<Vec<BattleContactFacts>> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Contacts,
    );
    let unit = contact_observer(world, observer)?;
    let illumination = super::searchlight::IlluminationContext::new(world);
    Ok(unit
        .contacts
        .keys()
        .filter_map(|&target| {
            contact_facts_with_unit(world, observer, &unit, target, Some(&illumination))
                .ok()
                .flatten()
        })
        .collect())
}
