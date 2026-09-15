//! Separate gunner-station ownership and lifecycle, independent of parent cockpit state.
use crate::{Flag, Kind, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Saved station fields retain independent aiming state and signed database sentinels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleGunnerStation {
    /// Four saved station TIC words, separate from the parent unit's firing groups.
    #[serde(default)]
    pub tics: [u32; 4],
    /// Trajectory correction owned by this station's current selection.
    #[serde(default)]
    pub artillery_adjustment: u8,
    /// Committed seconds until this station's independently owned lock settles.
    #[serde(default)]
    pub lock_remaining: u8,
    pub parent: ObjectId,
    pub gunner: ObjectId,
    pub target: ObjectId,
    pub target_coordinates: [i16; 3],
    pub lock_modes: i32,
    pub arcs: i32,
}

impl Default for BattleGunnerStation {
    /// An unattached reference station starts with zero parent/gunner and unset targeting coordinates.
    fn default() -> Self {
        Self {
            tics: [0; 4],
            artillery_adjustment: 0,
            lock_remaining: 0,
            parent: ObjectId(0),
            gunner: ObjectId(0),
            target: ObjectId(-1),
            target_coordinates: [-1, -1, 0],
            lock_modes: 0,
            arcs: 0,
        }
    }
}

/// Explicit admission context for actions performed with a parent's equipment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleGunnerContext {
    pub station: ObjectId,
    pub parent: ObjectId,
    pub gunner: ObjectId,
    pub arcs: i32,
}

impl super::BtechState {
    /// Inspect saved stations without granting control of their parent equipment.
    pub fn gunner_stations(&self) -> &BTreeMap<ObjectId, BattleGunnerStation> {
        &self.gunner_stations
    }
}

/// Keep station identity ownership distinct from maps and constructed units.
pub(super) fn validate(world: &World) -> Result<()> {
    for (id, station) in world.btech.gunner_stations.iter() {
        ensure!(
            station.tics.iter().all(|value| *value <= i32::MAX as u32),
            "Invalid station TIC value"
        );
        ensure!(station.lock_remaining <= 8, "Invalid gunner lock countdown");
        ensure!(
            station.lock_remaining == 0 || station.target_selection().is_some(),
            "Gunner countdown requires a target"
        );
        ensure!(
            world
                .btech
                .registrations
                .get(id)
                .is_some_and(|kind| kind == "TURRET"),
            "Gunner station lacks TURRET registration"
        );
        ensure!(
            world
                .objects
                .get(id)
                .is_some_and(|object| object.kind == Kind::Thing),
            "Gunner station requires a thing object"
        );
    }
    Ok(())
}

/// Author a new station for any supported constructed parent without changing its pilot.
pub fn register_gunner_station(
    world: &mut World,
    actor: ObjectId,
    station: ObjectId,
    parent: ObjectId,
    arcs: i32,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(world.objects.get(&station).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Gunner station requires an available thing object");
    ensure!(
        !world.btech.registrations.contains_key(&station),
        "Object already has BattleTech state"
    );
    parent_available(world, parent)?;
    Arc::make_mut(&mut world.btech.registrations).insert(station, "TURRET".into());
    Arc::make_mut(&mut world.btech.gunner_stations).insert(
        station,
        BattleGunnerStation {
            parent,
            gunner: ObjectId(-1),
            arcs,
            ..Default::default()
        },
    );
    Ok(())
}

/// Deferred parents remain observable in saved data but cannot drive supported-unit actions.
fn parent_available(world: &World, parent: ObjectId) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&parent)
            .is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going))
            && world
                .btech
                .registrations
                .get(&parent)
                .is_some_and(|kind| kind == "MECH")
            && super::scanner::scanner_unit(world, parent).is_some(),
        "Error: Turret's parentage is unknown or unsupported."
    );
    Ok(())
}

/// A station must be one of the actor, location or carried command candidates.
fn station_for(world: &World, station: ObjectId, actor: ObjectId) -> Result<&BattleGunnerStation> {
    ensure!(
        world
            .objects
            .get(&actor)
            .is_some_and(|object| object.kind != Kind::Garbage
                && !object.flags.contains(Flag::Going)
                && (actor == station
                    || object.location == Some(station)
                    || world
                        .objects
                        .get(&station)
                        .is_some_and(|station| station.location == Some(actor)))),
        "Gunner station is not accessible"
    );
    ensure!(world.objects.get(&station).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Gunner station is unavailable");
    let state = world
        .btech
        .gunner_stations
        .get(&station)
        .context("Object is not a gunner station")?;
    parent_available(world, state.parent)?;
    Ok(state)
}

/// Admission captures the gunner identity without temporarily replacing the parent's pilot.
pub fn gunner_context(
    world: &World,
    station: ObjectId,
    actor: ObjectId,
) -> Result<BattleGunnerContext> {
    let state = station_for(world, station, actor)?;
    ensure!(
        state.gunner.0 >= 0,
        "The turret hasn't been initialized yet!"
    );
    ensure!(state.gunner == actor, "You aren't the registered gunner!");
    let pilot = world
        .btech
        .constructed_units()
        .get(&state.parent)
        .and_then(|unit| unit.pilot())
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&state.parent)
                .and_then(|unit| unit.pilot())
        });
    ensure!(pilot != Some(actor), "You cannot pilot and gun at once");
    Ok(BattleGunnerContext {
        station,
        parent: state.parent,
        gunner: actor,
        arcs: state.arcs,
    })
}

/// Initialize or release a station with atomic occupant notifications and takeover checks.
pub fn gunner_station_action(
    scripts: &Scripts,
    station: ObjectId,
    actor: ObjectId,
    initialize: bool,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let state = station_for(&before, station, actor)?;
        if initialize && state.gunner == actor {
            return super::notify_message(
                scripts,
                super::BattleMessageTarget::Player(actor),
                "You grap firmer hold on the joystick..",
            );
        }
        if initialize {
            if let Some(previous) = before.objects.get(&state.gunner) {
                ensure!(
                    !previous.flags.contains(Flag::Connected)
                        || previous.location != before.objects[&actor].location,
                    "You need {} to leave or disconnect first.",
                    crate::text::plain(&previous.name)
                );
            }
        } else {
            ensure!(state.gunner == actor, "You aren't gunner!");
        }
        let message = format!(
            "{} {} as gunner.",
            crate::text::escape(&before.objects[&actor].name),
            if initialize {
                "initialized"
            } else {
                "deinitialized"
            }
        );
        super::notify_message(scripts, super::BattleMessageTarget::Unit(station), &message)?;
        Arc::make_mut(&mut scripts.world_mut().btech.gunner_stations)
            .get_mut(&station)
            .expect("validated station")
            .gunner = if initialize { actor } else { ObjectId(-1) };
        scripts.effects.validate()?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native lifecycle commands ignore trailing text, as do their cockpit counterparts.
fn command(ctx: &crate::CommandContext<'_>, initialize: bool) -> Result<crate::CommandAction> {
    let result = (|| {
        let station = super::special_dispatch::object(ctx)?;
        gunner_station_action(ctx.scripts, station, ctx.player, initialize)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Claim the selected station.
pub(crate) fn initialize_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, true)
}

/// Release the selected station.
pub(crate) fn deinitialize_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, false)
}

/// Saved lock-purpose bits belong to the station's aiming state.
const LOCK_MODES: i32 = 0x000f8000;

impl BattleGunnerStation {
    /// Project saved aiming fields to the shared lock type without rewriting imported metadata.
    pub fn target_selection(&self) -> Option<super::BattleTargetSelection> {
        use super::{BattleHexTargetMode as M, BattleTargetSelection as S};
        if self.target.0 >= 0 {
            return Some(S::Unit(super::BattleTargetLock {
                target: self.target,
                remaining: self.lock_remaining,
            }));
        }
        let [x, y, _] = self.target_coordinates;
        if x < 0 || y < 0 {
            return None;
        }
        let mode = if self.lock_modes & 0x10000 != 0 {
            M::Building
        } else if self.lock_modes & 0x20000 != 0 {
            M::Hex
        } else if self.lock_modes & 0x40000 != 0 {
            M::Ignite
        } else if self.lock_modes & 0x80000 != 0 {
            M::Clear
        } else {
            M::UnitAtHex
        };
        Some(S::Hex(super::BattleHexLock {
            hex: super::BattleHexCoordinate {
                x: i32::from(x),
                y: i32::from(y),
            },
            mode,
            remaining: self.lock_remaining,
        }))
    }

    /// Store a shared lock, retaining unrelated flag bits and independent parent targeting.
    pub(super) fn set_target_selection(&mut self, selection: Option<super::BattleTargetSelection>) {
        self.artillery_adjustment = 0;
        use super::{BattleHexTargetMode as M, BattleTargetSelection as S};
        self.target = ObjectId(-1);
        self.target_coordinates = [-1, -1, 0];
        self.lock_modes &= !LOCK_MODES;
        self.lock_remaining = selection.map_or(0, |lock| lock.remaining());
        match selection {
            Some(S::Unit(lock)) => {
                self.target = lock.target;
                self.lock_modes |= 0x8000;
            }
            Some(S::Hex(lock)) => {
                self.target_coordinates = [
                    i16::try_from(lock.hex.x).expect("admitted station coordinate"),
                    i16::try_from(lock.hex.y).expect("admitted station coordinate"),
                    0,
                ];
                self.lock_modes |= match lock.mode {
                    M::UnitAtHex => 0x8000,
                    M::Building => 0x10000,
                    M::Hex => 0x20000,
                    M::Ignite => 0x40000,
                    M::Clear => 0x80000,
                };
            }
            None => {}
        }
    }
}

impl BattleGunnerContext {
    /// Revalidate captured admission before using it; reassignment or arc edits invalidate the context.
    pub fn validate(self, world: &World) -> Result<()> {
        ensure!(
            gunner_context(world, self.station, self.gunner)? == self,
            "Gunner station context has changed"
        );
        Ok(())
    }

    /// Connected station crew is independent of the parent pilot's presence or connection state.
    fn active_operator(self, world: &World) -> Result<Option<ObjectId>> {
        self.validate(world)?;
        Ok(world.objects[&self.gunner]
            .flags
            .contains(Flag::Connected)
            .then_some(self.gunner))
    }

    /// Resolve conventional weapon skill through the same chassis/family policy as the cockpit.
    /// This read-only query does not establish mechanical readiness or authorize a shot.
    pub fn gunnery_target(self, world: &World, weapon_index: usize, extended: bool) -> Result<i16> {
        let operator = self.active_operator(world)?;
        let weapon = super::skills::installed_weapon(world, self.parent, weapon_index)?;
        super::skills::operator_weapon_gunnery_target(
            world,
            self.parent,
            weapon,
            operator,
            extended,
        )
    }

    /// Resolve the artillery skill using the shared dedicated artillery fallback.
    pub fn artillery_gunnery_target(self, world: &World) -> Result<i16> {
        super::skills::operator_artillery_gunnery_target(world, self.active_operator(world)?)
    }
}

/// A detached conventional targeting preview; it does not admit a shot or consume parent dice.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleGunnerAim {
    pub parent: ObjectId,
    pub target: Option<ObjectId>,
    pub coordinate: Option<super::BattleHexCoordinate>,
    pub aim: super::BattleSightAim,
}

impl BattleGunnerContext {
    /// Keep equipment identity and target ownership explicit through shared combat calculations.
    pub(super) fn target_source(self, world: &World) -> Result<super::fire_target::TargetSource> {
        self.validate(world)?;
        Ok(super::fire_target::TargetSource {
            unit: self.parent,
            owner: self.station,
        })
    }

    /// Preview conventional unit or terrain aim using this station's selection and operator skill.
    /// Like cockpit aim inspection, this neither checks firing readiness/arcs nor authorizes launch.
    pub fn aim(
        self,
        world: &World,
        weapon_index: usize,
        target: super::BattleFireTarget,
        extended_gunnery: bool,
        mut rules: super::BattleAimRules,
    ) -> Result<BattleGunnerAim> {
        let source = self.target_source(world)?;
        let weapon = super::skills::installed_weapon(world, self.parent, weapon_index)?;
        ensure!(
            !weapon.is_artillery(),
            "Artillery requires artillery aim rules"
        );
        let gunnery = self.gunnery_target(world, weapon_index, extended_gunnery)?;
        rules.override_weapon_arcs = self.arcs != 0;
        let requested = super::fire_target::FireTargetRequest::Target(target).resolve_for_source(
            world,
            source,
            weapon_index,
        )?;
        let resolved = super::fire_target::resolve_conventional_for_source(
            world,
            source,
            weapon_index,
            requested,
        )?;
        let (target, coordinate, aim) = match resolved {
            super::fire_target::ResolvedFireTarget::Unit { unit, coordinate } => (
                Some(unit),
                coordinate,
                super::BattleSightAim::Unit(super::aim::aim_modifiers_for_source(
                    world,
                    source,
                    unit,
                    weapon_index,
                    gunnery,
                    rules,
                )?),
            ),
            super::fire_target::ResolvedFireTarget::Hex(hex) => (
                None,
                Some(hex),
                super::BattleSightAim::Hex(super::hex_aim::modifiers_for_source(
                    world,
                    source,
                    hex,
                    weapon_index,
                    gunnery,
                    rules,
                )?),
            ),
        };
        Ok(BattleGunnerAim {
            parent: self.parent,
            target,
            coordinate,
            aim,
        })
    }
}
