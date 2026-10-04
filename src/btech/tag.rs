//! Owned TAG links and thirty-second lock/recycle timers; target ownership is derived rather than stored twice.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// A target with a positive timer is settling; a timer without a target is recycling.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagState {
    pub target: Option<ObjectId>,
    pub remaining: u8,
}

impl TagState {
    /// Losing a selected link starts the shared recycle clock; idle systems stay idle.
    pub(super) fn stop(&mut self) -> bool {
        if self.target.take().is_none() {
            return false;
        }
        self.remaining = 30;
        true
    }

    /// Validate persisted timing and object identity without requiring a still-live target.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(self.remaining <= 30, "Invalid TAG timer");
        ensure!(
            self.target.is_none_or(|target| target.0 > 0),
            "Invalid TAG target"
        );
        Ok(())
    }
}

impl Mech {
    /// Saved TAG selection and countdown; current geometry is checked separately.
    pub fn tag(&self) -> TagState {
        self.tag
    }

    /// Standalone TAG and integrated C3 master equipment share their live damage gate.
    pub fn tag_available(&self) -> Result<bool> {
        Ok(self.tag_hardware()?.1)
    }

    /// Installation and operation are separate so absent equipment has its own cockpit reply.
    pub(super) fn tag_hardware(&self) -> Result<(bool, bool)> {
        Ok(hardware(
            self.loadout()?
                .systems
                .into_iter()
                .filter(|part| part.system == System::Tag)
                .map(|part| !self.critical_unavailable(part.location)),
            self.c3_hardware()?,
        ))
    }
}

impl Vehicle {
    /// Saved illumination and recycling use the same state as Mech TAG systems.
    pub fn tag(&self) -> TagState {
        self.tag
    }

    /// Check all installed TAG components and any integrated command computer.
    pub fn tag_available(&self) -> Result<bool> {
        Ok(self.tag_hardware()?.1)
    }

    /// Adapt vehicle slots to the shared TAG equipment rule.
    pub(super) fn tag_hardware(&self) -> Result<(bool, bool)> {
        Ok(hardware(
            self.loadout()?
                .systems
                .into_iter()
                .filter(|part| part.system == System::Tag)
                .map(|part| !self.critical_unavailable(part.location)),
            self.c3_hardware()?,
        ))
    }
}

/// Losing either installed system disables TAG, including a combined TAG/C3 installation.
fn hardware(parts: impl Iterator<Item = bool>, computers: C3Hardware) -> (bool, bool) {
    let mut installed = computers.masters > 0;
    let mut operational = computers.masters == 0 || computers.working_masters > 0;
    for working in parts {
        installed = true;
        operational &= working;
    }
    (installed, installed && operational)
}

/// Read installation and damage facts through either chassis adapter.
pub(super) fn unit_hardware(world: &World, id: ObjectId) -> Result<(bool, bool)> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    unit.tag_hardware()
}

/// Read a shared TAG selection without maintaining another owner index.
pub(super) fn state(world: &World, id: ObjectId) -> Option<TagState> {
    world
        .btech
        .vehicles()
        .get(&id)
        .map(Vehicle::tag)
        .or_else(|| world.btech.constructed_units().get(&id).map(Mech::tag))
}

/// Update one already-resolved participant in the enclosing transaction.
fn set_state(world: &mut World, id: ObjectId, state: TagState) {
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.tag = state;
    })
}

/// Validate unique ownership across both stores, allowing stale targets until reconciliation.
pub(super) fn validate(state: &BtechState) -> Result<()> {
    let mut targets = std::collections::BTreeSet::new();
    let selections = state
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.tag()))
        .chain(state.vehicles().iter().map(|(&id, unit)| (id, unit.tag())));
    for (id, tag) in selections {
        tag.validate()?;
        if let Some(target) = tag.target {
            ensure!(
                target != id && targets.insert(target),
                "Invalid or duplicate TAG ownership"
            );
            let installed = if let Some(unit) = state.vehicles().get(&id) {
                unit.tag_hardware()?.0
            } else {
                state.constructed_units()[&id].tag_hardware()?.0
            };
            ensure!(installed, "TAG selection requires installed equipment");
        }
    }
    Ok(())
}

/// A live link requires a running, operational tagger and an acquired contact with unblocked geometry within fifteen hexes.
fn link_valid(world: &World, source: ObjectId, target: ObjectId) -> bool {
    if source == target {
        return false;
    }
    if [source, target].into_iter().any(|id| {
        world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(Flag::Going))
    }) {
        return false;
    }
    let Some(unit) = super::scanner::scanner_unit(world, source) else {
        return false;
    };
    unit.power == Power::Running
        && !unit.destroyed
        && unit_hardware(world, source).is_ok_and(|(_, available)| available)
        && unit_range(world, source, target).is_ok_and(|range| range.spatial <= 15.0)
        && visible_contact(world, source, target).is_ok_and(|contact| contact.is_some())
        && super::visibility::unit_unblocked(world, source, target).unwrap_or(false)
}

/// Current owner of a target's TAG illumination, including a connection still settling.
/// Stale saved links never authorize consumers before the next heartbeat cleans them up.
pub fn tagged_by(world: &World, target: ObjectId) -> Option<ObjectId> {
    super::scanner::scanner_ids(world).into_iter().find(|&id| {
        state(world, id).is_some_and(|tag| tag.target == Some(target))
            && link_valid(world, id, target)
    })
}

/// Start or stop a controlled TAG connection; all admission checks precede mutation.
pub fn select_tag(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<Vec<Notice>> {
    admission(world, id, pilot)?;
    let current = state(world, id).context("Unit is unavailable")?;
    let mut notices = Vec::new();
    let Some(target) = target else {
        ensure!(
            current.target.is_some(),
            "You are not currently tagging anything!"
        );
        set_state(
            world,
            id,
            TagState {
                target: None,
                remaining: 30,
            },
        );
        notices.push(Notice {
            unit: id,
            text: "Your TAG connection has been broken.".into(),
        });
        return Ok(notices);
    };
    let invalid = "That is not a valid TAG targetID. Try again.";
    let source = super::scanner::scanner_unit(world, id).context(invalid)?;
    let victim = super::scanner::scanner_unit(world, target).context(invalid)?;
    let visible = visible_contact(world, id, target)?.context(invalid)?;
    ensure!(
        super::visibility::unit_unblocked(world, id, target)?,
        "{invalid}"
    );
    ensure!(
        source.signature.team != victim.signature.team,
        "You can't TAG friendly units!"
    );
    ensure!(target != id, "You can't TAG yourself!");
    ensure!(
        unit_range(world, id, target)?.spatial <= 15.0,
        "Out of range! TAG ranges are 5/10/15"
    );
    let target_name = format!(
        "{} [{}]",
        crate::text::escape(&visible.name),
        victim.label().unwrap_or_else(|| "??".into())
    );
    let displaced: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|&other| {
            other != id && state(world, other).is_some_and(|tag| tag.target == Some(target))
        })
        .collect();
    for other in displaced {
        set_state(
            world,
            other,
            TagState {
                target: None,
                remaining: 30,
            },
        );
        notices.push(Notice {
            unit: other,
            text: "Your TAG connection has been broken.".into(),
        });
    }
    set_state(
        world,
        id,
        TagState {
            target: Some(target),
            remaining: 30,
        },
    );
    notices.push(Notice {
        unit: id,
        text: format!("You light up {target_name} with your TAG."),
    });
    Ok(notices)
}

/// Advance committed timers and reconcile movement, shutdown, damage, deletion and lost target ownership.
pub fn advance_tags(world: &mut World) -> Vec<Notice> {
    let updates: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter_map(|id| {
            let state = state(world, id)?;
            (state.target.is_some() || state.remaining > 0).then(|| {
                (
                    id,
                    state,
                    state
                        .target
                        .is_none_or(|target| link_valid(world, id, target)),
                )
            })
        })
        .collect();
    let mut notices = Vec::new();
    for (id, mut state, valid) in updates {
        if !valid {
            state = TagState {
                target: None,
                remaining: 30,
            };
            notices.push(Notice {
                unit: id,
                text: "Your TAG connection has been broken.".into(),
            });
        } else if state.remaining > 0 {
            state.remaining -= 1;
            if state.remaining == 0
                && unit_hardware(world, id).is_ok_and(|(_, available)| available)
            {
                notices.push(Notice {
                    unit: id,
                    text: if state.target.is_some() {
                        "Your TAG system has achieved a stable lock."
                    } else {
                        "Your TAG system has finished recycling."
                    }
                    .into(),
                });
            }
        }
        set_state(world, id, state);
    }
    notices
}

/// Equipment and recycling checks precede native argument parsing and target resolution.
pub(super) fn admission(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    super::targeting::controlled(world, id, pilot)?;
    let (installed, available) = unit_hardware(world, id)?;
    ensure!(installed, "This unit is not equipped with TAG!");
    ensure!(available, "Your TAG system is destroyed!");
    ensure!(
        state(world, id).unwrap().remaining == 0,
        "Your TAG system is recycling!"
    );
    Ok(())
}
