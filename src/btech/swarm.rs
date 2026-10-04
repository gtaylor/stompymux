//! One missile-flight resolver shared by Mech and vehicle launchers and targets.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// One attack along a Swarm flight; a miss preserves every incoming missile.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SwarmHop {
    pub target: ObjectId,
    pub incoming: u8,
    pub roll: u8,
    pub remaining: u8,
    pub salvo: Option<TargetSalvo>,
}

/// Ordered consequences of a single launch, including secondary attacks and their unused missiles.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SwarmReport {
    pub launched: u8,
    pub remaining: u8,
    pub traveled: f64,
    pub hops: Vec<SwarmHop>,
    pub notices: Vec<Notice>,
    /// Private damage checks ordered across every target in the flight.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Immutable launch facts; secondary attacks retain the original threshold and glancing state.
pub(super) struct SwarmRequest<'a> {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub weapon: super::salvo::SalvoWeapon,
    pub rules: ShotRules,
    pub first_hit: bool,
    pub first_roll: u8,
    pub target_number: i32,
    pub glancing: bool,
    pub character: bool,
    pub experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
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

/// Resolve an admitted flight inside its launcher's existing world/effect transaction.
pub(super) fn resolve(world: &mut World, request: SwarmRequest<'_>) -> Result<SwarmReport> {
    let shooter = request.shooter;
    let source = super::scanner::scanner_unit(world, shooter).context("Launcher unavailable")?;
    let map = source.position.context("Launcher is not placed")?.map;
    let team = source.signature.team;
    let weapon = request.weapon.weapon;
    let mut report = SwarmReport {
        launched: weapon.profile().missiles,
        remaining: weapon.profile().missiles,
        traveled: 0.0,
        hops: Vec::new(),
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    let mut previous = shooter;
    let mut target = request.target;
    while report.remaining > 0 {
        report.traveled += unit_range(world, previous, target)?.spatial;
        if weapon
            .range_modifier(report.traveled, request.rules.aim.extended_ranges)?
            .is_none()
        {
            report.notices.push(Notice {
                unit: target,
                text: "Luckily, the missiles fall short of you!".into(),
            });
            break;
        }
        let roll = if report.hops.is_empty() {
            request.first_roll
        } else {
            super::dice::unit_dice_mut(world, shooter)?.generic_roll()
        };
        let hit = if report.hops.is_empty() {
            request.first_hit
        } else {
            i32::from(roll) >= request.target_number
        };
        let incoming = report.remaining;
        let salvo = if hit {
            let mut experience = request.experience;
            if let Some(context) = &mut experience {
                context.request.target = target;
            }
            let salvo = if world.btech.vehicles().contains_key(&target) {
                let distance = unit_range(world, shooter, target)?.spatial;
                super::target_salvo::resolve_vehicle_target(
                    world,
                    shooter,
                    target,
                    VehicleSalvoRequest {
                        range_damage: request.rules.range_damage,
                        damage_penalty: request.weapon.damage_penalty,
                        weapon,
                        ammunition: request.weapon.ammunition_mode,
                        fire_mode: request.weapon.fire_mode,
                        gatling_damage: request.weapon.gatling_damage,
                        distance,
                        glancing: request.glancing,
                        guidance_blocked: false,
                        angel_blocked: false,
                        intercepted: 0,
                    },
                    request.rules.hit_arc_mode,
                    request.rules.vehicle_impact,
                    super::vehicle_salvo::SalvoContext {
                        submerged: false,
                        woods_damage: request.rules.aim.woods_damage,
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
                        submerged: false,
                        woods_damage: request.rules.aim.woods_damage,
                        range_damage: request.rules.range_damage,
                        aimed: None,
                        incoming: Some(incoming),
                        rules: FallRules {
                            vehicle_impact: request.rules.vehicle_impact,
                            stacking: request.rules.stacking,
                            stagger: request.rules.stagger,
                            hit: request.rules.hit,
                            extended_piloting: request.rules.extended_piloting,
                            toughness: request.rules.target_toughness,
                        },
                        hit_arc_mode: request.rules.hit_arc_mode,
                        glancing: request.glancing,
                        character: request.character,
                        intercepted: 0,
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
                        if report.hops.is_empty() {
                            "You hit"
                        } else {
                            "The swarm hits"
                        },
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
            roll,
            remaining: report.remaining,
            salvo,
        });
        // The reference stops after the attack following ten previously visited targets.
        if report.remaining == 0 || report.hops.len() == 11 {
            break;
        }
        let mut next = None;
        for candidate in super::map_slots::all_unit_order(world, map)? {
            if report.hops.iter().any(|hop| hop.target == candidate)
                || combat_safe(world, candidate)?
            {
                continue;
            }
            let Some(unit) = super::scanner::scanner_unit(world, candidate) else {
                continue;
            };
            if request.weapon.ammunition_mode.munition() == AmmunitionMode::Swarm1
                && unit.signature.team == team
            {
                continue;
            }
            if unit_range(world, target, candidate)?.spatial < 1.9 && sees(world, target, candidate)
            {
                next = Some(candidate);
                break;
            }
        }
        let Some(next) = next else {
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
        previous = target;
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
