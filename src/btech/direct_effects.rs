//! Shared non-damage weapon effects; launcher carriers do not own target anatomy or heat rules.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Completed launch facts and hit-location policy supplied by an enclosing shot transaction.
pub(super) struct DirectEffectRequest {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub weapon: Weapon,
    pub ammunition: AmmunitionMode,
    pub fire_mode: FireMode,
    pub hit: bool,
    pub launched: bool,
    pub intercepted: bool,
    pub vehicle_impact: VehicleImpactRules,
    pub distance: f64,
    pub gatling_damage: Option<u8>,
    pub coordinate: Option<HexCoordinate>,
}

/// Immediate attachment or heat changes, separate from ordinary damage packets.
#[derive(Default)]
pub(super) struct DirectEffects {
    pub narc: Option<NarcReport<UnitSection>>,
    /// Outside heat a heat-mode hit added to a heat-tracking target, in place of damage;
    /// zero when the turn's outside-heat cap was already full. `None` for any other shot.
    pub heat_transfer: Option<u8>,
    pub missed_terrain: Option<WoodlandImpact>,
    /// A miss from the next hex striking the building around a target inside one.
    pub missed_building: Option<super::StrayHit>,
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
            )?
        } else {
            super::narc::attach(world, request.shooter, request.target, shot)?
                .map_section(UnitSection::Mech)
        });
    }
    if !request.hit {
        let fall_rules = super::FallRules {
            vehicle_impact: request.vehicle_impact,
            ..super::FallRules::STANDARD
        };
        // A miss from the hex next door strikes the building the target is in (Total Warfare
        // p. 171); its damage is rolled only when there is a building to strike.
        let sheltered = request.launched
            && request.coordinate.is_none()
            && super::building_interior::unit_inside(world, request.target)?.is_some();
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
                HexCoordinate {
                    x: position.x.into(),
                    y: position.y.into(),
                }
            };
            report.missed_terrain = Some(resolve_woodland_attack(
                world,
                WoodlandAttack {
                    shooter: request.shooter,
                    coordinate,
                    weapon: request.weapon,
                    ammunition: request.ammunition,
                    damage,
                    intent: WoodlandIntent::Incidental,
                },
            )?);
            if sheltered {
                report.missed_building = super::building_interior::stray_shot(
                    world,
                    request.shooter,
                    request.target,
                    damage,
                    fall_rules,
                )?;
            }
        } else if sheltered {
            let damage = terrain_damage(world, &request)?;
            report.missed_building = super::building_interior::stray_shot(
                world,
                request.shooter,
                request.target,
                damage,
                fall_rules,
            )?;
        }
        return Ok(report);
    }
    if request.weapon == Weapon::ClanPlasmaCannon {
        // A plasma cannon deals a Mech no damage, only 2D6 outside heat (Total Warfare p. 139,
        // capped at fifteen a turn per p. 159); other targets take 3D6 damage instead.
        if let Some(target) = world.btech.constructed.get_mut(&request.target) {
            let heat = target.dice.d6() + target.dice.d6();
            report.heat_transfer = Some(target.absorb_outside_heat(heat));
        }
        return Ok(report);
    }
    if request.fire_mode != FireMode::Heat {
        return Ok(report);
    }
    // A heat-causing weapon's heat applies to Mechs only; a vehicle
    // tracks no heat, so the flamer's Damage Value lands as ordinary damage instead (Total
    // Warfare p. 113, Heat-Causing Weapon; p. 303 note). On a Mech the heat is outside heat,
    // counted against the fifteen-point cap a turn (Total Warfare p. 159).
    let Some(target) = world.btech.constructed.get_mut(&request.target) else {
        return Ok(report);
    };
    report.heat_transfer = Some(target.absorb_outside_heat(request.weapon.profile().damage));
    Ok(report)
}

/// Nominal shot damage for terrain, without a burst/pellet count or a hit-location roll.
/// Thermal hits and incidental misses share the existing range and component-damage rules.
fn terrain_damage(world: &mut World, request: &DirectEffectRequest) -> Result<u16> {
    let packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            weapon: request.weapon,
            ammunition: if request.ammunition == AmmunitionMode::Cluster {
                AmmunitionMode::Normal
            } else {
                request.ammunition
            },
            fire_mode: FireMode::Normal,
            gatling_damage: request.gatling_damage,
            distance: Some(request.distance),
            glancing: false,
            guidance_blocked: false,
            angel_blocked: false,
            target_beacon: false,
            artemis_v: super::artemis::artemis_v(world, request.shooter),
            ams_engaged: false,
            indirect: false,
            apollo: false,
            array: None,
        },
        super::dice::unit_dice_mut(world, request.shooter)?,
    )?;
    Ok(packets.damage.first().copied().unwrap_or(1).max(1))
}

/// Direct heat-transfer feedback, ordered before electronic and damage consequences.
pub(super) fn thermal_notices(
    shooter: ObjectId,
    target: ObjectId,
    heat_transfer: Option<u8>,
) -> Vec<Notice> {
    let mut notices = Vec::new();
    if heat_transfer.is_some() {
        notices.push(Notice {
            unit: target,
            text: "The flaming plasma sprays all over you!".into(),
        });
        notices.push(Notice {
            unit: shooter,
            text: "You cover your target in flaming plasma!".into(),
        });
    }
    notices
}
