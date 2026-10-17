//! Conventional weapon aim combines range, movement, equipment, perception and target settling.
use super::{
    CriticalLocation, Mech, MechSection, PrecisionAim, SemiGuidedAim, StealthRange, System, Weapon,
    WeaponRange,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Scenario options affecting the to-hit number; the rules themselves are fixed.
#[derive(Debug, Clone, Copy)]
pub struct AimRules {
    /// Bypass firing arcs and the multiple-targets penalty, matching the game arc override.
    pub override_weapon_arcs: bool,
}

impl AimRules {
    /// The ordinary to-hit rules.
    pub const STANDARD: Self = Self {
        override_weapon_arcs: false,
    };

    /// Shared live-game policy for weapon previews and firing adapters.
    pub(crate) fn configured(_config: &crate::config::BattleTechConfig) -> Self {
        Self::STANDARD
    }
}

/// How the aiming unit currently perceives its target and what that costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PerceptionAim {
    /// How the shooter itself perceives the target; absent for a target only a teammate has
    /// spotted, or a clairvoyant view of one nobody has.
    pub channel: Option<super::DetectionChannel>,
    /// Terrain leaves a line of fire; a target behind a hill needs indirect fire.
    pub direct_fire: bool,
    pub modifier: i16,
}

/// Observer contributions to an LRM indirect shot (Total Warfare p. 111): +1 for firing
/// indirectly, the spotter's own attacker-movement modifier, and +1 more when the spotter
/// also attacked this turn. Semi-guided missiles at a TAG-designated target ignore all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IndirectAim {
    pub spotter: ObjectId,
    /// The spotter's attacker-movement modifier: walked +1, ran +2, jumped +3, prone +2.
    pub movement: u8,
    /// The spotter launched a weapon within the last turn, so it is attacking as well as
    /// spotting (TW p. 111).
    pub spotter_attacked: bool,
    /// Semi-guided missiles at a target a friendly TAG designates ignore the indirect-fire,
    /// spotter-movement and spotter-terrain modifiers (TW p. 142).
    pub tag_guided: bool,
}

impl IndirectAim {
    /// The indirect-fire part of the to-hit number: +1, plus the spotter's movement, plus +1
    /// when the spotter attacked this turn; nothing for TAG-guided semi-guided missiles.
    pub fn modifier(self) -> i32 {
        if self.tag_guided {
            return 0;
        }
        1 + i32::from(self.movement) + i32::from(self.spotter_attacked)
    }
}

/// An inspectable subtotal, not firing permission; posture and advanced equipment remain separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AimModifiers {
    /// Observer sensor aim replaces the firing unit sensor aim when present.
    pub indirect: Option<IndirectAim>,
    pub gunnery: i16,
    /// Weapon range in hexes from the shooter's hex to the target's (Total Warfare p. 109);
    /// elevation does not add to it.
    pub distance: f64,
    /// Active command-network range calculation, independent of physical distance and visibility permission.
    pub network_range: Option<super::NetworkRange>,
    pub range: Option<WeaponRange>,
    pub attacker_movement: u8,
    /// +1 while this unit spots for another unit's indirect fire (TW p. 111): a spotter that
    /// also attacks pays on its own attacks.
    pub spotting: u8,
    /// Target movement, immobility and prone posture together; [`unit_target_movement`]
    /// gives the parts.
    pub target_movement: i8,
    /// Minus two against a target stuck in soft ground.
    pub bogged_down: i8,
    /// An intact orbital cocoon makes the target easier to hit.
    pub orbital_drop: i16,
    /// Darkness on the target's map, less what searchlights and target heat offset.
    pub light: i16,
    /// Wind and gravity on the shooter's map for this weapon's class; `None` when the wind is
    /// too strong for the weapon to fire, which leaves the shot without a target number.
    pub environment: Option<i16>,
    /// +2 for a ranged weapon attack made on a battlefield under electromagnetic interference
    /// (Tactical Operations: Advanced Rules p. 53).
    pub interference: u8,
    pub heat: u8,
    pub sensors: u8,
    /// Accumulated vehicle commander and sensor critical penalties, separate from Mech sensor hits.
    pub control_damage: u8,
    pub mounting_section: u8,
    /// Signed intrinsic equipment accuracy, including pulse laser bonuses.
    pub weapon_accuracy: i8,
    /// Signed accuracy adjustment from selected ammunition.
    pub ammunition_accuracy: i8,
    /// Haywire interference and iNarc homing assistance.
    pub beacon_accuracy: i8,
    /// Accuracy assistance from an installed, functional targeting computer.
    pub targeting_computer: i8,
    /// Anatomical head-target penalty; directed computer fire uses the computer term.
    pub aimed_section: i8,
    pub target_lock: u8,
    /// None means the aiming unit has no acquired contact it can currently perceive.
    pub perception: Option<PerceptionAim>,
}

impl AimModifiers {
    /// Sum supported contributions only; unseen or out-of-range shots have no subtotal.
    pub fn subtotal(&self) -> Option<i32> {
        self.subtotal_with_terrain(self.perception?.modifier)
    }

    /// Shared arithmetic; hex targets supply their intervening-terrain term in place of a
    /// unit's perception and cover term.
    pub(super) fn subtotal_with_terrain(&self, perception: i16) -> Option<i32> {
        Some(
            i32::from(self.gunnery)
                + i32::from(self.range?.modifier)
                + i32::from(self.attacker_movement)
                + i32::from(self.spotting)
                + i32::from(self.target_movement)
                + i32::from(self.bogged_down)
                + i32::from(self.orbital_drop)
                + i32::from(self.light)
                + i32::from(self.environment?)
                + i32::from(self.interference)
                + i32::from(self.heat)
                + i32::from(self.sensors)
                + i32::from(self.control_damage)
                + i32::from(self.mounting_section)
                + i32::from(self.weapon_accuracy)
                + i32::from(self.beacon_accuracy)
                + i32::from(self.ammunition_accuracy)
                + i32::from(self.targeting_computer)
                + i32::from(self.aimed_section)
                + i32::from(self.target_lock)
                + i32::from(perception)
                + self.indirect.map_or(0, IndirectAim::modifier),
        )
    }
}

impl Mech {
    /// Jumping and stabilization precede ground speed and the current walking threshold.
    pub fn attacker_movement_modifier(&self) -> u8 {
        if self.airborne() {
            return 3;
        }
        if self.jump_stabilization() > 0 {
            return 2;
        }
        if (self.posture() == super::Posture::Prone
            || matches!(self.stand_timer(), Some(super::StandTimer::Rising { .. })))
            && let Some(penalty) = self.prone_fire_modifier()
        {
            return penalty;
        }
        // The current walking MP separates walking from running fire (Total Warfare
        // p. 106): leg damage and heat lower it (p. 159) and hot myomer raises it (p. 139), so
        // a heat-slowed Mech moving faster than its reduced walking speed fires as running.
        ground_attacker_movement(self.motion(), self.walking_speed())
    }

    /// Firing when down (Total Warfare p. 113; Tactical Operations p. 83): +2 for a prone
    /// Mech, +3 for a prone biped propped on its only remaining arm, and nothing for a
    /// four-legged Mech that still has all four legs and no hip critical, which fires as
    /// though standing. `None` means the ordinary ground-movement arithmetic applies.
    fn prone_fire_modifier(&self) -> Option<u8> {
        let chassis = self.chassis();
        if chassis == super::MechChassis::Quad
            && self.unavailable_legs() == 0
            && !chassis.legs().iter().any(|leg| self.hip_destroyed(*leg))
        {
            return None;
        }
        let arms_destroyed = [MechSection::LeftArm, MechSection::RightArm]
            .into_iter()
            .filter(|arm| !chassis.is_leg(*arm) && self.sections()[arm].internal == 0)
            .count();
        Some(if arms_destroyed == 1 { 3 } else { 2 })
    }

    /// Whether the hip actuator of `leg` has been destroyed.
    fn hip_destroyed(&self, leg: MechSection) -> bool {
        self.definition().sections[&leg]
            .criticals
            .iter()
            .any(|(slot, critical)| {
                System::named(&critical.equipment) == Some(System::ShoulderOrHip)
                    && self.critical_destroyed(CriticalLocation {
                        section: leg,
                        slot: *slot,
                    })
            })
    }

    /// Destroyed leg actuators in one leg, for the bootlegger manoeuvre's difficulty: a
    /// destroyed hip supersedes the actuators below it, otherwise each destroyed upper, lower
    /// or foot actuator counts one.
    pub fn leg_actuator_penalty(&self, section: MechSection) -> u8 {
        if !self.chassis().is_leg(section) {
            return 0;
        }
        let damaged: Vec<_> = self.definition().sections[&section]
            .criticals
            .iter()
            .filter(|(slot, _)| {
                self.critical_destroyed(CriticalLocation {
                    section,
                    slot: **slot,
                })
            })
            .filter_map(|(_, critical)| System::named(&critical.equipment))
            .collect();
        if damaged.contains(&System::ShoulderOrHip) {
            return 0;
        }
        damaged
            .iter()
            .filter(|system| {
                matches!(
                    system,
                    System::UpperActuator | System::LowerActuator | System::HandOrFootActuator
                )
            })
            .count() as u8
    }

    /// Arm actuator damage penalises weapons fired from that arm (Total Warfare p. 127): a
    /// destroyed shoulder +4, each destroyed upper or lower arm actuator +1. Leg actuators
    /// affect movement and piloting only, never weapon fire.
    pub fn mounting_modifier(&self, section: MechSection) -> u8 {
        let arm = !self.chassis().is_leg(section)
            && matches!(section, MechSection::LeftArm | MechSection::RightArm);
        if !arm {
            return 0;
        }
        let damaged: Vec<_> = self.definition().sections[&section]
            .criticals
            .iter()
            .filter(|(slot, _)| {
                self.critical_destroyed(CriticalLocation {
                    section,
                    slot: **slot,
                })
            })
            .filter_map(|(_, critical)| System::named(&critical.equipment))
            .collect();
        if damaged.contains(&System::ShoulderOrHip) {
            return 4;
        }
        damaged
            .iter()
            .filter(|system| matches!(system, System::UpperActuator | System::LowerActuator))
            .count() as u8
    }
}

/// Ground firing penalties: +1 at or below the current walking (cruising) speed and +2 above it
/// (Total Warfare p. 106).
fn ground_attacker_movement(motion: Option<super::Motion>, walking: f64) -> u8 {
    let Some(motion) = motion else {
        return 0;
    };
    if motion.speed == 0.0 {
        return 0;
    }
    if motion.speed > walking + 0.1 {
        return 2;
    }
    1
}

impl super::Vehicle {
    /// Ground speed and optional hull-turn penalty; turret rotation does not add a movement penalty.
    /// Cruising is judged against the current cruising speed, so motive damage that lowers it
    /// turns a formerly cruising speed into flank fire (Total Warfare pp. 106 and 193).
    pub fn attacker_movement_modifier(&self) -> u8 {
        ground_attacker_movement(self.motion(), self.cruising_speed())
    }
}

/// The parts of the target movement modifier (Total Warfare p. 117), kept apart because
/// precision ammunition and TAG guidance touch only the movement itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetMovement {
    /// Hexes moved this turn, plus one for a jumping Mech or an airborne VTOL.
    pub motion: i8,
    /// -4 against an immobile target.
    pub immobile: i8,
    /// -2 against a prone Mech from an adjacent hex, +1 from any other.
    pub prone: i8,
    /// +2 against a unit that skidded this turn (Total Warfare p. 62).
    pub skidding: i8,
}

impl TargetMovement {
    /// The complete target movement modifier.
    pub fn total(self) -> i8 {
        self.motion + self.immobile + self.prone + self.skidding
    }
}

/// Target movement before ammunition or sensor adjustments, for Mechs and ground vehicles.
/// This read-only subtotal does not grant contact, range or firing permission.
pub fn unit_target_movement_modifier(world: &World, target: ObjectId, distance: f64) -> Result<i8> {
    unit_target_movement(world, target, distance).map(TargetMovement::total)
}

/// The target movement modifier in its parts (TW p. 117, p. 197): the movement band, +1
/// for a jumping Mech or an airborne VTOL (hovering included), -4 for an immobile target,
/// the prone adjustments and +2 for a target that skidded this turn (p. 62).
pub fn unit_target_movement(
    world: &World,
    target: ObjectId,
    distance: f64,
) -> Result<TargetMovement> {
    ensure!(
        distance.is_finite() && distance >= 0.0,
        "Invalid target range"
    );
    ensure!(
        world
            .objects
            .get(&target)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let (_, skidding) = super::recent_fire::skid_modifiers(world, target);
    if let Some(vehicle) = world.btech.vehicles().get(&target) {
        let immobile = super::aimed_target::immobile(world, target)?;
        return Ok(TargetMovement {
            motion: target_motion(vehicle.motion().map_or(0.0, |motion| motion.speed))
                + i8::from(vtol_airborne(vehicle)),
            immobile: if immobile { -4 } else { 0 },
            prone: 0,
            skidding,
        });
    }
    let defender = world
        .btech
        .constructed_units()
        .get(&target)
        .context("Target construction is unavailable")?;
    let immobile = super::aimed_target::mech_immobile(world, defender);
    let speed = if defender.airborne() {
        let position = defender
            .position()
            .context("Airborne target is not placed")?;
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Map not found")?;
        defender.jump_capacity(map.gravity)?.speed
    } else {
        defender.motion().map_or(0.0, |motion| motion.speed)
    };
    Ok(TargetMovement {
        motion: target_motion(speed) + i8::from(defender.airborne()),
        immobile: if immobile { -4 } else { 0 },
        prone: if defender.posture() == super::Posture::Prone {
            if distance <= 1.0 { -2 } else { 1 }
        } else {
            0
        },
        skidding,
    })
}

/// A VTOL in the air, hovering or not (TW p. 197); a landed one rolling on the ground is an
/// ordinary ground target.
pub(super) fn vtol_airborne(vehicle: &super::Vehicle) -> bool {
    vehicle.vtol_flight().is_some_and(|flight| {
        matches!(
            flight.phase,
            super::VtolFlightPhase::Airborne | super::VtolFlightPhase::Falling
        )
    })
}

/// Target Movement Modifier (Total Warfare p. 117) by hexes moved per turn at 10.75 kph per
/// hex: 0–2 +0, 3–4 +1, 5–6 +2, 7–9 +3, 10–17 +4, 18–24 +5, 25 or more +6.
pub(super) fn target_motion(speed: f64) -> i8 {
    let speed = speed.abs();
    if speed <= 21.5 {
        return 0;
    }
    if speed <= 43.0 {
        return 1;
    }
    if speed <= 64.5 {
        return 2;
    }
    if speed <= 96.75 {
        return 3;
    }
    if speed <= 182.75 {
        return 4;
    }
    if speed <= 258.0 {
        return 5;
    }
    6
}

/// Perceive the target afresh; a TAG-guided shot replaces the perception term with zero.
pub(super) fn perception_aim(
    world: &World,
    viewer: ObjectId,
    target: ObjectId,
    tag_guided: bool,
) -> Result<Option<PerceptionAim>> {
    let attacker = super::scanner::scanner_unit(world, viewer).context("Shooter is unavailable")?;
    if attacker.power != super::Power::Running {
        return Ok(None);
    }
    // Visibility privileges do not fabricate an acquisition: the shot is effectively impossible.
    let unacquired = attacker.visibility.clairvoyant.then_some(PerceptionAim {
        channel: None,
        direct_fire: true,
        modifier: 10_000,
    });
    // A target spotted by any friendly unit may be engaged (Tactical Operations p. 191); the
    // shooter still needs its own line of fire and clears its own cover.
    let reader = super::contacts::ContactReader::new(world, viewer)?;
    let Some(facts) = reader.facts(target)? else {
        return Ok(unacquired);
    };
    let (terrain, range) = super::los::unit_terrain_geometry(world, viewer, target)?;
    let (channel, direct_fire) = if attacker.visibility.clairvoyant {
        // A clairvoyant's contact rows show everything; its shots still need the target
        // perceived by the unit itself or spotted by its side.
        let perception = super::perception::perceive_prepared(
            world,
            viewer,
            reader.profile(),
            target,
            Some((terrain, range)),
            None,
        )?;
        if perception.is_none() && !reader.team_spots(target) {
            return Ok(unacquired);
        }
        (
            perception.map(|perception| perception.channel),
            perception.map_or(!terrain.blocked, |perception| perception.line_of_fire),
        )
    } else {
        (facts.detection, facts.line_of_fire)
    };
    let modifier = super::perception::cover_aim(world, reader.profile(), target, terrain, range);
    Ok(Some(PerceptionAim {
        channel,
        direct_fire,
        modifier: if tag_guided { 0 } else { modifier },
    }))
}

/// Shared firing admission: the target must be a perceived contact, and a direct shot needs a
/// line of fire.
pub(super) fn ensure_perceived(aim: &AimModifiers) -> Result<()> {
    let perception = aim
        .perception
        .context("Target is not a current acquired contact")?;
    ensure!(
        aim.indirect.is_some() || perception.direct_fire,
        "That target is behind cover you cannot shoot through; use indirect fire."
    );
    Ok(())
}

/// Multiple targets, the realtime reading of Total Warfare p. 108: the selected lock is the
/// declared primary target and takes no modifier, and a shot at any other unit is a secondary
/// attack at +1. Without a lock nothing has been declared, so every shot is secondary at +1.
/// The book's +2 for a secondary target outside the forward arc is deliberately not used:
/// a single flat +1 keeps the lock meaningful without punishing arc geometry twice.
pub(super) fn lock_modifier(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    override_arcs: bool,
) -> Result<u8> {
    if override_arcs {
        return Ok(0);
    }
    Ok(match source.selection(world) {
        Some(super::TargetSelection::Unit(lock)) if lock.target == target => 0,
        _ => 1,
    })
}

/// Inspect numeric contributions without consuming ammunition/dice or establishing firing permission.
pub fn aim_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    gunnery: i16,
    rules: AimRules,
) -> Result<AimModifiers> {
    aim_modifiers_for_source(world, shooter.into(), target, weapon_index, gunnery, rules)
}

/// Shared aim arithmetic for a unit's own selection.
/// Inspection only: no dice are drawn and no firing permission is implied.
pub(super) fn aim_modifiers_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    weapon_index: usize,
    gunnery: i16,
    rules: AimRules,
) -> Result<AimModifiers> {
    let shooter = source.unit;
    if world.btech.vehicles().contains_key(&shooter) {
        return super::vehicle_aim::modifiers(world, source, target, weapon_index, gunnery, rules);
    }
    for id in [shooter, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
    }
    let attacker = world
        .btech
        .constructed_units()
        .get(&shooter)
        .context("Shooter construction is unavailable")?;
    let loadout = attacker.loadout()?;
    let mount = loadout
        .weapons
        .get(weapon_index)
        .context("Weapon index out of bounds")?;
    let indirect = super::spotter::indirect_aim(world, source, target, weapon_index)?;
    let distance = super::unit_range(world, shooter, target)?.hexes();
    let target_terms = target_modifiers(
        world,
        shooter,
        target,
        mount.weapon,
        attacker.ammunition_mode(weapon_index)?,
        distance,
    )?;
    let mut modifiers = weapon_modifiers(attacker, weapon_index, mount, distance, gunnery)?;
    modifiers.attacker_movement += super::recent_fire::skid_modifiers(world, shooter).0;
    let indirect = indirect.map(|aim| IndirectAim {
        tag_guided: target_terms.tag_guided,
        ..aim
    });
    modifiers.indirect = indirect;
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        mount.weapon,
        &mut modifiers,
    )?;
    super::network_range::apply(
        world,
        shooter,
        super::network_range::NetworkTarget::Unit(target),
        (mount.weapon, attacker.ammunition_mode(weapon_index)?),
        submerged,
        &mut modifiers,
    )?;
    super::aimed_target::apply_aim(
        world,
        shooter,
        target,
        super::aimed_target::AimedAttack {
            weapon: mount.weapon,
            fire_mode: attacker.fire_mode(weapon_index)?,
            ammunition: attacker.ammunition_mode(weapon_index)?,
            indirect: indirect.is_some(),
            volley: attacker.array_volley(weapon_index)?.is_some(),
        },
        &mut modifiers,
    )?;
    modifiers.spotting = u8::from(super::spotter::spotting_for_others(world, shooter));
    modifiers.range = modifiers
        .range
        .map(|range| range.against_stealth(target_terms.concealed));
    modifiers.ammunition_accuracy += target_terms.ammunition_accuracy;
    // A hyper-assault Gauss rifle's flak attack gets no targeting computer help (Total
    // Warfare p. 136).
    if target_terms.flak && mount.weapon.is_hag() {
        modifiers.targeting_computer = 0;
    }
    modifiers.target_movement = target_terms.movement;
    modifiers.bogged_down = target_terms.bogged_down;
    modifiers.orbital_drop = target_terms.orbital_drop;
    modifiers.light = target_terms.light;
    modifiers.environment = super::planetary_conditions::aim_modifier(world, shooter, mount.weapon);
    modifiers.interference = super::planetary_conditions::interference_aim_modifier(world, shooter);
    modifiers.beacon_accuracy += target_terms.beacon_accuracy;
    modifiers.target_lock = if indirect.is_some() {
        0
    } else {
        lock_modifier(world, source, target, rules.override_weapon_arcs)?
    };
    modifiers.perception = perception_aim(
        world,
        indirect.map_or(shooter, |aim| aim.spotter),
        target,
        // The spotter's terrain modifiers apply however it designated the target (Total
        // Warfare p. 111); only a TAG-guided shot drops them.
        indirect.is_some_and(|aim| aim.tag_guided),
    )?;
    Ok(modifiers)
}

/// Weapon, movement and equipment terms shared by unit and terrain targets.
pub(super) fn weapon_modifiers(
    attacker: &Mech,
    weapon_index: usize,
    mount: &super::WeaponMount,
    distance: f64,
    gunnery: i16,
) -> Result<AimModifiers> {
    let ammunition = attacker.ammunition_mode(weapon_index)?;
    let mut aim = weapon_base(
        mount.weapon,
        distance,
        gunnery,
        attacker.fire_mode(weapon_index)?,
        ammunition,
    )?;
    aim.attacker_movement = attacker.attacker_movement_modifier();
    aim.heat = attacker.heat().to_hit_modifier();
    aim.sensors = attacker.sensor_aim_modifier();
    // A weapon split between a torso and an arm is a torso weapon, so the arm's actuators do
    // not touch it (Total Warfare p. 105).
    aim.mounting_section = attacker.mounting_modifier(mount.firing_section());
    aim.beacon_accuracy = i8::from(attacker.has_beacon(super::BeaconKind::Haywire));
    // A surviving MRM Apollo system negates the MRM's +1 to hit (Tactical Operations: Advanced
    // Units & Equipment p. 143).
    if mount.weapon.is_mrm() && attacker.apollo_operational(weapon_index)? {
        aim.weapon_accuracy -= 1;
    }
    let loadout = attacker.loadout()?;
    if mount.computer_assists(
        ammunition,
        loadout
            .systems
            .iter()
            .filter(|part| part.system == System::TargetingComputer)
            .map(|part| !attacker.critical_unavailable(part.location)),
    ) {
        aim.targeting_computer = -1;
    }
    Ok(aim)
}

/// Target-independent weapon arithmetic shared by every supported chassis and target kind.
pub(super) fn weapon_base(
    weapon: super::Weapon,
    distance: f64,
    gunnery: i16,
    fire_mode: super::FireMode,
    ammunition: super::AmmunitionMode,
) -> Result<AimModifiers> {
    Ok(AimModifiers {
        indirect: None,
        gunnery,
        distance,
        network_range: None,
        range: weapon.range_modifier_for_ammunition(distance, fire_mode, ammunition)?,
        attacker_movement: 0,
        spotting: 0,
        target_movement: 0,
        bogged_down: 0,
        orbital_drop: 0,
        light: 0,
        environment: Some(0),
        interference: 0,
        heat: 0,
        sensors: 0,
        control_damage: 0,
        mounting_section: 0,
        weapon_accuracy: weapon.accuracy_modifier(),
        beacon_accuracy: 0,
        ammunition_accuracy: match ammunition {
            super::AmmunitionMode::Cluster => -1,
            super::AmmunitionMode::ArmorPiercing => 1,
            _ => 0,
        },
        targeting_computer: 0,
        aimed_section: 0,
        target_lock: 0,
        perception: None,
    })
}

/// Inspect aim using the present pilot's saved skill instead of a supplied gunnery target.
/// Live adapters pass the configured extended-gunnery choice; firing permission remains separate.
pub fn pilot_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    extended_gunnery: bool,
    rules: AimRules,
) -> Result<AimModifiers> {
    let gunnery = super::unit_gunnery_target(world, shooter, weapon_index, extended_gunnery)?;
    aim_modifiers(world, shooter, target, weapon_index, gunnery, rules)
}

/// Close-range target movement for grounded biped physical attacks.
/// Eligibility rejects airborne targets before this calculation; immobility stacks with posture.
pub(super) fn ground_physical_target_modifier(world: &World, target: &Mech) -> i8 {
    let immobile = super::aimed_target::mech_immobile(world, target);
    let skidding = if target.skidded_this_turn(world.btech.simulation_time()) {
        2
    } else {
        0
    };
    target_motion(target.motion().map_or(0.0, |motion| motion.speed))
        - if immobile { 4 } else { 0 }
        - if target.posture() == super::Posture::Prone {
            2
        } else {
            0
        }
        + skidding
}

/// Target-owned aim contributions, independent of the firing unit's construction.
pub(super) struct TargetAimModifiers {
    /// Target-specific ammunition adjustment added to the weapon base.
    pub ammunition_accuracy: i8,
    /// Minus two against a target stuck in soft ground.
    pub bogged_down: i8,
    /// An intact orbital cocoon makes the target easier to hit.
    pub orbital_drop: i16,
    /// Darkness on the target's map, less what searchlights and target heat offset.
    pub light: i16,
    pub movement: i8,
    pub beacon_accuracy: i8,
    pub concealed: bool,
    /// Semi-guided missiles at a target a friendly TAG designates (TW p. 142).
    pub tag_guided: bool,
    /// The attack is a flak attack (TW p. 114), which a targeting computer cannot assist when
    /// a hyper-assault Gauss rifle makes it (p. 136).
    pub flak: bool,
}

/// Apply target movement, friendly TAG assistance, concealment and homing pods identically for every shooter.
pub(super) fn target_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: Weapon,
    ammunition: super::AmmunitionMode,
    distance: f64,
) -> Result<TargetAimModifiers> {
    let attacker = super::scanner::scanner_unit(world, shooter)
        .context("Shooter construction is unavailable")?;
    let defender = super::scanner::scanner_unit(world, target)
        .context("Target construction is unavailable")?;
    let movement = unit_target_movement(world, target, distance)?;
    let artemis_v_guided = ammunition.munition() == super::AmmunitionMode::Artemis
        && super::artemis::artemis_v(world, shooter)
        && !super::electronic_field(world, shooter)?.blocks_outgoing_guidance()
        && !super::electronic_field(world, target)?.blocks_incoming_guidance();
    // The shooter's own TAG designates as well as any other friendly unit's (TW p. 142).
    let tag_guided = ammunition.munition() == super::AmmunitionMode::SemiGuided
        && super::tagged_by(world, target).is_some_and(|tagger| {
            super::scanner::scanner_unit(world, tagger)
                .is_some_and(|unit| unit.signature.team == attacker.signature.team)
        });
    let airborne = target_airborne(world, target);
    Ok(TargetAimModifiers {
        ammunition_accuracy: airborne_ammunition_adjustment(weapon, ammunition, airborne),
        bogged_down: super::bog_down::target_modifier(world, target),
        orbital_drop: super::orbital_drop_state::current(world, target)
            .map_or(0, super::OrbitalDrop::target_modifier),
        light: super::light_aim::modifier(world, target, false)?,
        movement: ammunition
            .tag_movement_modifier(ammunition.precision_movement(movement), tag_guided)
            .total(),
        // Homing beacons and Artemis V guidance each make the shot one easier.
        beacon_accuracy: -i8::from(
            super::narc::has_beacon(world, target, super::BeaconKind::Homing)
                && ammunition.munition() == super::AmmunitionMode::Narc
                && !weapon.is_narc(),
        ) - i8::from(artemis_v_guided),
        concealed: defender.concealed,
        tag_guided,
        flak: airborne && flak_capable(weapon, ammunition),
    })
}

/// Whether an attack has the flak effect (TW p. 114): LB-X cluster rounds (p. 141) and
/// hyper-assault Gauss rifles (p. 136).
fn flak_capable(weapon: Weapon, ammunition: super::AmmunitionMode) -> bool {
    weapon.is_hag() || ammunition.munition() == super::AmmunitionMode::Cluster
}

/// Flak (TW p. 114): against any target in the air, an airborne VTOL, a jumping Mech or a
/// dropping unit, LB-X cluster rounds take -2 (p. 141) and a hyper-assault Gauss rifle -1 more,
/// -3 in all (p. 136). A parked VTOL is no flak target.
fn airborne_ammunition_adjustment(
    weapon: Weapon,
    ammunition: super::AmmunitionMode,
    airborne: bool,
) -> i8 {
    if !airborne || !flak_capable(weapon, ammunition) {
        return 0;
    }
    if weapon.is_hag() { -3 } else { -2 }
}

/// Whether the target is in the air: an airborne Mech, a flying or falling VTOL, or a unit on
/// an orbital descent. Ground launch preparation is not enough.
pub(super) fn target_airborne(world: &World, target: ObjectId) -> bool {
    world
        .btech
        .constructed_units()
        .get(&target)
        .is_some_and(|unit| unit.airborne())
        || world
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|unit| unit.orbital_drop().is_some() || vtol_airborne(unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flak applies to every airborne target and to nothing else: LB-X cluster rounds -2 and
    /// hyper-assault Gauss rifles -3 (TW pp. 114, 136, 141); other attacks get nothing.
    #[test]
    fn flak_applies_only_to_cluster_rounds_and_hags_against_airborne_targets() {
        use super::super::AmmunitionMode as Mode;
        for (airborne, cluster, hag) in [(false, 0, 0), (true, -2, -3)] {
            assert_eq!(
                airborne_ammunition_adjustment(Weapon::Lbx10, Mode::Cluster, airborne),
                cluster
            );
            assert_eq!(
                airborne_ammunition_adjustment(Weapon::ClanHag30, Mode::Normal, airborne),
                hag
            );
            assert_eq!(
                airborne_ammunition_adjustment(Weapon::Lbx10, Mode::Normal, airborne),
                0
            );
            assert_eq!(
                airborne_ammunition_adjustment(Weapon::Lrm10, Mode::MmlLrmSwarm, airborne),
                0
            );
        }
    }

    /// The indirect term is +1, the spotter's movement and +1 for a spotter that attacked;
    /// TAG guidance removes all of it (TW p. 111, p. 142).
    #[test]
    fn indirect_modifier_adds_spotter_movement_and_attack() {
        let base = IndirectAim {
            spotter: ObjectId(5),
            movement: 0,
            spotter_attacked: false,
            tag_guided: false,
        };
        assert_eq!(base.modifier(), 1);
        assert_eq!(
            IndirectAim {
                movement: 2,
                ..base
            }
            .modifier(),
            3
        );
        assert_eq!(
            IndirectAim {
                movement: 3,
                spotter_attacked: true,
                ..base
            }
            .modifier(),
            5
        );
        assert_eq!(
            IndirectAim {
                movement: 3,
                spotter_attacked: true,
                tag_guided: true,
                ..base
            }
            .modifier(),
            0
        );
    }

    /// The parts of the target movement modifier add up to the whole.
    #[test]
    fn target_movement_parts_sum() {
        let movement = TargetMovement {
            motion: 3,
            immobile: -4,
            prone: 1,
            skidding: 2,
        };
        assert_eq!(movement.total(), 2);
        assert_eq!(TargetMovement::default().total(), 0);
    }

    #[test]
    fn target_motion_bands_preserve_boundaries_reverse_and_extended_truncation() {
        for (speed, expected) in [
            (0.0, 0),
            (21.5, 0),
            (21.501, 1),
            (43.0, 1),
            (43.001, 2),
            (64.5, 2),
            (64.501, 3),
            (96.75, 3),
            (96.751, 4),
            (182.75, 4),
            (182.751, 5),
            (258.0, 5),
            (258.001, 6),
            (1000.0, 6),
        ] {
            assert_eq!(target_motion(speed), expected);
            assert_eq!(target_motion(-speed), expected);
        }
    }
}
