//! Conventional weapon aim combines range, movement, equipment, perception and target settling.
use super::{BattleSection, BattleSystem, BattleUnit, BattleWeapon, CriticalLocation};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Conventional range bracket after the game's fractional-distance rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleRangeBracket {
    Minimum,
    Short,
    Medium,
    Long,
    Extreme,
}

/// In-range weapon contribution; absence means beyond the effective physical range limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleWeaponRange {
    pub bracket: BattleRangeBracket,
    pub modifier: u8,
}

impl BattleWeapon {
    /// Preserve raw minimum/maximum checks and the rounded brackets used between them.
    pub fn range_modifier(
        self,
        distance: f64,
        extended: bool,
    ) -> Result<Option<BattleWeaponRange>> {
        self.range_modifier_for_mode(distance, extended, super::BattleFireMode::Normal, false)
    }

    /// Hotloading removes minimum-range penalties without changing the ordinary range brackets.
    pub fn range_modifier_for_mode(
        self,
        distance: f64,
        extended: bool,
        mode: super::BattleFireMode,
        half_minimum: bool,
    ) -> Result<Option<BattleWeaponRange>> {
        self.range_modifier_for_ammunition(
            distance,
            extended,
            mode,
            half_minimum,
            super::BattleAmmunitionMode::Normal,
        )
    }

    /// Ammunition-specific reach retains the ordinary minimum and intermediate brackets.
    pub fn range_modifier_for_ammunition(
        self,
        distance: f64,
        extended: bool,
        mode: super::BattleFireMode,
        half_minimum: bool,
        ammunition: super::BattleAmmunitionMode,
    ) -> Result<Option<BattleWeaponRange>> {
        ensure!(
            ammunition.supports(self),
            "Unsupported ammunition for weapon"
        );
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid weapon range"
        );
        ensure!(
            !self.is_artillery(),
            "Artillery requires artillery aim rules"
        );
        let mut profile = self.profile_for_ammunition(ammunition);
        let minimum = profile.minimum_range;
        if mode == super::BattleFireMode::Hotload {
            ensure!(self.supports_hotload(), "Weapon cannot be hotloaded");
            profile.minimum_range = 0;
        }
        let maximum = self.effective_range_for_ammunition(extended, ammunition)
            + if ammunition.munition() == super::BattleAmmunitionMode::Stinger {
                7
            } else {
                0
            };
        if distance > f64::from(maximum) {
            return Ok(None);
        }
        if mode == super::BattleFireMode::Hotload
            && half_minimum
            && minimum > 0
            && distance <= f64::from(minimum)
        {
            return Ok(Some(BattleWeaponRange {
                bracket: BattleRangeBracket::Short,
                modifier: ((f64::from(minimum) - distance + 2.0) / 2.0).floor() as u8,
            }));
        }
        if profile.minimum_range > 0 && distance <= f64::from(profile.minimum_range) {
            return Ok(Some(BattleWeaponRange {
                bracket: BattleRangeBracket::Minimum,
                modifier: (f64::from(profile.minimum_range) - distance + 1.0).floor() as u8,
            }));
        }
        let rounded = (distance + 0.95).floor() as u8;
        let (bracket, modifier) = if rounded > profile.long_range {
            (BattleRangeBracket::Extreme, 8)
        } else if rounded > profile.medium_range {
            (BattleRangeBracket::Long, 4)
        } else if rounded > profile.short_range {
            (BattleRangeBracket::Medium, 2)
        } else if profile.minimum_range > 0 && rounded <= profile.minimum_range {
            (
                BattleRangeBracket::Minimum,
                profile.minimum_range - rounded + 1,
            )
        } else {
            (BattleRangeBracket::Short, 0)
        };
        Ok(Some(BattleWeaponRange { bracket, modifier }))
    }
}

/// Game configuration affecting the supported movement and range contributions.
#[derive(Debug, Clone, Copy)]
pub struct BattleAimRules {
    /// Select the woods-damage variant's occupied-forest accuracy calculation.
    pub woods_damage: bool,
    /// Target cover modifier, bounded to the signed modifier range by configuration.
    pub dig_bonus: i16,
    /// Restrict digging cover to attacks from the defender’s front arc.
    pub dig_only_front: bool,
    /// Shared configured arc geometry for determining the defender’s facing.
    pub hit_arc_mode: i64,
    pub fasa_turning: bool,
    pub extended_movement: bool,
    pub extended_ranges: bool,
    pub hotload_half_minimum: bool,
    /// Bypass the ordinary unstable-lock penalty, matching the game arc override.
    pub override_weapon_arcs: bool,
}

impl BattleAimRules {
    /// Shared live-game policy for weapon previews and firing adapters.
    pub(crate) fn configured(config: &crate::config::BattleTechConfig) -> Self {
        Self {
            woods_damage: config.moddamagewithwoods != 0,
            dig_bonus: config
                .digbonus
                .clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16,
            dig_only_front: config.dig_only_fs != 0,
            hit_arc_mode: config.hit_arcs,
            fasa_turning: config.fasaturn != 0,
            extended_movement: config.extendedmovemod != 0,
            extended_ranges: config.erange != 0,
            hotload_half_minimum: config.hotloadaddshalfbthmod != 0,
            override_weapon_arcs: false,
        }
    }
}

/// How the aiming unit currently perceives its target and what that costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattlePerceptionAim {
    /// Absent only for a clairvoyant view of a target nobody has acquired.
    pub channel: Option<super::BattleDetectionChannel>,
    /// Terrain leaves a line of fire; probe contacts behind hills need indirect fire.
    pub direct_fire: bool,
    pub modifier: i16,
}

/// Additional observer contributions to a conventional indirect shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleIndirectAim {
    pub spotter: ObjectId,
    pub spotting: i16,
    pub movement: u8,
    pub target_lock: u8,
}

impl BattleIndirectAim {
    /// Coordination costs one, with the observer's skill measured against target four.
    pub fn modifier(self) -> i32 {
        1 + i32::from(self.spotting) - 4 + i32::from(self.movement) + i32::from(self.target_lock)
    }
}

/// An inspectable subtotal, not firing permission; posture and advanced equipment remain separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleAimModifiers {
    /// Coolant self-application does not require an acquired contact.
    pub self_target: bool,
    /// Observer sensor aim replaces the firing unit sensor aim when present.
    pub indirect: Option<BattleIndirectAim>,
    pub gunnery: i16,
    pub distance: f64,
    /// Active command-network range calculation, independent of physical distance and visibility permission.
    pub network_range: Option<super::BattleNetworkRange>,
    pub range: Option<BattleWeaponRange>,
    pub attacker_movement: u8,
    /// Firing at a unit while below the surface in water.
    pub attacker_water: u8,
    pub target_movement: i8,
    /// Cover from a dug-in target, shared across shooter classes.
    pub dug_in: i16,
    /// Accuracy credit for a target within occupied woods under the damage-cover option.
    pub woods_cover: i8,
    /// An intact orbital cocoon makes the target easier to hit.
    pub orbital_drop: i16,
    pub heat: u8,
    pub sensors: u8,
    /// Accumulated vehicle commander and sensor critical penalties, separate from Mech sensor hits.
    pub control_damage: u8,
    pub mounting_section: u8,
    /// Signed intrinsic equipment accuracy, including pulse laser bonuses.
    pub weapon_accuracy: i8,
    /// Accuracy lost to critical weapon degradation.
    pub weapon_damage: u8,
    /// Signed accuracy adjustment from selected ammunition.
    pub ammunition_accuracy: i8,
    /// Haywire interference and iNarc homing assistance.
    pub beacon_accuracy: i8,
    /// Accuracy assistance from an installed, functional targeting computer.
    pub targeting_computer: i8,
    /// Scenario-selected tracking mode, independent of installed targeting-computer equipment.
    pub targeting_mode: i8,
    /// Anatomical head-target penalty; directed computer fire uses the computer term.
    pub aimed_section: i8,
    pub target_lock: u8,
    /// None means the aiming unit has no acquired contact it can currently perceive.
    pub perception: Option<BattlePerceptionAim>,
}

impl BattleAimModifiers {
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
                + i32::from(self.attacker_water)
                + i32::from(self.target_movement)
                + i32::from(self.dug_in)
                + i32::from(self.woods_cover)
                + i32::from(self.orbital_drop)
                + i32::from(self.heat)
                + i32::from(self.sensors)
                + i32::from(self.control_damage)
                + i32::from(self.mounting_section)
                + i32::from(self.weapon_accuracy)
                + i32::from(self.weapon_damage)
                + i32::from(self.beacon_accuracy)
                + i32::from(self.ammunition_accuracy)
                + i32::from(self.targeting_computer)
                + i32::from(self.targeting_mode)
                + i32::from(self.aimed_section)
                + i32::from(self.target_lock)
                + i32::from(perception)
                + self.indirect.map_or(0, BattleIndirectAim::modifier),
        )
    }
}

impl BattleUnit {
    /// Jumping and stabilization precede ground speed and the template's walking threshold.
    pub fn attacker_movement_modifier(&self, fasa_turning: bool) -> u8 {
        if self.airborne() {
            return 3;
        }
        if self.jump_stabilization() > 0 {
            return 2;
        }
        if self.posture() == super::BattlePosture::Prone
            || matches!(
                self.stand_timer(),
                Some(super::BattleStandTimer::Rising { .. })
            )
        {
            return 2;
        }
        let maximum = self.template_speed()
            + if self.triple_myomer_active() {
                1.5 * 10.75
            } else {
                0.0
            };
        ground_attacker_movement(self.motion(), maximum, fasa_turning)
    }

    /// Derive section accuracy penalties without accumulating hit-order-dependent modifiers.
    pub fn mounting_modifier(&self, section: BattleSection) -> u8 {
        let leg = self.chassis().is_leg(section);
        let arm = !leg && matches!(section, BattleSection::LeftArm | BattleSection::RightArm);
        if !arm && !leg {
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
            .filter_map(|(_, critical)| BattleSystem::parse(&critical.equipment).ok())
            .collect();
        if damaged.contains(&BattleSystem::ShoulderOrHip) {
            return if arm { 4 } else { 0 };
        }
        damaged
            .iter()
            .filter(|system| {
                matches!(
                    system,
                    BattleSystem::UpperActuator | BattleSystem::LowerActuator
                ) || (leg && **system == BattleSystem::HandOrFootActuator)
            })
            .count() as u8
    }
}

/// Ground firing penalties use the construction's walking threshold, including after motive damage.
fn ground_attacker_movement(
    motion: Option<super::BattleMotion>,
    maximum: f64,
    fasa_turning: bool,
) -> u8 {
    let Some(motion) = motion else {
        return 0;
    };
    let turning = u8::from(fasa_turning && motion.heading != motion.desired_heading);
    if motion.speed == 0.0 {
        return turning;
    }
    if motion.speed > maximum * 2.0 / 3.0 + 0.1 {
        return 2;
    }
    turning + 1
}

impl super::BattleVehicle {
    /// Ground speed and optional hull-turn penalty; turret rotation does not add a movement penalty.
    pub fn attacker_movement_modifier(&self, fasa_turning: bool) -> u8 {
        ground_attacker_movement(self.motion(), self.template_speed(), fasa_turning)
    }
}

/// Target movement before ammunition or sensor adjustments, for Mechs and ground vehicles.
/// This read-only subtotal does not grant contact, range or firing permission.
pub fn unit_target_movement_modifier(
    world: &World,
    target: ObjectId,
    distance: f64,
    extended: bool,
) -> Result<i8> {
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
        return Ok(target_motion(
            vehicle.motion().map_or(0.0, |motion| motion.speed),
            extended,
        ) - if immobile { 4 } else { 0 });
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
        target_motion(speed, extended) + i8::from(defender.airborne())
            - if immobile { 4 } else { 0 }
            + if defender.posture() == super::BattlePosture::Prone {
                if distance <= 1.0 { -2 } else { 1 }
            } else {
                0
            },
    )
}

/// Supported target motion bands, retaining the reference's truncation for extended fast movement.
pub(super) fn target_motion(speed: f64, extended: bool) -> i8 {
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
    if !extended {
        return 4;
    }
    4 + ((speed - 107.5) / 43.0).trunc() as i8
}

/// Perceive the target afresh; coordinate spotting replaces the perception term with zero.
pub(super) fn perception_aim(
    world: &World,
    viewer: ObjectId,
    target: ObjectId,
    coordinate_spotting: bool,
) -> Result<Option<BattlePerceptionAim>> {
    let attacker = super::scanner::scanner_unit(world, viewer).context("Shooter is unavailable")?;
    if attacker.power != super::BattlePower::Running {
        return Ok(None);
    }
    // Visibility privileges do not fabricate an acquisition: the shot is effectively impossible.
    let unacquired = attacker
        .visibility
        .clairvoyant
        .then_some(BattlePerceptionAim {
            channel: None,
            direct_fire: true,
            modifier: 10_000,
        });
    if !attacker.contacts.contains_key(&target) {
        return Ok(unacquired);
    }
    let Some(perception) = super::perceive(world, viewer, target)? else {
        return Ok(unacquired);
    };
    Ok(Some(BattlePerceptionAim {
        channel: Some(perception.channel),
        direct_fire: perception.identified,
        modifier: if coordinate_spotting {
            0
        } else {
            perception.aim_modifier
        },
    }))
}

/// Shared firing admission: the target must be a perceived contact, and a direct shot needs a
/// line of fire. Self-applied coolant needs neither.
pub(super) fn ensure_perceived(aim: &BattleAimModifiers) -> Result<()> {
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

/// Conventional single-target lock penalty; arc follows the selected target, even for another shot.
pub(super) fn lock_modifier(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    override_arcs: bool,
) -> Result<u8> {
    if override_arcs {
        return Ok(0);
    }
    let shooter = source.unit;
    let selection = source.selection(world);
    if matches!(selection, Some(super::BattleTargetSelection::Unit(lock)) if lock.target == target && (lock.remaining == 0 || super::targeting_mode::mode(world, shooter) == 3))
    {
        return Ok(0);
    }
    Ok(match selected_front(world, source)? {
        Some(true) => 1,
        Some(false) | None => 2,
    })
}

/// Shared selected-target arc used by lock settling and multiple-target tracking.
pub(super) fn selected_front(
    world: &World,
    source: super::fire_target::TargetSource,
) -> Result<Option<bool>> {
    let shooter = source.unit;
    let attacker =
        super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let heading = attacker
        .heading
        .context("Shooter has no battlefield motion")?;
    let bearing = match source.selection(world) {
        Some(super::BattleTargetSelection::Unit(lock)) => {
            super::unit_range(world, shooter, lock.target)?.bearing
        }
        Some(super::BattleTargetSelection::Hex(lock)) => attacker
            .point
            .context("Shooter has no battlefield motion")?
            .bearing(lock.hex.center())?,
        None => return Ok(None),
    }
    .unwrap_or(180.0);
    Ok(Some(
        super::BattleSensorArc::from_bearing(bearing, heading, attacker.facing)?
            == super::BattleSensorArc::Front,
    ))
}

/// Inspect numeric contributions without consuming ammunition/dice or establishing firing permission.
pub fn aim_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon_index: usize,
    gunnery: i16,
    rules: BattleAimRules,
) -> Result<BattleAimModifiers> {
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
    rules: BattleAimRules,
) -> Result<BattleAimModifiers> {
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
    let indirect =
        super::spotter::indirect_aim(world, source, target, weapon_index, rules.fasa_turning)?;
    let distance = super::unit_range(world, shooter, target)?.spatial;
    let target_terms = target_modifiers(
        world,
        shooter,
        target,
        mount.weapon,
        attacker.ammunition_mode(weapon_index)?,
        distance,
        rules,
    )?;
    let mut modifiers = weapon_modifiers(attacker, weapon_index, mount, distance, gunnery, rules)?;
    let submerged = super::weapon_geometry::apply_water_range(
        world,
        shooter,
        weapon_index,
        mount.weapon,
        rules.extended_ranges,
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
    modifiers.weapon_damage = attacker
        .weapon_damage_effects(weapon_index)?
        .accuracy(modifiers.range.map(|range| range.bracket));
    super::aimed_target::apply_aim(world, shooter, target, mount.weapon, &mut modifiers)?;
    modifiers.attacker_water = water_modifier(world, shooter)?;
    modifiers.self_target = shooter == target && mount.weapon == BattleWeapon::CoolantGun;
    modifiers.indirect = indirect;
    modifiers.range = modifiers
        .range
        .map(|range| range.against_stealth(target_terms.concealed));
    modifiers.ammunition_accuracy += target_terms.ammunition_accuracy;
    modifiers.target_movement = target_terms.movement;
    modifiers.dug_in = target_terms.dug_in;
    modifiers.woods_cover = target_terms.woods_cover;
    modifiers.orbital_drop = target_terms.orbital_drop;
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
    super::targeting_mode::apply(
        world,
        source,
        Some(target),
        mount.weapon,
        attacker.ammunition_mode(weapon_index)?,
        &mut modifiers,
    )?;
    Ok(modifiers)
}

/// Weapon, movement and equipment terms shared by unit and terrain targets.
pub(super) fn weapon_modifiers(
    attacker: &BattleUnit,
    weapon_index: usize,
    mount: &super::WeaponMount,
    distance: f64,
    gunnery: i16,
    rules: BattleAimRules,
) -> Result<BattleAimModifiers> {
    let ammunition = attacker.ammunition_mode(weapon_index)?;
    let mut aim = weapon_base(
        mount.weapon,
        distance,
        gunnery,
        rules,
        attacker.fire_mode(weapon_index)?,
        ammunition,
    )?;
    aim.weapon_damage = attacker
        .weapon_damage_effects(weapon_index)?
        .accuracy(aim.range.map(|range| range.bracket));
    aim.attacker_movement = attacker.attacker_movement_modifier(rules.fasa_turning);
    aim.heat = attacker.heat().to_hit_modifier();
    aim.sensors = match attacker.system_hits(BattleSystem::Sensors) {
        0 => 0,
        1 => 2,
        _ => 75,
    };
    aim.mounting_section = attacker.mounting_modifier(mount.criticals[0].section);
    aim.beacon_accuracy = i8::from(attacker.has_beacon(super::BattleBeaconKind::Haywire));
    let loadout = attacker.loadout()?;
    if mount.computer_assists(
        ammunition,
        loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::TargetingComputer)
            .map(|part| !attacker.critical_unavailable(part.location)),
    ) {
        aim.targeting_computer = -1;
    }
    Ok(aim)
}

/// Target-independent weapon arithmetic shared by every supported chassis and target kind.
pub(super) fn weapon_base(
    weapon: super::BattleWeapon,
    distance: f64,
    gunnery: i16,
    rules: BattleAimRules,
    fire_mode: super::BattleFireMode,
    ammunition: super::BattleAmmunitionMode,
) -> Result<BattleAimModifiers> {
    Ok(BattleAimModifiers {
        self_target: false,
        indirect: None,
        gunnery,
        distance,
        network_range: None,
        range: weapon.range_modifier_for_ammunition(
            distance,
            rules.extended_ranges,
            fire_mode,
            rules.hotload_half_minimum,
            ammunition,
        )?,
        attacker_movement: 0,
        attacker_water: 0,
        target_movement: 0,
        dug_in: 0,
        woods_cover: 0,
        orbital_drop: 0,
        heat: 0,
        sensors: 0,
        control_damage: 0,
        mounting_section: 0,
        weapon_accuracy: weapon.accuracy_modifier(),
        weapon_damage: 0,
        beacon_accuracy: 0,
        ammunition_accuracy: match ammunition {
            super::BattleAmmunitionMode::Cluster => -1,
            super::BattleAmmunitionMode::ArmorPiercing => 1,
            _ => 0,
        },
        targeting_computer: 0,
        targeting_mode: 0,
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
    rules: BattleAimRules,
) -> Result<BattleAimModifiers> {
    let gunnery = super::unit_gunnery_target(world, shooter, weapon_index, extended_gunnery)?;
    aim_modifiers(world, shooter, target, weapon_index, gunnery, rules)
}

/// Close-range target movement for grounded biped physical attacks.
/// Eligibility rejects airborne targets before this calculation; immobility stacks with posture.
pub(super) fn ground_physical_target_modifier(
    world: &World,
    target: &BattleUnit,
    extended: bool,
) -> i8 {
    let immobile = super::aimed_target::mech_immobile(world, target);
    target_motion(target.motion().map_or(0.0, |motion| motion.speed), extended)
        - if immobile { 4 } else { 0 }
        - if target.posture() == super::BattlePosture::Prone {
            2
        } else {
            0
        }
}

/// The water term depends on the attacker's actual terrain and signed elevation, not its target.
/// Coordinate-only attacks retain their independent base calculation.
pub(super) fn water_modifier(world: &World, shooter: ObjectId) -> Result<u8> {
    let position = super::scanner::scanner_unit(world, shooter)
        .and_then(|unit| unit.position)
        .context("Shooter is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    Ok(u8::from(
        tile.terrain() == super::Terrain::Water
            && super::unit_elevation(world, shooter)?.is_some_and(|z| z < 0),
    ))
}

/// Target-owned aim contributions, independent of the firing unit's construction.
pub(super) struct TargetAimModifiers {
    /// Target-specific ammunition adjustment added to the weapon base.
    pub ammunition_accuracy: i8,
    pub dug_in: i16,
    pub woods_cover: i8,
    /// An intact orbital cocoon makes the target easier to hit.
    pub orbital_drop: i16,
    pub movement: i8,
    pub beacon_accuracy: i8,
    pub concealed: bool,
}

/// Apply target movement, friendly TAG assistance, concealment and homing pods identically for every shooter.
pub(super) fn target_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    ammunition: super::BattleAmmunitionMode,
    distance: f64,
    rules: BattleAimRules,
) -> Result<TargetAimModifiers> {
    let attacker = super::scanner::scanner_unit(world, shooter)
        .context("Shooter construction is unavailable")?;
    let defender = super::scanner::scanner_unit(world, target)
        .context("Target construction is unavailable")?;
    let movement = ammunition.target_movement_modifier(unit_target_movement_modifier(
        world,
        target,
        distance,
        rules.extended_movement,
    )?);
    let artemis_v_guided = ammunition.munition() == super::BattleAmmunitionMode::Artemis
        && super::artemis::artemis_v(world, shooter)
        && !super::electronic_field(world, shooter)?.blocks_outgoing_guidance()
        && !super::electronic_field(world, target)?.blocks_incoming_guidance();
    let friendly_tag = ammunition.munition() == super::BattleAmmunitionMode::SemiGuided
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
            super::BattleVtolFlightPhase::Airborne | super::BattleVtolFlightPhase::Falling
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
        dug_in: super::dig::cover_modifier(world, shooter, target, rules)?,
        woods_cover: if rules.woods_damage {
            -occupied_woods(world, target)?
        } else {
            0
        },
        orbital_drop: super::orbital_drop_state::current(world, target)
            .map_or(0, super::BattleOrbitalDrop::target_modifier),
        movement: ammunition.tag_movement_modifier(movement, friendly_tag)
            + i8::from(moving_aircraft),
        // Homing beacons and Artemis V guidance each make the shot one easier.
        beacon_accuracy: -i8::from(
            super::narc::has_beacon(world, target, super::BattleBeaconKind::Homing)
                && ammunition.munition() == super::BattleAmmunitionMode::Narc
                && !weapon.is_narc(),
        ) - i8::from(artemis_v_guided),
        concealed: defender.concealed,
    })
}

/// Underlying forest protects targets up to and including two elevation levels above its ground.
/// Smoke and fire overlays do not replace the source terrain used by this rule.
pub(super) fn occupied_woods(world: &World, target: ObjectId) -> Result<i8> {
    let position = super::scanner::scanner_unit(world, target)
        .and_then(|unit| unit.position)
        .context("Target is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = super::unit_elevation(world, target)?.context("Target is not placed")?;
    if elevation > i32::from(tile.elevation()) + 2 {
        return Ok(0);
    }
    Ok(tile.terrain().woods_density() as i8)
}

/// LBX gains two more points against rotorcraft; Stinger distinguishes flight from orbital orbital descents.
fn airborne_ammunition_adjustment(
    ammunition: super::BattleAmmunitionMode,
    rotorcraft: bool,
    flying: bool,
    cocoon: bool,
) -> i8 {
    match ammunition.munition() {
        super::BattleAmmunitionMode::Cluster if rotorcraft => -2,
        super::BattleAmmunitionMode::Stinger if flying => -3,
        super::BattleAmmunitionMode::Stinger if cocoon => -1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Cluster keeps its base -1 and Stinger has no extra bonus against a jumping Mech.
    #[test]
    fn airborne_ammunition_distinguishes_rotorcraft_orbital_descents_and_jumps() {
        use super::super::BattleAmmunitionMode as Mode;
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
            (107.5, 4),
            (150.5, 5),
            (193.5, 6),
        ] {
            assert_eq!(target_motion(speed, true), expected);
            assert_eq!(target_motion(-speed, true), expected);
            assert_eq!(target_motion(speed, false), expected.min(4));
        }
    }
}
