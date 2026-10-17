//! Automatic scans: every running unit perceives its map each tick and commits contact changes.
use super::{ContactTransition, Mech, Perception, Power};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Scenario-owned signature facts: team and scenario lighting.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSignature {
    pub team: i32,
    pub illuminated: bool,
}

/// One acquisition/loss transition, staged for cockpit notification after persistence succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContactEvent {
    /// Identification captured before a lost observation is removed.
    pub identified: bool,
    pub observer: ObjectId,
    pub target: ObjectId,
    pub acquired: bool,
    /// This contact loss also canceled the observer's selected target.
    pub lock_lost: bool,
}

impl ContactEvent {
    /// Capture cockpit feedback without changing acquisition or lock state.
    /// Observer units suppress routine contact chatter but still announce a lost weapon lock.
    pub fn notice(&self, world: &World) -> Option<super::Notice> {
        let unit = scanner_unit(world, self.observer)?;
        let mut lines = Vec::new();
        if !unit.observer
            && let Some(text) = self.routine_notice(world)
        {
            lines.push(text);
        }
        if self.lock_lost {
            lines.push("Weapon system reports the lock has been lost.".into());
        }
        if lines.is_empty() {
            return None;
        }
        Some(super::Notice {
            unit: self.observer,
            text: lines.join("\r\n"),
        })
    }

    /// Resolve current arc using identification captured by the observation that produced this event.
    fn routine_notice(&self, world: &World) -> Option<String> {
        let observer = scanner_unit(world, self.observer)?;
        let target = scanner_unit(world, self.target)?;
        if target.power != Power::Running && !observer.autocon_shutdown {
            return None;
        }
        let clear = self.identified;
        let same_team = observer.signature.team == target.signature.team;
        let friendly = same_team && clear;
        let settings = observer.brief;
        if !settings.announces(friendly) {
            return None;
        }
        let range = super::unit_range(world, self.observer, self.target).ok()?;
        let arc = observer
            .facing
            .contact_arc(observer.heading?, range.bearing.unwrap_or(180.0))
            .ok()?;
        let arc = match arc {
            super::ContactArc::Front => "Forward",
            super::ContactArc::Right if observer.vehicle => "Right Side",
            super::ContactArc::Right => "Right Arm",
            super::ContactArc::Left if observer.vehicle => "Left Side",
            super::ContactArc::Left => "Left Arm",
            super::ContactArc::Rear => "Rear",
        };
        let name = if clear {
            crate::text::plain(target.name)
        } else {
            "something".into()
        };
        let mut label = target.label()?;
        if clear && same_team {
            label.make_ascii_lowercase();
        }
        let identity = crate::text::escape(&format!("{name} [{label}]"));
        let long = matches!(settings.automatic, 0 | 2);
        let text = match (self.acquired, long) {
            (true, true) => format!("You notice {identity} in your {arc} arc."),
            (true, false) => format!("Seen: {identity}, {arc} arc."),
            (false, true) => format!(
                "You have lost {identity} from your scanners. It was last in your {arc} arc."
            ),
            (false, false) => format!("Lost: {identity}, {arc} arc."),
        };
        if friendly || settings.automatic >= 4 {
            return Some(text);
        }
        Some(format!(
            "[fg={}]{}[reset]",
            if self.acquired { "red" } else { "yellow" },
            text
        ))
    }
}

pub(super) fn default_perception() -> i16 {
    6
}

impl Mech {
    /// Saved team and target visibility facts.
    pub fn signature(&self) -> UnitSignature {
        self.signature
    }
    /// Perception target captured at startup completion, used throughout that engine run.
    pub fn scanner_perception(&self) -> i16 {
        self.scanner_perception
    }

    /// This turn's 2d6 sensor sweep, once rolled.
    pub fn sensor_sweep(&self) -> Option<u8> {
        self.sensor_sweep
    }
}

impl super::Vehicle {
    /// Saved team and scenario illumination facts for vehicle observations.
    pub fn signature(&self) -> UnitSignature {
        self.signature
    }

    /// Perception captured when startup completes; it remains fixed until the next startup.
    pub fn scanner_perception(&self) -> i16 {
        self.scanner_perception
    }

    /// This turn's 2d6 sensor sweep, once rolled.
    pub fn sensor_sweep(&self) -> Option<u8> {
        self.sensor_sweep
    }
}

/// Trusted scenario edit; caller owns administrative authority and the commit boundary.
pub fn set_unit_signature(world: &mut World, id: ObjectId, signature: UnitSignature) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Unit must be a live thing");
    let unit = world
        .btech
        .unit_mut(id)
        .context("Unit construction state is unavailable")?;
    super::with_unit_mut!(unit, |unit| {
        if unit.signature.team != signature.team {
            unit.c3_network = None;
            unit.c3i_network = None;
        }
        unit.signature = signature;
    });
    Ok(())
}

/// Read-only sensor and display facts shared by the supported construction classes.
pub(super) struct ScannerUnit<'a> {
    pub(super) position: Option<super::Position>,
    pub(super) power: Power,
    pub(super) signature: UnitSignature,
    pub(super) perception: i16,
    pub(super) fired_recently: bool,
    pub(super) contacts: &'a std::collections::BTreeMap<ObjectId, super::Contact>,
    pub(super) selected: Option<ObjectId>,
    pub(super) heading: Option<f64>,
    pub(super) facing: super::Facing,
    pub(super) name: &'a str,
    label_override: Option<&'a str>,
    pub(super) brief: super::BriefSettings,
    pub(super) observer: bool,
    pub(super) visibility: super::Visibility,
    pub(super) concealed: bool,
    pub(super) sensor_sweep: Option<u8>,
    pub(super) autocon_shutdown: bool,
    pub(super) vehicle: bool,
    pub(super) speed: f64,
    pub(super) travel_heading: Option<f64>,
    pub(super) destroyed: bool,
    pub(super) slot: Option<u32>,
    pub(super) point: Option<super::Point>,
}

impl ScannerUnit<'_> {
    /// Format identity only for presentation. Sensor/geometry queries need no label allocation.
    pub(super) fn label(&self) -> Option<String> {
        self.slot.map(|slot| {
            self.label_override
                .map(str::to_owned)
                .unwrap_or_else(|| super::observer::battlefield_label(slot))
        })
    }
}

/// Borrow scanner inputs without coupling vehicle state to Mech construction.
pub(super) fn scanner_unit(world: &World, id: ObjectId) -> Option<ScannerUnit<'_>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some(ScannerUnit {
            point: unit.motion().map(|motion| motion.point),
            speed: unit.motion().map_or(0.0, |m| m.speed),
            travel_heading: unit.motion().map(|m| m.heading),
            destroyed: unit.is_destroyed(),
            slot: unit.map_slot(),
            position: unit.position(),
            power: unit.power(),
            signature: unit.signature(),
            perception: unit.scanner_perception(),
            fired_recently: unit.fired_recently(),
            contacts: unit.contacts(),
            selected: unit.target_lock().map(|lock| lock.target),
            heading: unit.motion().map(|m| m.heading),
            facing: Default::default(),
            name: world
                .btech
                .unit_configuration
                .get(&id)
                .and_then(|configuration| configuration.display_name.as_deref())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| unit.display_name.effective(&unit.definition().name)),
            label_override: unit.battlefield_label.as_deref(),
            brief: unit.brief_settings(),
            observer: unit.is_observer(),
            visibility: unit.visibility,
            concealed: false,
            sensor_sweep: unit.sensor_sweep,
            autocon_shutdown: unit.autocon_shutdown(),
            vehicle: true,
        });
    }
    let unit = world.btech.constructed_units().get(&id)?;
    Some(ScannerUnit {
        point: unit.motion().map(|motion| motion.point),
        speed: unit.motion().map_or(0.0, |m| m.speed),
        travel_heading: unit.travel_heading(),
        destroyed: unit.is_destroyed(),
        slot: unit.map_slot(),
        position: unit.position(),
        power: unit.power(),
        signature: unit.signature(),
        perception: unit.scanner_perception(),
        fired_recently: unit.fired_recently(),
        contacts: unit.contacts(),
        selected: unit.target_lock().map(|lock| lock.target),
        heading: unit.motion().map(|m| m.heading),
        facing: unit.facing(),
        name: world
            .btech
            .unit_configuration
            .get(&id)
            .and_then(|configuration| configuration.display_name.as_deref())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| unit.display_name.effective(&unit.definition().name)),
        label_override: unit.battlefield_label.as_deref(),
        brief: unit.brief_settings(),
        observer: unit.is_observer(),
        visibility: unit.visibility,
        concealed: unit.range_concealed(),
        sensor_sweep: unit.sensor_sweep,
        autocon_shutdown: unit.autocon_shutdown(),
        vehicle: false,
    })
}

/// Stable object order spans both construction stores.
pub(super) fn scanner_ids(world: &World) -> std::collections::BTreeSet<ObjectId> {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .collect()
}

/// Whether a live unit has a valid location on decoded terrain.
fn available(world: &World, id: ObjectId) -> bool {
    let Some(position) = scanner_unit(world, id).and_then(|unit| unit.position) else {
        return false;
    };
    world
        .objects
        .get(&id)
        .is_some_and(|object| !object.flags.contains(Flag::Going))
        && world
            .btech
            .maps()
            .get(&position.map)
            .and_then(|map| map.hex(i64::from(position.x), i64::from(position.y)).ok())
            .is_some()
}

/// Snapshot observers eligible for a tick; newly completed startups begin scanning on the next tick.
pub fn contact_observers(world: &World) -> Vec<ObjectId> {
    let ids = scanner_ids(world);
    ids.iter()
        .copied()
        .filter(|id| {
            let unit = scanner_unit(world, *id).expect("known construction");
            unit.power == Power::Running
                && available(world, *id)
                && ids.iter().any(|target| {
                    let other = scanner_unit(world, *target).expect("known construction");
                    target != id
                        && other.position.map(|p| p.map) == unit.position.map(|p| p.map)
                        && available(world, *target)
                })
        })
        .collect()
}

/// Refresh contacts in stable object order, committing all contacts and events together.
///
/// Perception is resolved against the unchanged world, sharing one observer profile and
/// lighting cache per observer. Commits follow in the same order; nothing they change
/// (contacts, locks) feeds back into perception.
pub fn refresh_contacts(world: &mut World, observers: &[ObjectId]) -> Result<Vec<ContactEvent>> {
    world.attempt(|world| {
        let mut events = Vec::new();
        // A unit that has not swept its sensors yet sweeps them now.
        for &observer in observers {
            if scanner_unit(world, observer).is_some_and(|unit| {
                unit.power == Power::Running
                    && unit.position.is_some()
                    && unit.sensor_sweep.is_none()
            }) && available(world, observer)
            {
                super::perception::sweep_unit(world, observer)?;
            }
        }
        let observations = observe(world, observers)?;
        for observation in observations {
            let observer = observation.observer;
            for (target, perception) in observation.targets {
                let unit = scanner_unit(world, observer).expect("validated observer");
                let previously_identified = unit
                    .contacts
                    .get(&target)
                    .is_some_and(|contact| contact.identified);
                let selected = unit.selected == Some(target);
                let update = super::contacts::apply_contact(world, observer, target, perception)?;
                if !matches!(
                    update.transition,
                    ContactTransition::Acquired | ContactTransition::Lost
                ) {
                    continue;
                }
                let identified = if update.transition == ContactTransition::Lost {
                    previously_identified
                } else {
                    update.contact.is_some_and(|contact| contact.identified)
                };
                events.push(ContactEvent {
                    identified,
                    observer,
                    target,
                    acquired: update.transition == ContactTransition::Acquired,
                    lock_lost: selected && update.transition == ContactTransition::Lost,
                });
            }
        }
        Ok(events)
    })
}

/// One observer's view of its map, resolved before any contact is committed.
struct Observation {
    observer: ObjectId,
    targets: Vec<(ObjectId, Option<Perception>)>,
}

/// Perceive every same-map target for each running, placed observer in stable object order.
fn observe(world: &World, observers: &[ObjectId]) -> Result<Vec<Observation>> {
    let illumination = super::searchlight::IlluminationContext::new(world);
    let mut observations = Vec::new();
    for observer in observers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let Some(unit) = scanner_unit(world, observer) else {
            continue;
        };
        if unit.power != Power::Running || !available(world, observer) {
            continue;
        }
        let map = unit.position.expect("available observer").map;
        let profile = super::perception_profile(world, observer)?;
        let mut targets = Vec::new();
        for target in scanner_ids(world) {
            let Some(other) = scanner_unit(world, target) else {
                continue;
            };
            if target == observer
                || other.position.is_none_or(|p| p.map != map)
                || !available(world, target)
            {
                continue;
            }
            let perception = super::perception::perceive_prepared(
                world,
                observer,
                &profile,
                target,
                None,
                Some(&illumination),
            )?;
            targets.push((target, perception));
        }
        observations.push(Observation { observer, targets });
    }
    Ok(observations)
}

/// Stage contact feedback and experience diagnostics before the server commit.
pub(crate) fn notify_contact(scripts: &crate::Scripts, event: ContactEvent) -> Result<()> {
    let Some(notice) = event.notice(&scripts.world.borrow()) else {
        return Ok(());
    };
    super::notify_unit(scripts, notice)
}
