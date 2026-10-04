//! Durable contacts with explicit acquisition, retention and loss transitions.
use super::{
    BattleAcquisitionRules, BattleDetection, BattleDetectionChannel, BattlePerception,
    BattlePerceptionProfile, BattleUnit,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An acquired target as of its most recent contact update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleContact {
    /// Whether the last retained observation identified the target through clear terrain.
    #[serde(default)]
    pub identified: bool,
}

/// Count acquired enemy contacts without rerunning detection or changing saved observations.
pub(super) fn enemy_count(world: &World, id: ObjectId) -> Result<usize> {
    let observer = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    Ok(observer
        .contacts
        .keys()
        .filter(|target| {
            super::scanner::scanner_unit(world, **target)
                .is_some_and(|unit| unit.signature.team != observer.signature.team)
        })
        .count())
}

/// Contact-row code for a detection: the channel letter, lowercase behind blocking terrain.
/// Clairvoyant views of units nobody actually perceives show a blank.
pub(super) fn detection_code(detection: Option<BattleDetectionChannel>, identified: bool) -> char {
    detection.map_or(' ', |channel| channel.code(identified))
}

/// Change in ownership of a contact; retained observations do not reroll acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleContactTransition {
    Unseen,
    Acquired,
    Retained,
    Lost,
}

/// Target facts supplied by the scanner for one observer/target update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleContactRules {
    pub hostile: bool,
    pub hidden: bool,
    /// The observing pilot's perception skill target.
    pub perception: i16,
    /// False refreshes existing observations without trying to acquire unseen targets.
    pub acquire: bool,
}

/// A committed observation and the hidden-unit search, when one was needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleContactUpdate {
    pub transition: BattleContactTransition,
    pub contact: Option<BattleContact>,
    pub detection: Option<BattleDetection>,
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

/// Perceive one target now and commit the resulting acquisition, retention or loss.
pub fn update_contact(
    world: &mut World,
    observer: ObjectId,
    target: ObjectId,
    rules: BattleContactRules,
) -> Result<BattleContactUpdate> {
    ensure!(observer != target, "A unit cannot acquire itself");
    let perception = super::perceive(world, observer, target)?;
    apply_contact(world, observer, target, perception, rules)
}

/// Commit an already computed perception. New targets are acquired at once, except hidden
/// hostile units, which need a probe or a successful search; known targets are retained for
/// as long as any channel still reaches them. Loss clears the observer's lock on the target.
pub(super) fn apply_contact(
    world: &mut World,
    observer: ObjectId,
    target: ObjectId,
    perception: Option<BattlePerception>,
    rules: BattleContactRules,
) -> Result<BattleContactUpdate> {
    ensure!(observer != target, "A unit cannot acquire itself");
    let (power, known) = super::with_unit!(
        world
            .btech
            .unit(observer)
            .context("Unit construction state is unavailable")?,
        |unit| { (unit.power(), unit.contacts.contains_key(&target)) }
    );
    ensure!(power == super::BattlePower::Running, "Start the unit first");
    let mut contact = perception.map(|perception| BattleContact {
        identified: perception.identified,
    });
    let mut detection = None;
    if !known {
        match perception {
            Some(perception) if rules.acquire => {
                let attempt = super::perception::acquire(
                    world,
                    observer,
                    &perception,
                    BattleAcquisitionRules {
                        hostile: rules.hostile,
                        hidden: rules.hidden,
                        perception: rules.perception,
                    },
                )?;
                if !attempt.detected {
                    contact = None;
                }
                detection = Some(attempt);
            }
            _ => contact = None,
        }
    }
    let transition = match (known, contact.is_some()) {
        (false, false) => BattleContactTransition::Unseen,
        (false, true) => BattleContactTransition::Acquired,
        (true, true) => BattleContactTransition::Retained,
        (true, false) => BattleContactTransition::Lost,
    };
    let contacts = if world.btech.vehicles().contains_key(&observer) {
        let unit = world
            .btech
            .vehicles
            .get_mut(&observer)
            .expect("validated observer");
        if contact.is_none() && unit.target_lock().is_some_and(|lock| lock.target == target) {
            unit.target_lock = None;
        }
        &mut unit.contacts
    } else {
        let unit = world
            .btech
            .constructed
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
        detection,
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
    for (observer, vehicle) in state.vehicles.iter_mut() {
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
    for (observer, unit) in state.constructed.iter_mut() {
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
    pub coordinate: super::HexCoordinate,
    pub elevation: i32,
    /// Plain compact row shared by native C1/C2/C3 and Lua.
    pub short_text: String,
    /// Plain multiline C0 report, shared with Lua.
    pub verbose_text: String,
    /// Current clear-terrain identification, independent of retained acquisition.
    pub identified: bool,
    /// General direction relative to the observer torso, not a mount firing guarantee.
    pub weapon_arc: super::BattleContactArc,
    /// How the target is currently perceived; absent for a clairvoyant view nobody perceives.
    pub detection: Option<BattleDetectionChannel>,
    /// Five visible status columns; terrain-obscured identities receive blanks.
    pub status: String,
    pub target: ObjectId,
    pub name: String,
    pub friendly: bool,
    pub range: super::BattleRange,
    /// Closest usable command-network sighting distance; absent when the observer has no active network.
    pub network_range: Option<f64>,
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
    /// A networked observer gains a `c:` column with the shared aiming distance.
    fn compact_text(&self, movement: &str) -> String {
        let name: String = crate::text::plain(&self.name).chars().take(12).collect();
        let network = self
            .network_range
            .map_or_else(String::new, |range| format!(" c:{range:>4.1}"));
        format!(
            "{}{}{}[{}]{} {:<12} x:{:>3} y:{:>3} z:{:>3} r:{:>4.1}{network} b:{:>3} s:{:>5.1} h:{:>3} S:{}",
            detection_code(self.detection, self.identified),
            ' ',
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
    pub(crate) detection: Option<BattleDetectionChannel>,
    pub(crate) known_destroyed: bool,
    pub(crate) range: super::BattleRange,
}

/// Build an acquired contact or a clairvoyant observation using current mixed-class geometry.
fn contact_facts_with_unit(
    world: &World,
    observer: ObjectId,
    unit: &super::scanner::ScannerUnit<'_>,
    profile: &BattlePerceptionProfile,
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
    if other.position.is_none_or(|p| p.map != position.map)
        || world
            .objects
            .get(&target)
            .is_none_or(|object| object.flags.contains(crate::Flag::Going))
    {
        return Ok(None);
    }
    // Clairvoyant views skip the trace: they neither need nor imply a real detection.
    let (detection, identified, range) = if unit.visibility.clairvoyant {
        (None, true, super::unit_range(world, observer, target)?)
    } else {
        let Some(perception) = super::perception::perceive_prepared(
            world,
            observer,
            profile,
            target,
            None,
            illumination,
        )
        .ok()
        .flatten() else {
            return Ok(None);
        };
        (
            Some(perception.channel),
            perception.identified,
            perception.range,
        )
    };
    let friendly = identified && unit.signature.team == other.signature.team;
    let position = other.position.context("Contact has no position")?;
    Ok(Some(BattleContactFacts {
        target,
        position,
        identified,
        friendly,
        detection,
        // contact_status hides all condition columns behind the same clear
        // terrain rule.  The target's authoritative destruction bit plus the
        // already-computed identification result is the equivalent fact,
        // without allocating the five-column status string.
        known_destroyed: identified && other.destroyed,
        range,
    }))
}

/// One observer's contact reads over an immutable world.
///
/// The observer's perception profile and lighting cache are built once and shared by every
/// target, so loops over many targets (map displays, pathfinding occupancy) stay cheap. Nothing
/// here survives the borrow: build a new reader after any world mutation.
pub(crate) struct ContactReader<'w> {
    world: &'w World,
    observer: ObjectId,
    unit: super::scanner::ScannerUnit<'w>,
    profile: BattlePerceptionProfile,
    illumination: super::searchlight::IlluminationContext<'w>,
}

impl<'w> ContactReader<'w> {
    /// Admit a running, placed observer and prepare its perception profile.
    pub(crate) fn new(world: &'w World, observer: ObjectId) -> Result<Self> {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Contacts,
        );
        Ok(Self {
            world,
            observer,
            unit: contact_observer(world, observer)?,
            profile: super::perception_profile(world, observer)?,
            illumination: super::searchlight::IlluminationContext::new(world),
        })
    }

    /// Live facts for one target, or `None` when it is not a current contact.
    pub(crate) fn facts(&self, target: ObjectId) -> Result<Option<BattleContactFacts>> {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Contacts,
        );
        contact_facts_with_unit(
            self.world,
            self.observer,
            &self.unit,
            &self.profile,
            target,
            Some(&self.illumination),
        )
    }

    /// The display view of one target, or `None` when it is not a current contact.
    pub(crate) fn view(&self, target: ObjectId) -> Result<Option<BattleContactView>> {
        contact_view(
            self.world,
            self.observer,
            &self.unit,
            &self.profile,
            target,
            Some(&self.illumination),
        )
    }
}

/// Build an acquired contact or a clairvoyant observation using current mixed-class geometry.
fn contact_view(
    world: &World,
    observer: ObjectId,
    unit: &super::scanner::ScannerUnit<'_>,
    profile: &BattlePerceptionProfile,
    target: ObjectId,
    illumination: Option<&super::searchlight::IlluminationContext<'_>>,
) -> Result<Option<BattleContactView>> {
    let Some(facts) =
        contact_facts_with_unit(world, observer, unit, profile, target, illumination)?
    else {
        return Ok(None);
    };
    let status = super::contact_status::contact_status(world, observer, target)?;
    view_from_facts(world, unit, facts, status).map(Some)
}

/// Present a sighting reported by a command-network peer as the observer's own row.
/// The observer supplies geometry and arcs; identification comes from the peer's view.
fn network_view(
    world: &World,
    observer: ObjectId,
    unit: &super::scanner::ScannerUnit<'_>,
    target: ObjectId,
    identified: bool,
) -> Result<BattleContactView> {
    let facts = relayed_facts(world, observer, unit.signature.team, target, identified)?;
    let status = if identified {
        super::contact_status::known_status(world, observer, target)?
    } else {
        "     ".into()
    };
    view_from_facts(world, unit, facts, status)
}

/// Facts for a sighting relayed by a command-network peer. Geometry is measured from the
/// observer; identification, and so allegiance and known destruction, come from the peer.
pub(super) fn relayed_facts(
    world: &World,
    observer: ObjectId,
    observer_team: i32,
    target: ObjectId,
    identified: bool,
) -> Result<BattleContactFacts> {
    let other = super::scanner::scanner_unit(world, target).context("Contact disappeared")?;
    Ok(BattleContactFacts {
        target,
        position: other.position.context("Contact has no position")?,
        identified,
        friendly: identified && observer_team == other.signature.team,
        detection: None,
        known_destroyed: identified && other.destroyed,
        range: super::unit_range(world, observer, target)?,
    })
}

/// Build the display row from established facts and the already-resolved status columns.
fn view_from_facts(
    world: &World,
    unit: &super::scanner::ScannerUnit<'_>,
    facts: BattleContactFacts,
    status: String,
) -> Result<BattleContactView> {
    let target = facts.target;
    let other = super::scanner::scanner_unit(world, target).context("Contact disappeared")?;
    let heading = unit.heading.context("Observer has no motion state")?;
    let bearing = facts.range.bearing.unwrap_or(180.0);
    let mut label = other.label().context("Contact has no battlefield label")?;
    if facts.friendly {
        label.make_ascii_lowercase();
    }
    let mut view = BattleContactView {
        label,
        coordinate: super::HexCoordinate {
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
        detection: facts.detection,
        status,
        target,
        name: if facts.identified {
            other.name.to_owned()
        } else {
            "something".into()
        },
        friendly: facts.friendly,
        range: facts.range,
        network_range: None,
        heading: other.travel_heading.context("Contact has no motion")?,
        speed: other.speed,
    };
    view.short_text = view.compact_text(movement_type(world, target));
    view.verbose_text = view.verbose_text(world, unit.vehicle)?;
    Ok(view)
}

/// Attach the shared aiming distance and re-render the compact row.
fn attach_network_range(
    world: &World,
    network: &super::network_contacts::NetworkSightings<'_>,
    view: &mut BattleContactView,
) -> Result<()> {
    view.network_range = Some(network.range(view.target, view.range.spatial)?.distance);
    view.short_text = view.compact_text(movement_type(world, view.target));
    Ok(())
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
    ContactReader::new(world, observer)?.view(target)
}

/// One contact as the cockpit displays it: a direct sighting, or one relayed by an active
/// command-network peer, with the shared aiming distance attached either way.
/// Relayed rows never satisfy the firing, locking, or spotting rules built on `visible_contact`.
pub fn displayed_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<Option<BattleContactView>> {
    let reader = ContactReader::new(world, observer)?;
    let mut view = reader.view(target)?;
    let Some(mut network) = super::network_contacts::NetworkSightings::new(world, observer)? else {
        return Ok(view);
    };
    if view.is_none()
        && observer != target
        && let Some(identified) = network.identified(target)?
    {
        view = Some(network_view(
            world,
            observer,
            &reader.unit,
            target,
            identified,
        )?);
    }
    if let Some(view) = &mut view {
        attach_network_range(world, &network, view)?;
    }
    Ok(view)
}

/// List eligible acquired contacts, or all same-map units for a clairvoyant observer.
/// This read-only display never attempts acquisition or updates the saved observation.
pub fn visible_contacts(world: &World, observer: ObjectId) -> Result<Vec<BattleContactView>> {
    let reader = ContactReader::new(world, observer)?;
    let mut views = Vec::new();
    let targets: Vec<_> = if reader.unit.visibility.clairvoyant {
        super::map_slots::all_unit_order(world, reader.unit.position.unwrap().map)?
    } else {
        reader.unit.contacts.keys().copied().collect()
    };
    for target in targets {
        if let Some(view) = reader.view(target)? {
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

/// The cockpit contact list: direct sightings plus targets relayed by active command-network
/// peers, every row carrying the shared aiming distance. See [`displayed_contact`].
pub fn displayed_contacts(world: &World, observer: ObjectId) -> Result<Vec<BattleContactView>> {
    let mut views = visible_contacts(world, observer)?;
    let Some(mut network) = super::network_contacts::NetworkSightings::new(world, observer)? else {
        return Ok(views);
    };
    let reader = ContactReader::new(world, observer)?;
    let map = reader
        .unit
        .position
        .expect("validated observer placement")
        .map;
    for target in super::map_slots::all_unit_order(world, map)? {
        if target == observer || views.iter().any(|view| view.target == target) {
            continue;
        }
        if let Some(identified) = network.identified(target)? {
            views.push(network_view(
                world,
                observer,
                &reader.unit,
                target,
                identified,
            )?);
        }
    }
    for view in &mut views {
        attach_network_range(world, &network, view)?;
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

/// Read acquired facts in stable unit order, resolving observer state once.
/// Unlike the contact display, clairvoyance does not enumerate unacquired enemies.
pub(crate) fn acquired_contact_facts(
    world: &World,
    observer: ObjectId,
) -> Result<Vec<BattleContactFacts>> {
    let reader = ContactReader::new(world, observer)?;
    Ok(reader
        .unit
        .contacts
        .keys()
        .filter_map(|&target| reader.facts(target).ok().flatten())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Facts for one pair through a freshly built reader, as single-target callers read them.
    fn contact_facts(
        world: &World,
        observer: ObjectId,
        target: ObjectId,
    ) -> Result<Option<BattleContactFacts>> {
        ContactReader::new(world, observer)?.facts(target)
    }
    use crate::{
        BattlePower, BattleUnitSignature, BattleUnitTemplate, Config, Kind, MapAsset, ObjectId,
        World, create_battle_map, place_battle_unit, set_battle_unit_signature,
    };

    fn facts_fixture(blocked: bool) -> (World, ObjectId, ObjectId) {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let map = world.create(&config, "Contact facts map".into(), Kind::Room);
        let middle = if blocked { ".3" } else { ".0" };
        create_battle_map(
            &mut world,
            map,
            "contact.facts",
            MapAsset::from_cells(&format!("1 3\n.0\n{middle}\n.0\n")).unwrap(),
        )
        .unwrap();
        let observer = world.create(&config, "Contact facts observer".into(), Kind::Thing);
        let target = world.create(&config, "Contact facts target".into(), Kind::Thing);
        for (id, y) in [(observer, 0), (target, 2)] {
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            BattleUnitTemplate::parse(
                "JR7-D",
                &format!(
                    "specials = [\"SearchLight\"]\n{}",
                    include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml")
                ),
            )
            .unwrap()
            .create(&mut world, id)
            .unwrap();
            place_battle_unit(&mut world, id, map, 0, y).unwrap();
            world.btech.constructed.get_mut(&id).unwrap().power = BattlePower::Running;
        }
        set_battle_unit_signature(
            &mut world,
            target,
            BattleUnitSignature {
                team: 1,
                ..Default::default()
            },
        )
        .unwrap();
        world
            .btech
            .constructed
            .get_mut(&observer)
            .unwrap()
            .contacts
            .insert(target, BattleContact { identified: true });
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
        assert_eq!(facts.detection, view.detection);
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
            let unit = world.btech.constructed.get_mut(&observer).unwrap();
            unit.motion.as_mut().unwrap().heading = 180.0;
            unit.inferno_remaining = if step == 1 { 10 } else { 0 };
            unit.searchlight.on = step % 2 == 0;
            unit.searchlight.destroyed = step == 2;
            unit.signature.illuminated = step == 3;
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
    fn batch_observation_matches_scalar_across_chassis_conditions_and_live_changes() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let templates = [
            include_str!("../../game/mechs/JR7-D.toml"),
            include_str!("../../game/mechs/Demolisher.toml"),
            include_str!("../../game/mechs/Flatbed_Truck.toml"),
            include_str!("../../game/mechs/Fulcrum.toml"),
        ];
        for template in templates {
            let (mut world, _, target) = facts_fixture(false);
            let map = world.btech.constructed_units()[&target]
                .position()
                .unwrap()
                .map;
            let observer = world.create(&config, "Batch observer".into(), Kind::Thing);
            BattleUnitTemplate::parse("observer", template)
                .unwrap()
                .create(&mut world, observer)
                .unwrap();
            place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
            let contact = BattleContact { identified: true };
            crate::btech::with_unit_mut!(world.btech.unit_mut(observer).unwrap(), |unit| {
                unit.power = BattlePower::Running;
                unit.contacts.insert(target, contact);
            });
            for light in 0..=2 {
                for visibility in [0, 10, 60] {
                    let map = world.btech.maps.get_mut(&map).unwrap();
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
            // Every call owns a fresh immutable context; the pure topology cache remains warm.
            for step in 0..6 {
                match step {
                    0 => {
                        world
                            .btech
                            .constructed
                            .get_mut(&target)
                            .unwrap()
                            .signature
                            .illuminated = true
                    }
                    1 => {
                        world
                            .btech
                            .constructed
                            .get_mut(&target)
                            .unwrap()
                            .visibility
                            .invisible = true
                    }
                    2 => {
                        world.btech.constructed.get_mut(&target).unwrap().power = BattlePower::Off;
                        place_battle_unit(&mut world, target, map, 0, 1).unwrap();
                        world.btech.constructed.get_mut(&target).unwrap().power =
                            BattlePower::Running;
                    }
                    3 => {
                        world
                            .btech
                            .constructed
                            .get_mut(&target)
                            .unwrap()
                            .visibility
                            .invisible = false
                    }
                    4 => {
                        world.btech.maps.get_mut(&map).unwrap().light = 0;
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
        let unit = world.btech.constructed.get_mut(&observer).unwrap();
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
        world
            .btech
            .constructed
            .get_mut(&target)
            .unwrap()
            .visibility
            .invisible = true;
        assert!(contact_facts(&world, observer, target).unwrap().is_none());
        assert!(visible_contact(&world, observer, target).unwrap().is_none());

        let (mut world, observer, target) = facts_fixture(false);
        world
            .btech
            .constructed
            .get_mut(&observer)
            .unwrap()
            .visibility
            .clairvoyant = true;
        world
            .btech
            .constructed
            .get_mut(&target)
            .unwrap()
            .visibility
            .invisible = true;
        assert_parity(&world, observer, target);
    }
}
