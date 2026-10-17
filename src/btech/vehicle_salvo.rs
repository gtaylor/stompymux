//! Grouped vehicle hits reuse weapon packet rules and apply every impact in one transaction.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Damage inputs from an already successful attack, after launch expenditure and defense resolution.
#[derive(Debug, Clone, Copy)]
pub struct VehicleSalvoRequest {
    pub weapon: Weapon,
    pub ammunition: AmmunitionMode,
    pub fire_mode: FireMode,
    pub gatling_damage: Option<u8>,
    /// Actual spatial range, before aim-bracket rounding.
    pub distance: f64,
    pub glancing: bool,
    pub guidance_blocked: bool,
    pub angel_blocked: bool,
    /// An anti-missile system engaged the flight, shifting the cluster roll.
    pub ams_engaged: bool,
    /// Spotter-directed indirect fire gets no Artemis or Narc cluster bonus.
    pub indirect: bool,
    /// An MRM "Apollo" fire-control system guided the launcher: -1 on the Cluster Hits roll
    /// (Tactical Operations: Advanced Units & Equipment p. 143).
    pub apollo: bool,
    /// Machine guns fired together as a machine gun array: one Cluster Hits roll, every hit
    /// in one location (Total Warfare p. 137).
    pub array: Option<u8>,
}

/// One independently located packet and its complete vehicle consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleSalvoGroup {
    pub damage: u16,
    pub impact: VehicleImpact,
}

/// Ordered target effects on a vehicle; the enclosing attack still owns launch,
/// experience and publication.
pub type VehicleSalvoReport = super::SalvoReport<VehicleSalvoGroup, VehicleInfernoHit>;

/// Resolve a successful salvo atomically using only the target's saved dice stream.
/// Each packet receives its own location and damage entry, including packets following fatal hull loss.
/// Inferno shares missile clustering and interception, then uses the vehicle burning lifecycle.
pub fn resolve_vehicle_salvo(
    world: &mut World,
    target: ObjectId,
    arc: HitArc,
    request: VehicleSalvoRequest,
    rules: VehicleImpactRules,
) -> Result<VehicleSalvoReport> {
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
    request: VehicleSalvoRequest,
    rules: VehicleImpactRules,
    context: SalvoContext<'_>,
) -> Result<VehicleSalvoReport> {
    world.attempt(|world| {
        resolve_with_context_in_candidate(world, target, direction, request, rules, context)
    })
}

/// Resolve all groups directly in the caller's world; the caller owns rollback.
pub(super) fn resolve_with_context_in_candidate(
    world: &mut World,
    target: ObjectId,
    direction: super::hit_direction::HitDirection,
    request: VehicleSalvoRequest,
    rules: VehicleImpactRules,
    context: SalvoContext<'_>,
) -> Result<VehicleSalvoReport> {
    let SalvoContext {
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
        weapon.beacon_kind(request.ammunition).is_none(),
        "Vehicle beacon effects require dedicated target resolution"
    );
    // Heat mode has no effect on a unit that tracks no heat, so a flamer set to heat delivers
    // its Damage Value as damage to a vehicle (Total Warfare p. 113, p. 303 note).
    let fire_mode = if request.fire_mode == FireMode::Heat {
        FireMode::Normal
    } else {
        request.fire_mode
    };
    ensure!(
        match (request.fire_mode, request.gatling_damage) {
            (FireMode::Gatling, Some(damage)) => (1..=7).contains(&damage),
            (FireMode::Gatling, None) => false,
            (_, None) => true,
            _ => false,
        },
        "Invalid gatling launch damage"
    );
    let target_beacon =
        vehicle.has_beacon(BeaconKind::Narc) || vehicle.has_beacon(BeaconKind::Homing);
    let artemis_v = attacker.is_some_and(|shooter| super::artemis::artemis_v(world, shooter));
    let interference = attacker
        .is_some_and(|shooter| super::planetary_conditions::under_interference(world, shooter));
    let mut packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            weapon,
            ammunition: request.ammunition,
            fire_mode,
            gatling_damage: request.gatling_damage,
            distance: Some(request.distance),
            glancing: request.glancing,
            guidance_blocked: request.guidance_blocked,
            angel_blocked: request.angel_blocked,
            target_beacon,
            artemis_v,
            // A Swarm flight meets an anti-missile system on its own terms below.
            ams_engaged: request.ams_engaged && incoming.is_none(),
            indirect: request.indirect,
            apollo: request.apollo,
            array: request.array,
            unit_without_heat: true,
            interference,
        },
        &mut world.btech.vehicles.get_mut(&target).unwrap().dice,
    )?;
    if let Some(incoming) = incoming {
        packets.swarm_flight(incoming, request.ams_engaged)?;
    }
    let mut report = VehicleSalvoReport {
        building: None,
        collapse: None,
        cluster_roll: packets.cluster_roll,
        missiles_before_defense: None,
        missiles_intercepted: None,
        groups: Vec::new(),
        inferno: None,
        experience: Vec::new(),
        experience_messages: Vec::new(),
    };
    if weapon.profile().missiles > 0 {
        report.missiles_before_defense = packets.missiles_before_defense;
        report.missiles_intercepted = packets.missiles_intercepted;
        if request.ammunition == AmmunitionMode::Inferno {
            let surviving = packets.missile_count();
            if surviving > 0 {
                let arc = direction.current(world, target)?;
                report.inferno = Some(super::vehicle_burning::resolve_inferno_from(
                    world, target, arc, surviving, rules, attacker,
                )?);
            }
            world.btech.validate_action(world)?;
            return Ok(report);
        }
    }
    report.building =
        super::building_interior::absorb(world, attacker, target, &mut packets.damage)?;
    let single_location = packets.single_location;
    let groups = packets.damage;
    // A machine gun array's hits all strike the location its first hit found.
    let mut shared_section = None;
    for damage in groups {
        let arc = direction.current(world, target)?;
        let award = experience
            .map(|context| context.award(world, damage))
            .transpose()?
            .flatten();
        if let Some(context) = experience {
            report
                .experience_messages
                .extend(context.messages(world, damage, award.as_ref()));
        }
        report.experience.push(award);
        let preferred = aimed
            .map(|aimed| aimed.preferred(world, target, arc, false))
            .transpose()?
            .flatten();
        let forced = match (shared_section, preferred) {
            (Some(section), _) => Some(VehicleHit {
                section,
                through_armor_critical: false,
                motive: None,
            }),
            (None, Some(UnitSection::Vehicle(section))) => Some(VehicleHit {
                section,
                through_armor_critical: false,
                motive: None,
            }),
            _ => None,
        };
        let impact = super::vehicle_impact::resolve_directed_followup(
            world,
            target,
            arc,
            super::vehicle_impact::ImpactRequest {
                amount: u32::from(damage),
                armor_piercing: (request.ammunition == AmmunitionMode::ArmorPiercing)
                    .then_some(weapon),
                rear: forced.is_some() && arc == HitArc::Rear,
                attacker,
                class: DamageClass::of_weapon(weapon),
            },
            rules,
            request.glancing,
            forced,
        )?;
        if single_location && shared_section.is_none() {
            shared_section = impact.hit.as_ref().map(|hit| hit.section);
        }
        report.groups.push(VehicleSalvoGroup { damage, impact });
    }
    if let Some(cover) = &report.building {
        let fall_rules = super::FallRules {
            vehicle_impact: rules,
            ..super::FallRules::STANDARD
        };
        report.collapse = super::building_interior::collapse(world, target, cover, fall_rules)?;
    }
    world.btech.validate_action(world)?;
    Ok(report)
}
