//! Direct terrain shots with candidate-world launch, woodland impacts and shooter recoil.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A shot aimed at a coordinate, with no fabricated unit damage recipient.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "Publish shooter consequences and terrain notices with the enclosing shot"]
pub struct HexShotReport {
    pub shooter: ObjectId,
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub weapon_index: usize,
    pub aim: HexAimModifiers,
    pub target_number: Option<i32>,
    pub roll: u8,
    pub hit: bool,
    pub launched: bool,
    pub jammed: bool,
    pub loader_destroyed: bool,
    pub feed_broken: bool,
    pub feed_critical_roll: Option<u8>,
    pub expenditure: WeaponUse,
    pub misload: Option<LaunchMisload>,
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<Notice>,
    pub cluster_roll: Option<u8>,
    pub terrain: Vec<WoodlandImpact>,
    pub recoil: Option<RecoilReport>,
    pub surfaces: Vec<SurfaceWeaponImpact>,
    pub buildings: Vec<BuildingImpact>,
    /// Minefields a Thunder salvo laid in place of terrain damage.
    pub thunder: ThunderReport,
    /// Smoke a Smoke missile salvo laid in place of terrain damage.
    pub smoke: Option<SmokeCloud>,
}

/// Smoke a Smoke missile salvo laid: light when the salvo delivered 5 points of damage or
/// less, heavy otherwise (Tactical Operations: Advanced Units & Equipment p. 183).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SmokeCloud {
    pub coordinate: HexCoordinate,
    pub heavy: bool,
    /// Seconds until it clears, in the End Phase three turns after this one.
    pub seconds: i64,
}

/// Resolve a direct non-character terrain shot atomically.
/// Unit-at-hex occupants use the unit-shot resolver. Observer-directed terrain
/// fire shares this path; character casualties require the host adapter.
pub fn resolve_hex_shot(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    weapon_index: usize,
    rules: ShotRules,
) -> Result<HexShotReport> {
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
    coordinate: HexCoordinate,
    weapon_index: usize,
    rules: ShotRules,
) -> Result<HexShotReport> {
    resolve_hex_shot_inner(world, shooter, pilot, coordinate, weapon_index, rules, true)
}

/// Shared admission and candidate resolution for tactical and character-aware callers.
fn resolve_hex_shot_inner(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    weapon_index: usize,
    rules: ShotRules,
    character: bool,
) -> Result<HexShotReport> {
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
    let ready = super::with_unit!(world.btech.unit(shooter).expect("admitted unit"), |unit| {
        unit.validate()?;
        let readiness = unit.weapon_readiness(weapon_index)?;
        unit.check_spotter_fire(shooter, weapon_index)?;
        readiness.ready
    });
    // Firing a gun its machine gun array links fires every ready linked gun as one attack
    // (Total Warfare p. 137), at a hex as at a unit.
    let volley = super::with_unit!(world.btech.unit(shooter).expect("admitted unit"), |unit| {
        unit.array_volley(weapon_index)?
    });
    let super::weapon_geometry::WeaponGeometry {
        weapon,
        submerged,
        bears,
    } = super::weapon_geometry::geometry(world, shooter, weapon_index, coordinate.center())?;
    let indirect = super::spotter::indirect_hex_for_source(world, operator.source, weapon_index)?;
    if super::spotter::observed_hex_for_source(world, operator.source, weapon_index)?.is_none() {
        ensure!(
            super::spotter::observed_target_for_source(world, operator.source, weapon_index)?
                .is_none(),
            "Occupied observer target requires a unit shot"
        );
    }
    ensure!(ready, "Weapon is not ready");
    super::weapon_geometry::check_water(world, shooter, weapon, submerged)?;
    super::planetary_conditions::check_wind(world, shooter, weapon)?;
    if indirect.is_none() {
        super::weapon_geometry::check_leg_cover_hex(world, shooter, weapon_index, coordinate)?;
    }
    ensure!(
        !weapon.is_torpedo()
            || record
                .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?
                .is_open_water(),
        "Torpedoes can only strike targets in the water!"
    );
    let aim = super::hex_aim::modifiers_for_source(
        world,
        operator.source,
        coordinate,
        weapon_index,
        super::unit_gunnery_target(world, shooter, weapon_index, rules.extended_gunnery)?,
    )?;
    ensure!(aim.visible, "Target hex is not visible");
    // Smoke and Thunder missiles fire at the hex whoever stands in it (Tactical Operations:
    // Advanced Units & Equipment pp. 183, 185).
    let hex_only = weapon.profile().missiles > 0
        && super::with_unit!(world.btech.unit(shooter).expect("admitted unit"), |unit| {
            unit.ammunition_mode(weapon_index)?
        })
        .targets_hexes_only();
    if aim.mode == HexTargetMode::UnitAtHex && !hex_only {
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
    let fall = FallRules {
        vehicle_impact: rules.vehicle_impact,
        extended_piloting: rules.extended_piloting,
        toughness,
    };
    let target_number = aim.subtotal();
    world.attempt(|world| {
        let mut launch = super::coordinate_launch::resolve(
            world,
            super::weapon_launch::WeaponLaunchRequest {
                shooter,
                pilot,
                weapon_index,
                distance: aim.modifiers.distance,
                target_number,
                streak_confused: field.angel_disturbed,
                fall,
                character_shooter,
            },
            false,
        )?;
        // The array's other guns spend their ammunition and recycle with the one fired.
        let array = super::machine_gun_array::fire_linked_guns(
            world,
            shooter,
            pilot,
            weapon_index,
            volley.as_deref(),
            launch.launched,
            &mut launch.launch_notices,
        )?;
        let intent = match aim.mode {
            HexTargetMode::Ignite => WoodlandIntent::Ignite,
            HexTargetMode::Clear => WoodlandIntent::Clear,
            _ => WoodlandIntent::Incidental,
        };
        let mut cluster_roll = None;
        let mut terrain = Vec::new();
        let mut surfaces = Vec::new();
        let mut buildings = Vec::new();
        let mut thunder = ThunderReport::default();
        let mut smoke = None;
        let munition = launch.expenditure.ammunition_mode;
        if launch.launched
            && let Some(number) = target_number
            && hex_only
            && munition.targets_hexes_only()
        {
            if munition.is_thunder() {
                let launcher = u16::from(weapon.profile().missiles);
                let landing = if launch.hit {
                    Some(coordinate)
                } else {
                    let margin = indirect.map(|_| {
                        u16::try_from(number - i32::from(launch.roll)).unwrap_or(u16::MAX)
                    });
                    super::thunder::scatter(world, shooter, map, coordinate, margin)?
                };
                if let Some(landing) = landing {
                    thunder = super::thunder::lay(
                        world,
                        shooter,
                        map,
                        landing,
                        munition,
                        (launcher, !launch.hit),
                    )?;
                }
            } else if launch.hit {
                let packets = super::weapon_groups::roll_weapon_groups(
                    super::weapon_groups::WeaponGroupRequest {
                        weapon,
                        ammunition: munition,
                        fire_mode: launch.expenditure.fire_mode,
                        gatling_damage: None,
                        distance: Some(aim.modifiers.distance),
                        glancing: false,
                        guidance_blocked: false,
                        angel_blocked: field.angel_disturbed,
                        target_beacon: false,
                        artemis_v: false,
                        ams_engaged: false,
                        indirect: indirect.is_some(),
                        apollo: false,
                        array: None,
                        unit_without_heat: false,
                        interference: super::planetary_conditions::under_interference(
                            world, shooter,
                        ),
                    },
                    super::dice::unit_dice_mut(world, shooter)?,
                )?;
                cluster_roll = packets.cluster_roll;
                let cloud = SmokeCloud {
                    coordinate,
                    heavy: packets.damage.iter().sum::<u16>() > 5,
                    seconds: super::decorations::smoke_seconds(world),
                };
                super::decorations::raise_smoke(
                    world,
                    map,
                    coordinate,
                    cloud.seconds,
                    cloud.heavy,
                )?;
                smoke = Some(cloud);
            }
        } else if launch.launched && target_number.is_some() {
            if launch.hit {
                let packets = super::weapon_groups::roll_weapon_groups(
                    super::weapon_groups::WeaponGroupRequest {
                        weapon,
                        ammunition: if launch.expenditure.ammunition_mode
                            == AmmunitionMode::Flechette
                        {
                            AmmunitionMode::Normal
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
                        artemis_v: super::artemis::artemis_v(world, shooter),
                        ams_engaged: false,
                        indirect: false,
                        apollo: weapon.is_mrm()
                            && super::artemis::apollo_operational(world, shooter, weapon_index)?,
                        array,
                        unit_without_heat: false,
                        interference: super::planetary_conditions::under_interference(
                            world, shooter,
                        ),
                    },
                    super::dice::unit_dice_mut(world, shooter)?,
                )?;
                cluster_roll = packets.cluster_roll;
                // An array's hits all strike one place, so they land as one blow.
                let packets_damage = if packets.single_location {
                    vec![packets.damage.iter().sum()]
                } else {
                    packets.damage
                };
                if aim.mode != HexTargetMode::UnitAtHex {
                    let damage = if launch.expenditure.ammunition_mode == AmmunitionMode::Inferno {
                        // Inferno terrain exposure occurs once with no clearing or building damage.
                        vec![0]
                    } else {
                        packets_damage
                    };
                    for damage in damage {
                        if aim.mode == HexTargetMode::Building {
                            if let Some(impact) =
                                super::building_damage::resolve(world, shooter, coordinate, damage)?
                            {
                                buildings.push(impact);
                            }
                            continue;
                        }
                        terrain.push(resolve_woodland_attack(
                            world,
                            WoodlandAttack {
                                shooter,
                                coordinate,
                                weapon,
                                ammunition: launch.expenditure.ammunition_mode,
                                damage,
                                intent,
                            },
                        )?);
                        if aim.mode == HexTargetMode::Hex
                            && let Some(impact) = super::surface_weapon::resolve(
                                world,
                                shooter,
                                coordinate,
                                ((weapon, launch.expenditure.ammunition_mode), damage),
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
                    // A missed array volley puts every gun's rounds into the hex.
                    let guns = volley.as_ref().map_or(1, Vec::len) as u16;
                    weapon
                        .damage_groups_at_range(None, aim.modifiers.distance)?
                        .into_iter()
                        .sum::<u16>()
                        * guns
                };
                terrain.push(resolve_woodland_attack(
                    world,
                    WoodlandAttack {
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
        Ok(HexShotReport {
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
            feed_broken: launch.feed_broken,
            feed_critical_roll: launch.feed_critical_roll,
            expenditure: launch.expenditure,
            misload: launch.misload,
            ammunition_warning: launch.ammunition_warning,
            launch_notices: launch.launch_notices,
            cluster_roll,
            terrain,
            surfaces,
            buildings,
            recoil,
            thunder,
            smoke,
        })
    })
}

impl HexShotReport {
    /// Mechanical and terrain notices; no unit hit/miss or target identity is fabricated.
    pub fn notices(&self) -> Vec<Notice> {
        self.notices_with_feedback(&mut Vec::new())
    }

    /// Retain private checks at their original positions for host publication.
    pub(crate) fn notices_with_feedback(
        &self,
        private: &mut Vec<super::PilotNotice>,
    ) -> Vec<Notice> {
        if !self.launched {
            let mut notices = self.launch_notices.clone();
            notices.push(Notice {
                unit: self.shooter,
                text: "Your streak fails to lock on.".into(),
            });
            return notices;
        }
        let mut notices = self.launch_notices.clone();
        if let Some(text) = &self.ammunition_warning {
            notices.push(Notice {
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
        notices.extend(self.thunder.notices(self.shooter));
        if let Some(smoke) = self.smoke {
            notices.push(Notice {
                unit: self.shooter,
                text: format!(
                    "Your smoke missiles fill {},{} with {} smoke.",
                    smoke.coordinate.x,
                    smoke.coordinate.y,
                    if smoke.heavy { "heavy" } else { "light" }
                ),
            });
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
