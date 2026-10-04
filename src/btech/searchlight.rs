//! Owned searchlight switching and damage state, with illumination derived from current geometry.
use super::{Mech, Notice, Power};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Durable hardware state; pending transitions invert the current setting after five seconds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Searchlight {
    pub on: bool,
    pub destroyed: bool,
    pub remaining: u8,
    /// Occupant-selected switching policy; automatic lamps follow battlefield darkness.
    #[serde(default)]
    pub mode: SearchlightMode,
}

/// How the lamp chooses its state. Automatic lamps switch on at night and off otherwise,
/// re-evaluated only when map light changes, the carrier changes maps or finishes starting up.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchlightMode {
    #[default]
    Auto,
    On,
    Off,
}

impl SearchlightMode {
    /// Decode the Lua constant value, where zero is automatic.
    pub fn from_stored(value: i64) -> Result<Self> {
        Ok(match value {
            0 => Self::Auto,
            1 => Self::On,
            2 => Self::Off,
            _ => anyhow::bail!("Searchlight mode must be 0, 1 or 2"),
        })
    }

    /// Encode this mode as its Lua constant value.
    pub fn stored(self) -> i64 {
        match self {
            Self::Auto => 0,
            Self::On => 1,
            Self::Off => 2,
        }
    }

    /// Lowercase name used by commands and status displays.
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

impl std::str::FromStr for SearchlightMode {
    type Err = anyhow::Error;

    /// Parse the `slite` command argument.
    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => anyhow::bail!("Usage: slite [on|off|auto]"),
        }
    }
}

impl Searchlight {
    /// Reject impossible hardware or countdown combinations at the ownership boundary.
    pub(super) fn validate(self, installed: bool) -> Result<()> {
        ensure!(
            installed || self == Self::default(),
            "Searchlight state without installed equipment"
        );
        ensure!(self.remaining <= 5, "Invalid searchlight countdown");
        ensure!(
            !self.destroyed || (!self.on && self.remaining == 0),
            "Destroyed searchlight is active"
        );
        Ok(())
    }
}

impl Mech {
    /// Persisted lamp and switch state, independent of the unit's current illumination.
    pub fn searchlight(&self) -> Searchlight {
        self.searchlight
    }
}

impl super::Vehicle {
    /// Saved searchlight hardware and pending switch state.
    pub fn searchlight(&self) -> Searchlight {
        self.searchlight
    }
}

/// Read live lamp state without resolving static equipment flags.
fn lamp_state(world: &World, id: ObjectId) -> Option<Searchlight> {
    world.btech.unit(id).map(|unit| unit.searchlight())
}

/// Read installed hardware independently of anatomy and cockpit admission.
fn hardware(world: &World, id: ObjectId) -> Option<(Searchlight, bool)> {
    let unit = world.btech.unit(id)?;
    Some(super::with_unit!(unit, |unit| (
        unit.searchlight,
        unit.definition().has_special("Searchlight"),
    )))
}

/// Borrow admitted hardware without duplicating switch or damage rules.
fn hardware_mut(world: &mut World, id: ObjectId) -> &mut Searchlight {
    super::with_unit_mut!(world.btech.unit_mut(id).expect("admitted lamp"), |unit| {
        &mut unit.searchlight
    })
}

/// Constructed emitter identities in stable order across anatomy stores.
pub(super) fn emitter_ids(world: &World) -> impl Iterator<Item = ObjectId> + '_ {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
}

/// Current emitter state shared by unit and terrain illumination.
pub(super) fn beam(world: &World, id: ObjectId) -> Option<(super::Position, super::Point, f64)> {
    let lamp = lamp_state(world, id)?;
    // Most battlefield units have no active beam. Reject them before building
    // a full scanner view; illumination is queried for every observed pair.
    if !lamp.on || lamp.destroyed {
        return None;
    }
    let scanner = super::scanner::scanner_unit(world, id)?;
    if scanner.destroyed
        || world
            .objects
            .get(&id)
            .is_none_or(|o| o.flags.contains(crate::Flag::Going))
    {
        return None;
    }
    Some((
        scanner.position?,
        scanner.point?,
        scanner.heading? + scanner.facing.torso.offset(),
    ))
}

/// Inferno lifetime is shared across unit and terrain lighting.
fn inferno(world: &World, id: ObjectId) -> u32 {
    world.btech.constructed_units().get(&id).map_or_else(
        || {
            world
                .btech
                .vehicles()
                .get(&id)
                .map_or(0, |u| u.inferno_remaining())
        },
        |u| u.inferno_remaining(),
    )
}

/// Request a guarded toggle; repeated requests preserve the existing transition.
/// Toggling is a manual choice, so the lamp leaves automatic mode and holds the new target.
pub fn toggle_searchlight(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    super::power::controlled(world, id, pilot)?;
    super::power::require_running_unit(world, id)?;
    let (lamp, installed) = hardware(world, id).context("Unit is unavailable")?;
    ensure!(installed, "Your 'mech isn't equipped with searchlight!");
    ensure!(
        !lamp.destroyed,
        "Your searchlight has been destroyed already!"
    );
    let on = lamp.on;
    // Both a fresh and an already pending switch end with the lamp inverted.
    hardware_mut(world, id).mode = if on {
        SearchlightMode::Off
    } else {
        SearchlightMode::On
    };
    let text = if lamp.remaining > 0 {
        if on {
            "Your searchlight is already in the process of turning off."
        } else {
            "Your searchlight is already in the process of turning on."
        }
    } else {
        hardware_mut(world, id).remaining = 5;
        if on {
            "Your searchlight starts to cool down."
        } else {
            "Your searchlight starts to warm up."
        }
    };
    let notice = Notice {
        unit: id,
        text: text.into(),
    };
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(notice)
}

/// Select a switching policy and immediately steer the lamp toward it.
/// The mode may be chosen while powered down; it then applies once startup completes.
pub fn set_searchlight_mode(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    mode: SearchlightMode,
) -> Result<Notice> {
    super::power::controlled(world, id, pilot)?;
    let (lamp, installed) = hardware(world, id).context("Unit is unavailable")?;
    ensure!(installed, "Your 'mech isn't equipped with searchlight!");
    ensure!(
        !lamp.destroyed,
        "Your searchlight has been destroyed already!"
    );
    hardware_mut(world, id).mode = mode;
    let mut text = match mode {
        SearchlightMode::Auto => {
            "Your searchlight will now switch on at night and off otherwise.".to_owned()
        }
        SearchlightMode::On => "Your searchlight is now set to stay on.".to_owned(),
        SearchlightMode::Off => "Your searchlight is now set to stay off.".to_owned(),
    };
    match reconcile(world, id) {
        Some(SearchlightAdjustment::WarmUp) => text.push_str(" It starts to warm up."),
        Some(SearchlightAdjustment::CoolDown) => text.push_str(" It starts to cool down."),
        Some(SearchlightAdjustment::Cancelled) => {
            text.push_str(" Its pending switch is cancelled.")
        }
        None => {}
    }
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(Notice { unit: id, text })
}

/// How [`reconcile`] steered a lamp toward its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchlightAdjustment {
    WarmUp,
    CoolDown,
    Cancelled,
}

/// The state a lamp's mode asks for, or `None` when automatic mode has no battlefield to read.
fn desired(world: &World, id: ObjectId, mode: SearchlightMode) -> Option<bool> {
    match mode {
        SearchlightMode::On => Some(true),
        SearchlightMode::Off => Some(false),
        SearchlightMode::Auto => {
            let position = super::scanner::scanner_unit(world, id)?.position?;
            let light = world.btech.maps().get(&position.map)?.light_level().ok()?;
            Some(light == super::Light::Night)
        }
    }
}

/// Schedule or cancel a five-second switch so an intact, running lamp converges on its mode.
/// Called from events that can change the answer rather than from the simulation tick; the
/// tick's switch completion publishes the result.
pub(super) fn reconcile(world: &mut World, id: ObjectId) -> Option<SearchlightAdjustment> {
    let (lamp, installed) = hardware(world, id)?;
    if !installed || lamp.destroyed {
        return None;
    }
    let scanner = super::scanner::scanner_unit(world, id)?;
    if scanner.power != Power::Running
        || scanner.destroyed
        || world
            .objects
            .get(&id)
            .is_none_or(|o| o.flags.contains(crate::Flag::Going))
    {
        return None;
    }
    let want = desired(world, id, lamp.mode)?;
    let pending = lamp.remaining > 0;
    if (lamp.on != pending) == want {
        return None;
    }
    let lamp = hardware_mut(world, id);
    if pending {
        // The lamp already shows the wanted state; drop the switch away from it.
        lamp.remaining = 0;
        return Some(SearchlightAdjustment::Cancelled);
    }
    lamp.remaining = 5;
    Some(if want {
        SearchlightAdjustment::WarmUp
    } else {
        SearchlightAdjustment::CoolDown
    })
}

/// Re-evaluate every lamp on one battlefield after its light level changes.
pub(super) fn reconcile_map(world: &mut World, map: ObjectId) {
    let ids: Vec<_> = emitter_ids(world)
        .filter(|&id| {
            super::scanner::scanner_unit(world, id)
                .and_then(|unit| unit.position)
                .is_some_and(|position| position.map == map)
        })
        .collect();
    for id in ids {
        reconcile(world, id);
    }
}

/// Advance switches atomically with the server tick; an unpowered expiry leaves the lamp unchanged.
pub fn advance_searchlights(world: &mut World) -> Vec<Notice> {
    let ids: Vec<_> = emitter_ids(world)
        .filter(|&id| hardware(world, id).is_some_and(|(lamp, _)| lamp.remaining > 0))
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let scanner = super::scanner::scanner_unit(world, id).expect("constructed emitter");
        let running = scanner.power == Power::Running && !scanner.destroyed;
        let lamp = hardware_mut(world, id);
        lamp.remaining -= 1;
        if lamp.remaining > 0 || !running {
            continue;
        }
        lamp.on = !lamp.on;
        let on = lamp.on;
        notices.push(Notice {
            unit: id,
            text: if on {
                "Your searchlight comes on to full power."
            } else {
                "Your searchlight shuts off."
            }
            .into(),
        });
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            if on {
                "turns on a searchlight!"
            } else {
                "turns off a searchlight!"
            },
        ));
    }
    notices
}

/// A target is lit by inferno fire, its own lamp, scenario lighting or an unobstructed forward beam.
pub fn unit_illuminated(world: &World, target: ObjectId) -> bool {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Illumination,
    );
    if world
        .objects
        .get(&target)
        .is_none_or(|o| o.flags.contains(crate::Flag::Going))
    {
        return false;
    }
    lamp_state(world, target).is_some_and(|_| {
        inferno(world, target) > 0
            || laser_heat_sink_glow(world, target)
            || beam(world, target).is_some()
            || externally_illuminated(world, target)
    })
}

/// Running laser heat sinks glow, so the unit counts as illuminated in darkness.
fn laser_heat_sink_glow(world: &World, target: ObjectId) -> bool {
    let technology = super::Technology::LaserHeatSinks;
    world
        .btech
        .constructed_units()
        .get(&target)
        .map(|unit| {
            unit.definition().has_technology(technology) && unit.power() == super::Power::Running
        })
        .or_else(|| {
            world.btech.vehicles().get(&target).map(|unit| {
                unit.definition().has_technology(technology)
                    && unit.power() == super::Power::Running
            })
        })
        .unwrap_or(false)
}

/// Scenario lighting, nearby infernos and other lamps can trigger external-light warnings.
fn externally_illuminated(world: &World, target: ObjectId) -> bool {
    externally_illuminated_with_sources(world, target, illumination_sources(world))
}

fn externally_illuminated_with_sources(
    world: &World,
    target: ObjectId,
    emitters: impl Iterator<Item = (ObjectId, bool)>,
) -> bool {
    if world
        .objects
        .get(&target)
        .is_none_or(|o| o.flags.contains(crate::Flag::Going))
    {
        return false;
    }
    let unit = world.btech.constructed_units().get(&target);
    if unit.is_some_and(|unit| unit.signature().illuminated)
        || world
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|vehicle| vehicle.signature().illuminated)
    {
        return true;
    }
    let Some(position) = unit.and_then(|unit| unit.position()).or_else(|| {
        world
            .btech
            .vehicles()
            .get(&target)
            .and_then(|vehicle| vehicle.position())
    }) else {
        return false;
    };
    external_sources_illuminate(world, target, position, emitters)
}

/// Candidates are derived once while the caller holds an immutable world borrow.
fn illumination_sources(world: &World) -> impl Iterator<Item = (ObjectId, bool)> + '_ {
    world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| {
            let burning = unit.inferno_remaining() > 0;
            (burning || (unit.searchlight.on && !unit.searchlight.destroyed))
                .then_some((id, burning))
        })
        .chain(world.btech.vehicles().iter().filter_map(|(&id, unit)| {
            let burning = unit.inferno_remaining() > 0;
            (burning || (unit.searchlight.on && !unit.searchlight.destroyed))
                .then_some((id, burning))
        }))
}

fn external_sources_illuminate(
    world: &World,
    target: ObjectId,
    position: super::Position,
    emitters: impl Iterator<Item = (ObjectId, bool)>,
) -> bool {
    emitters.into_iter().any(|(source, burning)| {
        if source == target
            || world
                .objects
                .get(&source)
                .is_none_or(|o| o.flags.contains(crate::Flag::Going))
        {
            return false;
        }
        let Some(scanner) = super::scanner::scanner_unit(world, source) else {
            return false;
        };
        let Some(source_position) = scanner.position.filter(|p| p.map == position.map) else {
            return false;
        };
        let source_hex = super::HexCoordinate {
            x: i32::from(source_position.x),
            y: i32::from(source_position.y),
        };
        let target_hex = super::HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        };
        if burning && source_hex.distance(target_hex) <= 1 {
            return true;
        }
        let Some((_, _, heading)) = beam(world, source) else {
            return false;
        };
        let Ok(range) = super::unit_range(world, source, target) else {
            return false;
        };
        let angle = (range.bearing.unwrap_or(heading) - heading).rem_euclid(360.0);
        range.spatial <= 30.0
            && !(angle > 60.0 && angle < 300.0)
            && super::unit_terrain_los(world, source, target)
                .is_ok_and(|los| !los.blocked && los.woods <= 2 && los.water == 0)
    })
}

impl Mech {
    /// Whether changes in external illumination notify the occupants.
    pub fn searchlight_warning(&self) -> bool {
        self.searchlight_warning
    }
}

/// Iterate saved observation cursors without imposing a chassis-specific sensor policy.
fn illumination_observations(world: &World) -> impl Iterator<Item = (ObjectId, bool)> + '_ {
    world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.illumination_observed))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.illumination_observed)),
        )
}

/// Detect notification work even when both the lamps and their targets are stationary.
pub fn illumination_pending(world: &World) -> bool {
    illumination_observations(world)
        .any(|(id, observed)| observed != externally_illuminated(world, id))
}

/// Commit illumination observations and emit each opted-in transition once.
/// Notification cursors share the world transaction; they never decide visibility.
pub fn refresh_illumination(world: &mut World) -> Vec<Notice> {
    let changes: Vec<_> = illumination_observations(world)
        .filter_map(|(id, observed)| {
            let lit = externally_illuminated(world, id);
            (observed != lit).then_some((id, lit))
        })
        .collect();
    let mut notices = Vec::new();
    for (id, lit) in changes {
        let warning = super::with_unit_mut!(
            world.btech.unit_mut(id).expect("observed vehicle"),
            |unit| {
                unit.illumination_observed = lit;
                unit.searchlight_warning
            }
        );
        if warning
            && world
                .objects
                .get(&id)
                .is_some_and(|o| !o.flags.contains(crate::Flag::Going))
        {
            notices.push(Notice {
                unit: id,
                text: if lit {
                    "You are being illuminated!"
                } else {
                    "You are no longer being illuminated."
                }
                .into(),
            });
        }
    }
    notices
}

/// Resolve an exposed lamp's common destruction rolls before the caller applies armor damage.
/// Return notices separately so material-only callers can discard publication.
pub(super) fn strike(world: &mut World, id: ObjectId) -> Option<(Vec<Notice>, Vec<Notice>)> {
    let (lamp, installed) = hardware(world, id)?;
    if !installed || lamp.destroyed {
        return None;
    }
    let dice = crate::btech::with_unit_mut!(world.btech.unit_mut(id)?, |unit| { &mut unit.dice });
    if dice.generic_roll() <= 6 || (!lamp.on && dice.generic_roll() <= 5) {
        return None;
    }
    let broadcasts =
        super::broadcast::observer_notices(world, id, "'s searchlight is blown apart!");
    *hardware_mut(world, id) = Searchlight {
        destroyed: true,
        mode: lamp.mode,
        ..Default::default()
    };
    Some((
        vec![Notice {
            unit: id,
            text: "[fg=yellow bold]Your searchlight is destroyed![reset]".into(),
        }],
        broadcasts,
    ))
}

impl Searchlight {
    /// Cut lamp power during shutdown; any pending countdown can expire without switching on.
    pub(super) fn shutdown(&mut self) -> bool {
        std::mem::take(&mut self.on)
    }
}

/// Illumination reused only inside one read-only observation, never across world mutations.
pub(super) struct IlluminationContext<'w> {
    world: &'w World,
    sources: std::cell::OnceCell<Vec<(ObjectId, bool)>>,
    targets: std::cell::RefCell<std::collections::BTreeMap<ObjectId, bool>>,
}
impl<'w> IlluminationContext<'w> {
    pub(super) fn new(world: &'w World) -> Self {
        Self {
            world,
            sources: Default::default(),
            targets: Default::default(),
        }
    }
    pub(super) fn illuminated(&self, target: ObjectId) -> bool {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Illumination,
        );
        if let Some(value) = self.targets.borrow().get(&target) {
            return *value;
        }
        let world = self.world;
        let live = world
            .objects
            .get(&target)
            .is_some_and(|o| !o.flags.contains(crate::Flag::Going));
        let value = live
            && lamp_state(world, target).is_some_and(|_| {
                inferno(world, target) > 0
                    || laser_heat_sink_glow(world, target)
                    || beam(world, target).is_some()
                    || externally_illuminated_with_sources(
                        world,
                        target,
                        self.sources
                            .get_or_init(|| illumination_sources(world).collect())
                            .iter()
                            .copied(),
                    )
            });
        self.targets.borrow_mut().insert(target, value);
        value
    }
}
