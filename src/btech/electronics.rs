//! Installed electronic suites, cockpit controls and committed map-field observations.
use super::{
    ElectronicField, ElectronicMode as Mode, ElectronicSource, Mech, Notice, Power, System,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Independently controlled electronic-warfare suite families. Communications equipment
/// generates ECCM only (Tactical Operations: Advanced Rules p. 98).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElectronicSuite {
    Guardian,
    Angel,
    Communications,
}

/// Selected modes and the last committed field observation; emissions are derived from equipment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Electronics {
    pub guardian: Mode,
    pub angel: Mode,
    /// Communications equipment generating an ECCM field, or off; it never generates ECM.
    #[serde(default)]
    pub communications: Mode,
    pub field: ElectronicField,
}

/// The fewest tons of communications equipment that generate an ECCM field (Tactical
/// Operations: Advanced Rules p. 98).
pub const COMMUNICATIONS_ECCM_TONS: u8 = 3;

impl ElectronicSuite {
    /// Critical identity implementing this suite.
    fn system(self) -> System {
        match self {
            Self::Guardian => System::Ecm,
            Self::Angel => System::AngelEcm,
            Self::Communications => System::CommunicationsEquipment,
        }
    }
}

impl Mech {
    /// Saved electronic controls and last committed field observation.
    pub fn electronics(&self) -> Electronics {
        self.electronics
    }

    /// Guardian presence is sufficient; a biped Angel suite needs two surviving slots.
    /// A destroyed or flooded part disables the whole corresponding suite family.
    /// Communications equipment generates ECCM from three tons, a ton to each slot, while its
    /// slots survive and the Mech's sensors and cockpit are undamaged (TechManual p. 212;
    /// Tactical Operations: Advanced Rules p. 98).
    pub fn electronic_suite_available(&self, suite: ElectronicSuite) -> Result<bool> {
        let loadout = self.loadout()?;
        let parts: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == suite.system())
            .collect();
        let minimum = match suite {
            ElectronicSuite::Guardian => 1,
            ElectronicSuite::Angel => 2,
            ElectronicSuite::Communications => usize::from(COMMUNICATIONS_ECCM_TONS),
        };
        if suite == ElectronicSuite::Communications
            && (self.system_hits(System::Sensors) > 0 || self.system_hits(System::Cockpit) > 0)
        {
            return Ok(false);
        }
        Ok(parts.len() >= minimum
            && parts
                .iter()
                .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Tons of communications equipment installed: a ton to each slot (TechManual p. 212).
    pub fn communications_tons(&self) -> Result<u8> {
        let count = self
            .loadout()?
            .systems
            .iter()
            .filter(|part| part.system == System::CommunicationsEquipment)
            .count();
        Ok(u8::try_from(count).unwrap_or(u8::MAX))
    }

    /// Shutdown clears both selections; damage disables only the affected suite family.
    pub(super) fn reconcile_electronics(&mut self) {
        self.reconcile_stealth();
        self.reconcile_null_signature();
        let guardian = self
            .electronic_suite_available(ElectronicSuite::Guardian)
            .unwrap_or(false);
        let angel = self
            .electronic_suite_available(ElectronicSuite::Angel)
            .unwrap_or(false);
        let communications = self
            .electronic_suite_available(ElectronicSuite::Communications)
            .unwrap_or(false);
        self.electronics.reconcile(
            self.power == Power::Running,
            guardian,
            angel,
            communications,
        );
    }
}

impl Electronics {
    /// Clear emissions when power or the corresponding equipment is lost.
    fn reconcile(&mut self, running: bool, guardian: bool, angel: bool, communications: bool) {
        if !running || !guardian {
            self.guardian = Mode::Off;
        }
        if !running || !angel {
            self.angel = Mode::Off;
        }
        if !running || !communications {
            self.communications = Mode::Off;
        }
    }
}

impl super::Vehicle {
    /// Saved electronic controls and last committed field observation.
    pub fn electronics(&self) -> Electronics {
        self.electronics
    }

    /// Vehicle electronic suites occupy independent equipment slots. Communications equipment
    /// generates ECCM from three tons while it survives and the vehicle's sensors are
    /// undamaged (TechManual p. 212; Tactical Operations: Advanced Rules p. 98).
    pub fn electronic_suite_available(&self, suite: ElectronicSuite) -> Result<bool> {
        let loadout = self.loadout()?;
        let parts: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == suite.system())
            .collect();
        if suite == ElectronicSuite::Communications
            && (self.sensor_hits() > 0 || self.communications_tons()? < COMMUNICATIONS_ECCM_TONS)
        {
            return Ok(false);
        }
        Ok(!parts.is_empty()
            && parts
                .iter()
                .all(|part| !self.critical_unavailable(part.location)))
    }

    /// Tons of communications equipment installed: the tons its one item states (TechManual
    /// p. 212).
    pub fn communications_tons(&self) -> Result<u8> {
        let loadout = self.loadout()?;
        Ok(loadout
            .systems
            .iter()
            .filter_map(|part| self.definition().communications_tons(part))
            .sum())
    }

    /// Reconcile equipment-dependent controls after damage or shutdown.
    pub(super) fn reconcile_electronics(&mut self) {
        self.settle_command_networks();
        self.electronics = self.settled_electronics();
    }

    /// Leave command networks whose computers no longer work.
    pub(super) fn settle_command_networks(&mut self) {
        if !self.c3_operational().unwrap_or(false) {
            self.c3_network = None;
        }
        if !self.c3_hardware().is_ok_and(|h| h.c3i_operational) {
            self.c3i_network = None;
        }
    }

    /// Emission controls after power or equipment loss switches off what cannot run.
    fn settled_electronics(&self) -> Electronics {
        let guardian = self
            .electronic_suite_available(ElectronicSuite::Guardian)
            .unwrap_or(false);
        let angel = self
            .electronic_suite_available(ElectronicSuite::Angel)
            .unwrap_or(false);
        let communications = self
            .electronic_suite_available(ElectronicSuite::Communications)
            .unwrap_or(false);
        let mut electronics = self.electronics;
        electronics.reconcile(
            self.power == Power::Running,
            guardian,
            angel,
            communications,
        );
        electronics
    }

    /// Whether the emission controls already agree with power and surviving equipment.
    pub(super) fn electronics_settled(&self) -> bool {
        self.settled_electronics() == self.electronics
    }
}

/// Borrow the common electronic state from either construction store.
fn state(world: &World, id: ObjectId) -> Result<Electronics> {
    let unit = world
        .btech
        .unit(id)
        .context("Unit construction state is unavailable")?;
    Ok(unit.electronics())
}

/// Borrow common controls while keeping construction-specific storage private.
fn state_mut(world: &mut World, id: ObjectId) -> &mut Electronics {
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        &mut unit.electronics
    })
}

/// Query construction-specific equipment availability for a common suite.
fn available(world: &World, id: ObjectId, suite: ElectronicSuite) -> Result<bool> {
    let unit = world
        .btech
        .unit(id)
        .context("Unit construction state is unavailable")?;
    unit.electronic_suite_available(suite)
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
    suite: ElectronicSuite,
    requested: Mode,
) -> Result<Mode> {
    super::power::controlled(world, id, pilot)?;
    let unit = super::scanner::scanner_unit(world, id)
        .context("Unit construction state is unavailable")?;
    ensure!(unit.power == Power::Running, "Start the unit first");
    ensure!(unit.position.is_some(), "Unit is not placed");
    if suite == ElectronicSuite::Communications {
        ensure!(
            requested == Mode::Eccm,
            "Communications equipment generates ECCM only"
        );
        ensure!(
            available(world, id, suite)?,
            "This unit does not have {COMMUNICATIONS_ECCM_TONS} or more tons of working communications equipment"
        );
    }
    ensure!(
        available(world, id, suite)?,
        "This unit does not have a working {suite:?} ECM suite"
    );
    let mode = {
        let electronics = state_mut(world, id);
        let mode = match suite {
            ElectronicSuite::Guardian => &mut electronics.guardian,
            ElectronicSuite::Angel => &mut electronics.angel,
            ElectronicSuite::Communications => &mut electronics.communications,
        };
        *mode = mode.toggle(requested);
        *mode
    };
    let _ = super::autopilot::manual_takeover(world, id);
    Ok(mode)
}

/// One unit's working emissions on its map, before they are measured from a subject.
#[derive(Debug, Clone, Copy)]
struct Emitter {
    id: ObjectId,
    team: i32,
    guardian: Mode,
    angel: Mode,
    /// Half-suites of communications ECCM, or zero.
    communications_eccm: u8,
}

impl Emitter {
    /// The emissions as they reach a subject `distance` hexes away, given the suites' `reach`.
    fn source(self, distance: f64, reach: f64) -> ElectronicSource {
        ElectronicSource {
            team: self.team,
            distance,
            range: reach,
            guardian: self.guardian,
            angel: self.angel,
            personal: Mode::Off,
            communications_eccm: self.communications_eccm,
        }
    }
}

/// Every running, placed unit on `map` with a suite switched on, and what its working
/// equipment emits.
fn emitters(world: &World, map: ObjectId) -> Result<Vec<Emitter>> {
    let mut emitters = Vec::new();
    for other in identities(world) {
        // Most units carry no active suite; reading the switch first keeps this scan cheap.
        let state = state(world, other)?;
        if state.guardian == Mode::Off
            && state.angel == Mode::Off
            && state.communications == Mode::Off
        {
            continue;
        }
        let emitter = super::scanner::scanner_unit(world, other).unwrap();
        if emitter.position.is_none_or(|p| p.map != map)
            || emitter.power != Power::Running
            || emitter.destroyed
            || world
                .objects
                .get(&other)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        // Engaged stealth armor takes over its ECM suite, which then makes no field; an Angel
        // suite may still generate ECCM (Total Warfare p. 142; Tactical Operations: Advanced
        // Rules p. 98).
        let stealth = world
            .btech
            .constructed_units()
            .get(&other)
            .is_some_and(|mech| mech.stealth().enabled);
        emitters.push(Emitter {
            id: other,
            team: emitter.signature.team,
            guardian: if !stealth && available(world, other, ElectronicSuite::Guardian)? {
                state.guardian
            } else {
                Mode::Off
            },
            angel: if (!stealth || state.angel == Mode::Eccm)
                && available(world, other, ElectronicSuite::Angel)?
            {
                state.angel
            } else {
                Mode::Off
            },
            communications_eccm: if state.communications == Mode::Eccm
                && available(world, other, ElectronicSuite::Communications)?
            {
                super::communications_eccm(communications_tons(world, other)?)
            } else {
                0
            },
        });
    }
    Ok(emitters)
}

/// A subject's placement and team, and whether it suffers self-interference: engaged stealth
/// armor or an attached iNarc ECM pod leaves it as if inside hostile ECM (Total Warfare
/// pp. 141-142).
fn subject(world: &World, id: ObjectId) -> Result<(Option<super::Position>, i32, bool)> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        return Ok((
            vehicle.position(),
            vehicle.signature().team,
            vehicle.has_beacon(super::BeaconKind::Ecm),
        ));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    Ok((
        unit.position(),
        unit.signature().team,
        unit.stealth_active() || unit.has_beacon(super::BeaconKind::Ecm),
    ))
}

/// How far ECM suites reach on `map`: every emitter shares the subject's map, so one
/// battlefield condition sets their reach.
fn map_reach(world: &World, map: ObjectId) -> f64 {
    super::ecm_range(
        world
            .btech
            .maps()
            .get(&map)
            .is_some_and(super::StoredMap::electromagnetic_interference),
    )
}

/// Compute current effects from same-map emitters without consuming dice or changing observations.
pub fn electronic_field(world: &World, id: ObjectId) -> Result<ElectronicField> {
    let (position, team, self_interference) = subject(world, id)?;
    let Some(position) = position else {
        return Ok(ElectronicField::default());
    };
    let reach = map_reach(world, position.map);
    let mut sources = Vec::new();
    for emitter in emitters(world, position.map)? {
        let distance = super::unit_range(world, id, emitter.id)?.hexes();
        if distance > reach {
            continue;
        }
        sources.push(emitter.source(distance, reach));
    }
    super::resolve_electronic_field(team, sources, self_interference)
}

/// The ECM suites hostile to `spotter` that a sensor sweep of `target` meets: every one
/// working at either unit's hex, each counted once, and the spotter's own self-interference
/// as one more (Tactical Operations: Advanced Rules p. 191; Total Warfare p. 142).
pub fn hostile_ecm_against(
    world: &World,
    spotter: ObjectId,
    target: ObjectId,
) -> Result<super::EcmSystems> {
    let (position, team, self_interference) = subject(world, spotter)?;
    let Some(position) = position else {
        return Ok(super::EcmSystems::default());
    };
    let reach = map_reach(world, position.map);
    let mut views = Vec::new();
    for emitter in emitters(world, position.map)? {
        let from_spotter = super::unit_range(world, spotter, emitter.id)?.hexes();
        let from_target = super::unit_range(world, target, emitter.id)?.hexes();
        if from_spotter > reach && from_target > reach {
            continue;
        }
        views.push([
            emitter.source(from_spotter, reach),
            emitter.source(from_target, reach),
        ]);
    }
    let mut systems = super::hostile_ecm_systems(team, &views)?;
    if self_interference {
        systems.guardian = systems.guardian.saturating_add(1);
    }
    Ok(systems)
}

/// Tons of communications equipment a unit of either store carries.
fn communications_tons(world: &World, id: ObjectId) -> Result<u8> {
    let unit = world
        .btech
        .unit(id)
        .context("Unit construction state is unavailable")?;
    unit.communications_tons()
}

/// Recompute every observation from one world snapshot, then publish changes together.
pub fn refresh_electronic_fields(world: &mut World) -> Result<Vec<Notice>> {
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
            let working = available(world, id, ElectronicSuite::Guardian)?
                || available(world, id, ElectronicSuite::Angel)?;
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
pub(super) fn refresh_receiver(world: &mut World, id: ObjectId) -> Result<Vec<Notice>> {
    let field = electronic_field(world, id)?;
    let working = available(world, id, ElectronicSuite::Guardian)?
        || available(world, id, ElectronicSuite::Angel)?;
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
            && (state(world, id).is_ok_and(|state| state != Electronics::default())
                || world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .is_some_and(|unit| {
                        unit.stealth().enabled || unit.has_beacon(super::BeaconKind::Ecm)
                    })
                || world
                    .btech
                    .vehicles()
                    .get(&id)
                    .is_some_and(|unit| unit.has_beacon(super::BeaconKind::Ecm)))
    })
}

/// Native and Lua controls share mode publication and complete rollback on notification failure.
pub(crate) fn configure(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
    suite: ElectronicSuite,
    requested: Mode,
) -> Result<Mode> {
    scripts.atomic(|_| {
        let mode =
            toggle_electronics(&mut scripts.world.borrow_mut(), id, pilot, suite, requested)?;
        let name = match suite {
            ElectronicSuite::Guardian => "ECM suite",
            ElectronicSuite::Angel => "Angel ECM suite",
            ElectronicSuite::Communications => "communications equipment",
        };
        let text = match mode {
            Mode::Off => format!("You turn your {name} offline."),
            Mode::Ecm => format!("You turn your {name} online (ECM mode)."),
            Mode::Eccm => format!("You turn your {name} online (ECCM mode)."),
        };
        super::notify_unit_text(scripts, id, &text)?;
        let notices = refresh_electronic_fields(&mut scripts.world.borrow_mut())?;
        for notice in notices {
            super::notify_unit_text(scripts, notice.unit, &notice.text)?;
        }
        Ok(mode)
    })
}

/// Four cockpit commands select Guardian or Angel ECM/ECCM modes, and `commseccm` switches
/// communications ECCM.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let suite = if input.name.starts_with('a') {
        ElectronicSuite::Angel
    } else if input.name.starts_with('c') {
        ElectronicSuite::Communications
    } else {
        ElectronicSuite::Guardian
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
