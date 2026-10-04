//! Cocoon damage and breach continuations share one adapter across all supported ground anatomies.
use super::orbital_drop_state::current;
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Breaches compare raw terrain elevation, including the bed below ice and water.
/// Off-map protection retires at its retained height without scheduling an impossible map fall.
fn surface(world: &World, id: ObjectId, drop: BattleOrbitalDrop) -> Result<i32> {
    let Some(position) = super::scanner::scanner_unit(world, id).and_then(|unit| unit.position)
    else {
        return Ok(drop.elevation());
    };
    Ok(i32::from(
        world
            .btech
            .maps()
            .get(&position.map)
            .context("Drop map is unavailable")?
            .base_hex(i64::from(position.x), i64::from(position.y))?
            .surface_height(),
    ))
}

/// Divert a complete material packet on a protected roll above eight; immunity and dead sections precede this.
/// No protection consumes no interception dice. A miss keeps the roll but proceeds with ordinary damage.
pub(super) fn intercept(
    world: &mut World,
    id: ObjectId,
    attacker: Option<ObjectId>,
    amount: u32,
) -> Result<Option<Vec<BattleNotice>>> {
    let Some(mut drop) = current(world, id).filter(|drop| drop.protected()) else {
        return Ok(None);
    };
    if amount == 0 {
        return Ok(None);
    }
    let roll = crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.dice.generic_roll()
    });
    if roll <= 8 {
        return Ok(None);
    }
    let result = drop.intercept(
        amount,
        roll,
        surface(world, id, drop)?,
        super::jump_thrust::speed(world, id)? >= 10.75,
    )?;
    let mut notices = Vec::new();
    if let Some(attacker) = attacker {
        notices.push(BattleNotice {
            unit: attacker,
            text: format!("[fg=green]You hit the cocoon for {amount} points of damage![reset]"),
        });
    }
    notices.push(BattleNotice {
        unit: id,
        text: format!(
            "[fg=yellow bold]Your cocoon has been hit for {amount} points of damage![reset]"
        ),
    });
    continue_after_breach(world, id, drop, result.breach);
    match result.breach {
        Some(BattleDropBreach::JumpJets) => notices.push(BattleNotice {
            unit: id,
            text: "You initiate your jumpjets to compensate for the breached cocoon!".into(),
        }),
        Some(BattleDropBreach::FreeFall) => {
            notices.push(BattleNotice {
                unit: id,
                text: "Your cocoon has been destroyed - have a nice fall!".into(),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "starts plummeting down, as the final blast blows the cocoon apart!",
            ));
        }
        _ => {}
    }
    Ok(Some(notices))
}

/// An admitted firing attempt opens protection even when a Streak launcher fails to lock.
/// Mechanical failures return before this stage and leave their own internal damage in charge.
pub(super) fn open_for_fire(world: &mut World, id: ObjectId) -> Result<Vec<BattleNotice>> {
    let Some(mut drop) = current(world, id) else {
        return Ok(Vec::new());
    };
    let Some(breach) = drop.open_for_fire(
        surface(world, id, drop)?,
        super::jump_thrust::speed(world, id)? >= 10.75,
    ) else {
        return Ok(Vec::new());
    };
    continue_after_breach(world, id, drop, Some(breach));
    let text = match breach {
        BattleDropBreach::JumpJets => {
            "You initiate your jumpjets to compensate for the opened cocoon!"
        }
        BattleDropBreach::FreeFall => "Your action splits open the cocoon - have a nice fall!",
        BattleDropBreach::AtSurface => return Ok(Vec::new()),
    };
    let mut notices = vec![BattleNotice {
        unit: id,
        text: text.into(),
    }];
    if breach == BattleDropBreach::FreeFall {
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            "starts plummeting down, as the cocoon opens!.",
        ));
    }
    Ok(notices)
}

/// Keep compensation or hand the same height to the shared forced-descent clock.
/// Starting a fall on ice can be at its upper surface; the scheduled fall resolves that contact.
fn continue_after_breach(
    world: &mut World,
    id: ObjectId,
    drop: BattleOrbitalDrop,
    breach: Option<BattleDropBreach>,
) {
    let retired = matches!(
        breach,
        Some(BattleDropBreach::FreeFall | BattleDropBreach::AtSurface)
    );
    let fall =
        (breach == Some(BattleDropBreach::FreeFall)).then(|| BattleFreeFall::new(drop.elevation()));
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.orbital_drop = (!retired).then_some(drop);
        if let Some(fall) = fall {
            unit.free_fall = Some(fall);
            unit.flight = None;
            unit.ground_elevation = None;
            unit.jump_stabilization = 0;
            unit.stand_timer = None;
            if let Some(motion) = &mut unit.motion {
                motion.stop_translation();
            }
        } else if retired {
            unit.ground_elevation = Some(f64::from(drop.elevation()));
        }
    } else {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        unit.orbital_drop = (!retired).then_some(drop);
        if let Some(fall) = fall {
            unit.free_fall = Some(fall);
            unit.ground_elevation = None;
            unit.dig = BattleDigState::default();
            unit.building_entry = None;
            unit.halt();
        } else if retired {
            unit.ground_elevation = Some(f64::from(drop.elevation()));
        }
    }
}
