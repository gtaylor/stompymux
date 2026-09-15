//! Shared BattleMech physical targeting, damage and transactional leg, fist and hand-weapon attacks.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The leg used by a kick or trip; the other must still support the unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleLeg {
    Left,
    Right,
}
impl BattleLeg {
    /// The selected attack leg: front pair on quads.
    pub fn section(self, chassis: BattleMechChassis) -> BattleSection {
        match self {
            Self::Left if chassis == BattleMechChassis::Quad => BattleSection::LeftArm,
            Self::Right if chassis == BattleMechChassis::Quad => BattleSection::RightArm,
            Self::Left => BattleSection::LeftLeg,
            Self::Right => BattleSection::RightLeg,
        }
    }
}

/// An arm selected for a physical attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleArm {
    Left,
    Right,
}

impl BattleArm {
    /// The attacking biped section.
    pub fn section(self) -> BattleSection {
        match self {
            Self::Left => BattleSection::LeftArm,
            Self::Right => BattleSection::RightArm,
        }
    }
}

/// The physical attack determines reach, arc, base aim, damage and balance rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BattlePhysicalAttack {
    Kick { leg: BattleLeg },
    Trip { leg: BattleLeg },
    Punch { arm: BattleArm },
    Axe { arm: BattleArm },
    Sword { arm: BattleArm },
    Mace { arm: BattleArm },
    Saw { arm: BattleArm },
    Claw { arm: BattleArm },
    Club,
}

impl BattlePhysicalAttack {
    /// Limb that owns this attack's recovery timer for the attacking chassis.
    pub fn section(self, chassis: BattleMechChassis) -> BattleSection {
        match self {
            Self::Club => BattleSection::RightArm,
            Self::Kick { leg } | Self::Trip { leg } => leg.section(chassis),
            Self::Punch { arm }
            | Self::Axe { arm }
            | Self::Sword { arm }
            | Self::Mace { arm }
            | Self::Saw { arm }
            | Self::Claw { arm } => arm.section(),
        }
    }
    /// Check chassis anatomy and supporting legs before geometric or timing eligibility.
    pub fn validate_chassis_support(self, unit: &BattleUnit) -> Result<()> {
        let chassis = unit.chassis();
        if !self.uses_leg() {
            ensure!(
                chassis != BattleMechChassis::Quad,
                "What are you going to {} with, your front right leg?",
                self.verb()
            );
            return Ok(());
        }
        let maximum_missing = usize::from(chassis == BattleMechChassis::Quad);
        ensure!(
            unit.unavailable_legs() <= maximum_missing,
            "Too few supporting legs for this attack"
        );
        ensure!(
            !unit.leg_unavailable(self.section(chassis)),
            "The attacking leg is unavailable"
        );
        for &leg in chassis.legs() {
            if unit.leg_unavailable(leg) {
                continue;
            }
            ensure!(
                actuator(unit, leg, 0, BattleSystem::ShoulderOrHip)?,
                "You cannot attack with a destroyed hip"
            );
        }
        Ok(())
    }

    /// Whether the attack uses leg reach and the real forward arc.
    fn uses_leg(self) -> bool {
        matches!(self, Self::Kick { .. } | Self::Trip { .. })
    }

    /// Trips force balance rather than applying direct impact damage.
    fn is_trip(self) -> bool {
        matches!(self, Self::Trip { .. })
    }
    /// Installed arm weapon, including mechanical weapons that need no hand.
    fn hand_weapon(self) -> Option<BattleArmAttack> {
        match self {
            Self::Axe { .. } => Some(BattleArmAttack::Axe),
            Self::Sword { .. } => Some(BattleArmAttack::Sword),
            Self::Mace { .. } => Some(BattleArmAttack::Mace),
            Self::Saw { .. } => Some(BattleArmAttack::Saw),
            Self::Claw { .. } => Some(BattleArmAttack::Claw),
            _ => None,
        }
    }

    /// Player-facing attack verb.
    fn verb(self) -> &'static str {
        match self {
            Self::Kick { .. } => "kick",
            Self::Trip { .. } => "trip",
            Self::Punch { .. } => "punch",
            Self::Axe { .. } => "axe",
            Self::Sword { .. } => "chop",
            Self::Mace { .. } => "mace",
            Self::Saw { .. } => "saw",
            Self::Claw { .. } => "claw",
            Self::Club => "club",
        }
    }
}

/// Physical arm attack selected independently of left/right/both arm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleArmAttack {
    Punch,
    Axe,
    Sword,
    Mace,
    Saw,
    Claw,
}

impl BattleArmAttack {
    /// Bind one selected arm to the shared physical attack description.
    fn attack(self, arm: BattleArm) -> BattlePhysicalAttack {
        match self {
            Self::Punch => BattlePhysicalAttack::Punch { arm },
            Self::Axe => BattlePhysicalAttack::Axe { arm },
            Self::Sword => BattlePhysicalAttack::Sword { arm },
            Self::Mace => BattlePhysicalAttack::Mace { arm },
            Self::Saw => BattlePhysicalAttack::Saw { arm },
            Self::Claw => BattlePhysicalAttack::Claw { arm },
        }
    }

    /// Count operational same-arm parts using the attack's observable minimum, not its mass divisor.
    pub(super) fn available(self, unit: &BattleUnit, section: BattleSection) -> Result<bool> {
        let (system, minimum) = match self {
            Self::Punch => return Ok(true),
            Self::Axe => (BattleSystem::Axe, unit.definition().tons / 15),
            Self::Sword => (BattleSystem::Sword, (unit.definition().tons + 15) / 20),
            Self::Mace => (BattleSystem::Mace, unit.definition().tons / 10),
            Self::Saw => (BattleSystem::DualSaw, 7),
            Self::Claw => (BattleSystem::Claw, unit.definition().tons / 15),
        };
        Ok(unit
            .loadout()?
            .systems
            .iter()
            .filter(|part| {
                part.system == system
                    && part.location.section == section
                    && !unit.critical_unavailable(part.location)
            })
            .count()
            >= usize::from(minimum))
    }
}

/// Inspect an arm attack without advancing dice or any recovery timer.
pub fn arm_attack_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arm: BattleArm,
    kind: BattleArmAttack,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        kind.attack(arm),
        rules,
        None,
    )
}

/// Policy shared by physical inspection and attack resolution.
#[derive(Debug, Clone, Copy)]
pub struct BattlePhysicalRules {
    pub use_pilot_skill: bool,
    pub fasa_turning: bool,
    pub extended_movement: bool,
    pub hit_arc_mode: i64,
    pub glancing: BattleGlancingMode,
    pub fall: BattleFallRules,
}

/// Read-only physical attack calculation. The attack roll and all consequences are separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattlePhysicalProfile {
    pub attack: BattlePhysicalAttack,
    pub target_number: i32,
    pub base: i16,
    pub actuators: u8,
    /// Aim penalty inherent to the selected physical weapon.
    pub weapon_modifier: u8,
    pub attacker_movement: i16,
    pub target_movement: i8,
    pub terrain: u8,
    /// Direct material damage; trips always have zero.
    pub damage: u16,
    /// Location table for direct impacts; unused by trips and fixed-location attacks.
    pub hit_table: BattleHitTable,
    /// Direct location used instead of a table roll by claws and saws.
    pub fixed_location: Option<BattleSection>,
    pub hit_arc: BattleHitArc,
}

/// Completed attack with already-applied damage, balance and fall consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattlePhysicalReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: BattlePhysicalProfile,
    pub roll: u8,
    pub hit: bool,
    pub glancing: bool,
    pub impact: Option<BattleTacticalImpact>,
    /// Piloting XP applied before eligible direct damage; absent for trips and ineligible combat.
    pub experience: Option<BattleExperienceAward>,
    /// Accepted damage and control-check XP diagnostics captured during resolution.
    pub experience_messages: Vec<super::BattleChannelMessage>,
    /// Kick balance check, missed-mace balance, or target balance after a successful trip; absent for punches and missed trips.
    pub balance: Option<BattlePilotingCheck>,
    pub fall: Option<BattleFallReport>,
    /// Private balance feedback ordered among attack and fall notices.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

/// Test one mechanical actuator at its canonical slot, including flooding and slot loss.
pub(super) fn actuator(
    unit: &BattleUnit,
    section: BattleSection,
    slot: u8,
    system: BattleSystem,
) -> Result<bool> {
    let location = CriticalLocation { section, slot };
    Ok(!unit.critical_unavailable(location)
        && unit
            .loadout()?
            .systems
            .iter()
            .any(|part| part.location == location && part.system == system))
}

/// Inspect a kick without consuming dice or changing state.
pub fn kick_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: BattleLeg,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        BattlePhysicalAttack::Kick { leg },
        rules,
        None,
    )
}

/// Inspect a trip without consuming dice or changing state. Trips have zero direct damage.
pub fn trip_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: BattleLeg,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        BattlePhysicalAttack::Trip { leg },
        rules,
        None,
    )
}

/// Inspect one punch without consuming dice or changing state.
pub fn punch_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arm: BattleArm,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        BattlePhysicalAttack::Punch { arm },
        rules,
        None,
    )
}

/// Per-action arm recovery allowance and character casualty publication capability.
#[derive(Default)]
struct AttackContext {
    completed_arm: Option<BattleSection>,
    character: bool,
}

/// Validate the attack envelope; only this action's first arm may already be recovering.
fn attack_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: BattlePhysicalAttack,
    rules: BattlePhysicalRules,
    completed_arm: Option<BattleSection>,
) -> Result<BattlePhysicalProfile> {
    attack_profile_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm,
            character: false,
        },
    )
}

/// Shared targeting and equipment checks for either casualty publication mode.
fn attack_profile_inner(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: BattlePhysicalAttack,
    rules: BattlePhysicalRules,
    context: AttackContext,
) -> Result<BattlePhysicalProfile> {
    let completed_arm = context.completed_arm;
    super::power::controlled_unit(world, attacker, pilot)?;
    ensure!(attacker != target, "You cannot attack yourself");
    for id in [attacker, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
        ensure!(
            context.character || !object.flags.contains(Flag::InCharacter),
            "Physical attacks require non-character units"
        );
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
    }
    let source = &world.btech.constructed_units()[&attacker];
    let victim = &world.btech.constructed_units()[&target];
    attack.validate_chassis_support(source)?;
    ensure!(
        source.power() == BattlePower::Running,
        "Start the unit first"
    );
    ensure!(
        !source.airborne() && source.free_fall().is_none(),
        "You can't perform physical attacks while in the air!"
    );
    ensure!(
        source.stand_timer().is_none(),
        "You are still trying to stand up!"
    );
    ensure!(
        source
            .limb_recycle
            .keys()
            .all(|section| Some(*section) == completed_arm)
            && !source
                .limb_recycle
                .contains_key(&attack.section(source.chassis())),
        "Your limbs are still recovering from another attack"
    );
    ensure!(
        source.posture() != BattlePosture::Prone,
        "You cannot attack a mech from a prone position."
    );
    ensure!(
        source.stun_remaining() == 0,
        "You are still recovering from your stunning experience!"
    );
    if matches!(attack, BattlePhysicalAttack::Punch { .. }) {
        ensure!(
            source.carried_club.map(BattleArm::section) != Some(attack.section(source.chassis())),
            "You are carrying a club in this arm and cannot punch with it"
        );
    }
    let club = attack == BattlePhysicalAttack::Club;
    let required_sections = if club {
        vec![BattleSection::LeftArm, BattleSection::RightArm]
    } else if attack.uses_leg() && source.chassis() == BattleMechChassis::Biped {
        vec![BattleSection::LeftLeg, BattleSection::RightLeg]
    } else if matches!(attack, BattlePhysicalAttack::Claw { .. }) {
        vec![]
    } else {
        vec![attack.section(source.chassis())]
    };
    for section in required_sections {
        if club {
            ensure!(
                actuator(source, section, 3, BattleSystem::HandOrFootActuator)?,
                "Both hands must work to club"
            );
        }
        ensure!(
            source.sections()[&section].internal > 0,
            "The required limb is destroyed"
        );
        ensure!(
            actuator(source, section, 0, BattleSystem::ShoulderOrHip)?,
            "The required hip or shoulder is destroyed"
        );
    }
    if let Some(kind) = attack.hand_weapon() {
        ensure!(
            kind.available(source, attack.section(source.chassis()))?,
            "No usable {:?} installed in this arm",
            kind
        );
        ensure!(
            matches!(kind, BattleArmAttack::Saw | BattleArmAttack::Claw)
                || actuator(
                    source,
                    attack.section(source.chassis()),
                    3,
                    BattleSystem::HandOrFootActuator
                )?,
            "The required hand is destroyed or missing"
        );
    }
    let loadout = source.loadout()?;
    ensure!(
        !loadout
            .weapons
            .iter()
            .enumerate()
            .any(|(index, mount)| mount
                .criticals
                .iter()
                .any(|slot| slot.section == attack.section(source.chassis())
                    || (club && slot.section == BattleSection::LeftArm))
                && source.weapon_recycle().contains_key(&index)),
        "You have weapons recycling on your selected limb."
    );
    ensure!(
        !victim.airborne() && victim.free_fall().is_none(),
        "You can't perform physical attacks on airborne mechs!"
    );
    if attack.is_trip() {
        ensure!(
            victim.posture() != BattlePosture::Prone
                && !matches!(victim.stand_timer(), Some(BattleStandTimer::Rising { .. })),
            "Your target is already down!"
        );
    }
    let position = source.position().context("Unit is not on a battlefield")?;
    let target_position = victim
        .position()
        .context("Target is not on a battlefield")?;
    ensure!(
        position.map == target_position.map,
        "Target is on another battlefield"
    );
    let map = &world.btech.maps()[&position.map];
    ensure!(
        map.flags & 512 == 0,
        "You cannot perform physical attacks here!"
    );
    if source.sensor_signature().team == victim.sensor_signature().team {
        ensure!(
            !source.friendly_fire_safety(),
            "You can't attack a teammate with FFSafeties on!"
        );
        ensure!(
            !map.blocks_friendly_fire(),
            "Friendly Fire? I don't think so..."
        );
    }
    let range = unit_range(world, attacker, target)?;
    ensure!(range.spatial < 1.0, "Target out of range!");
    ensure!(
        super::visibility::unit_unblocked(world, attacker, target)?,
        "Target is not in line of sight!"
    );
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    if club {
        ensure!(
            source.carried_club.is_some()
                || matches!(
                    map.hex(i64::from(position.x), i64::from(position.y))?
                        .terrain,
                    Terrain::LightForest | Terrain::HeavyForest
                ),
            "You can not seem to find any trees around to club with."
        );
    }
    let target_tile = map.base_hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let elevation = source.elevation_level(tile);
    let target_elevation = victim.elevation_level(target_tile);
    ensure!(
        (elevation - target_elevation).abs() <= 1,
        "You can't attack, the elevation difference is too large."
    );
    if attack.uses_leg() {
        ensure!(
            elevation >= target_elevation,
            "The target is too high in elevation for you to kick at."
        );
        ensure!(
            elevation == target_elevation || victim.posture() != BattlePosture::Prone,
            "The target is too low in elevation for you to kick."
        );
    } else {
        ensure!(
            (!matches!(attack, BattlePhysicalAttack::Punch { .. })
                || elevation <= target_elevation)
                && (elevation < target_elevation || victim.posture() != BattlePosture::Prone),
            "The target is too low for this arm attack."
        );
    }
    let heading = source.motion().map_or(0.0, |motion| motion.heading);
    let twist = if attack.uses_leg() {
        0.0
    } else {
        source.facing().torso.offset()
    };
    let angle = (range.bearing.unwrap_or(heading) - heading - twist).rem_euclid(360.0);
    let forward = angle <= 60.0 || angle >= 300.0;
    let side = match attack {
        BattlePhysicalAttack::Punch {
            arm: BattleArm::Left,
        }
        | BattlePhysicalAttack::Axe {
            arm: BattleArm::Left,
        }
        | BattlePhysicalAttack::Sword {
            arm: BattleArm::Left,
        }
        | BattlePhysicalAttack::Mace {
            arm: BattleArm::Left,
        }
        | BattlePhysicalAttack::Saw {
            arm: BattleArm::Left,
        }
        | BattlePhysicalAttack::Claw {
            arm: BattleArm::Left,
        } => (240.0..300.0).contains(&angle),
        BattlePhysicalAttack::Punch {
            arm: BattleArm::Right,
        }
        | BattlePhysicalAttack::Axe {
            arm: BattleArm::Right,
        }
        | BattlePhysicalAttack::Sword {
            arm: BattleArm::Right,
        }
        | BattlePhysicalAttack::Mace {
            arm: BattleArm::Right,
        }
        | BattlePhysicalAttack::Saw {
            arm: BattleArm::Right,
        }
        | BattlePhysicalAttack::Claw {
            arm: BattleArm::Right,
        } => angle > 60.0 && angle <= 120.0,
        _ => false,
    };
    ensure!(
        forward || side,
        "Target is outside the attacking limb's arc!"
    );
    let upper = actuator(
        source,
        attack.section(source.chassis()),
        1,
        BattleSystem::UpperActuator,
    )?;
    let lower = actuator(
        source,
        attack.section(source.chassis()),
        2,
        BattleSystem::LowerActuator,
    )?;
    let foot = actuator(
        source,
        attack.section(source.chassis()),
        3,
        BattleSystem::HandOrFootActuator,
    )?;
    let actuators = if club {
        let mut penalty = 0;
        for section in [BattleSection::LeftArm, BattleSection::RightArm] {
            for (slot, system) in [
                (1, BattleSystem::UpperActuator),
                (2, BattleSystem::LowerActuator),
            ] {
                let location = CriticalLocation { section, slot };
                let right = CriticalLocation {
                    section: BattleSection::RightArm,
                    slot,
                };
                let installed = source
                    .loadout()?
                    .systems
                    .iter()
                    .any(|part| part.location == right && part.system == system);
                penalty += 2 * u8::from(source.critical_unavailable(location) || !installed);
            }
        }
        penalty
    } else if attack.is_trip() {
        0
    } else {
        2 * u8::from(!upper)
            + 2 * u8::from(!lower)
            + u8::from(!foot && attack.hand_weapon().is_none())
    };
    let specialist = world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Melee_Specialist"));
    let base = if rules.use_pilot_skill {
        unit_piloting_target(world, attacker, rules.fall.extended_piloting)?
            - match attack {
                BattlePhysicalAttack::Kick { .. }
                | BattlePhysicalAttack::Trip { .. }
                | BattlePhysicalAttack::Sword { .. } => 2,
                BattlePhysicalAttack::Axe { .. }
                | BattlePhysicalAttack::Mace { .. }
                | BattlePhysicalAttack::Saw { .. }
                | BattlePhysicalAttack::Club => 1,
                _ => 0,
            }
    } else if attack.uses_leg() || matches!(attack, BattlePhysicalAttack::Sword { .. }) {
        3
    } else {
        4
    };
    let weapon_modifier = match attack {
        BattlePhysicalAttack::Mace { .. } => 2,
        BattlePhysicalAttack::Saw { .. } | BattlePhysicalAttack::Claw { .. } => 1,
        _ => 0,
    };
    let movement = i16::from(source.attacker_movement_modifier(rules.fasa_turning));
    let attacker_movement = if specialist {
        (movement - 1).min(0)
    } else {
        movement
    };
    let target_movement =
        super::aim::ground_physical_target_modifier(world, victim, rules.extended_movement);
    let terrain = match target_tile.terrain {
        Terrain::Smoke | Terrain::HeavyForest => 2,
        Terrain::LightForest => 1,
        _ => 0,
    };
    let tons = source.definition().tons;
    let mut damage = match attack {
        BattlePhysicalAttack::Sword { .. } => (tons + 5) / 10 + 1,
        BattlePhysicalAttack::Punch { .. } => tons / 10,
        BattlePhysicalAttack::Mace { .. } => tons / 4,
        BattlePhysicalAttack::Saw { .. } => 7,
        BattlePhysicalAttack::Claw { .. } => tons / 7,
        _ => tons / 5,
    };
    if source.triple_myomer_active() && !matches!(attack, BattlePhysicalAttack::Saw { .. }) {
        damage *= 2;
    }
    damage += u16::from(specialist);
    if !lower && attack.hand_weapon().is_none() && !club {
        damage /= 2;
    }
    if !upper && attack.hand_weapon().is_none() && !club {
        damage /= 2;
    }
    if attack.is_trip() {
        damage = 0;
    }
    let direction = unit_range(world, target, attacker)?;
    let hit_arc = BattleHitArc::from_bearing(
        direction.bearing.unwrap_or(180.0),
        victim.motion().map_or(0.0, |m| m.heading),
        rules.hit_arc_mode,
    )?;
    Ok(BattlePhysicalProfile {
        attack,
        target_number: i32::from(base)
            + i32::from(actuators)
            + i32::from(weapon_modifier)
            + i32::from(attacker_movement)
            + i32::from(target_movement)
            + i32::from(terrain),
        base,
        actuators,
        weapon_modifier,
        attacker_movement,
        target_movement,
        terrain,
        damage,
        hit_table: if victim.posture() == BattlePosture::Prone {
            BattleHitTable::Weapon
        } else if elevation > target_elevation
            || (matches!(attack, BattlePhysicalAttack::Punch { .. })
                && elevation == target_elevation)
        {
            BattleHitTable::Punch
        } else if (attack.hand_weapon().is_some() || club) && elevation == target_elevation {
            BattleHitTable::Weapon
        } else {
            BattleHitTable::Kick
        },
        fixed_location: matches!(
            attack,
            BattlePhysicalAttack::Saw { .. } | BattlePhysicalAttack::Claw { .. }
        )
        .then_some(BattleSection::LeftArm),
        hit_arc,
    })
}

/// Derive the affected pilot's protection while preserving explicitly supplied fall policy.
pub(super) fn participant_fall_rules(
    world: &World,
    id: ObjectId,
    mut rules: BattleFallRules,
) -> BattleFallRules {
    rules.toughness |= world.btech.constructed_units()[&id]
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    rules
}

/// Resolve one kick atomically, including its balance check.
pub fn resolve_kick(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: BattleLeg,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalReport> {
    resolve_attack(
        world,
        attacker,
        pilot,
        target,
        BattlePhysicalAttack::Kick { leg },
        rules,
        None,
    )
}

/// Resolve a trip atomically; only a successful attempt forces the target's balance check.
pub fn resolve_trip(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: BattleLeg,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalReport> {
    resolve_attack(
        world,
        attacker,
        pilot,
        target,
        BattlePhysicalAttack::Trip { leg },
        rules,
        None,
    )
}

/// Resolve a validated physical attack without publishing an incomplete impact cascade.
fn resolve_attack(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: BattlePhysicalAttack,
    rules: BattlePhysicalRules,
    completed_arm: Option<BattleSection>,
) -> Result<BattlePhysicalReport> {
    resolve_attack_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm,
            character: false,
        },
    )
}

/// Single physical attack inside a host action that owns injury and casualty publication.
pub(super) fn resolve_attack_in_action(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: BattlePhysicalAttack,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalReport> {
    resolve_attack_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm: None,
            character: true,
        },
    )
}

/// Apply the shared hit, damage and balance sequence with explicit publication capability.
fn resolve_attack_inner(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: BattlePhysicalAttack,
    rules: BattlePhysicalRules,
    context: AttackContext,
) -> Result<BattlePhysicalReport> {
    let character = context.character;
    let profile = attack_profile_inner(world, attacker, pilot, target, attack, rules, context)?;
    let mut candidate = world.clone();
    let unit = Arc::make_mut(&mut candidate.btech.constructed)
        .get_mut(&attacker)
        .unwrap();
    let roll = unit.dice.generic_roll();
    unit.limb_recycle.insert(attack.section(unit.chassis()), 60);
    if attack == BattlePhysicalAttack::Club {
        unit.limb_recycle.insert(BattleSection::LeftArm, 60);
    }
    let threshold =
        profile.target_number - i32::from(rules.glancing == BattleGlancingMode::BelowTarget);
    let hit = i32::from(roll) >= threshold;
    let glancing =
        hit && rules.glancing != BattleGlancingMode::Disabled && i32::from(roll) == threshold;
    let mut pilot_notices = Vec::new();
    let mut notices = vec![BattleNotice {
        unit: attacker,
        text: format!(
            "You try to {} #{}.  BTH:  {},\tRoll:  {}",
            attack.verb(),
            target.0,
            profile.target_number,
            roll
        ),
    }];
    if candidate.btech.constructed_units()[&target].power() == BattlePower::Running {
        notices.push(BattleNotice {
            unit: target,
            text: format!("#{} tries to {} you!", attacker.0, attack.verb()),
        });
    }
    notices.extend(super::broadcast::interaction_notices(
        &candidate,
        attacker,
        target,
        &if hit {
            format!("{}s", attack.verb())
        } else {
            format!("attempts to {}", attack.verb())
        },
    ));
    let mut impact = None;
    let mut experience = None;
    let mut experience_messages = Vec::new();
    if glancing {
        notices.extend(super::broadcast::observer_notices(
            &candidate,
            target,
            "is nicked by a glancing blow!",
        ));
        notices.push(BattleNotice {
            unit: target,
            text: "You are nicked by a glancing blow!".into(),
        });
    }
    if hit && attack == BattlePhysicalAttack::Club {
        let unit = Arc::make_mut(&mut candidate.btech.constructed)
            .get_mut(&attacker)
            .unwrap();
        if unit.carried_club.take().is_some() {
            notices.push(BattleNotice {
                unit: attacker,
                text: "Your club shatters on contact.".into(),
            });
            notices.extend(super::broadcast::observer_notices(
                &candidate,
                attacker,
                "'s club shatters with a loud *CRACK*!",
            ));
        }
    }
    if hit && !attack.is_trip() {
        let victim = &candidate.btech.constructed_units()[&target];
        let mut dice = victim.dice.clone();
        let location = if let Some(section) = profile.fixed_location {
            BattleHit {
                section,
                rear_armor: false,
                through_armor_critical: false,
                crew_stun: false,
            }
        } else if profile.hit_table == BattleHitTable::Weapon {
            let roll = dice.generic_roll();
            rules
                .fall
                .hit
                .resolve(victim, profile.hit_arc, roll, &mut dice)?
        } else {
            BattleHit {
                section: profile.hit_table.location(
                    victim.chassis(),
                    profile.hit_arc,
                    dice.d6(),
                )?,
                rear_armor: profile.hit_arc == BattleHitArc::Rear,
                through_armor_critical: false,
                crew_stun: false,
            }
        };
        Arc::make_mut(&mut candidate.btech.constructed)
            .get_mut(&target)
            .unwrap()
            .dice = dice;
        let damage = if glancing {
            profile.damage.div_ceil(2)
        } else {
            profile.damage
        };
        if character
            && let Some(award) = super::physical_experience::award(
                &mut candidate,
                attacker,
                pilot,
                target,
                damage,
                crate::clock::wall_time(),
                rules.fall.extended_piloting,
            )?
        {
            experience = Some(award.award);
            experience_messages.extend(award.message);
        }
        let fall_rules = participant_fall_rules(&candidate, target, rules.fall);
        let result = super::impact::resolve_attack_in_candidate(
            &mut candidate,
            target,
            location,
            damage,
            fall_rules,
            super::impact::AttackImpact {
                attacker: Some(attacker),
                weapon_effect: None,
                character,
                followup: false,
            },
        )?;
        super::piloting::append_feedback(
            &mut pilot_notices,
            result.pilot_notices.clone(),
            notices.len(),
        );
        notices.extend(result.notices.iter().cloned());
        impact = Some(result);
    }
    let (balance, fall) = if matches!(attack, BattlePhysicalAttack::Kick { .. })
        || (attack.is_trip() && hit)
        || (matches!(attack, BattlePhysicalAttack::Mace { .. }) && !hit)
    {
        let balancing = if hit { target } else { attacker };
        if !hit {
            notices.push(BattleNotice {
                unit: attacker,
                text: "You miss and try to remain standing!".into(),
            });
        }
        let modifier = if matches!(attack, BattlePhysicalAttack::Mace { .. }) {
            2
        } else {
            0
        };
        let mut balance = roll_piloting(
            &mut candidate,
            balancing,
            modifier,
            rules.fall.extended_piloting,
        )?;
        super::piloting::capture_feedback(
            balancing,
            candidate.btech.constructed_units()[&balancing].pilot(),
            &balance,
            &mut notices,
            &mut pilot_notices,
        );
        if character {
            experience_messages.extend(super::piloting::award_control_check(
                &mut candidate,
                balancing,
                &mut balance,
                rules.fall.extended_piloting,
            )?);
        }
        let fall = if !balance.success
            && !candidate.btech.constructed_units()[&balancing].is_destroyed()
            && candidate.btech.constructed_units()[&balancing].posture() != BattlePosture::Prone
        {
            if attack.is_trip() {
                notices.push(BattleNotice {
                    unit: attacker,
                    text: format!("You trip #{}!", target.0),
                });
            }
            if !attack.is_trip()
                || candidate.btech.constructed_units()[&target].power() == BattlePower::Running
            {
                notices.push(BattleNotice {
                    unit: balancing,
                    text: if attack.is_trip() {
                        "You are tripped and fall to the ground!"
                    } else if hit {
                        "The kick knocks you to the ground!"
                    } else {
                        "You lose your balance and fall down!"
                    }
                    .into(),
                });
            }
            notices.extend(super::broadcast::observer_notices(
                &candidate,
                balancing,
                if attack.is_trip() {
                    "trips up and falls down!"
                } else {
                    "stumbles and falls down!"
                },
            ));
            let fall_rules = participant_fall_rules(&candidate, balancing, rules.fall);
            let result = if character
                && candidate.objects[&balancing]
                    .flags
                    .contains(Flag::InCharacter)
            {
                super::fall::resolve_character_fall(&mut candidate, balancing, 1, fall_rules)?
            } else {
                resolve_fall(&mut candidate, balancing, 1, fall_rules)?
            };
            result.append_notices(balancing, &mut notices, &mut pilot_notices);
            Some(result)
        } else {
            None
        };
        if attack.is_trip() && fall.is_none() {
            notices.extend(super::broadcast::observer_notices(
                &candidate,
                target,
                "manages to stay upright!",
            ));
        }
        (Some(balance), fall)
    } else {
        (None, None)
    };
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(BattlePhysicalReport {
        attacker,
        target,
        profile,
        roll,
        hit,
        glancing,
        impact,
        experience,
        experience_messages,
        balance,
        fall,
        pilot_notices,
        notices,
    })
}

impl BattleUnit {
    /// Physical recovery timers keyed by the limb that performed an attack.
    pub fn limb_recycle(&self) -> &std::collections::BTreeMap<BattleSection, u16> {
        &self.limb_recycle
    }
}

/// Arms attempted in a physical command, always left before right when both are selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleArmSelection {
    Left,
    Right,
    Both,
}

/// An arm that could not attack, without discarding another arm's completed punch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleArmRejection {
    pub arm: BattleArm,
    pub reason: String,
}

/// Ordered arm attacks and per-arm failures, committed as one action with staged notices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleArmAttackReport {
    pub attacks: Vec<BattlePhysicalReport>,
    pub rejections: Vec<BattleArmRejection>,
    /// Private feedback with positions in the combined arm sequence.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
}

/// Resolve a punch action using the shared arm-attack sequencer.
pub fn resolve_punch(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arms: BattleArmSelection,
    rules: BattlePhysicalRules,
) -> Result<BattleArmAttackReport> {
    resolve_arm_attack(
        world,
        attacker,
        pilot,
        target,
        arms,
        BattleArmAttack::Punch,
        rules,
    )
}

/// Arm selection and physical weapon used by one sequenced host action.
#[derive(Debug, Clone, Copy)]
pub struct BattleArmAttackChoice {
    pub arms: BattleArmSelection,
    pub kind: BattleArmAttack,
}

/// Attempt the selected arms in order; an impact error rolls back the entire action.
/// Expected per-arm targeting failures do not undo a successful attack with the other arm.
/// Punches may use both arms; a completed axe/sword swing blocks another arm through recovery.
pub fn resolve_arm_attack(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arms: BattleArmSelection,
    kind: BattleArmAttack,
    rules: BattlePhysicalRules,
) -> Result<BattleArmAttackReport> {
    resolve_arm_attack_inner(
        world,
        attacker,
        pilot,
        target,
        BattleArmAttackChoice { arms, kind },
        rules,
        false,
    )
}

/// Sequence selected arms inside a host checkpoint that can publish character casualties.
pub(super) fn resolve_arm_attack_in_action(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    choice: BattleArmAttackChoice,
    rules: BattlePhysicalRules,
) -> Result<BattleArmAttackReport> {
    resolve_arm_attack_inner(world, attacker, pilot, target, choice, rules, true)
}

/// Shared availability, rejection and ordered hit processing for both publication modes.
fn resolve_arm_attack_inner(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    choice: BattleArmAttackChoice,
    rules: BattlePhysicalRules,
    character: bool,
) -> Result<BattleArmAttackReport> {
    let BattleArmAttackChoice { arms, kind } = choice;
    super::power::controlled_unit(world, attacker, pilot)?;
    ensure!(
        world.btech.constructed_units()[&attacker]
            .limb_recycle
            .is_empty(),
        "Your limbs are still recovering from another attack"
    );
    let auto_select = arms == BattleArmSelection::Both;
    let arms: &[BattleArm] = match arms {
        BattleArmSelection::Left => &[BattleArm::Left],
        BattleArmSelection::Right => &[BattleArm::Right],
        BattleArmSelection::Both => &[BattleArm::Left, BattleArm::Right],
    };
    let mut candidate = world.clone();
    let mut report = BattleArmAttackReport {
        attacks: Vec::new(),
        rejections: Vec::new(),
        pilot_notices: Vec::new(),
        notices: Vec::new(),
    };
    let mut completed_arm = None;
    for &arm in arms {
        if auto_select
            && !kind.available(
                &candidate.btech.constructed_units()[&attacker],
                arm.section(),
            )?
        {
            continue;
        }
        let attack = kind.attack(arm);
        if let Err(error) = attack_profile_inner(
            &candidate,
            attacker,
            pilot,
            target,
            attack,
            rules,
            AttackContext {
                completed_arm,
                character,
            },
        ) {
            let reason = format!("{}: {error:#}", arm.section().name().replace('_', " "));
            report.notices.push(BattleNotice {
                unit: attacker,
                text: reason.clone(),
            });
            report.rejections.push(BattleArmRejection { arm, reason });
            continue;
        }
        let result = resolve_attack_inner(
            &mut candidate,
            attacker,
            pilot,
            target,
            attack,
            rules,
            AttackContext {
                completed_arm,
                character,
            },
        )?;
        if matches!(kind, BattleArmAttack::Punch | BattleArmAttack::Claw) {
            completed_arm = Some(arm.section());
        }
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            result.pilot_notices.clone(),
            report.notices.len(),
        );
        report.notices.extend(result.notices.iter().cloned());
        report.attacks.push(result);
    }
    if report.attacks.is_empty() {
        let reasons = report
            .rejections
            .iter()
            .map(|r| r.reason.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        ensure!(
            !reasons.is_empty(),
            "No usable {:?} in the selected arms",
            kind
        );
        anyhow::bail!("No selected arm could attack: {reasons}");
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// Parse one or both arms, defaulting to both.
fn selected_arms(value: Option<&str>) -> Result<BattleArmSelection> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("b" | "both") => Ok(BattleArmSelection::Both),
        Some("l" | "left") => Ok(BattleArmSelection::Left),
        Some("r" | "right") => Ok(BattleArmSelection::Right),
        _ => anyhow::bail!("Choose left, right, or both arms"),
    }
}

/// Shared native/Lua arm-attack selection with acquired-contact checks for an explicit target.
pub(crate) fn configured_arm_attack(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    arms: Option<&str>,
    target: Option<ObjectId>,
    kind: BattleArmAttack,
) -> Result<BattleArmAttackReport> {
    let (chosen, arms) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let arms = selected_arms(arms)?;
        (selected_target(&world, id, target)?, arms)
    };
    let choice = BattleArmAttackChoice { arms, kind };
    super::arm_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        choice,
        configured_rules(config),
    )
}

/// Resolve the selected contact without acquiring one or advancing a lock timer.
fn selected_target(world: &World, id: ObjectId, target: Option<ObjectId>) -> Result<ObjectId> {
    let chosen = target
        .or_else(|| {
            world.btech.constructed_units()[&id]
                .target_lock()
                .map(|lock| lock.target)
        })
        .context("You do not have a target set!")?;
    if target.is_some() {
        ensure!(
            visible_contact(world, id, chosen)?.is_some(),
            "Target is not in line of sight!"
        );
    }
    Ok(chosen)
}

/// Parse one leg; default selection is the right leg.
fn selected_leg(value: Option<&str>) -> Result<BattleLeg> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("r" | "right") => Ok(BattleLeg::Right),
        Some("l" | "left") => Ok(BattleLeg::Left),
        _ => anyhow::bail!("Choose a single left or right leg"),
    }
}

/// Build one kick policy from the saved game settings.
pub(crate) fn configured_rules(config: &crate::Config) -> BattlePhysicalRules {
    let settings = &config.battletech;
    BattlePhysicalRules {
        use_pilot_skill: settings.phys_use_pskill != 0,
        fasa_turning: settings.fasaturn != 0,
        extended_movement: settings.extendedmovemod != 0,
        hit_arc_mode: settings.hit_arcs,
        glancing: BattleGlancingMode::from_setting(settings.glancing_blows),
        fall: BattleFallRules {
            vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
            stacking: BattleStackingRules {
                mode: settings.stacking,
                damage_percent: settings.stackdamage,
                hit_arcs: settings.hit_arcs,
            },
            stagger: BattleStaggerMode::from_setting(settings.newstagger),
            hit: BattleHitRules {
                fasa_criticals: settings.fasacrit != 0,
                inferno_penalty: settings.inferno_penalty != 0,
                exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
            },
            extended_piloting: settings.extended_piloting != 0,
            toughness: false,
        },
    }
}

/// Shared native/Lua target selection; explicit ids must identify a currently acquired contact.
pub(crate) fn configured_kick(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    leg: Option<&str>,
    target: Option<ObjectId>,
) -> Result<BattlePhysicalReport> {
    let (chosen, leg) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let leg = selected_leg(leg)?;
        (selected_target(&world, id, target)?, leg)
    };
    let attack = BattlePhysicalAttack::Kick { leg };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        attack,
        configured_rules(config),
    )
}

/// Shared native/Lua trip selection with acquired-contact checks for an explicit target.
pub(crate) fn configured_trip(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    leg: Option<&str>,
    target: Option<ObjectId>,
) -> Result<BattlePhysicalReport> {
    let (chosen, leg) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let leg = selected_leg(leg)?;
        (selected_target(&world, id, target)?, leg)
    };
    let attack = BattlePhysicalAttack::Trip { leg };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        attack,
        configured_rules(config),
    )
}

/// Physical native command routing, independent of selected arm or leg.
#[derive(Clone, Copy)]
enum PhysicalCommand {
    Kick,
    Punch,
    Trip,
    Axe,
    Sword,
    Mace,
    Saw,
    Claw,
    Club,
    GrabClub,
    Charge,
}

/// Native mace adapter.
pub(crate) fn mace_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Mace)
}

/// Native axe adapter.
pub(crate) fn axe_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Axe)
}

/// Native sword adapter.
pub(crate) fn sword_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Sword)
}

/// Native trip adapter.
pub(crate) fn trip_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Trip)
}

/// Native kick adapter.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Kick)
}

/// Native punch adapter.
pub(crate) fn punch_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Punch)
}

/// A physical command owns both its world checkpoint and staged cockpit messages.
fn command_for(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    kind: PhysicalCommand,
) -> Result<crate::CommandAction> {
    let usage = match kind {
        PhysicalCommand::Kick => "Usage: kick [left|right] [#unit]",
        PhysicalCommand::Trip => "Usage: trip [left|right] [#unit]",
        PhysicalCommand::Punch => "Usage: punch [left|right|both] [#unit]",
        PhysicalCommand::Axe => "Usage: axe [left|right|both] [#unit]",
        PhysicalCommand::Sword if input.name == "chop" => "Usage: chop [left|right|both] [#unit]",
        PhysicalCommand::Sword => "Usage: sword [left|right|both] [#unit]",
        PhysicalCommand::Mace => "Usage: mace [left|right|both] [#unit]",
        PhysicalCommand::Saw => "Usage: saw [left|right|both] [#unit]",
        PhysicalCommand::Claw => "Usage: claw [left|right|both] [#unit]",
        PhysicalCommand::Club => "Usage: club [#unit]",
        PhysicalCommand::GrabClub => "Usage: grabclub [left|right|-]",
        PhysicalCommand::Charge => "Usage: charge [#unit|-]",
    };
    let before = ctx.scripts.world.borrow().clone();
    let checkpoint = ctx.scripts.effects.checkpoint();
    let result = (|| {
        let mut args = input.args.split_whitespace().peekable();
        let leg = if args.peek().is_some_and(|arg| !arg.starts_with('#')) {
            args.next()
        } else {
            None
        };
        let target = args
            .next()
            .map(|arg| -> Result<ObjectId> {
                Ok(ObjectId(
                    arg.strip_prefix('#')
                        .context(usage)?
                        .parse()
                        .context("Invalid target number")?,
                ))
            })
            .transpose()?;
        ensure!(args.next().is_none(), "{usage}");
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notices = match kind {
            PhysicalCommand::Charge => {
                let selection = match (leg, target) {
                    (None, None) => BattleChargeSelection::Default,
                    (None, Some(target)) => BattleChargeSelection::Target(target),
                    (Some("-"), None) => BattleChargeSelection::Cancel,
                    _ => anyhow::bail!("{usage}"),
                };
                select_charge(
                    &mut ctx.scripts.world.borrow_mut(),
                    id,
                    ctx.player,
                    selection,
                )?
            }
            PhysicalCommand::Club => {
                ensure!(leg.is_none(), "{usage}");
                configured_club(ctx.scripts, ctx.config, id, ctx.player, target)?;
                Vec::new()
            }
            PhysicalCommand::GrabClub => {
                ensure!(target.is_none(), "{usage}");
                super::club::grab_club(&mut ctx.scripts.world.borrow_mut(), id, ctx.player, leg)?
            }
            PhysicalCommand::Kick => {
                configured_kick(ctx.scripts, ctx.config, id, ctx.player, leg, target)?;
                Vec::new()
            }
            PhysicalCommand::Trip => {
                configured_trip(ctx.scripts, ctx.config, id, ctx.player, leg, target)?;
                Vec::new()
            }
            PhysicalCommand::Punch
            | PhysicalCommand::Axe
            | PhysicalCommand::Sword
            | PhysicalCommand::Mace
            | PhysicalCommand::Saw
            | PhysicalCommand::Claw => {
                let attack = match kind {
                    PhysicalCommand::Axe => BattleArmAttack::Axe,
                    PhysicalCommand::Sword => BattleArmAttack::Sword,
                    PhysicalCommand::Mace => BattleArmAttack::Mace,
                    PhysicalCommand::Saw => BattleArmAttack::Saw,
                    PhysicalCommand::Claw => BattleArmAttack::Claw,
                    _ => BattleArmAttack::Punch,
                };
                configured_arm_attack(
                    ctx.scripts,
                    ctx.config,
                    id,
                    ctx.player,
                    leg,
                    target,
                    attack,
                )?;
                Vec::new()
            }
        };
        for notice in notices {
            super::notify_unit(ctx.scripts, notice)?;
        }
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            *ctx.scripts.world.borrow_mut() = before;
            ctx.scripts.effects.restore(checkpoint);
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Attempt a mechanical saw swing through the shared physical transaction.
pub(crate) fn saw_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Saw)
}

/// Attempt claw attacks through the shared physical transaction.
pub(crate) fn claw_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Claw)
}

/// Resolve a two-handed club swing with configured targeting and combat policy.
pub(crate) fn configured_club(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<BattlePhysicalReport> {
    let chosen = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        selected_target(&world, id, target)?
    };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        BattlePhysicalAttack::Club,
        configured_rules(config),
    )
}

/// Inspect a two-handed club swing without changing state.
pub fn club_profile(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalProfile> {
    attack_profile(
        world,
        id,
        pilot,
        target,
        BattlePhysicalAttack::Club,
        rules,
        None,
    )
}

/// Commit one two-handed club swing and its damage, recovery and breakage.
pub fn resolve_club(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: BattlePhysicalRules,
) -> Result<BattlePhysicalReport> {
    resolve_attack(
        world,
        id,
        pilot,
        target,
        BattlePhysicalAttack::Club,
        rules,
        None,
    )
}

/// Native two-handed club adapter.
pub(crate) fn club_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Club)
}

/// Native carried-tree adapter.
pub(crate) fn grabclub_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::GrabClub)
}

/// Native charge selection adapter.
pub(crate) fn charge_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Charge)
}
