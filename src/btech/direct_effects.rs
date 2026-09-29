//! Shared non-damage weapon effects; launcher carriers do not own target anatomy or heat rules.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Completed launch facts and hit-location policy supplied by an enclosing shot transaction.
pub(super) struct DirectEffectRequest {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub fire_mode: BattleFireMode,
    pub hit: bool,
    pub launched: bool,
    pub intercepted: bool,
    pub hit_rules: BattleHitRules,
    pub vehicle_impact: BattleVehicleImpactRules,
    pub hit_arc_mode: i64,
    pub woods_damage: bool,
    pub range_damage: bool,
    /// Whether the launching mount is below the waterline.
    pub submerged: bool,
    pub damage_penalty: u8,
    pub distance: f64,
    pub gatling_damage: Option<u8>,
    pub coordinate: Option<BattleHexCoordinate>,
}

/// Immediate attachment or heat changes, separate from ordinary damage packets.
#[derive(Default)]
pub(super) struct DirectEffects {
    pub narc: Option<BattleNarcReport<BattleUnitSection>>,
    pub cooling: Option<f64>,
    pub heat_transfer: u8,
    pub woods: Option<BattleWoodsAbsorption>,
    pub missed_terrain: Option<BattleWoodlandImpact>,
}

/// Apply effects inside the caller's unpublished candidate, preserving normal pod and heat ordering.
pub(super) fn resolve(world: &mut World, request: DirectEffectRequest) -> Result<DirectEffects> {
    let mut report = DirectEffects::default();
    if let Some(kind) = request.weapon.beacon_kind(request.ammunition)
        && request.launched
    {
        let shot = super::narc::PodShot {
            kind,
            hit: request.hit,
            intercepted: request.intercepted,
        };
        report.narc = Some(if world.btech.vehicles().contains_key(&request.target) {
            super::vehicle_narc::attach(
                world,
                request.shooter,
                request.target,
                shot,
                request.vehicle_impact,
                request.hit_arc_mode,
            )?
        } else {
            super::narc::attach(
                world,
                request.shooter,
                request.target,
                shot,
                request.hit_rules,
                request.hit_arc_mode,
            )?
            .map_section(BattleUnitSection::Mech)
        });
    }
    if !request.hit {
        if request.launched
            && request.weapon.profile().missiles == 0
            && !request.weapon.is_artillery()
            && report.narc.is_none()
        {
            let damage = terrain_damage(world, &request)?;
            let coordinate = if let Some(coordinate) = request.coordinate {
                coordinate
            } else {
                let position = super::scanner::scanner_unit(world, request.target)
                    .and_then(|unit| unit.position)
                    .context("Target is not placed")?;
                BattleHexCoordinate {
                    x: position.x.into(),
                    y: position.y.into(),
                }
            };
            report.missed_terrain = Some(resolve_woodland_attack(
                world,
                BattleWoodlandAttack {
                    shooter: request.shooter,
                    coordinate,
                    weapon: request.weapon,
                    ammunition: request.ammunition,
                    damage,
                    intent: BattleWoodlandIntent::Incidental,
                },
            )?);
        }
        return Ok(report);
    }
    let cooling = request.weapon == BattleWeapon::CoolantGun;
    if !cooling && request.fire_mode != BattleFireMode::Heat {
        return Ok(report);
    }
    if request.woods_damage {
        let damage = terrain_damage(world, &request)?;
        report.woods = super::woods_absorption::resolve_shells(
            world,
            request.shooter,
            request.target,
            request.weapon,
            request.ammunition,
            &mut [damage],
            false,
        )?;
    }
    let heat = stored_heat_mut(world, request.target)?;
    if cooling {
        let removed = f64::from(request.weapon.profile().damage);
        *heat -= removed;
        report.cooling = Some(removed);
        return Ok(report);
    }
    report.heat_transfer = request.weapon.profile().damage;
    *heat += f64::from(report.heat_transfer);
    Ok(report)
}

/// Nominal shot damage for terrain, without a burst/pellet count or a hit-location roll.
/// Thermal hits and incidental misses share the existing range and component-damage rules.
fn terrain_damage(world: &mut World, request: &DirectEffectRequest) -> Result<u16> {
    let packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            submerged: request.submerged,
            range_damage: request.range_damage,
            damage_penalty: request.damage_penalty,
            weapon: request.weapon,
            ammunition: if request.ammunition == BattleAmmunitionMode::Cluster {
                BattleAmmunitionMode::Normal
            } else {
                request.ammunition
            },
            fire_mode: BattleFireMode::Normal,
            gatling_damage: request.gatling_damage,
            distance: Some(request.distance),
            glancing: false,
            guidance_blocked: false,
            angel_blocked: false,
            target_beacon: false,
            artemis_v: super::artemis::artemis_v(world, request.shooter),
        },
        super::dice::unit_dice_mut(world, request.shooter)?,
    )?;
    Ok(packets.damage.first().copied().unwrap_or(1).max(1))
}

/// Common coolant and direct heat feedback, ordered before electronic and damage consequences.
pub(super) fn thermal_notices(
    shooter: ObjectId,
    target: ObjectId,
    cooling: Option<f64>,
    heat_transfer: u8,
) -> Vec<BattleNotice> {
    let mut notices = Vec::new();
    if cooling.is_some() {
        if shooter != target {
            notices.push(BattleNotice {
                unit: shooter,
                text: "[fg=cyan]You hit with the stream of coolant!![reset]".into(),
            });
        }
        notices.push(BattleNotice {
            unit: target,
            text: "[fg=cyan]Coolant washes over your systems!![reset]".into(),
        });
    }
    if heat_transfer > 0 {
        notices.push(BattleNotice {
            unit: target,
            text: "The flaming plasma sprays all over you!".into(),
        });
        notices.push(BattleNotice {
            unit: shooter,
            text: "You cover your target in flaming plasma!".into(),
        });
    }
    notices
}

/// Adapt heat storage without imposing a Mech thermal lifecycle on other unit classes.
fn stored_heat_mut(world: &mut World, target: ObjectId) -> Result<&mut f64> {
    if let Some(vehicle) = world.btech.vehicles.get_mut(&target) {
        return Ok(&mut vehicle.weapon_heat);
    }
    Ok(&mut world
        .btech
        .constructed
        .get_mut(&target)
        .context("Target has no heat storage")?
        .heat
        .stored)
}
