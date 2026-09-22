//! Installed electronic suites, cockpit controls and committed map-field observations.
use super::{
    BattleElectronicField, BattleElectronicMode as Mode, BattleElectronicSource, BattleNotice,
    BattlePower, BattleSystem, BattleUnit,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Independently controlled electronic-warfare suite families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleElectronicSuite {
    Guardian,
    Angel,
}

/// Selected modes and the last committed field observation; emissions are derived from equipment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleElectronics {
    pub guardian: Mode,
    pub angel: Mode,
    pub field: BattleElectronicField,
}

impl BattleElectronicSuite {
    /// Critical identity implementing this suite.
    fn system(self) -> BattleSystem {
        match self {
            Self::Guardian => BattleSystem::Ecm,
            Self::Angel => BattleSystem::AngelEcm,
        }
    }
}

impl BattleUnit {
    /// Saved electronic controls and last committed field observation.
    pub fn electronics(&self) -> BattleElectronics {
        self.electronics
    }

    /// Guardian presence is sufficient; a biped Angel suite needs two surviving slots.
    /// A destroyed or flooded part disables the whole corresponding suite family.
    pub fn electronic_suite_available(&self, suite: BattleElectronicSuite) -> Result<bool> {
        let loadout = self.loadout()?;
        let parts: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == suite.system())
            .collect();
        let minimum = if suite == BattleElectronicSuite::Angel {
            2
        } else {
            1
        };
        Ok(parts.len() >= minimum
            && parts
                .iter()
                .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Shutdown clears both selections; damage disables only the affected suite family.
    pub(super) fn reconcile_electronics(&mut self) {
        self.reconcile_active_probes();
        self.reconcile_stealth();
        self.reconcile_null_signature();
        let guardian = self
            .electronic_suite_available(BattleElectronicSuite::Guardian)
            .unwrap_or(false);
        let angel = self
            .electronic_suite_available(BattleElectronicSuite::Angel)
            .unwrap_or(false);
        self.electronics
            .reconcile(self.power == BattlePower::Running, guardian, angel);
    }
}

impl BattleElectronics {
    /// Clear emissions when power or the corresponding equipment is lost.
    fn reconcile(&mut self, running: bool, guardian: bool, angel: bool) {
        if !running || !guardian {
            self.guardian = Mode::Off;
        }
        if !running || !angel {
            self.angel = Mode::Off;
        }
    }
}

impl super::BattleVehicle {
    /// Saved electronic controls and last committed field observation.
    pub fn electronics(&self) -> BattleElectronics {
        self.electronics
    }

    /// Vehicle electronic suites occupy independent equipment slots.
    pub fn electronic_suite_available(&self, suite: BattleElectronicSuite) -> Result<bool> {
        let loadout = self.loadout()?;
        let parts: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == suite.system())
            .collect();
        Ok(!parts.is_empty()
            && parts
                .iter()
                .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Reconcile equipment-dependent controls after damage or shutdown.
    pub(super) fn reconcile_electronics(&mut self) {
        self.reconcile_active_probes();
        if !self.c3_operational().unwrap_or(false) {
            self.c3_network = None;
        }
        if !self.c3_hardware().is_ok_and(|h| h.c3i_operational) {
            self.c3i_network = None;
        }
        let guardian = self
            .electronic_suite_available(BattleElectronicSuite::Guardian)
            .unwrap_or(false);
        let angel = self
            .electronic_suite_available(BattleElectronicSuite::Angel)
            .unwrap_or(false);
        self.electronics
            .reconcile(self.power == BattlePower::Running, guardian, angel);
    }
}

/// Borrow the common electronic state from either construction store.
fn state(world: &World, id: ObjectId) -> Result<BattleElectronics> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        return Ok(vehicle.electronics());
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?
        .electronics())
}

/// Borrow common controls while keeping construction-specific storage private.
fn state_mut(world: &mut World, id: ObjectId) -> &mut BattleElectronics {
    if world.btech.vehicles().contains_key(&id) {
        return &mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .electronics;
    }
    &mut Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap()
        .electronics
}

/// Query construction-specific equipment availability for a common suite.
fn available(world: &World, id: ObjectId, suite: BattleElectronicSuite) -> Result<bool> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        return vehicle.electronic_suite_available(suite);
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?
        .electronic_suite_available(suite)
}

/// Visit both stores in a stable order for field snapshots.
fn identities(world: &World) -> impl Iterator<Item = ObjectId> + '_ {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
}

/// Select an exclusive suite mode, toggling off when the same mode is selected again.
pub fn toggle_electronics(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    suite: BattleElectronicSuite,
    requested: Mode,
) -> Result<Mode> {
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
    } else {
        super::power::controlled_unit(world, id, pilot)?;
    }
    let unit = super::scanner::scanner_unit(world, id)
        .context("Unit construction state is unavailable")?;
    ensure!(unit.power == BattlePower::Running, "Start the unit first");
    ensure!(unit.position.is_some(), "Unit is not placed");
    ensure!(
        available(world, id, suite)?,
        "This unit does not have a working {suite:?} ECM suite"
    );
    let mode = {
        let electronics = state_mut(world, id);
        let mode = match suite {
            BattleElectronicSuite::Guardian => &mut electronics.guardian,
            BattleElectronicSuite::Angel => &mut electronics.angel,
        };
        *mode = mode.toggle(requested);
        *mode
    };
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(mode)
}

/// Compute current effects from same-map emitters without consuming dice or changing observations.
pub fn electronic_field(world: &World, id: ObjectId) -> Result<BattleElectronicField> {
    let (position, previous, team, self_interference) =
        if let Some(vehicle) = world.btech.vehicles().get(&id) {
            (
                vehicle.position(),
                vehicle.electronics.field,
                vehicle.sensor_signature().team,
                vehicle.has_beacon(super::BattleBeaconKind::Ecm),
            )
        } else {
            let unit = world
                .btech
                .constructed_units()
                .get(&id)
                .context("Unit construction state is unavailable")?;
            (
                unit.position(),
                unit.electronics.field,
                unit.sensor_signature().team,
                unit.stealth().enabled || unit.has_beacon(super::BattleBeaconKind::Ecm),
            )
        };
    let Some(position) = position else {
        return Ok(BattleElectronicField::default());
    };
    let mut sources = Vec::new();
    for other in identities(world) {
        let emitter = super::scanner::scanner_unit(world, other).unwrap();
        if emitter.position.is_none_or(|p| p.map != position.map)
            || emitter.power != BattlePower::Running
            || emitter.destroyed
            || world
                .objects
                .get(&other)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let state = state(world, other)?;
        if state.guardian == Mode::Off && state.angel == Mode::Off {
            continue;
        }
        let distance = super::unit_range(world, id, other)?.spatial;
        if distance > 6.0 {
            continue;
        }
        sources.push(BattleElectronicSource {
            team: emitter.signature.team,
            distance,
            guardian: if available(world, other, BattleElectronicSuite::Guardian)? {
                state.guardian
            } else {
                Mode::Off
            },
            angel: if available(world, other, BattleElectronicSuite::Angel)? {
                state.angel
            } else {
                Mode::Off
            },
            personal: Mode::Off,
        });
    }
    super::resolve_electronic_field(previous, team, sources, self_interference)
}

/// Recompute every observation from one world snapshot, then publish changes together.
pub fn refresh_electronic_fields(world: &mut World) -> Result<Vec<BattleNotice>> {
    if !electronic_fields_pending(world) {
        return Ok(Vec::new());
    }
    let observations = identities(world)
        .filter(|id| {
            world
                .objects
                .get(id)
                .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|id| {
            let field = electronic_field(world, id)?;
            let working = available(world, id, BattleElectronicSuite::Guardian)?
                || available(world, id, BattleElectronicSuite::Angel)?;
            Ok((
                id,
                field,
                field.notices(state(world, id)?.field, id, working),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut notices = Vec::new();
    for (id, field, changes) in observations {
        state_mut(world, id).field = field;
        notices.extend(changes);
    }
    Ok(notices)
}

/// Commit one receiver after a local pod effect without refreshing unrelated units.
pub(super) fn refresh_receiver(world: &mut World, id: ObjectId) -> Result<Vec<BattleNotice>> {
    let field = electronic_field(world, id)?;
    let working = available(world, id, BattleElectronicSuite::Guardian)?
        || available(world, id, BattleElectronicSuite::Angel)?;
    let notices = field.notices(state(world, id)?.field, id, working);
    state_mut(world, id).field = field;
    Ok(notices)
}

/// Active emitters and uncleared observations require a world-field heartbeat.
pub fn electronic_fields_pending(world: &World) -> bool {
    identities(world).any(|id| {
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going))
            && (state(world, id).is_ok_and(|state| state != BattleElectronics::default())
                || world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .is_some_and(|unit| {
                        unit.stealth().enabled || unit.has_beacon(super::BattleBeaconKind::Ecm)
                    })
                || world
                    .btech
                    .vehicles()
                    .get(&id)
                    .is_some_and(|unit| unit.has_beacon(super::BattleBeaconKind::Ecm)))
    })
}

/// Native and Lua controls share mode publication and complete rollback on notification failure.
pub(crate) fn configure(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
    suite: BattleElectronicSuite,
    requested: Mode,
) -> Result<Mode> {
    let before = scripts.world.borrow().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let mode =
            toggle_electronics(&mut scripts.world.borrow_mut(), id, pilot, suite, requested)?;
        let name = if suite == BattleElectronicSuite::Angel {
            "Angel ECM"
        } else {
            "ECM"
        };
        let text = match mode {
            Mode::Off => format!("You turn your {name} suite offline."),
            Mode::Ecm => format!("You turn your {name} suite online (ECM mode)."),
            Mode::Eccm => format!("You turn your {name} suite online (ECCM mode)."),
        };
        super::notify_unit_text(scripts, id, &text)?;
        let notices = refresh_electronic_fields(&mut scripts.world.borrow_mut())?;
        for notice in notices {
            super::notify_unit_text(scripts, notice.unit, &notice.text)?;
        }
        Ok(mode)
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Four cockpit commands select Guardian or Angel ECM/ECCM modes.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let suite = if input.name.starts_with('a') {
        BattleElectronicSuite::Angel
    } else {
        BattleElectronicSuite::Guardian
    };
    let mode = if input.name.ends_with("eccm") {
        Mode::Eccm
    } else {
        Mode::Ecm
    };
    let id = ctx
        .scripts
        .world
        .borrow()
        .objects
        .get(&ctx.player)
        .and_then(|player| player.location);
    let result = id
        .context("Enter a unit first")
        .and_then(|id| configure(ctx.scripts, id, ctx.player, suite, mode));
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
