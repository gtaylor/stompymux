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

/// Additional observer contributions to a conventional indirect shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IndirectAim {
    pub spotter: ObjectId,
    pub spotting: i16,
    pub movement: u8,
}

impl IndirectAim {
    /// Coordination costs one, with the observer's skill measured against target four.
    pub fn modifier(self) -> i32 {
        1 + i32::from(self.spotting) - 4 + i32::from(self.movement)
    }
}

/// An inspectable subtotal, not firing permission; posture and advanced equipment remain separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AimModifiers {
    /// Coolant self-application does not require an acquired contact.
    pub self_target: bool,
    /// Observer sensor aim replaces the firing unit sensor aim when present.
    pub indirect: Option<IndirectAim>,
    pub gunnery: i16,
    pub distance: f64,
    /// Active command-network range calculation, independent of physical distance and visibility permission.
    pub network_range: Option<super::NetworkRange>,
    pub range: Option<WeaponRange>,
    pub attacker_movement: u8,
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
        let perception = if self.self_target {
            0
        } else {
            self.perception?.modifier
        };
        self.subtotal_with_terrain(perception)
    }

    /// Shared arithmetic; terrain targets supply zero instead of a unit sensor contribution.
    pub(super) fn subtotal_with_terrain(&self, perception: i16) -> Option<i32> {
        Some(
            i32::from(self.gunnery)
                + i32::from(self.range?.modifier)
                + i32::from(self.attacker_movement)
                + i32::from(self.target_movement)
                + i32::from(self.bogged_down)
                + i32::from(self.orbital_drop)
                + i32::from(self.light)
                + i32::from(self.environment?)
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
    /// Jumping and stabilization precede ground speed and the template's walking threshold.
    pub fn attacker_movement_modifier(&self) -> u8 {
        if self.airborne() {
            return 3;
        }
        if self.jump_stabilization() > 0 {
            return 2;
        }
        if self.posture() == super::Posture::Prone
            || matches!(self.stand_timer(), Some(super::StandTimer::Rising { .. }))
        {
            return 2;
        }
        let maximum = self.template_speed()
            + if self.triple_myomer_active() {
                1.5 * 10.75
            } else {
                0.0
            };
        ground_attacker_movement(self.motion(), maximum)
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

/// Ground firing penalties use the construction's walking threshold, including after motive damage.
fn ground_attacker_movement(motion: Option<super::Motion>, maximum: f64) -> u8 {
    let Some(motion) = motion else {
        return 0;
    };
    if motion.speed == 0.0 {
        return 0;
    }
    if motion.speed > maximum * 2.0 / 3.0 + 0.1 {
        return 2;
    }
    1
}

impl super::Vehicle {
    /// Ground speed and optional hull-turn penalty; turret rotation does not add a movement penalty.
    pub fn attacker_movement_modifier(&self) -> u8 {
        ground_attacker_movement(self.motion(), self.template_speed())
    }
}

/// Target movement before ammunition or sensor adjustments, for Mechs and ground vehicles.
/// This read-only subtotal does not grant contact, range or firing permission.
pub fn unit_target_movement_modifier(world: &World, target: ObjectId, distance: f64) -> Result<i8> {
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
    if let Some(vehicle) = world.btech.vehicles().get(&target) {
        let immobile = super::aimed_target::immobile(world, target)?;
        return Ok(
            target_motion(vehicle.motion().map_or(0.0, |motion| motion.speed))
                - if immobile { 4 } else { 0 },
        );
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
    Ok(
        target_motion(speed) + i8::from(defender.airborne()) - if immobile { 4 } else { 0 }
            + if defender.posture() == super::Posture::Prone {
                if distance <= 1.0 { -2 } else { 1 }
            } else {
                0
            },
    )
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

/// Perceive the target afresh; coordinate spotting replaces the perception term with zero.
pub(super) fn perception_aim(
    world: &World,
    viewer: ObjectId,
    target: ObjectId,
    coordinate_spotting: bool,
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
        modifier: if coordinate_spotting { 0 } else { modifier },
    }))
}

/// Shared firing admission: the target must be a perceived contact, and a direct shot needs a
/// line of fire. Self-applied coolant needs neither.
pub(super) fn ensure_perceived(aim: &AimModifiers) -> Result<()> {
    if aim.self_target {
        return Ok(());
    }
    let perception = aim
        .perception
        .context("Target is not a current acquired contact")?;
    ensure!(
        aim.indirect.is_some() || perception.direct_fire,
        "That target is behind cover you cannot shoot through; use indirect fire."
    );
    Ok(())
}

/// Multiple targets (Total Warfare p. 108): the selected lock is the declared primary target.
/// A shot at any other unit is a secondary attack and takes +1 when that target lies in the
/// attacker's forward arc, after any torso twist or turret rotation, or +2 in a side or rear
/// arc. With no selection the shot's own target is the primary target.
pub(super) fn lock_modifier(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    override_arcs: bool,
) -> Result<u8> {
    if override_arcs {
        return Ok(0);
    }
    match source.selection(world) {
        None => Ok(0),
        Some(super::TargetSelection::Unit(lock)) if lock.target == target => Ok(0),
        Some(_) => Ok(if target_in_forward_arc(world, source.unit, target)? {
            1
        } else {
            2
        }),
    }
}

/// Whether a unit lies in the attacker's forward arc as rotated by its torso twist or, for a
/// turreted vehicle, its turret.
fn target_in_forward_arc(world: &World, shooter: ObjectId, target: ObjectId) -> Result<bool> {
    let attacker =
        super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let heading = attacker
        .heading
        .context("Shooter has no battlefield motion")?;
    let bearing = super::unit_range(world, shooter, target)?
        .bearing
        .unwrap_or(180.0);
    let rotation = match world.btech.vehicles().get(&shooter) {
        Some(vehicle) => vehicle
            .turret_heading()
            .map_or(0.0, |turret| turret - heading),
        None => attacker.facing.torso.offset(),
    };
    anyhow::ensure!(
        bearing.is_finite() && heading.is_finite() && rotation.is_finite(),
        "Invalid target direction"
    );
    let relative =
        (bearing.rem_euclid(360.0) - heading.rem_euclid(360.0) - rotation).rem_euclid(360.0);
    Ok(relative <= 60.0 || relative >= 300.0)
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
    let distance = super::unit_range(world, shooter, target)?.spatial;
    let target_terms = target_modifiers(
        world,
        shooter,
        target,
        mount.weapon,
        attacker.ammunition_mode(weapon_index)?,
        distance,
    )?;
    let mut modifiers = weapon_modifiers(attacker, weapon_index, mount, distance, gunnery)?;
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
        mount.weapon,
        submerged,
        &mut modifiers,
    )?;
    super::aimed_target::apply_aim(world, shooter, target, mount.weapon, &mut modifiers)?;
    modifiers.self_target = shooter == target && mount.weapon == Weapon::CoolantGun;
    modifiers.indirect = indirect;
    modifiers.range = modifiers
        .range
        .map(|range| range.against_stealth(target_terms.concealed));
    modifiers.ammunition_accuracy += target_terms.ammunition_accuracy;
    modifiers.target_movement = target_terms.movement;
    modifiers.bogged_down = target_terms.bogged_down;
    modifiers.orbital_drop = target_terms.orbital_drop;
    modifiers.light = target_terms.light;
    modifiers.environment = super::planetary_conditions::aim_modifier(world, shooter, mount.weapon);
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
        indirect.is_some_and(|aim| super::spotter::coordinate_target(world, aim.spotter)),
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
    aim.sensors = match attacker.system_hits(System::Sensors) {
        0 => 0,
        1 => 2,
        _ => 75,
    };
    aim.mounting_section = attacker.mounting_modifier(mount.criticals[0].section);
    aim.beacon_accuracy = i8::from(attacker.has_beacon(super::BeaconKind::Haywire));
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
        self_target: false,
        indirect: None,
        gunnery,
        distance,
        network_range: None,
        range: weapon.range_modifier_for_ammunition(distance, fire_mode, ammunition)?,
        attacker_movement: 0,
        target_movement: 0,
        bogged_down: 0,
        orbital_drop: 0,
        light: 0,
        environment: Some(0),
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
    target_motion(target.motion().map_or(0.0, |motion| motion.speed))
        - if immobile { 4 } else { 0 }
        - if target.posture() == super::Posture::Prone {
            2
        } else {
            0
        }
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
    let movement = ammunition
        .target_movement_modifier(unit_target_movement_modifier(world, target, distance)?);
    let artemis_v_guided = ammunition.munition() == super::AmmunitionMode::Artemis
        && super::artemis::artemis_v(world, shooter)
        && !super::electronic_field(world, shooter)?.blocks_outgoing_guidance()
        && !super::electronic_field(world, target)?.blocks_incoming_guidance();
    let friendly_tag = ammunition.munition() == super::AmmunitionMode::SemiGuided
        && super::tagged_by(world, target).is_some_and(|tagger| {
            tagger != shooter
                && super::scanner::scanner_unit(world, tagger)
                    .is_some_and(|unit| unit.signature.team == attacker.signature.team)
        });
    let aircraft = world
        .btech
        .vehicles()
        .get(&target)
        .and_then(|unit| unit.vtol_flight());
    let flying = aircraft.is_some_and(|flight| {
        matches!(
            flight.phase,
            super::VtolFlightPhase::Airborne | super::VtolFlightPhase::Falling
        )
    });
    let moving_aircraft =
        aircraft.is_some_and(|flight| defender.speed != 0.0 || flight.vertical_speed != 0.0);
    let cocoon = super::orbital_drop_state::current(world, target).is_some();
    Ok(TargetAimModifiers {
        ammunition_accuracy: airborne_ammunition_adjustment(
            ammunition,
            aircraft.is_some(),
            flying,
            cocoon,
        ),
        bogged_down: super::bog_down::target_modifier(world, target),
        orbital_drop: super::orbital_drop_state::current(world, target)
            .map_or(0, super::OrbitalDrop::target_modifier),
        light: super::light_aim::modifier(world, target, false)?,
        movement: ammunition.tag_movement_modifier(movement, friendly_tag)
            + i8::from(moving_aircraft),
        // Homing beacons and Artemis V guidance each make the shot one easier.
        beacon_accuracy: -i8::from(
            super::narc::has_beacon(world, target, super::BeaconKind::Homing)
                && ammunition.munition() == super::AmmunitionMode::Narc
                && !weapon.is_narc(),
        ) - i8::from(artemis_v_guided),
        concealed: defender.concealed,
    })
}

/// LBX gains two more points against rotorcraft; Stinger distinguishes flight from orbital orbital descents.
fn airborne_ammunition_adjustment(
    ammunition: super::AmmunitionMode,
    rotorcraft: bool,
    flying: bool,
    cocoon: bool,
) -> i8 {
    match ammunition.munition() {
        super::AmmunitionMode::Cluster if rotorcraft => -2,
        super::AmmunitionMode::Stinger if flying => -3,
        super::AmmunitionMode::Stinger if cocoon => -1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Cluster keeps its base -1 and Stinger has no extra bonus against a jumping Mech.
    #[test]
    fn airborne_ammunition_distinguishes_rotorcraft_orbital_descents_and_jumps() {
        use super::super::AmmunitionMode as Mode;
        for (rotorcraft, flying, cocoon, cluster, stinger) in [
            (false, false, false, 0, 0),
            (true, false, false, -2, 0),
            (true, true, false, -2, -3),
            (false, false, true, 0, -1),
            (true, true, true, -2, -3),
        ] {
            assert_eq!(
                airborne_ammunition_adjustment(Mode::Cluster, rotorcraft, flying, cocoon),
                cluster
            );
            assert_eq!(
                airborne_ammunition_adjustment(Mode::Stinger, rotorcraft, flying, cocoon),
                stinger
            );
            assert_eq!(
                airborne_ammunition_adjustment(Mode::Normal, rotorcraft, flying, cocoon),
                0
            );
        }
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
