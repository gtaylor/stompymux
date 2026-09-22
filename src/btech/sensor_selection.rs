//! Durable optical mode selection and committed ten-second switching countdowns.
use super::{BattleNotice, BattlePower, BattleSensorMode as Sensor, BattleUnit};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Primary and secondary optical modes currently installed in the scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSensorPair {
    pub primary: Sensor,
    pub secondary: Sensor,
}
impl Default for BattleSensorPair {
    fn default() -> Self {
        Self {
            primary: Sensor::Visual,
            secondary: Sensor::Visual,
        }
    }
}

/// An in-progress selection keeps the old pair active until its countdown expires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSensorChange {
    pub wanted: BattleSensorPair,
    pub remaining: u8,
}

/// Owned sensor settings and pending change, serialized with the unit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleSensorSelection {
    pub active: BattleSensorPair,
    pub pending: Option<BattleSensorChange>,
}

impl BattleSensorSelection {
    /// Compact active pair, followed by uppercase requested sensors while a switch is pending.
    pub fn field_text(self) -> String {
        let mut text =
            String::from_iter([self.active.primary.letter(), self.active.secondary.letter()]);
        if let Some(change) = self.pending {
            text.push(change.wanted.primary.letter().to_ascii_uppercase());
            text.push(change.wanted.secondary.letter().to_ascii_uppercase());
        }
        text
    }
}

impl BattleSensorPair {
    /// Validate equipment for both slots, including requests restored from storage.
    pub(super) fn supported_by(self, unit: &BattleUnit) -> bool {
        ((self.primary != Sensor::Radar && self.secondary != Sensor::Radar) || unit.has_radar())
            && [self.primary, self.secondary].into_iter().all(|sensor| {
                sensor
                    .active_probe()
                    .is_none_or(|probe| unit.has_active_probe(probe).unwrap_or(false))
            })
    }

    /// A damaged probe cannot become active when selection is requested or completed.
    fn available_on(self, unit: &BattleUnit) -> bool {
        self.supported_by(unit)
            && [self.primary, self.secondary].into_iter().all(|sensor| {
                sensor
                    .active_probe()
                    .is_none_or(|probe| unit.active_probe_available(probe).unwrap_or(false))
            })
    }

    /// Validate vehicle installation independently of current damage or query implementation.
    pub(super) fn supported_by_vehicle(self, unit: &super::BattleVehicle) -> bool {
        [self.primary, self.secondary].into_iter().all(|sensor| {
            (sensor != Sensor::Radar || unit.definition().has_special("AntiAircraft"))
                && sensor
                    .active_probe()
                    .is_none_or(|probe| unit.has_active_probe(probe).unwrap_or(false))
        })
    }

    /// Requested probe modes need a surviving installation at selection and completion.
    fn available_on_vehicle(self, unit: &super::BattleVehicle) -> bool {
        self.supported_by_vehicle(unit)
            && [self.primary, self.secondary].into_iter().all(|sensor| {
                sensor
                    .active_probe()
                    .is_none_or(|probe| unit.active_probe_available(probe).unwrap_or(false))
            })
    }

    fn requires_darkness(self) -> bool {
        self.primary == Sensor::LightAmplification || self.secondary == Sensor::LightAmplification
    }

    fn daylight(self) -> Self {
        Self {
            primary: if self.primary == Sensor::LightAmplification {
                Sensor::Visual
            } else {
                self.primary
            },
            secondary: if self.secondary == Sensor::LightAmplification {
                Sensor::Visual
            } else {
                self.secondary
            },
        }
    }
}

impl BattleUnit {
    /// Current and requested optical modes, excluding all random/contact state.
    pub fn sensor_selection(&self) -> BattleSensorSelection {
        self.sensor_selection
    }
}

impl super::BattleVehicle {
    /// Active and pending sensors, persisted independently of contact acquisition.
    pub fn sensor_selection(&self) -> BattleSensorSelection {
        self.sensor_selection
    }
}

/// Schedule a controlled mode change; repeating the active pair leaves a pending request alone.
pub fn select_optical_sensors(
    world: &mut World,
    unit: ObjectId,
    pilot: ObjectId,
    wanted: BattleSensorPair,
) -> Result<()> {
    if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        super::vehicle_power::controlled(world, unit, pilot)?;
        ensure!(
            vehicle.power() == BattlePower::Running,
            "Start the unit first"
        );
        ensure!(
            wanted.available_on_vehicle(vehicle),
            "You lack the requested working sensors!"
        );
        if vehicle.sensor_selection.active == wanted {
            return Ok(());
        }
        let position = vehicle.position().context("Unit is not placed")?;
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Map not found")?;
        ensure!(
            !wanted.requires_darkness() || map.light <= 1,
            "Too bright for light-amplification sensors"
        );
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&unit)
            .unwrap()
            .sensor_selection
            .pending = Some(BattleSensorChange {
            wanted,
            remaining: 10,
        });
        let _ = super::autopilot::manual_takeover(world, unit);
        return Ok(());
    }
    super::power::controlled_unit(world, unit, pilot)?;
    let current = &world.btech.constructed_units()[&unit];
    ensure!(
        current.power() == BattlePower::Running,
        "Start the unit first"
    );
    ensure!(
        wanted.available_on(current),
        "You lack the requested working sensors!"
    );
    if current.sensor_selection.active == wanted {
        return Ok(());
    }
    let position = current.position().context("Unit is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    ensure!(
        !wanted.requires_darkness() || map.light <= 1,
        "Too bright for light-amplification sensors"
    );
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&unit)
        .unwrap()
        .sensor_selection
        .pending = Some(BattleSensorChange {
        wanted,
        remaining: 10,
    });
    let _ = super::autopilot::manual_takeover(world, unit);
    Ok(())
}

/// Recheck only distinct active sensor modes; a matching secondary slot is left unchanged.
/// The reference light-change operation does not clear locks or cancel pending selections.
fn recheck_light(pair: &mut BattleSensorPair) -> bool {
    let same = pair.primary == pair.secondary;
    let mut changed = false;
    if pair.primary == Sensor::LightAmplification {
        pair.primary = Sensor::Visual;
        changed = true;
    }
    if !same && pair.secondary == Sensor::LightAmplification {
        pair.secondary = Sensor::Visual;
        changed = true;
    }
    changed
}

/// Apply a changed map light in membership order, with cockpit warnings only for live running units.
pub(super) fn reconcile_map_light(world: &mut World, map: ObjectId) -> Vec<BattleNotice> {
    if world.btech.maps()[&map].light <= 1 {
        return Vec::new();
    }
    let mut members: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(_, u)| u.position().is_some_and(|p| p.map == map))
        .map(|(&id, u)| (u.map_slot(), id))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .filter(|(_, u)| u.position().is_some_and(|p| p.map == map))
                .map(|(&id, u)| (u.map_slot(), id)),
        )
        .collect();
    members.sort_unstable();
    let mut notices = Vec::new();
    for (_, id) in members {
        let (changed, audible) =
            if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
                (
                    recheck_light(&mut unit.sensor_selection.active),
                    unit.power() == BattlePower::Running && !unit.is_destroyed(),
                )
            } else {
                let unit = Arc::make_mut(&mut world.btech.vehicles)
                    .get_mut(&id)
                    .unwrap();
                (
                    recheck_light(&mut unit.sensor_selection.active),
                    unit.power() == BattlePower::Running && !unit.is_destroyed(),
                )
            };
        if changed && audible {
            notices.push(BattleNotice {
                unit: id,
                text: "The light's kinda too bright now to use Light-amplification!".into(),
            });
        }
    }
    notices
}

/// Advance even on stopped units; an expired request is discarded unless the engine is running.
pub fn advance_sensor_selection(world: &mut World) -> Vec<BattleNotice> {
    let updates: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| {
            let pending = unit.sensor_selection.pending?;
            let bright = unit
                .position()
                .and_then(|p| world.btech.maps().get(&p.map))
                .is_none_or(|map| map.light > 1);
            Some((id, pending, bright))
        })
        .collect();
    let mut notices = Vec::new();
    for (id, pending, bright) in updates {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap();
        let available = pending.remaining == 1
            && unit.power() == BattlePower::Running
            && !unit.is_destroyed()
            && pending.wanted.available_on(unit);
        if !advance_change(&mut unit.sensor_selection, pending, bright, available) {
            continue;
        }
        unit.target_lock = None;
        notices.push(BattleNotice {
            unit: id,
            text: "Your sensors finish changing.".to_owned(),
        });
    }
    let updates: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, unit)| {
            let pending = unit.sensor_selection.pending?;
            let bright = unit
                .position()
                .and_then(|position| world.btech.maps().get(&position.map))
                .is_none_or(|map| map.light > 1);
            Some((id, pending, bright))
        })
        .collect();
    for (id, pending, bright) in updates {
        let unit = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap();
        let available = pending.remaining == 1
            && unit.power() == BattlePower::Running
            && !unit.is_destroyed()
            && pending.wanted.available_on_vehicle(unit);
        if !advance_change(&mut unit.sensor_selection, pending, bright, available) {
            continue;
        }
        unit.target_lock = None;
        notices.push(BattleNotice {
            unit: id,
            text: "Your sensors finish changing.".into(),
        });
    }
    notices
}

/// Advance one request, discarding an expired unavailable selection and reporting successful completion.
fn advance_change(
    selection: &mut BattleSensorSelection,
    mut pending: BattleSensorChange,
    bright: bool,
    available: bool,
) -> bool {
    pending.remaining = pending.remaining.saturating_sub(1);
    if pending.remaining > 0 {
        selection.pending = Some(pending);
        return false;
    }
    selection.pending = None;
    if !available {
        return false;
    }
    selection.active = if bright {
        pending.wanted.daylight()
    } else {
        pending.wanted
    };
    true
}

impl std::str::FromStr for Sensor {
    type Err = anyhow::Error;

    /// Parse supported optical sensor names and command letters.
    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "b" | "beagle" | "beagle_probe" => Ok(Self::BeagleProbe),
            "a" | "light_probe" => Ok(Self::LightProbe),
            "h" | "bloodhound" | "bloodhound_probe" => Ok(Self::BloodhoundProbe),
            "r" | "radar" => Ok(Self::Radar),
            "e" | "electromagnetic" => Ok(Self::Electromagnetic),
            "s" | "seismic" => Ok(Self::Seismic),
            "i" | "infrared" => Ok(Self::Infrared),
            "v" | "visual" | "vislight" => Ok(Self::Visual),
            "l" | "light-amplification" | "light_amplification" => Ok(Self::LightAmplification),
            _ => anyhow::bail!(
                "Supported sensors: V (visual), L (light-amplification), I (infrared), S (seismic), E (electromagnetic), R (radar), B (Beagle), A (light probe), H (Bloodhound)"
            ),
        }
    }
}

impl Sensor {
    /// Compact command identity shared by sensor field inspection.
    fn letter(self) -> char {
        match self {
            Self::Visual => 'v',
            Self::LightAmplification => 'l',
            Self::Infrared => 'i',
            Self::Electromagnetic => 'e',
            Self::Seismic => 's',
            Self::Radar => 'r',
            Self::BeagleProbe => 'b',
            Self::LightProbe => 'a',
            Self::BloodhoundProbe => 'h',
        }
    }

    /// Stable player-facing name for optical mode inspection.
    pub fn name(self) -> &'static str {
        match self {
            Self::Visual => "Visual",
            Self::Infrared => "Infrared",
            Self::Seismic => "Seismic",
            Self::Electromagnetic => "Electromagnetic",
            Self::Radar => "Radar",
            Self::BeagleProbe => "Beagle ActiveProbe",
            Self::LightProbe => "Light Beagle ActiveProbe",
            Self::BloodhoundProbe => "Bloodhound ActiveProbe",
            Self::LightAmplification => "Light-amplification",
        }
    }
}
