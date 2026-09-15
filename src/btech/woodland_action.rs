//! Transactional woodland impact resolution shared by terrain shots and stray weapon impacts.
use super::{
    BattleAmmunitionMode, BattleDecoration, BattleDecorationKind, BattleHexCoordinate,
    BattleNotice, BattleWeapon, BattleWoodlandEffect, BattleWoodlandIntent,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One weapon damage group reaching a terrain cell after the caller's firing checks.
#[derive(Debug, Clone, Copy)]
pub struct BattleWoodlandAttack {
    pub shooter: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub damage: u16,
    pub intent: BattleWoodlandIntent,
}

/// Terrain consequence and messages owned by the surrounding attack transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Commit the enclosing attack and publish terrain notices together"]
pub struct BattleWoodlandImpact {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub effect: BattleWoodlandEffect,
    pub notices: Vec<BattleNotice>,
}

/// Resolve and apply woodland checks atomically, without authorizing or expending a weapon.
/// The shooter supplies attack dice; the map's independent stream owns subsequent fire events.
/// Weapon fire leaves minefields intact; admission and structural impacts belong to the caller.
pub fn resolve_woodland_attack(
    world: &mut World,
    attack: BattleWoodlandAttack,
) -> Result<BattleWoodlandImpact> {
    let BattleWoodlandAttack {
        shooter,
        coordinate,
        weapon,
        ammunition,
        damage,
        intent,
    } = attack;
    ensure!(
        world
            .objects
            .get(&shooter)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Shooter is unavailable"
    );
    let map = super::scanner::scanner_unit(world, shooter)
        .and_then(|unit| unit.position)
        .context("Shooter is not placed")?
        .map;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    let tile = record.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let mut candidate = world.clone();
    let dice = super::dice::unit_dice_mut(&mut candidate, shooter)?;
    let effect =
        super::resolve_woodland_effect(tile.terrain, weapon, ammunition, damage, intent, dice);
    let verb = match effect {
        BattleWoodlandEffect::None => None,
        BattleWoodlandEffect::Clear { terrain } => {
            let _change =
                super::apply_woodland_clearing(&mut candidate, map, coordinate, tile, terrain)?;
            Some("clear")
        }
        BattleWoodlandEffect::Ignite { seconds } => {
            // Establish map randomness before attacks so replays never create a fresh event stream.
            ensure!(
                record.fire_dice.is_some(),
                "Reload the map asset to establish its fire random stream"
            );
            super::set_map_decoration(
                &mut candidate,
                map,
                coordinate,
                Some(BattleDecoration::new(
                    BattleDecorationKind::Fire,
                    i64::from(seconds),
                    None,
                )),
            )?;
            Some("ignite")
        }
    };
    let mut notices = Vec::new();
    if let Some(verb) = verb {
        let intentional = intent != BattleWoodlandIntent::Incidental;
        notices.push(BattleNotice {
            unit: shooter,
            text: format!(
                "You {}{verb} {},{}{}",
                if intentional { "" } else { "accidentally " },
                coordinate.x,
                coordinate.y,
                if intentional { "." } else { "!" }
            ),
        });
        let text = format!(
            "'s {}shot {verb}s {},{}!",
            if intentional { "" } else { "stray " },
            coordinate.x,
            coordinate.y
        );
        notices.extend(super::broadcast::observer_notices(world, shooter, &text));
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(BattleWoodlandImpact {
        map,
        coordinate,
        effect,
        notices,
    })
}
