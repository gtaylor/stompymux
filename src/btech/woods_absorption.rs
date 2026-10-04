//! Occupied-woods damage and terrain consequences shared by anatomical target adapters.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One occupied-woods absorption with committed terrain feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleWoodsAbsorption {
    pub damage_before: u16,
    /// Per-shell damage retains one before glancing; missile totals may reach zero.
    pub damage_after: u16,
    pub terrain: BattleWoodlandImpact,
    pub notices: Vec<BattleNotice>,
}

/// Direct shells use a per-projectile damage floor; missiles and pellets use total absorption.
pub(super) fn direct_shells(weapon: BattleWeapon, mode: BattleAmmunitionMode) -> bool {
    weapon.profile().missiles == 0
        && !weapon.is_artillery()
        && mode != BattleAmmunitionMode::Cluster
}

/// Apply occupied cover before glancing and use pre-absorption damage for intentional terrain effects.
/// The enclosing salvo owns rollback of damage, terrain, dice and pending notices together.
pub(super) fn resolve_shells(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    mode: BattleAmmunitionMode,
    packets: &mut [u16],
    glancing: bool,
) -> Result<Option<BattleWoodsAbsorption>> {
    let Some(&damage) = packets.first() else {
        return Ok(None);
    };
    ensure!(
        packets.iter().all(|packet| *packet == damage),
        "Direct shells must share per-projectile damage"
    );
    let strength = super::aim::occupied_woods(world, target)? as u16;
    let absorption = if strength == 0 {
        None
    } else {
        let damage_before = damage;
        let damage_after = damage.saturating_sub(strength * 2).max(1);
        packets.fill(damage_after);
        let terrain = terrain_effect(world, shooter, target, weapon, mode, damage_before)?;
        let mut notices = vec![
            BattleNotice {
                unit: shooter,
                text: "The woods absorb some of your shot!".into(),
            },
            BattleNotice {
                unit: target,
                text: "The woods absorb some of the damage!".into(),
            },
        ];
        notices.extend(terrain.notices.iter().cloned());
        Some(BattleWoodsAbsorption {
            damage_before,
            damage_after,
            terrain,
            notices,
        })
    };
    if glancing {
        for packet in packets {
            *packet = packet.div_ceil(2);
        }
    }
    Ok(absorption)
}

/// LBX damage determination affects terrain before its independent pellet-count absorption.
/// The reduced nominal shell value is discarded; each actual pellet still carries one damage.
pub(super) fn begin_pellets(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    mode: BattleAmmunitionMode,
) -> Result<Option<BattleWoodsAbsorption>> {
    if mode != BattleAmmunitionMode::Cluster || !weapon.is_lbx() {
        return Ok(None);
    }
    resolve_shells(
        world,
        shooter,
        target,
        weapon,
        mode,
        &mut [u16::from(weapon.profile().damage)],
        false,
    )
}

/// Missile and pellet totals lose whole projectiles; Inferno resolves its exposure separately.
pub(super) fn resolve_projectiles(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    mode: BattleAmmunitionMode,
    packets: &mut super::weapon_groups::WeaponGroups,
) -> Result<Option<BattleWoodsAbsorption>> {
    if packets.damage.is_empty() {
        return Ok(None);
    }
    let strength = super::aim::occupied_woods(world, target)? as u16;
    if strength == 0 {
        return Ok(None);
    }
    let (damage_before, damage_after) = packets.absorb_missiles(strength * 2);
    let before = damage_before / packets.missile_payload();
    let after = damage_after / packets.missile_payload();
    let terrain = terrain_effect(world, shooter, target, weapon, mode, damage_before)?;
    let noun = match (mode == BattleAmmunitionMode::Cluster, before == 1) {
        (true, true) => "pellet",
        (true, false) => "pellets",
        (false, true) => "missile",
        (false, false) => "missiles",
    };
    let quantity = if before == 1 {
        "The"
    } else if after == 0 {
        "All of the"
    } else {
        "Some of the"
    };
    let notices = vec![
        BattleNotice {
            unit: shooter,
            text: format!(
                "{quantity} {noun} {} absorbed by the trees!",
                if before == 1 { "is" } else { "are" }
            ),
        },
        BattleNotice {
            unit: target,
            text: format!(
                "The trees absorb {} {noun}",
                if before == 1 || after == 0 {
                    "the"
                } else {
                    "some of the"
                }
            ),
        },
    ];
    let notices = terrain.notices.iter().cloned().chain(notices).collect();
    Ok(Some(BattleWoodsAbsorption {
        damage_before,
        damage_after,
        terrain,
        notices,
    }))
}

/// Cover checks use the target's base terrain; the existing terrain resolver owns overlay behavior.
fn terrain_effect(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    mode: BattleAmmunitionMode,
    damage: u16,
) -> Result<BattleWoodlandImpact> {
    let position = super::scanner::scanner_unit(world, target)
        .and_then(|unit| unit.position)
        .context("Target is not placed")?;
    resolve_woodland_attack(
        world,
        BattleWoodlandAttack {
            shooter,
            coordinate: HexCoordinate {
                x: position.x.into(),
                y: position.y.into(),
            },
            weapon,
            ammunition: mode,
            damage,
            intent: BattleWoodlandIntent::Clear,
        },
    )
}
