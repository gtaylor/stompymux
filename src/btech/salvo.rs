//! Conventional missile-cluster tables and atomic grouped weapon-hit resolution.
use super::{BattleHit, BattleHitArc, BattleHitRules, BattleImpactReport, BattleWeapon};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

impl BattleWeapon {
    /// Resolve an unmodified 2d6 cluster roll for a supported conventional launcher.
    pub fn missile_hits(self, roll: u8) -> Result<u8> {
        ensure!((2..=12).contains(&roll), "Invalid missile cluster roll");
        if self.is_streak() || self.is_thunderbolt() || self.is_narc() || self == Self::INarcBeacon
        {
            return Ok(self.profile().missiles);
        }
        let table = match self {
            Self::Mml3 => [1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3],
            Self::Mml7 => [2, 2, 3, 4, 4, 4, 4, 6, 6, 7, 7],
            Self::Mml9 => [3, 3, 4, 5, 5, 5, 5, 7, 7, 9, 9],
            Self::ClanAtm3 => [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3],
            Self::ClanAtm6 => [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6],
            Self::ClanAtm9 => [2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 9],
            Self::ClanAtm12 => [4, 4, 6, 6, 8, 8, 8, 10, 10, 12, 12],
            Self::ClanSrm2 | Self::Srm2 | Self::SrDfm2 | Self::ClanLbx2 | Self::Lbx2 => {
                [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]
            }
            Self::Mml5
            | Self::ClanLrm5
            | Self::Lrm5
            | Self::Elrm5
            | Self::LrDfm5
            | Self::ClanLbx5
            | Self::Lbx5 => [1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5],
            Self::ClanLrm10 => [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10],
            Self::ClanLrm15 => [5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15],
            Self::Lrm10 | Self::Rocket10 => [3, 4, 4, 5, 6, 6, 6, 8, 8, 10, 10],
            Self::Lrm15 | Self::Rocket15 => [5, 5, 9, 9, 9, 9, 9, 12, 12, 15, 15],
            Self::ClanSrm4 | Self::Srm4 | Self::SrDfm4 => [1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4],
            Self::ClanSrm6 | Self::Srm6 | Self::SrDfm6 => [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6],
            Self::ClanLrm20
            | Self::Rocket20
            | Self::Lrm20
            | Self::Mrm20
            | Self::Elrm20
            | Self::LrDfm20
            | Self::ClanLbx20
            | Self::Lbx20 => [6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20],
            Self::Elrm10 | Self::LrDfm10 => [3, 4, 4, 5, 6, 6, 6, 7, 7, 10, 10],
            Self::Elrm15 | Self::LrDfm15 => [5, 5, 9, 9, 9, 9, 12, 12, 12, 15, 15],
            Self::ClanLbx10 | Self::Lbx10 => [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10],
            Self::Mrm10 => [2, 3, 4, 5, 6, 6, 6, 8, 8, 10, 10],
            Self::Mrm30 => [10, 10, 12, 18, 18, 18, 18, 24, 24, 30, 30],
            Self::Mrm40 => [12, 12, 18, 24, 24, 24, 24, 32, 32, 40, 40],
            _ => bail!("Weapon is not a missile launcher"),
        };
        Ok(table[usize::from(roll - 2)])
    }

    /// Each group receives an independent location; dead-fire missiles always hit individually.
    pub fn damage_groups(self, cluster_roll: Option<u8>) -> Result<Vec<u16>> {
        self.damage_groups_for_hit(cluster_roll, false, None)
    }

    /// Group a hit using its actual spatial range, before any aim-bracket rounding.
    pub fn damage_groups_at_range(
        self,
        cluster_roll: Option<u8>,
        distance: f64,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_hit(cluster_roll, false, Some(distance))
    }

    /// Glancing direct hits round damage up; missiles instead shift the cluster roll down four.
    pub(super) fn damage_groups_for_hit(
        self,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_ammunition_hit(
            super::BattleAmmunitionMode::Normal,
            cluster_roll,
            glancing,
            distance,
        )
    }

    /// Resolve packet sizes for a selected ammunition type at the actual attack range.
    pub fn damage_groups_for_ammunition(
        self,
        mode: super::BattleAmmunitionMode,
        cluster_roll: Option<u8>,
        distance: f64,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_ammunition_hit(mode, cluster_roll, false, Some(distance))
    }

    /// Cluster rounds resolve individual pellets; slug rounds retain conventional direct damage.
    pub(super) fn damage_groups_for_ammunition_hit(
        self,
        mode: super::BattleAmmunitionMode,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>> {
        ensure!(!self.is_artillery(), "Artillery damage requires an arrival");
        ensure!(
            mode.supports(self),
            "{}",
            match mode {
                super::BattleAmmunitionMode::Cluster =>
                    "Weapon does not support cluster ammunition",
                super::BattleAmmunitionMode::SemiGuided =>
                    "Weapon does not support semi-guided ammunition",
                super::BattleAmmunitionMode::Artemis =>
                    "Weapon does not support Artemis ammunition",
                _ => "Weapon does not support specialized autocannon ammunition",
            }
        );
        ensure!(
            mode != super::BattleAmmunitionMode::Inferno,
            "Inferno hits use burn exposure instead of armor damage groups"
        );
        let cluster = mode == super::BattleAmmunitionMode::Cluster;
        let artemis = mode.munition() == super::BattleAmmunitionMode::Artemis;
        if let Some(distance) = distance {
            ensure!(
                distance.is_finite() && distance >= 0.0,
                "Invalid weapon damage range"
            );
        }
        if self.profile().missiles == 0 && !cluster {
            ensure!(
                cluster_roll.is_none(),
                "Non-missile weapon does not use a cluster roll"
            );
            let damage = if matches!(self, Self::SnubNosedPpc | Self::HeavyGaussRifle) {
                let distance = distance.context("Weapon damage requires an attack range")?;
                let profile = self.profile();
                let (medium, long) = if self == Self::HeavyGaussRifle {
                    (20, 10)
                } else {
                    (8, 5)
                };
                if distance > f64::from(profile.medium_range) {
                    long
                } else if distance > f64::from(profile.short_range) {
                    medium
                } else {
                    u16::from(profile.damage)
                }
            } else {
                u16::from(self.profile().damage)
            };
            let damage = mode.armored_damage(damage);
            return Ok(vec![if glancing { damage.div_ceil(2) } else { damage }]);
        }
        let roll = cluster_roll.context("Missile launcher requires a cluster roll")?;
        ensure!((2..=12).contains(&roll), "Invalid missile cluster roll");
        let adjusted = i16::from(roll) + if artemis { 2 } else { 0 } - if glancing { 4 } else { 0 };
        let mut hits = if self.is_streak() {
            self.profile().missiles
        } else if glancing && adjusted < 2 {
            1
        } else {
            self.missile_hits(adjusted.clamp(2, 12) as u8)?
        };
        let group_size = if mode.is_mml_lrm()
            || matches!(
                self,
                Self::ClanLrm5
                    | Self::ClanLrm10
                    | Self::ClanLrm15
                    | Self::ClanLrm20
                    | Self::Lrm5
                    | Self::Lrm10
                    | Self::Lrm15
                    | Self::Lrm20
                    | Self::Rocket10
                    | Self::Rocket15
                    | Self::Rocket20
                    | Self::Mrm10
                    | Self::Mrm20
                    | Self::Mrm30
                    | Self::Mrm40
                    | Self::Elrm5
                    | Self::Elrm10
                    | Self::Elrm15
                    | Self::Elrm20
            ) {
            5
        } else {
            1
        };
        let mut groups = Vec::new();
        while hits > 0 {
            let count = hits.min(group_size);
            groups.push(
                u16::from(count)
                    * if cluster {
                        1
                    } else {
                        u16::from(self.profile_for_ammunition(mode).damage)
                    },
            );
            hits -= count;
        }
        Ok(groups)
    }
}

/// One independently located damage group in a successful salvo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleSalvoGroup {
    pub damage: u16,
    pub hit: BattleHit,
    pub impact: BattleImpactReport,
    /// Applied by the tactical resolver; fall injuries are nested in the balance report.
    pub pilot_injuries: Vec<super::BattlePilotInjury>,
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<super::BattleNotice>,
    /// Balance consequences completed before the next damage group.
    pub balance: Vec<super::BattleBalanceReport>,
    pub flooding: Vec<super::BattleSectionExposureReport>,
}

/// Rolled cluster size and applied groups; resolution stops when the target is destroyed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleSalvoReport {
    /// Nominal LBX damage terrain check, before pellet counting and cover absorption.
    pub initial_woods: Option<super::BattleWoodsAbsorption>,
    pub woods: Option<super::BattleWoodsAbsorption>,
    /// Cluster hits before automatic defenses, absent for non-missile weapons.
    pub missiles_before_defense: Option<u8>,
    pub cluster_roll: Option<u8>,
    /// Inferno exposure replaces armor packets after automatic missile defense.
    pub inferno: Option<super::BattleInfernoHit>,
    pub groups: Vec<BattleSalvoGroup>,
    /// Pre-damage XP attempt for each group, in matching order; absent for ineligible groups.
    pub experience: Vec<Option<super::BattleShotExperienceAward>>,
    /// Diagnostic messages captured in damage-group order; the host owns channel delivery.
    pub experience_messages: Vec<super::BattleChannelMessage>,
}

/// Effects that the enclosing action can apply and publish between weapon groups.
#[derive(Clone, Copy)]
enum SalvoEffects<'a> {
    Material,
    Tactical(super::BattleFallRules),
    Character {
        rules: super::BattleFallRules,
        experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
    },
}

/// Resolve grouped character damage within a casualty-publishing host action.
pub(super) fn resolve_salvo_in_action(
    world: &mut World,
    target: ObjectId,
    weapon: BattleWeapon,
    arc: BattleHitArc,
    rules: super::BattleFallRules,
) -> Result<BattleSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::BattleAmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
            rules.hit,
            SalvoEffects::Character {
                rules,
                experience: None,
            },
            false,
        )
    })
}

/// Resolve a successful conventional weapon hit as one world mutation, including all groups and dice.
/// Firing permission, the to-hit roll, ammunition expenditure, heat and recycle are caller responsibilities.
/// Range-dependent weapons require the direct-shot resolver, which has attacker geometry.
/// Crew/fall effects in each impact report must be applied before the caller commits the whole action.
pub fn resolve_salvo(
    world: &mut World,
    target: ObjectId,
    weapon: BattleWeapon,
    arc: BattleHitArc,
    rules: BattleHitRules,
) -> Result<BattleSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::BattleAmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
            rules,
            SalvoEffects::Material,
            false,
        )
    })
}

/// Resolve tactical injuries and stun between damage groups, stopping on structural or pilot loss.
/// Firing guards, expenditure, hit probability and remaining casualty effects are still caller-owned.
pub fn resolve_tactical_salvo(
    world: &mut World,
    target: ObjectId,
    weapon: BattleWeapon,
    arc: BattleHitArc,
    rules: super::BattleFallRules,
) -> Result<BattleSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::BattleAmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
            rules.hit,
            SalvoEffects::Tactical(rules),
            false,
        )
    })
}

/// Damage policy and pre-impact XP facts supplied by the enclosing direct shot.
pub(super) struct ShotDamage<'a> {
    pub woods_damage: bool,
    pub range_damage: bool,
    pub submerged: bool,
    pub aimed: Option<super::aimed_hit::AimedShot>,
    pub incoming: Option<u8>,
    pub rules: super::BattleFallRules,
    pub hit_arc_mode: i64,
    pub glancing: bool,
    pub character: bool,
    pub intercepted: u8,
    pub experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
}

/// Recompute impact geometry and apply character effects and XP between direct-shot groups.
pub(super) fn resolve_salvo_from_shot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: impl Into<SalvoWeapon>,
    damage: ShotDamage<'_>,
) -> Result<BattleSalvoReport> {
    world.attempt(|world| resolve_salvo_in_candidate(world, shooter, target, weapon, damage))
}

/// Apply directly to an enclosing shot; every failure aborts that shot.
pub(super) fn resolve_salvo_in_candidate(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: impl Into<SalvoWeapon>,
    damage: ShotDamage<'_>,
) -> Result<BattleSalvoReport> {
    let weapon = weapon.into();
    let effects = if damage.character {
        SalvoEffects::Character {
            rules: damage.rules,
            experience: damage.experience,
        }
    } else {
        SalvoEffects::Tactical(damage.rules)
    };
    resolve_salvo_with_effects(
        world,
        target,
        (weapon.weapon, weapon.ammunition_mode),
        HitGeometry::Direct {
            range_damage: damage.range_damage,
            submerged: damage.submerged,
            woods_damage: damage.woods_damage,
            aimed: damage.aimed,
            damage_penalty: weapon.damage_penalty,
            shooter,
            hit_arc_mode: damage.hit_arc_mode,
            fire_mode: weapon.fire_mode,
            gatling_damage: weapon.gatling_damage,
            intercepted: damage.intercepted,
            incoming: damage.incoming,
        },
        damage.rules.hit,
        effects,
        damage.glancing,
    )
}

/// Shared launch facts consumed by target damage, independent of shooter inventory representation.
#[derive(Clone, Copy)]
pub(super) struct SalvoWeapon {
    pub(super) damage_penalty: u8,
    pub(super) weapon: BattleWeapon,
    pub(super) ammunition_mode: super::BattleAmmunitionMode,
    pub(super) fire_mode: super::BattleFireMode,
    pub(super) gatling_damage: Option<u8>,
}

impl From<&super::BattleWeaponUse> for SalvoWeapon {
    fn from(weapon: &super::BattleWeaponUse) -> Self {
        Self {
            damage_penalty: weapon.damage_penalty,
            weapon: weapon.weapon,
            ammunition_mode: weapon.ammunition_mode,
            fire_mode: weapon.fire_mode,
            gatling_damage: weapon.gatling_damage,
        }
    }
}

impl From<&super::BattleVehicleWeaponUse> for SalvoWeapon {
    fn from(weapon: &super::BattleVehicleWeaponUse) -> Self {
        Self {
            damage_penalty: 0,
            weapon: weapon.weapon,
            ammunition_mode: weapon.ammunition_mode,
            fire_mode: weapon.fire_mode,
            gatling_damage: weapon.gatling_damage,
        }
    }
}

/// An explicitly supplied attack direction or the live geometry of a direct shot.
#[derive(Clone, Copy)]
enum HitGeometry {
    Fixed(BattleHitArc),
    Direct {
        range_damage: bool,
        submerged: bool,
        woods_damage: bool,
        aimed: Option<super::aimed_hit::AimedShot>,
        shooter: ObjectId,
        damage_penalty: u8,
        hit_arc_mode: i64,
        fire_mode: super::BattleFireMode,
        gatling_damage: Option<u8>,
        intercepted: u8,
        incoming: Option<u8>,
    },
}

impl HitGeometry {
    /// Falls rotate the target and remove standing partial cover before later groups land.
    fn current(self, world: &World, target: ObjectId) -> Result<(BattleHitArc, bool)> {
        let (shooter, hit_arc_mode) = match self {
            Self::Fixed(arc) => return Ok((arc, false)),
            Self::Direct {
                shooter,
                hit_arc_mode,
                ..
            } => (shooter, hit_arc_mode),
        };
        let arc = super::hit_direction::HitDirection::Direct {
            shooter,
            mode: hit_arc_mode,
        }
        .current(world, target)?;
        Ok((
            arc,
            super::unit_terrain_los(world, shooter, target)?.partial_cover,
        ))
    }
}

/// Shared cluster and hit-location traversal prevents divergence between damage compositions.
fn resolve_salvo_with_effects(
    world: &mut World,
    target: ObjectId,
    weapon: (BattleWeapon, super::BattleAmmunitionMode),
    geometry: HitGeometry,
    rules: BattleHitRules,
    effects: SalvoEffects<'_>,
    glancing: bool,
) -> Result<BattleSalvoReport> {
    let (weapon, mode) = weapon;
    let character = matches!(effects, SalvoEffects::Character { .. });
    let tactical_rules = match effects {
        SalvoEffects::Material => None,
        SalvoEffects::Tactical(rules) | SalvoEffects::Character { rules, .. } => Some(rules),
    };
    if tactical_rules.is_some() && !character {
        ensure!(
            world
                .objects
                .get(&target)
                .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
            "Tactical salvo requires a non-character unit"
        );
    }
    ensure!(
        world
            .objects
            .get(&target)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&target)
        .context("Unit construction state is unavailable")?;
    super::validation_context::unit(target, unit)?;
    ensure!(
        !unit.is_destroyed()
            || matches!(
                geometry,
                HitGeometry::Direct {
                    incoming: Some(_),
                    ..
                }
            ),
        "Unit is already destroyed"
    );
    let distance = match geometry {
        HitGeometry::Fixed(_) => None,
        HitGeometry::Direct { shooter, .. } => {
            Some(super::unit_range(world, shooter, target)?.spatial)
        }
    };
    let (guidance_blocked, angel_blocked) = match geometry {
        HitGeometry::Fixed(_) => (false, false),
        HitGeometry::Direct { shooter, .. } => {
            let source = super::electronic_field(world, shooter)?;
            let target = super::electronic_field(world, target)?;
            (
                source.blocks_outgoing_guidance() || target.blocks_incoming_guidance(),
                source.angel_disturbed || target.angel_protected,
            )
        }
    };
    let initial_woods = if let HitGeometry::Direct {
        shooter,
        woods_damage: true,
        ..
    } = geometry
    {
        super::woods_absorption::begin_pellets(world, shooter, target, weapon, mode)?
    } else {
        None
    };
    let unit = world.btech.constructed.get_mut(&target).unwrap();
    let fire_mode = match geometry {
        HitGeometry::Direct { fire_mode, .. } => fire_mode,
        HitGeometry::Fixed(_) => super::BattleFireMode::Normal,
    };
    let gatling_damage = match geometry {
        HitGeometry::Direct { gatling_damage, .. } => gatling_damage,
        HitGeometry::Fixed(_) => None,
    };
    let shell_woods = matches!(
        geometry,
        HitGeometry::Direct {
            woods_damage: true,
            ..
        }
    ) && super::woods_absorption::direct_shells(weapon, mode);
    let mut packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            submerged: matches!(
                geometry,
                HitGeometry::Direct {
                    submerged: true,
                    ..
                }
            ),
            range_damage: match geometry {
                HitGeometry::Direct { range_damage, .. } => range_damage,
                HitGeometry::Fixed(_) => false,
            },
            damage_penalty: match geometry {
                HitGeometry::Direct { damage_penalty, .. } => damage_penalty,
                HitGeometry::Fixed(_) => 0,
            },
            weapon,
            ammunition: mode,
            fire_mode,
            gatling_damage,
            distance,
            glancing: glancing && (!shell_woods || fire_mode.rounds_per_cycle() > 1),
            guidance_blocked,
            angel_blocked,
            target_beacon: unit.has_beacon(super::BattleBeaconKind::Narc)
                || unit.has_beacon(super::BattleBeaconKind::Homing),
        },
        &mut unit.dice,
    )?;
    packets.finish_burst_glancing(fire_mode, glancing && !shell_woods);
    packets.limit_missiles(match geometry {
        HitGeometry::Direct { incoming, .. } => incoming,
        HitGeometry::Fixed(_) => None,
    });
    let mut report = BattleSalvoReport {
        initial_woods,
        woods: None,
        missiles_before_defense: None,
        cluster_roll: packets.cluster_roll,
        inferno: None,
        groups: Vec::new(),
        experience: Vec::new(),
        experience_messages: Vec::new(),
    };
    let intercepted = match geometry {
        HitGeometry::Direct { intercepted, .. } => intercepted,
        HitGeometry::Fixed(_) => 0,
    };
    if let Some((hits, surviving)) = packets.intercept(weapon, intercepted) {
        report.missiles_before_defense = Some(hits as u8);
        if mode == super::BattleAmmunitionMode::Inferno {
            if surviving > 0 {
                report.inferno = Some(super::resolve_inferno_hit(world, target, surviving)?);
            }
            return Ok(report);
        }
    }
    if (weapon.profile().missiles > 0 || mode == super::BattleAmmunitionMode::Cluster)
        && let HitGeometry::Direct {
            shooter,
            woods_damage: true,
            ..
        } = geometry
    {
        report.woods = super::woods_absorption::resolve_projectiles(
            world,
            shooter,
            target,
            weapon,
            mode,
            &mut packets,
        )?;
    }
    if shell_woods && let HitGeometry::Direct { shooter, .. } = geometry {
        report.woods = super::woods_absorption::resolve_shells(
            world,
            shooter,
            target,
            weapon,
            mode,
            &mut packets.damage,
            glancing,
        )?;
    }
    let damage_groups = packets.damage;
    let weapon_effect = if weapon == BattleWeapon::PlasmaRifle {
        Some(super::impact::WeaponEffect::Plasma)
    } else if mode == super::BattleAmmunitionMode::ArmorPiercing {
        Some(super::impact::WeaponEffect::ArmorPiercing(weapon))
    } else {
        Some(super::impact::WeaponEffect::Conventional)
    };
    for damage in damage_groups {
        let unit = &world.btech.constructed_units()[&target];
        if unit.is_destroyed() {
            break;
        }
        let (arc, partial_cover) = geometry.current(world, target)?;
        let preferred = match geometry {
            HitGeometry::Direct {
                aimed: Some(aimed), ..
            } => aimed.preferred(world, target, arc, partial_cover)?,
            _ => None,
        };
        let unit = &world.btech.constructed_units()[&target];
        let mut dice = unit.dice.clone();
        let hit = if let Some(super::BattleUnitSection::Mech(section)) = preferred {
            BattleHit {
                section,
                rear_armor: arc == BattleHitArc::Rear
                    && matches!(
                        section,
                        super::BattleSection::LeftTorso
                            | super::BattleSection::RightTorso
                            | super::BattleSection::CenterTorso
                    ),
                through_armor_critical: false,
                crew_stun: false,
            }
        } else if partial_cover {
            let section = super::BattleHitTable::Punch.location(unit.chassis(), arc, dice.d6())?;
            BattleHit {
                section,
                rear_armor: arc == BattleHitArc::Rear
                    && matches!(
                        section,
                        super::BattleSection::LeftTorso
                            | super::BattleSection::RightTorso
                            | super::BattleSection::CenterTorso
                    ),
                through_armor_critical: false,
                crew_stun: false,
            }
        } else {
            let roll = dice.generic_roll();
            rules.resolve(unit, arc, roll, &mut dice)?
        };
        world.btech.constructed.get_mut(&target).unwrap().dice = dice;
        let experience = if let SalvoEffects::Character {
            experience: Some(request),
            ..
        } = effects
        {
            request.award(world, damage)?
        } else {
            None
        };
        if let SalvoEffects::Character {
            experience: Some(request),
            ..
        } = effects
        {
            report
                .experience_messages
                .extend(request.messages(world, damage, experience.as_ref()));
        }
        let (impact, pilot_injuries, notices, pilot_notices, balance, flooding) =
            if let Some(rules) = tactical_rules {
                let attacker = match geometry {
                    HitGeometry::Direct { shooter, .. } => Some(shooter),
                    HitGeometry::Fixed(_) => None,
                };
                let result = super::impact::resolve_attack_in_candidate(
                    world,
                    target,
                    hit,
                    damage,
                    rules,
                    super::impact::AttackImpact {
                        attacker,
                        weapon_effect,
                        character,
                        followup: false,
                    },
                )?;
                (
                    result.impact,
                    result.pilot_injuries,
                    result.notices,
                    result.pilot_notices,
                    result.balance,
                    result.flooding,
                )
            } else {
                (
                    super::impact::resolve_in_candidate(
                        world,
                        target,
                        hit,
                        damage,
                        None,
                        weapon_effect,
                    )?
                    .impact,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            };
        report.experience.push(experience);
        report.groups.push(BattleSalvoGroup {
            damage,
            hit,
            impact,
            pilot_injuries,
            pilot_notices,
            notices,
            balance,
            flooding,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::BattleWeapon as W;

    #[test]
    fn snub_damage_uses_exact_range_before_glancing_rounding() {
        for (distance, damage, glancing) in [
            (0.0, 10, 5),
            (9.0, 10, 5),
            (9.001, 8, 4),
            (13.0, 8, 4),
            (13.001, 5, 3),
            (15.0, 5, 3),
            (26.0, 5, 3),
        ] {
            assert_eq!(
                W::SnubNosedPpc
                    .damage_groups_at_range(None, distance)
                    .unwrap(),
                [damage]
            );
            assert_eq!(
                W::SnubNosedPpc
                    .damage_groups_for_hit(None, true, Some(distance))
                    .unwrap(),
                [glancing]
            );
        }
        assert!(W::SnubNosedPpc.damage_groups(None).is_err());
        for distance in [-0.1, f64::NAN, f64::INFINITY] {
            assert!(
                W::SnubNosedPpc
                    .damage_groups_at_range(None, distance)
                    .is_err()
            );
        }
        // Aiming still rounds this distance into short range, while damage has already fallen.
        assert_eq!(
            W::SnubNosedPpc
                .range_modifier(9.001, false)
                .unwrap()
                .unwrap()
                .modifier,
            0
        );
    }

    /// Glancing LB-X rounds reduce pellet count while retaining one damage per pellet.
    #[test]
    fn lbx_glancing_cluster_tables() {
        for (weapon, counts) in [
            (W::Lbx2, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]),
            (W::Lbx5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Lbx10, [1, 1, 1, 1, 3, 3, 4, 6, 6, 6, 6]),
            (W::Lbx20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
        ] {
            for roll in 2..=12 {
                let groups = weapon
                    .damage_groups_for_ammunition_hit(
                        super::super::BattleAmmunitionMode::Cluster,
                        Some(roll),
                        true,
                        Some(1.0),
                    )
                    .unwrap();
                assert_eq!(groups.len(), counts[usize::from(roll - 2)]);
                assert!(groups.iter().all(|&damage| damage == 1));
            }
        }
    }

    /// Damage uses raw range while target-number brackets retain their own rounding.
    #[test]
    fn heavy_gauss_damage_range_and_glancing_boundaries() {
        for (distance, damage, glance) in [
            (0.0, 25, 13),
            (6.0, 25, 13),
            (6.001, 20, 10),
            (13.0, 20, 10),
            (13.001, 10, 5),
            (20.0, 10, 5),
        ] {
            assert_eq!(
                W::HeavyGaussRifle
                    .damage_groups_at_range(None, distance)
                    .unwrap(),
                [damage]
            );
            assert_eq!(
                W::HeavyGaussRifle
                    .damage_groups_for_hit(None, true, Some(distance))
                    .unwrap(),
                [glance]
            );
        }
        assert!(W::HeavyGaussRifle.damage_groups(None).is_err());
    }

    #[test]
    fn clan_lrm_glancing_uses_its_own_shifted_cluster_table() {
        for (weapon, counts) in [
            (W::ClanLrm10, [1, 1, 1, 1, 3, 3, 4, 6, 6, 6, 6]),
            (W::ClanLrm15, [1, 1, 1, 1, 5, 5, 6, 9, 9, 9, 9]),
        ] {
            for (i, count) in counts.into_iter().enumerate() {
                let groups = weapon
                    .damage_groups_for_hit(Some(i as u8 + 2), true, None)
                    .unwrap();
                assert_eq!(groups.iter().sum::<u16>(), count);
                assert!(groups.iter().all(|&g| (1..=5).contains(&g)));
            }
        }
    }

    #[test]
    fn thunderbolt_glancing_clusters_preserve_the_single_missile() {
        for weapon in [
            W::Thunderbolt5,
            W::Thunderbolt10,
            W::Thunderbolt15,
            W::Thunderbolt20,
        ] {
            for roll in 2..=12 {
                assert_eq!(
                    weapon
                        .damage_groups_for_hit(Some(roll), true, Some(1.0))
                        .unwrap(),
                    [u16::from(weapon.profile().damage)]
                );
            }
        }
    }

    #[test]
    fn glancing_damage_rounds_up_and_missiles_shift_clusters_without_halving_damage() {
        assert_eq!(
            W::MediumLaser
                .damage_groups_for_hit(None, true, None)
                .unwrap(),
            [3]
        );
        assert_eq!(
            W::Ac20.damage_groups_for_hit(None, true, None).unwrap(),
            [10]
        );
        for (weapon, expected) in [(W::LightAc2, 1), (W::LightAc5, 3), (W::HeavyMachineGun, 1)] {
            assert_eq!(
                weapon.damage_groups_for_hit(None, true, None).unwrap(),
                [expected]
            );
        }
        for (weapon, expected) in [
            (W::Srm2, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]),
            (W::Lrm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Rocket10, [1, 1, 1, 1, 3, 4, 4, 5, 6, 6, 6]),
            (W::Rocket15, [1, 1, 1, 1, 5, 5, 9, 9, 9, 9, 9]),
            (W::Rocket20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
            (W::Lrm10, [1, 1, 1, 1, 3, 4, 4, 5, 6, 6, 6]),
            (W::Lrm15, [1, 1, 1, 1, 5, 5, 9, 9, 9, 9, 9]),
            (W::Srm4, [1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3]),
            (W::Srm6, [1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4]),
            (W::Lrm20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
            (W::Mrm10, [1, 1, 1, 1, 2, 3, 4, 5, 6, 6, 6]),
            (W::Mrm20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
            (W::Mrm30, [1, 1, 1, 1, 10, 10, 12, 18, 18, 18, 18]),
            (W::Mrm40, [1, 1, 1, 1, 12, 12, 18, 24, 24, 24, 24]),
            (W::Elrm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Elrm10, [1, 1, 1, 1, 3, 4, 4, 5, 6, 6, 6]),
            (W::Elrm15, [1, 1, 1, 1, 5, 5, 9, 9, 9, 9, 12]),
            (W::Elrm20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
            (W::LrDfm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::LrDfm10, [1, 1, 1, 1, 3, 4, 4, 5, 6, 6, 6]),
            (W::LrDfm15, [1, 1, 1, 1, 5, 5, 9, 9, 9, 9, 12]),
            (W::LrDfm20, [1, 1, 1, 1, 6, 6, 9, 12, 12, 12, 12]),
            (W::SrDfm2, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]),
            (W::SrDfm4, [1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3]),
            (W::SrDfm6, [1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 4]),
        ] {
            for roll in 2..=12 {
                let groups = weapon
                    .damage_groups_for_hit(Some(roll), true, None)
                    .unwrap();
                assert_eq!(
                    groups.iter().sum::<u16>(),
                    expected[usize::from(roll - 2)] * u16::from(weapon.profile().damage)
                );
                assert!(groups.iter().all(|damage| *damage > 0 && *damage <= 5));
            }
            for roll in [0, 1, 13, 255] {
                assert!(
                    weapon
                        .damage_groups_for_hit(Some(roll), true, None)
                        .is_err()
                );
            }
        }
        assert!(
            W::MediumLaser
                .damage_groups_for_hit(Some(7), true, None)
                .is_err()
        );
        assert!(W::Srm4.damage_groups_for_hit(None, true, None).is_err());
    }
}

#[cfg(test)]
mod artemis_tests {
    use super::super::{BattleAmmunitionMode, BattleWeapon};

    /// Combine both modifiers before the below-table one-missile fallback and upper cap.
    #[test]
    fn artemis_glancing_table_boundaries() {
        for weapon in [BattleWeapon::Srm6, BattleWeapon::Lrm20, BattleWeapon::Mrm40] {
            for roll in 2..=12 {
                let actual = weapon
                    .damage_groups_for_ammunition_hit(
                        BattleAmmunitionMode::Artemis,
                        Some(roll),
                        true,
                        Some(8.0),
                    )
                    .unwrap();
                let hits = if roll < 4 {
                    1
                } else {
                    weapon.missile_hits(roll - 2).unwrap()
                };
                assert_eq!(
                    actual.iter().sum::<u16>(),
                    u16::from(hits) * u16::from(weapon.profile().damage)
                );
            }
        }
        assert_eq!(
            BattleWeapon::StreakSrm6
                .damage_groups_for_ammunition_hit(
                    BattleAmmunitionMode::Artemis,
                    Some(2),
                    true,
                    Some(8.0)
                )
                .unwrap()
                .iter()
                .sum::<u16>(),
            12
        );
    }
}

#[cfg(test)]
mod flechette_tests {
    use super::*;
    /// Odd shell damage is rounded down before a normal glancing hit rounds its half up.
    #[test]
    fn flechette_armored_damage_precedes_glancing() {
        for (weapon, normal, glancing) in [
            (BattleWeapon::Ac2, 1, 1),
            (BattleWeapon::Ac5, 2, 1),
            (BattleWeapon::Ac10, 5, 3),
            (BattleWeapon::Ac20, 10, 5),
            (BattleWeapon::LightAc2, 1, 1),
            (BattleWeapon::LightAc5, 2, 1),
        ] {
            for (glance, damage) in [(false, normal), (true, glancing)] {
                assert_eq!(
                    weapon
                        .damage_groups_for_ammunition_hit(
                            super::super::BattleAmmunitionMode::Flechette,
                            None,
                            glance,
                            Some(1.0)
                        )
                        .unwrap(),
                    vec![damage]
                );
            }
        }
    }
}
