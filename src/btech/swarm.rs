//! Swarm and Swarm-I LRM flights (Tactical Operations: Advanced Units & Equipment p. 183): one
//! resolver shared by Mech and vehicle launchers and targets.
//!
//! Missiles that do not attack a target fly on to the nearest unit, friend or foe, in its hex
//! or an adjacent one, starting with its own hex and choosing at random among equals; no line
//! of sight is needed and no unit is attacked twice by one flight. A missed attack sends every
//! missile on. Each later attack keeps the first attack's modifiers except that its own
//! target's movement and terrain replace the first target's, and rolls its own Cluster Hits
//! on the column for the missiles still flying. Swarm-I takes +2 against friendly units unless
//! they stand in hostile ECM or have lost sensors. In hostile ECM a Swarm-I flight acts as
//! Swarm, and a Swarm flight attacking a target in hostile ECM acts as an ordinary LRM flight:
//! its missiles that miss that target fly no farther.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// One attack along a Swarm flight; a miss preserves every incoming missile.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SwarmHop {
    pub target: ObjectId,
    pub incoming: u8,
    /// The attack's target number; later attacks take their own target's modifiers.
    pub target_number: i32,
    pub roll: u8,
    pub remaining: u8,
    pub salvo: Option<TargetSalvo>,
}

/// Ordered consequences of a single launch, including secondary attacks and their unused missiles.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SwarmReport {
    pub launched: u8,
    pub remaining: u8,
    pub hops: Vec<SwarmHop>,
    pub notices: Vec<Notice>,
    /// Private damage checks ordered across every target in the flight.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Immutable launch facts. The first attack was rolled with the launch; later ones roll here.
pub(super) struct SwarmRequest<'a> {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub weapon: super::salvo::SalvoWeapon,
    pub rules: ShotRules,
    pub first_hit: bool,
    pub first_roll: u8,
    pub first_glancing: bool,
    /// The first attack's target number.
    pub target_number: i32,
    /// The first attack's target number less its target's movement and terrain modifiers,
    /// which each later attack replaces with its own target's.
    pub base_target_number: i32,
    pub character: bool,
    pub experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
    /// An anti-missile system engaged the first attack.
    pub ams_engaged: bool,
}

/// A unit's target movement and terrain modifiers against `shooter`'s Swarm flight, read
/// without requiring line of sight (Tactical Operations: Advanced Units & Equipment p. 183).
fn target_terms(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: Weapon,
    ammunition: AmmunitionMode,
) -> Result<i32> {
    let distance = unit_range(world, shooter, target)?.hexes();
    let terms = super::aim::target_modifiers(world, shooter, target, weapon, ammunition, distance)?;
    let (terrain, range) = super::los::unit_terrain_geometry(world, shooter, target)?;
    let profile = super::perception_profile(world, shooter)?;
    let cover = super::perception::cover_aim(world, &profile, target, terrain, range);
    Ok(i32::from(terms.movement) + i32::from(terms.bogged_down) + i32::from(cover))
}

/// The first attack's modifiers without its target's movement and terrain terms.
pub(super) fn base_target_number(aim: &AimModifiers, target_number: i32) -> i32 {
    target_number
        - i32::from(aim.target_movement)
        - i32::from(aim.bogged_down)
        - aim
            .perception
            .map_or(0, |perception| i32::from(perception.modifier))
}

/// Whether `target` stands in ECM hostile to `shooter`'s side.
fn in_hostile_ecm(world: &World, target: ObjectId) -> Result<bool> {
    Ok(super::electronic_field(world, target)?.blocks_incoming_guidance())
}

/// Whether a unit has lost a sensor to a critical hit.
fn sensors_damaged(world: &World, id: ObjectId) -> bool {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit.sensor_hits() > 0;
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .is_some_and(|unit| unit.system_hits(System::Sensors) > 0)
}

/// Retargeting uses retained acquisition and clear terrain, without acquiring new contacts.
fn sees(world: &World, observer: ObjectId, target: ObjectId) -> bool {
    if observer == target {
        return true;
    }
    let Some(unit) = super::scanner::scanner_unit(world, observer) else {
        return false;
    };
    unit.visibility.clairvoyant
        || (unit.contacts.contains_key(&target)
            && super::visibility::unit_unblocked(world, observer, target).unwrap_or(false))
}

/// The next unit the flight turns on: the nearest unvisited unit in `from`'s hex or an
/// adjacent one, at random among equals.
fn next_target(
    world: &mut World,
    shooter: ObjectId,
    map: ObjectId,
    from: ObjectId,
    visited: &[ObjectId],
) -> Result<Option<ObjectId>> {
    let mut nearest: Vec<(u64, ObjectId)> = Vec::new();
    for candidate in super::map_slots::all_unit_order(world, map)? {
        if visited.contains(&candidate) || combat_safe(world, candidate)? {
            continue;
        }
        let Some(unit) = super::scanner::scanner_unit(world, candidate) else {
            continue;
        };
        if unit.destroyed || unit.position.is_none() {
            continue;
        }
        let distance = unit_range(world, from, candidate)?.hex_distance;
        if distance <= 1 {
            nearest.push((distance, candidate));
        }
    }
    let Some(closest) = nearest.iter().map(|(distance, _)| *distance).min() else {
        return Ok(None);
    };
    nearest.retain(|(distance, _)| *distance == closest);
    let pick = if nearest.len() > 1 {
        usize::from(
            super::dice::unit_dice_mut(world, shooter)?.die(u16::try_from(nearest.len())?)? - 1,
        )
    } else {
        0
    };
    Ok(Some(nearest[pick].1))
}

/// Resolve an admitted flight inside its launcher's existing world/effect transaction.
pub(super) fn resolve(world: &mut World, request: SwarmRequest<'_>) -> Result<SwarmReport> {
    let shooter = request.shooter;
    let source = super::scanner::scanner_unit(world, shooter).context("Launcher unavailable")?;
    let map = source.position.context("Launcher is not placed")?.map;
    let team = source.signature.team;
    let weapon = request.weapon.weapon;
    let ammunition = request.weapon.ammunition_mode;
    let mut report = SwarmReport {
        launched: weapon.profile().missiles,
        remaining: weapon.profile().missiles,
        hops: Vec::new(),
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    let mut target = request.target;
    loop {
        let first = report.hops.is_empty();
        let hostile_ecm = in_hostile_ecm(world, target)?;
        let (target_number, roll) = if first {
            (request.target_number, request.first_roll)
        } else {
            let friendly = super::scanner::scanner_unit(world, target)
                .is_some_and(|unit| unit.signature.team == team);
            // Swarm-I missiles avoid friends unless hostile ECM or lost sensors confuse them.
            let identify_friend = ammunition.munition() == AmmunitionMode::Swarm1
                && friendly
                && !hostile_ecm
                && !super::electronic_field(world, target)?.blocks_outgoing_guidance()
                && !sensors_damaged(world, target);
            let number = request.base_target_number
                + target_terms(world, shooter, target, weapon, ammunition)?
                + if identify_friend { 2 } else { 0 };
            (
                number,
                super::dice::unit_dice_mut(world, shooter)?.generic_roll(),
            )
        };
        let hit = if first {
            request.first_hit
        } else {
            i32::from(roll) >= target_number
        };
        let glancing = if first {
            request.first_glancing
        } else {
            hit && i32::from(roll) == target_number
        };
        let incoming = report.remaining;
        let salvo = if hit {
            let mut experience = request.experience;
            if let Some(context) = &mut experience {
                context.request.target = target;
            }
            let ams_engaged = request.ams_engaged && first;
            let salvo = if world.btech.vehicles().contains_key(&target) {
                let distance = unit_range(world, shooter, target)?.hexes();
                super::target_salvo::resolve_vehicle_target(
                    world,
                    shooter,
                    target,
                    VehicleSalvoRequest {
                        weapon,
                        ammunition,
                        fire_mode: request.weapon.fire_mode,
                        gatling_damage: request.weapon.gatling_damage,
                        distance,
                        glancing,
                        guidance_blocked: false,
                        angel_blocked: false,
                        ams_engaged,
                        indirect: false,
                        apollo: false,
                        array: None,
                    },
                    request.rules.vehicle_impact,
                    super::vehicle_salvo::SalvoContext {
                        aimed: None,
                        incoming: Some(incoming),
                        attacker: Some(shooter),
                        experience,
                    },
                )?
            } else {
                TargetSalvo::Mech(super::salvo::resolve_salvo_from_shot(
                    world,
                    shooter,
                    target,
                    request.weapon,
                    super::salvo::ShotDamage {
                        aimed: None,
                        incoming: Some(incoming),
                        rules: FallRules {
                            vehicle_impact: request.rules.vehicle_impact,
                            extended_piloting: request.rules.extended_piloting,
                            toughness: request.rules.target_toughness,
                        },
                        glancing,
                        character: request.character,
                        ams_engaged,
                        indirect: false,
                        apollo: false,
                        array: None,
                        submerged: false,
                        experience,
                    },
                )?)
            };
            report.remaining =
                incoming.saturating_sub(salvo.missiles_before_defense().unwrap_or(0));
            if sees(world, shooter, target) {
                let hits = incoming - report.remaining;
                report.notices.push(Notice {
                    unit: shooter,
                    text: format!(
                        "[fg=green]{} with {hits} missile{}![reset]",
                        if first { "You hit" } else { "The swarm hits" },
                        if hits > 1 { "s" } else { "" }
                    ),
                });
            }
            let mut private = Vec::new();
            let notices = salvo.notices_with_feedback(shooter, target, &mut private);
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                private,
                report.notices.len(),
            );
            report.notices.extend(notices);
            report.broadcasts.extend(salvo.broadcasts());
            Some(salvo)
        } else {
            None
        };
        report.hops.push(SwarmHop {
            target,
            incoming,
            target_number,
            roll,
            remaining: report.remaining,
            salvo,
        });
        // A Swarm flight attacking a target in hostile ECM is an ordinary LRM flight; a
        // Swarm-I flight there only loses its friend-or-foe recognition.
        if report.remaining == 0 || (hostile_ecm && ammunition.munition() == AmmunitionMode::Swarm)
        {
            break;
        }
        let visited: Vec<_> = report.hops.iter().map(|hop| hop.target).collect();
        let Some(next) = next_target(world, shooter, map, target, &visited)? else {
            break;
        };
        if next != shooter {
            report.notices.push(Notice {
                unit: next,
                text: "The missile-swarm turns towards you!".into(),
            });
        }
        if sees(world, shooter, target) {
            let name = if next == shooter {
                "YOU!!".into()
            } else {
                visible_contact(world, shooter, next)?.map_or_else(
                    || "something".into(),
                    |contact| format!("{} [{}]", contact.name, contact.label),
                )
            };
            report.notices.push(Notice {
                unit: shooter,
                text: format!(
                    "Your missile-swarm of {} missile{} targets {name}!",
                    report.remaining,
                    if report.remaining > 1 { "s" } else { "" }
                ),
            });
        }
        report
            .notices
            .extend(super::broadcast::swarm_notices(world, shooter, next));
        target = next;
    }
    Ok(report)
}

/// Toggle either Swarm supply through the shared multi-chassis ammunition controls.
pub fn toggle_swarm(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    friend_or_foe: bool,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    anyhow::ensure!(
        !ready.weapon.is_rocket(),
        "Rocket launchers' mode cannot be altered!"
    );
    let mode = if friend_or_foe {
        AmmunitionMode::Swarm1
    } else {
        AmmunitionMode::Swarm
    };
    anyhow::ensure!(
        super::weapon_controls::selectable_munition(world, id, index, mode),
        "That weapon cannot fire Swarm missiles!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world, id, index, mode,
    ))
}

/// Both commands reuse the ordinary selector, authority checks and effect transaction.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let friend_or_foe = input.name.eq_ignore_ascii_case("fireswarm1");
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_swarm(world, id, pilot, index, friend_or_foe).map(|mode| mode.swarm_message(index))
    })
}
