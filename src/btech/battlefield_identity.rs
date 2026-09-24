//! Scenario battlefield identities are independent of membership order and shared by every unit projection.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, sync::Arc};

/// Assign a preferred two-letter identity, resolving collisions with the unit's durable dice.
/// Trusted callers own wizard authority and publication; failure leaves the world unchanged.
/// Missing or short arguments use configured preferences, then random IDs; longer arguments override them.
pub fn assign_battlefield_id(
    world: &mut World,
    id: ObjectId,
    preferred: Option<&str>,
) -> Result<String> {
    let mut candidate = world.clone();
    let label = assign_in_candidate(&mut candidate, id, preferred)?;
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(label)
}

/// Select and save an ID inside an enclosing map reassignment transaction.
pub(super) fn assign_in_candidate(
    world: &mut World,
    id: ObjectId,
    preferred: Option<&str>,
) -> Result<String> {
    ensure!(
        world.objects.get(&id).is_some_and(
            |object| object.kind == crate::Kind::Thing && !object.flags.contains(Flag::Going)
        ),
        "Unit is unavailable"
    );
    let map = super::scanner::scanner_unit(world, id)
        .context("Unit is not constructed")?
        .position
        .context("Unit is not placed")?
        .map;
    let used = occupied_labels(world, map, id);
    let available = (b'A'..=b'Z')
        .flat_map(|a| (b'A'..=b'Z').map(move |b| format!("{}{}", char::from(a), char::from(b))))
        .find(|label| !used.contains(label))
        .context("No battlefield ID available")?;
    let mut dice = if let Some(unit) = world.btech.vehicles().get(&id) {
        unit.dice.clone()
    } else {
        world.btech.constructed_units()[&id].dice.clone()
    };
    let preferred = preferred
        .filter(|value| value.len() >= 2)
        .or(super::preferred_identity::preferred_id(world, id)?);
    let mut label = preferred.map(|value| {
        value.as_bytes()[..2]
            .iter()
            .map(|byte| char::from(byte.to_ascii_uppercase().clamp(b'A', b'Z')))
            .collect::<String>()
    });
    // Bound pathological collision streams without losing deterministic replay.
    for _ in 0..4096 {
        if label.as_ref().is_some_and(|label| !used.contains(label)) {
            break;
        }
        label = Some(
            (0..2)
                .map(|_| dice.die(26).map(|roll| char::from(b'A' + roll as u8 - 1)))
                .collect::<Result<String>>()?,
        );
    }
    let label = label
        .filter(|label| !used.contains(label))
        .unwrap_or(available);
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.battlefield_label = Some(label.clone());
        unit.dice = dice;
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap();
        unit.battlefield_label = Some(label.clone());
        unit.dice = dice;
    }
    Ok(label)
}

/// Saved overrides and derived identities must be printable, unambiguous and unique on each map.
pub(super) fn validate(state: &BtechState) -> Result<()> {
    let units = state
        .constructed_units()
        .values()
        .map(|unit| {
            (
                unit.position(),
                unit.battlefield_label.as_ref(),
                unit.battlefield_id(),
            )
        })
        .chain(state.vehicles().values().map(|unit| {
            (
                unit.position(),
                unit.battlefield_label.as_ref(),
                unit.battlefield_id(),
            )
        }));
    let mut used = BTreeSet::new();
    for (position, authored, label) in units {
        if let Some(label) = authored {
            ensure!(
                (2..=7).contains(&label.len())
                    && label
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()),
                "Invalid battlefield ID"
            );
        }
        if let Some(position) = position {
            ensure!(
                used.insert((position.map, label.context("Placed unit lacks an ID")?)),
                "Duplicate battlefield ID"
            );
        }
    }
    Ok(())
}

/// Collect identities through the common projection, including every supported chassis.
pub(super) fn occupied_labels(world: &World, map: ObjectId, except: ObjectId) -> BTreeSet<String> {
    super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|other| *other != except)
        .filter_map(|other| super::scanner::scanner_unit(world, other))
        .filter(|unit| unit.position.is_some_and(|position| position.map == map))
        .filter_map(|unit| unit.label())
        .collect()
}
