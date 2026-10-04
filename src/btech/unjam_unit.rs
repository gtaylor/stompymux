//! Unit-state adapters for the shared feed-clearing workflow; no recovery rules live here.
use super::{Power, Unjam, WeaponReadiness};
use crate::{ObjectId, World};
use anyhow::Result;

/// Current cockpit, mount and feed facts used by common admission and expiry rules.
pub(super) struct FeedState {
    pub power: Power,
    pub destroyed: bool,
    pub ready: WeaponReadiness,
    pub jammed: bool,
    pub pilot: Option<ObjectId>,
    pub jumping: bool,
    pub desired_speed: f64,
    pub cruise_speed: f64,
    pub recycling: bool,
    pub pending: Option<Unjam>,
    pub bin: Option<usize>,
}

/// Read the same domain facts from either unit's owned representation.
pub(super) fn state(world: &World, id: ObjectId, index: usize) -> Result<FeedState> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(FeedState {
            power: unit.power(),
            destroyed: unit.is_destroyed(),
            ready: unit.weapon_readiness(index)?,
            jammed: unit.weapon_jammed(index)?,
            pilot: unit.pilot(),
            jumping: false,
            desired_speed: unit.motion().map_or(0.0, |motion| motion.desired_speed),
            cruise_speed: unit.maximum_speed() * 2.0 / 3.0,
            recycling: !unit.weapon_recycle().is_empty(),
            pending: unit.unjam(),
            bin: unit
                .ammunition_feed(index, 1)?
                .first()
                .map(|draw| draw.bin_index),
        });
    }
    let unit = &world.btech.constructed_units()[&id];
    Ok(FeedState {
        power: unit.power(),
        destroyed: unit.is_destroyed(),
        ready: unit.weapon_readiness(index)?,
        jammed: unit.weapon_jammed(index)?,
        pilot: unit.pilot(),
        jumping: unit.airborne(),
        desired_speed: unit.motion().map_or(0.0, |motion| motion.desired_speed),
        cruise_speed: unit.movement_maximum_speed() * 2.0 / 3.0,
        recycling: !unit.weapon_recycle().is_empty(),
        pending: unit.unjam(),
        bin: unit
            .ammunition_feed(index, 1)?
            .first()
            .map(|draw| draw.bin_index),
    })
}

/// Borrow the owned countdown under the enclosing world transaction.
pub(super) fn pending(world: &mut World, id: ObjectId) -> Result<&mut Option<Unjam>> {
    super::with_unit_mut!(world.btech.unit_mut(id).expect("admitted unit"), |unit| {
        unit.validate()?;
        Ok(&mut unit.unjam)
    })
}

/// Store successful recovery and, when present, consume the selected surviving shell.
pub(super) fn clear(
    world: &mut World,
    id: ObjectId,
    index: usize,
    bin: Option<usize>,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        unit.clear_weapon_jam(index)?;
        if let Some(bin) = bin {
            unit.expend_ammunition(bin, 1)?;
        }
        return Ok(());
    }
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.clear_weapon_jam(index)?;
    if let Some(bin) = bin {
        unit.ammunition[bin] -= 1;
        unit.live_mass.invalidate();
    }
    Ok(())
}
