//! Direct terrain shots with candidate-world launch, woodland impacts and shooter recoil.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A shot aimed at a coordinate, with no fabricated unit damage recipient.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Publish shooter consequences and terrain notices with the enclosing shot"]
pub struct BattleHexShotReport {
    pub shooter: ObjectId,
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub weapon_index: usize,
    pub aim: BattleHexAimModifiers,
    pub target_number: Option<i32>,
    pub roll: u8,
    pub hit: bool,
    pub launched: bool,
    pub jammed: bool,
    pub loader_destroyed: bool,
    pub propellant_roll: Option<u8>,
    pub expenditure: BattleWeaponUse,
    pub misload: Option<BattleLaunchMisload>,
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<BattleNotice>,
    pub cluster_roll: Option<u8>,
    pub terrain: Vec<BattleWoodlandImpact>,
    pub recoil: Option<BattleRecoilReport>,
    pub surfaces: Vec<BattleSurfaceWeaponImpact>,
    pub buildings: Vec<BattleBuildingImpact>,
}

/// Resolve a direct non-character terrain shot atomically.
/// Unit-at-hex occupants use the unit-shot resolver. Observer-directed terrain
/// fire shares this path; character casualties require the host adapter.
pub fn resolve_hex_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    weapon_index: usize,
    rules: BattleShotRules,
) -> Result<BattleHexShotReport> {
    resolve_hex_shot_inner(
        world,
        shooter,
        pilot,
        coordinate,
        weapon_index,
        rules,
        false,
    )
}

/// Character-capable resolution inside an adapter that publishes injuries and casualties.
pub(super) fn resolve_hex_shot_in_action(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    weapon_index: usize,
    rules: BattleShotRules,
) -> Result<BattleHexShotReport> {
    resolve_hex_shot_inner(world, shooter, pilot, coordinate, weapon_index, rules, true)
}

/// Shared admission and candidate resolution for tactical and character-aware callers.
fn resolve_hex_shot_inner(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    weapon_index: usize,
    rules: BattleShotRules,
    character: bool,
) -> Result<BattleHexShotReport> {
    let operator = super::combat_operator::controlled(world, shooter, pilot)?;
    let vehicle = world.btech.vehicles().contains_key(&shooter);
    let object = world
        .objects
        .get(&shooter)
        .context("Shooter is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Shooter is unavailable"
    );
    ensure!(
        character || !object.flags.contains(Flag::InCharacter),
        "Direct tactical shots require non-character units"
    );
    let character_shooter = character && object.flags.contains(Flag::InCharacter);
    let toughness = if character_shooter {
        world
            .btech
            .vehicles()
            .get(&shooter)
            .and_then(|unit| unit.pilot())
            .or_else(|| {
                world
                    .btech
                    .constructed_units()
                    .get(&shooter)
                    .and_then(|unit| unit.pilot())
            })
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"))
    } else {
        rules.target_toughness
    };
    let facts =
        super::scanner::scanner_unit(world, shooter).context("Shooter is not constructed")?;
    ensure!(!facts.destroyed, "Unit is destroyed");
    let position = facts.position.context("Shooter is not placed")?;
    let map = position.map;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ready = if let Some(unit) = world.btech.vehicles().get(&shooter) {
        let readiness = unit.weapon_readiness(weapon_index)?;
        unit.check_spotter_fire(shooter, weapon_index)?;
        readiness.ready
    } else {
        let unit = &world.btech.constructed_units()[&shooter];
        unit.validate()?;
        let readiness = unit.weapon_readiness(weapon_index)?;
        unit.check_spotter_fire(shooter, weapon_index)?;
        readiness.ready
    };
    let super::weapon_geometry::WeaponGeometry {
        weapon,
        submerged,
        bears,
    } = super::weapon_geometry::geometry(world, shooter, weapon_index, coordinate.center())?;
    let indirect = super::spotter::indirect_hex_for_source(world, operator.source, weapon_index)?;
    if indirect.is_none() {
        ensure!(
            super::spotter::indirect_target_for_source(world, operator.source, weapon_index)?
                .is_none(),
            "Occupied observer target requires a unit shot"
        );
    }
    ensure!(ready, "Weapon is not ready");
    super::weapon_geometry::check_water(weapon, submerged)?;
    let aim = super::hex_aim::modifiers_for_source(
        world,
        operator.source,
        coordinate,
        weapon_index,
        super::unit_gunnery_target(world, shooter, weapon_index, rules.extended_gunnery)?,
        rules.aim,
    )?;
    ensure!(aim.visible, "Target hex is not visible");
    if aim.mode == BattleHexTargetMode::UnitAtHex {
        ensure!(
            hex_occupant(world, shooter, coordinate)?.is_none(),
            "Occupied coordinate requires a unit shot"
        );
    }
    ensure!(
        indirect.is_some() || rules.aim.override_weapon_arcs || bears,
        "Target is outside weapon arc"
    );
    let field = electronic_field(world, shooter)?;
    let fall = BattleFallRules {
        vehicle_impact: rules.vehicle_impact,
        stacking: rules.stacking,
        stagger: rules.stagger,
        hit: rules.hit,
        extended_piloting: rules.extended_piloting,
        toughness,
    };
    let target_number = aim.subtotal();
    world.attempt(|world| {
        let launch = super::coordinate_launch::resolve(
            world,
            super::weapon_launch::WeaponLaunchRequest {
                shooter,
                pilot,
                weapon_index,
                distance: aim.modifiers.distance,
                target_number,
                streak_confused: field.angel_disturbed,
                glancing: rules.glancing,
                fall,
                character_shooter,
            },
            false,
        )?;
        let intent = match aim.mode {
            BattleHexTargetMode::Ignite => BattleWoodlandIntent::Ignite,
            BattleHexTargetMode::Clear => BattleWoodlandIntent::Clear,
            _ => BattleWoodlandIntent::Incidental,
        };
        let mut cluster_roll = None;
        let mut terrain = Vec::new();
        let mut surfaces = Vec::new();
        let mut buildings = Vec::new();
        if launch.launched && target_number.is_some() {
            if launch.hit {
                let packets = super::weapon_groups::roll_weapon_groups(
                    super::weapon_groups::WeaponGroupRequest {
                        submerged,
                        range_damage: rules.range_damage,
                        damage_penalty: launch.expenditure.damage_penalty,
                        weapon,
                        ammunition: if launch.expenditure.ammunition_mode
                            == BattleAmmunitionMode::Flechette
                        {
                            BattleAmmunitionMode::Normal
                        } else {
                            launch.expenditure.ammunition_mode
                        },
                        fire_mode: launch.expenditure.fire_mode,
                        gatling_damage: launch.expenditure.gatling_damage,
                        distance: Some(aim.modifiers.distance),
                        glancing: false,
                        guidance_blocked: field.blocks_outgoing_guidance(),
                        angel_blocked: field.angel_disturbed,
                        target_beacon: false,
                    },
                    super::dice::unit_dice_mut(world, shooter)?,
                )?;
                cluster_roll = packets.cluster_roll;
                if aim.mode != BattleHexTargetMode::UnitAtHex {
                    let damage =
                        if launch.expenditure.ammunition_mode == BattleAmmunitionMode::Inferno {
                            // Inferno terrain exposure occurs once with no clearing or building damage.
                            vec![0]
                        } else {
                            packets.damage
                        };
                    for damage in damage {
                        if aim.mode == BattleHexTargetMode::Building {
                            if let Some(impact) =
                                super::building_damage::resolve(world, shooter, coordinate, damage)?
                            {
                                buildings.push(impact);
                            }
                            continue;
                        }
                        terrain.push(resolve_woodland_attack(
                            world,
                            BattleWoodlandAttack {
                                shooter,
                                coordinate,
                                weapon,
                                ammunition: launch.expenditure.ammunition_mode,
                                damage,
                                intent,
                            },
                        )?);
                        if aim.mode == BattleHexTargetMode::Hex
                            && let Some(impact) = super::surface_weapon::resolve(
                                world,
                                shooter,
                                coordinate,
                                (weapon, launch.expenditure.ammunition_mode),
                                fall,
                                character,
                            )?
                        {
                            surfaces.push(impact);
                        }
                    }
                }
            } else if weapon.profile().missiles == 0 {
                let damage = if let Some(damage) = launch.expenditure.gatling_damage {
                    u16::from(damage)
                } else {
                    weapon
                        .damage_groups_at_range(None, aim.modifiers.distance)?
                        .into_iter()
                        .sum()
                };
                terrain.push(resolve_woodland_attack(
                    world,
                    BattleWoodlandAttack {
                        shooter,
                        coordinate,
                        weapon,
                        ammunition: launch.expenditure.ammunition_mode,
                        damage,
                        intent,
                    },
                )?);
            }
        }
        let recoil = if vehicle {
            None
        } else {
            super::weapon_launch::resolve_recoil(world, shooter, weapon, fall, character_shooter)?
        };
        world.btech.validate_action(world)?;
        Ok(BattleHexShotReport {
            shooter,
            map,
            coordinate,
            weapon_index,
            aim,
            target_number,
            roll: launch.roll,
            hit: launch.hit,
            launched: launch.launched,
            jammed: launch.jammed,
            loader_destroyed: launch.loader_destroyed,
            propellant_roll: launch.propellant_roll,
            expenditure: launch.expenditure,
            misload: launch.misload,
            ammunition_warning: launch.ammunition_warning,
            launch_notices: launch.launch_notices,
            cluster_roll,
            terrain,
            surfaces,
            buildings,
            recoil,
        })
    })
}

impl BattleHexShotReport {
    /// Mechanical and terrain notices; no unit hit/miss or target identity is fabricated.
    pub fn notices(&self) -> Vec<BattleNotice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private checks at their original positions for host publication.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::BattlePilotNotice>,
    ) -> Vec<BattleNotice> {
        if let Some(misload) = &self.misload {
            super::piloting::append_feedback(private, misload.pilot_notices().to_vec(), 0);
            return misload.notices();
        }
        if self.jammed || self.loader_destroyed {
            return Vec::new();
        }
        if !self.launched {
            let mut notices = self.launch_notices.clone();
            notices.push(BattleNotice {
                unit: self.shooter,
                text: "Your streak fails to lock on.".into(),
            });
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(BattleNotice {
                unit: self.shooter,
                text: text.clone(),
            });
        }
        for impact in &self.terrain {
            notices.extend(impact.notices.clone());
        }
        for impact in &self.buildings {
            notices.extend(impact.notices.clone());
        }
        for impact in &self.surfaces {
            super::piloting::append_feedback(
                private,
                impact.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(impact.notices.clone());
        }
        if let Some(recoil) = &self.recoil {
            recoil.append_feedback(self.shooter, &mut notices, private);
        }
        notices
    }
}
