//! Grouped vehicle hits reuse weapon packet rules and apply every impact in one transaction.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Damage inputs from an already successful attack, after launch expenditure and defense resolution.
#[derive(Debug, Clone, Copy)]
pub struct BattleVehicleSalvoRequest {
    /// Apply the configured energy range rule before glancing damage.
    pub range_damage: bool,
    /// Damage lost to the launching weapon’s focusing components.
    pub damage_penalty: u8,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub fire_mode: BattleFireMode,
    pub gatling_damage: Option<u8>,
    /// Actual spatial range, before aim-bracket rounding.
    pub distance: f64,
    pub glancing: bool,
    pub guidance_blocked: bool,
    pub angel_blocked: bool,
    /// Missiles intercepted by a separately resolved defensive action.
    pub intercepted: u8,
}

/// One independently located packet and its complete vehicle consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleVehicleSalvoGroup {
    pub damage: u16,
    pub impact: BattleVehicleImpact,
}

/// Ordered target effects; the enclosing attack still owns launch, experience and publication.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Publish salvo notices and broadcasts with the enclosing attack transaction"]
pub struct BattleVehicleSalvoReport {
    /// Nominal LBX damage terrain check, before pellet counting and cover absorption.
    pub initial_woods: Option<BattleWoodsAbsorption>,
    pub woods: Option<BattleWoodsAbsorption>,
    pub cluster_roll: Option<u8>,
    pub missiles_before_defense: Option<u8>,
    pub groups: Vec<BattleVehicleSalvoGroup>,
    pub inferno: Option<BattleVehicleInfernoHit>,
    /// Per-packet pre-impact award evidence, including ineligible packets.
    pub experience: Vec<Option<BattleShotExperienceAward>>,
    /// Ordered award diagnostics for the enclosing host transaction.
    pub experience_messages: Vec<BattleChannelMessage>,
}

/// Resolve a successful salvo atomically using only the target's saved dice stream.
/// Each packet receives its own location and damage entry, including packets following fatal hull loss.
/// Inferno shares missile clustering and interception, then uses the vehicle burning lifecycle.
pub fn resolve_vehicle_salvo(
    world: &mut World,
    target: ObjectId,
    arc: BattleHitArc,
    request: BattleVehicleSalvoRequest,
    rules: BattleVehicleImpactRules,
) -> Result<BattleVehicleSalvoReport> {
    resolve_with_context(
        world,
        target,
        super::hit_direction::HitDirection::Fixed(arc),
        request,
        rules,
        SalvoContext::default(),
    )
}

/// An admitted shot retains its source independently of optional experience awards.
#[derive(Clone, Copy, Default)]
pub(super) struct SalvoContext<'a> {
    pub woods_damage: bool,
    pub submerged: bool,
    pub aimed: Option<super::aimed_hit::AimedShot>,
    pub incoming: Option<u8>,
    pub attacker: Option<ObjectId>,
    pub experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
}

/// Apply optional shooting awards immediately before each packet mutates its target.
pub(super) fn resolve_with_context(
    world: &mut World,
    target: ObjectId,
    direction: super::hit_direction::HitDirection,
    request: BattleVehicleSalvoRequest,
    rules: BattleVehicleImpactRules,
    context: SalvoContext<'_>,
) -> Result<BattleVehicleSalvoReport> {
    resolve_with_context_mode(
        world,
        target,
        direction,
        request,
        rules,
        context,
        super::shot_transaction::EffectMode::Atomic,
    )
}

/// Resolve all groups in the explicitly selected savepoint.
pub(super) fn resolve_with_context_mode(
    world: &mut World,
    target: ObjectId,
    direction: super::hit_direction::HitDirection,
    request: BattleVehicleSalvoRequest,
    rules: BattleVehicleImpactRules,
    context: SalvoContext<'_>,
    mode: super::shot_transaction::EffectMode,
) -> Result<BattleVehicleSalvoReport> {
    let SalvoContext {
        woods_damage,
        submerged,
        aimed,
        incoming,
        attacker,
        experience,
    } = context;
    let object = world
        .objects
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&target)
        .context("Vehicle is unavailable")?;
    ensure!(
        !vehicle.is_destroyed() || incoming.is_some(),
        "Vehicle is destroyed"
    );
    ensure!(
        request.distance.is_finite() && request.distance >= 0.0,
        "Invalid weapon damage range"
    );
    let weapon = request.weapon;
    ensure!(
        !weapon.is_artillery() && !weapon.is_ams(),
        "Weapon requires a dedicated firing action"
    );
    ensure!(
        request.ammunition.supports(weapon) && request.fire_mode.supports(weapon),
        "Weapon mode is not supported"
    );
    ensure!(
        weapon.beacon_kind(request.ammunition).is_none()
            && weapon != BattleWeapon::CoolantGun
            && request.fire_mode != BattleFireMode::Heat,
        "Vehicle beacon and thermal effects require dedicated target resolution"
    );
    ensure!(
        request.intercepted <= weapon.profile().missiles,
        "Invalid intercepted missile count"
    );
    ensure!(
        match (request.fire_mode, request.gatling_damage) {
            (BattleFireMode::Gatling, Some(damage)) => (1..=6).contains(&damage),
            (BattleFireMode::Gatling, None) => false,
            (_, None) => true,
            _ => false,
        },
        "Invalid gatling launch damage"
    );
    let target_beacon =
        vehicle.has_beacon(BattleBeaconKind::Narc) || vehicle.has_beacon(BattleBeaconKind::Homing);
    let mut candidate = super::shot_transaction::EffectCandidate::new(world, mode);
    let initial_woods = if woods_damage && let Some(shooter) = attacker {
        super::woods_absorption::begin_pellets(
            &mut candidate,
            shooter,
            target,
            weapon,
            request.ammunition,
        )?
    } else {
        None
    };
    let shell_woods = woods_damage
        && attacker.is_some()
        && super::woods_absorption::direct_shells(weapon, request.ammunition);
    let mut packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            submerged,
            range_damage: request.range_damage,
            damage_penalty: request.damage_penalty,
            weapon,
            ammunition: request.ammunition,
            fire_mode: request.fire_mode,
            gatling_damage: request.gatling_damage,
            distance: Some(request.distance),
            glancing: request.glancing
                && (!shell_woods || request.fire_mode.rounds_per_cycle() > 1),
            guidance_blocked: request.guidance_blocked,
            angel_blocked: request.angel_blocked,
            target_beacon,
        },
        &mut super::autopilot::diagnostics::make_mut(&mut candidate.btech.vehicles)
            .get_mut(&target)
            .unwrap()
            .dice,
    )?;
    packets.limit_missiles(incoming);
    packets.finish_burst_glancing(request.fire_mode, request.glancing && !shell_woods);
    let mut report = BattleVehicleSalvoReport {
        initial_woods,
        woods: None,
        cluster_roll: packets.cluster_roll,
        missiles_before_defense: None,
        groups: Vec::new(),
        inferno: None,
        experience: Vec::new(),
        experience_messages: Vec::new(),
    };
    if let Some((hits, surviving)) = packets.intercept(weapon, request.intercepted) {
        report.missiles_before_defense = Some(hits as u8);
        if request.ammunition == BattleAmmunitionMode::Inferno {
            if surviving > 0 {
                report.inferno = Some(super::vehicle_burning::resolve_inferno_from(
                    &mut candidate,
                    target,
                    surviving,
                    rules,
                    attacker,
                )?);
            }
            candidate.btech.validate(&candidate)?;
            candidate.commit();
            return Ok(report);
        }
    }
    if woods_damage
        && (weapon.profile().missiles > 0 || request.ammunition == BattleAmmunitionMode::Cluster)
        && let Some(shooter) = attacker
    {
        report.woods = super::woods_absorption::resolve_projectiles(
            &mut candidate,
            shooter,
            target,
            weapon,
            request.ammunition,
            &mut packets,
        )?;
    }
    if shell_woods && let Some(shooter) = attacker {
        report.woods = super::woods_absorption::resolve_shells(
            &mut candidate,
            shooter,
            target,
            weapon,
            request.ammunition,
            &mut packets.damage,
            request.glancing,
        )?;
    }
    let groups = packets.damage;
    for damage in groups {
        let arc = direction.current(&candidate, target)?;
        let award = experience
            .map(|context| context.award(&mut candidate, damage))
            .transpose()?
            .flatten();
        if let Some(context) = experience {
            report
                .experience_messages
                .extend(context.messages(&candidate, damage, award.as_ref()));
        }
        report.experience.push(award);
        let preferred = aimed
            .map(|aimed| aimed.preferred(&mut candidate, target, arc, false))
            .transpose()?
            .flatten();
        let forced = match preferred {
            Some(BattleUnitSection::Vehicle(section)) => Some(BattleVehicleHit {
                section,
                through_armor_critical: false,
                motive: None,
                motive_roll: None,
                piloting_penalty: 0,
            }),
            _ => None,
        };
        let impact = super::vehicle_impact::resolve_directed_followup(
            &mut candidate,
            target,
            arc,
            super::vehicle_impact::ImpactRequest {
                amount: u32::from(damage),
                armor_piercing: (request.ammunition == BattleAmmunitionMode::ArmorPiercing)
                    .then_some(weapon),
                rear: forced.is_some() && arc == BattleHitArc::Rear,
                attacker,
            },
            rules,
            forced,
        )?;
        report
            .groups
            .push(BattleVehicleSalvoGroup { damage, impact });
    }
    candidate.btech.validate(&candidate)?;
    candidate.commit();
    Ok(report)
}
