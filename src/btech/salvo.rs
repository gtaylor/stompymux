//! Conventional missile-cluster tables and atomic grouped weapon-hit resolution.
use super::{FlechetteDamage, Hit, HitArc, ImpactReport, Weapon};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// The Total Warfare Cluster Hits Table, rows indexed by the number of missiles or shots
/// and columns by the 2d6 roll: every column from 2 to 30, and 40.
const CLUSTER_HITS: &[(u8, [u8; 11])] = &[
    (2, [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]),
    (3, [1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3]),
    (4, [1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4]),
    (5, [1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5]),
    (6, [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6]),
    (7, [2, 2, 3, 4, 4, 4, 4, 6, 6, 7, 7]),
    (8, [3, 3, 4, 4, 5, 5, 5, 6, 6, 8, 8]),
    (9, [3, 3, 4, 5, 5, 5, 5, 7, 7, 9, 9]),
    (10, [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10]),
    (11, [4, 4, 5, 7, 7, 7, 7, 9, 9, 11, 11]),
    (12, [4, 4, 5, 8, 8, 8, 8, 10, 10, 12, 12]),
    (13, [4, 4, 5, 8, 8, 8, 8, 11, 11, 13, 13]),
    (14, [5, 5, 6, 9, 9, 9, 9, 11, 11, 14, 14]),
    (15, [5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15]),
    (16, [5, 5, 7, 10, 10, 10, 10, 13, 13, 16, 16]),
    (17, [5, 5, 7, 10, 10, 10, 10, 14, 14, 17, 17]),
    (18, [6, 6, 8, 11, 11, 11, 11, 14, 14, 18, 18]),
    (19, [6, 6, 8, 11, 11, 11, 11, 15, 15, 19, 19]),
    (20, [6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20]),
    (21, [7, 7, 9, 13, 13, 13, 13, 17, 17, 21, 21]),
    (22, [7, 7, 9, 14, 14, 14, 14, 18, 18, 22, 22]),
    (23, [7, 7, 10, 15, 15, 15, 15, 19, 19, 23, 23]),
    (24, [8, 8, 10, 16, 16, 16, 16, 20, 20, 24, 24]),
    (25, [8, 8, 10, 16, 16, 16, 16, 21, 21, 25, 25]),
    (26, [9, 9, 11, 17, 17, 17, 17, 21, 21, 26, 26]),
    (27, [9, 9, 11, 17, 17, 17, 17, 22, 22, 27, 27]),
    (28, [9, 9, 11, 17, 17, 17, 17, 23, 23, 28, 28]),
    (29, [10, 10, 12, 18, 18, 18, 18, 23, 23, 29, 29]),
    (30, [10, 10, 12, 18, 18, 18, 18, 24, 24, 30, 30]),
    (40, [12, 12, 18, 24, 24, 24, 24, 32, 32, 40, 40]),
];

/// Read one Cluster Hits Table cell for a rack size and an already modified 2d6 roll.
pub fn cluster_hits(size: u8, roll: u8) -> Result<u8> {
    ensure!((2..=12).contains(&roll), "Invalid missile cluster roll");
    let (_, row) = CLUSTER_HITS
        .iter()
        .find(|(rack, _)| *rack == size)
        .context("Missing cluster table row")?;
    Ok(row[usize::from(roll - 2)])
}

/// Guidance, defense and range facts that modify a missile or cluster hit's packet sizes.
#[derive(Debug, Clone, Copy)]
pub struct GuidedHit {
    pub mode: super::AmmunitionMode,
    pub cluster_roll: Option<u8>,
    /// A glancing blow halves direct damage, or shifts the cluster roll down four.
    pub glancing: bool,
    /// Actual spatial range, which halves Thunderbolt damage and the extended LRM cluster column
    /// inside their minimum ranges.
    pub distance: Option<f64>,
    /// Artemis V guidance adds three to the Artemis cluster bonus instead of two.
    pub artemis_v: bool,
    /// ECM blocks the ATM's integral guidance bonus.
    pub guidance_blocked: bool,
    /// An anti-missile system engaged the flight (Total Warfare p. 129): -4 on the cluster roll,
    /// never below 2, with a locked Streak launcher treated as a roll of 11.
    pub ams_engaged: bool,
    /// Electromagnetic interference covers the attacker: -2 on the cluster roll, never below 2
    /// (Tactical Operations: Advanced Rules p. 53).
    pub interference: bool,
}

/// Missile cluster hits and per-hit damage grouping for every weapon.
pub trait WeaponSalvo {
    /// Resolve an unmodified 2d6 cluster roll for a supported conventional launcher.
    fn missile_hits(self, roll: u8) -> Result<u8>;

    /// Each group receives an independent location.
    fn damage_groups(self, cluster_roll: Option<u8>) -> Result<Vec<u16>>;

    /// Group a hit using its actual spatial range, before any aim-bracket rounding.
    fn damage_groups_at_range(self, cluster_roll: Option<u8>, distance: f64) -> Result<Vec<u16>>;

    /// Glancing direct hits halve damage rounding down; missiles instead shift the cluster
    /// roll down four (Tactical Operations p. 79).
    fn damage_groups_for_hit(
        self,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>>;

    /// Resolve packet sizes for a selected ammunition type at the actual attack range.
    fn damage_groups_for_ammunition(
        self,
        mode: super::AmmunitionMode,
        cluster_roll: Option<u8>,
        distance: f64,
    ) -> Result<Vec<u16>>;

    /// Cluster rounds resolve individual pellets; slug rounds retain conventional direct damage.
    fn damage_groups_for_ammunition_hit(
        self,
        mode: super::AmmunitionMode,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>>;

    /// As [`Self::damage_groups_for_ammunition_hit`], with Artemis V guidance adding one more
    /// to the Artemis cluster bonus, ATMs adding their own guidance bonus unless ECM blocks it,
    /// and an engaged anti-missile system shifting the cluster roll.
    fn damage_groups_for_guided_hit(self, hit: GuidedHit) -> Result<Vec<u16>>;
}

impl WeaponSalvo for Weapon {
    fn missile_hits(self, roll: u8) -> Result<u8> {
        ensure!((2..=12).contains(&roll), "Invalid missile cluster roll");
        if self.is_streak() || self.is_thunderbolt() || self.is_narc() || self == Self::INarcBeacon
        {
            return Ok(self.profile().missiles);
        }
        let size = match self {
            Self::ClanSrm2
            | Self::Srm2
            | Self::ClanSrt2
            | Self::Srt2
            | Self::ClanLbx2
            | Self::Lbx2 => 2,
            Self::Mml3 | Self::ClanAtm3 => 3,
            Self::ClanSrm4 | Self::Srm4 | Self::ClanSrt4 | Self::Srt4 => 4,
            Self::Mml5
            | Self::ClanLrm5
            | Self::Lrm5
            | Self::ClanLrt5
            | Self::Lrt5
            | Self::Nlrm5
            | Self::Elrm5
            | Self::ClanLbx5
            | Self::Lbx5 => 5,
            Self::ClanSrm6 | Self::Srm6 | Self::ClanSrt6 | Self::Srt6 | Self::ClanAtm6 => 6,
            Self::Mml7 => 7,
            Self::Mml9 | Self::ClanAtm9 => 9,
            Self::ClanLrm10
            | Self::ClanLrt10
            | Self::Lrm10
            | Self::Lrt10
            | Self::Nlrm10
            | Self::Rocket10
            | Self::Elrm10
            | Self::ClanLbx10
            | Self::Lbx10
            | Self::Mrm10 => 10,
            Self::ClanAtm12 => 12,
            Self::ClanLrm15
            | Self::ClanLrt15
            | Self::Lrm15
            | Self::Lrt15
            | Self::Nlrm15
            | Self::Rocket15
            | Self::Elrm15 => 15,
            Self::ClanLrm20
            | Self::ClanLrt20
            | Self::Lrt20
            | Self::Rocket20
            | Self::Lrm20
            | Self::Nlrm20
            | Self::Mrm20
            | Self::Elrm20
            | Self::ClanLbx20
            | Self::Lbx20 => 20,
            Self::Mrm30 | Self::ClanHag30 => 30,
            Self::Mrm40 | Self::ClanHag40 => 40,
            Self::ClanHag20 => 20,
            _ => bail!("Weapon is not a missile launcher"),
        };
        cluster_hits(size, roll)
    }

    fn damage_groups(self, cluster_roll: Option<u8>) -> Result<Vec<u16>> {
        self.damage_groups_for_hit(cluster_roll, false, None)
    }

    fn damage_groups_at_range(self, cluster_roll: Option<u8>, distance: f64) -> Result<Vec<u16>> {
        self.damage_groups_for_hit(cluster_roll, false, Some(distance))
    }

    fn damage_groups_for_hit(
        self,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_ammunition_hit(
            super::AmmunitionMode::Normal,
            cluster_roll,
            glancing,
            distance,
        )
    }

    fn damage_groups_for_ammunition(
        self,
        mode: super::AmmunitionMode,
        cluster_roll: Option<u8>,
        distance: f64,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_ammunition_hit(mode, cluster_roll, false, Some(distance))
    }

    fn damage_groups_for_ammunition_hit(
        self,
        mode: super::AmmunitionMode,
        cluster_roll: Option<u8>,
        glancing: bool,
        distance: Option<f64>,
    ) -> Result<Vec<u16>> {
        self.damage_groups_for_guided_hit(GuidedHit {
            mode,
            cluster_roll,
            glancing,
            distance,
            artemis_v: false,
            guidance_blocked: false,
            ams_engaged: false,
            interference: false,
        })
    }

    fn damage_groups_for_guided_hit(self, hit: GuidedHit) -> Result<Vec<u16>> {
        let GuidedHit {
            mode,
            cluster_roll,
            glancing,
            distance,
            artemis_v,
            guidance_blocked,
            ams_engaged,
            interference,
        } = hit;
        ensure!(!self.is_artillery(), "Artillery damage requires an arrival");
        ensure!(
            mode.supports(self),
            "{}",
            match mode {
                super::AmmunitionMode::Cluster => "Weapon does not support cluster ammunition",
                super::AmmunitionMode::SemiGuided =>
                    "Weapon does not support semi-guided ammunition",
                super::AmmunitionMode::Artemis => "Weapon does not support Artemis ammunition",
                _ => "Weapon does not support specialized autocannon ammunition",
            }
        );
        ensure!(
            mode != super::AmmunitionMode::Inferno,
            "Inferno hits use burn exposure instead of armor damage groups"
        );
        let cluster = mode == super::AmmunitionMode::Cluster;
        let artemis = mode.munition() == super::AmmunitionMode::Artemis;
        if let Some(distance) = distance {
            ensure!(
                distance.is_finite() && distance >= 0.0,
                "Invalid weapon damage range"
            );
        }
        if self.profile().missiles == 0 && !cluster && !self.is_hag() {
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
            if !glancing {
                return Ok(vec![damage]);
            }
            // A one-point round glances off without effect.
            let damage = damage / 2;
            return Ok(if damage == 0 {
                Vec::new()
            } else {
                vec![damage]
            });
        }
        let roll = cluster_roll.context("Missile launcher requires a cluster roll")?;
        ensure!((2..=12).contains(&roll), "Invalid missile cluster roll");
        let artemis_bonus = match (artemis, artemis_v) {
            (true, true) => 3,
            (true, false) => 2,
            (false, _) => 0,
        };
        let atm_bonus = if self.is_atm() && !guidance_blocked {
            super::atm::ATM_CLUSTER_BONUS
        } else {
            0
        };
        // A hyper-assault Gauss rifle adds two to its cluster roll at short range and subtracts
        // two at long range (Total Warfare p. 136).
        // Brackets round the distance as weapon ranges do.
        let hag_bonus = match distance
            .filter(|_| self.is_hag())
            .map(|distance| (distance + 0.95).floor())
        {
            Some(range) if range > f64::from(self.profile().medium_range) => -2,
            Some(range) if range <= f64::from(self.profile().short_range) => 2,
            _ => 0,
        };
        // Neither the glancing modifier (Tactical Operations p. 79), the AMS modifier (Total
        // Warfare p. 129) nor electromagnetic interference (Tactical Operations: Advanced Rules
        // p. 53) takes the cluster roll below 2.
        let ams_penalty = if ams_engaged { 4 } else { 0 };
        let interference_penalty = if interference {
            i16::from(super::planetary_conditions::INTERFERENCE_CLUSTER_PENALTY)
        } else {
            0
        };
        let adjusted = (i16::from(roll) + artemis_bonus + atm_bonus + hag_bonus
            - if glancing { 4 } else { 0 }
            - ams_penalty
            - interference_penalty)
            .clamp(2, 12) as u8;
        let inside_minimum = distance.is_some_and(|distance| {
            distance < f64::from(self.profile_for_ammunition(mode).minimum_range)
        });
        let mut hits = if self.is_streak() {
            // An engaged AMS treats a locked Streak launcher as a roll of 11 before its -4, and
            // electromagnetic interference takes its two from that roll as well. Without an
            // engagement a locked Streak makes no Cluster Hits roll at all: every missile hits.
            if ams_engaged {
                cluster_hits(
                    self.profile().missiles,
                    (11 - 4 - interference_penalty) as u8,
                )?
            } else {
                self.profile().missiles
            }
        } else if matches!(
            self,
            Self::Elrm5 | Self::Elrm10 | Self::Elrm15 | Self::Elrm20
        ) && inside_minimum
        {
            // Boosters still burning: the column for half the rack, rounded up (TacOps: AUE p. 139).
            cluster_hits(self.profile().missiles.div_ceil(2), adjusted)?
        } else {
            self.missile_hits(adjusted)?
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
                    | Self::Nlrm5
                    | Self::Nlrm10
                    | Self::Nlrm15
                    | Self::Nlrm20
                    | Self::Lrt5
                    | Self::Lrt10
                    | Self::Lrt15
                    | Self::Lrt20
                    | Self::ClanLrt5
                    | Self::ClanLrt10
                    | Self::ClanLrt15
                    | Self::ClanLrt20
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
                    | Self::ClanStreakLrm5
                    | Self::ClanStreakLrm10
                    | Self::ClanStreakLrm15
                    | Self::ClanStreakLrm20
                    | Self::ClanHag20
                    | Self::ClanHag30
                    | Self::ClanHag40
            ) {
            5
        } else {
            1
        };
        let damage = if cluster || self.is_hag() {
            1
        } else if self.is_thunderbolt() && inside_minimum {
            // The warhead has not armed yet: half damage, rounded down (TacOps: AUE p. 159).
            u16::from(self.profile().damage) / 2
        } else {
            u16::from(self.profile_for_ammunition(mode).damage)
        };
        let mut groups = Vec::new();
        if self.is_atm() {
            // ATM damage lands in five-point clusters whatever the missile payload.
            let mut remaining = u16::from(hits) * damage;
            while remaining > 0 {
                let group = remaining.min(5);
                groups.push(group);
                remaining -= group;
            }
            return Ok(groups);
        }
        while hits > 0 {
            let count = hits.min(group_size);
            groups.push(u16::from(count) * damage);
            hits -= count;
        }
        Ok(groups)
    }
}

/// One independently located damage group in a successful salvo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SalvoGroup {
    pub damage: u16,
    pub hit: Hit,
    pub impact: ImpactReport,
    /// Applied by the tactical resolver; fall injuries are nested in the balance report.
    pub pilot_injuries: Vec<super::TacticalPilotInjury>,
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<super::Notice>,
    /// Balance consequences completed before the next damage group.
    pub balance: Vec<super::BalanceReport>,
    pub flooding: Vec<super::SectionExposureReport>,
}

/// Rolled cluster size and applied groups; resolution stops when the target is destroyed.
/// `G` is the chassis's group result and `I` its inferno result. [`MechSalvoReport`]
/// and [`super::VehicleSalvoReport`] name it for each chassis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SalvoReport<G, I> {
    /// The building around a target inside one, which took its share of every grouping.
    pub building: Option<super::BuildingCover>,
    /// The building's collapse once the groupings exhausted it.
    pub collapse: Option<super::SurfaceBreak>,
    /// Missile hits before an anti-missile engagement shifted the cluster roll.
    pub missiles_before_defense: Option<u8>,
    /// Hits the engagement removed from the Cluster Hits result.
    pub missiles_intercepted: Option<u8>,
    pub cluster_roll: Option<u8>,
    pub inferno: Option<I>,
    pub groups: Vec<G>,
    pub experience: Vec<Option<super::ShotExperienceAward>>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
}

/// A salvo's effects on a BattleMech.
pub type MechSalvoReport = SalvoReport<SalvoGroup, super::InfernoHit>;

/// Effects that the enclosing action can apply and publish between weapon groups.
#[derive(Clone, Copy)]
enum SalvoEffects<'a> {
    Material,
    Tactical(super::FallRules),
    Character {
        rules: super::FallRules,
        experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
    },
}

/// Resolve grouped character damage within a casualty-publishing host action.
pub(super) fn resolve_salvo_in_action(
    world: &mut World,
    target: ObjectId,
    weapon: Weapon,
    arc: HitArc,
    rules: super::FallRules,
) -> Result<MechSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::AmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
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
    weapon: Weapon,
    arc: HitArc,
) -> Result<MechSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::AmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
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
    weapon: Weapon,
    arc: HitArc,
    rules: super::FallRules,
) -> Result<MechSalvoReport> {
    world.attempt(|world| {
        resolve_salvo_with_effects(
            world,
            target,
            (weapon, super::AmmunitionMode::Normal),
            HitGeometry::Fixed(arc),
            SalvoEffects::Tactical(rules),
            false,
        )
    })
}

/// Damage policy and pre-impact XP facts supplied by the enclosing direct shot.
pub(super) struct ShotDamage<'a> {
    pub aimed: Option<super::aimed_hit::AimedShot>,
    pub incoming: Option<u8>,
    pub rules: super::FallRules,
    pub glancing: bool,
    pub character: bool,
    /// An anti-missile system engaged the flight, shifting the cluster roll.
    pub ams_engaged: bool,
    /// Spotter-directed indirect fire gets no Artemis or Narc cluster bonus.
    pub indirect: bool,
    /// An MRM "Apollo" fire-control system guided the launcher.
    pub apollo: bool,
    /// Machine guns fired together as a machine gun array.
    pub array: Option<u8>,
    pub experience: Option<super::gunnery_experience::GunneryAwardContext<'a>>,
}

/// Recompute impact geometry and apply character effects and XP between direct-shot groups.
pub(super) fn resolve_salvo_from_shot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: impl Into<SalvoWeapon>,
    damage: ShotDamage<'_>,
) -> Result<MechSalvoReport> {
    world.attempt(|world| resolve_salvo_in_candidate(world, shooter, target, weapon, damage))
}

/// Apply directly to an enclosing shot; every failure aborts that shot.
pub(super) fn resolve_salvo_in_candidate(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: impl Into<SalvoWeapon>,
    damage: ShotDamage<'_>,
) -> Result<MechSalvoReport> {
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
            aimed: damage.aimed,
            shooter,
            fire_mode: weapon.fire_mode,
            gatling_damage: weapon.gatling_damage,
            ams_engaged: damage.ams_engaged,
            indirect: damage.indirect,
            apollo: damage.apollo,
            array: damage.array,
            incoming: damage.incoming,
        },
        effects,
        damage.glancing,
    )
}

/// Shared launch facts consumed by target damage, independent of shooter inventory representation.
#[derive(Clone, Copy)]
pub(super) struct SalvoWeapon {
    pub(super) weapon: Weapon,
    pub(super) ammunition_mode: super::AmmunitionMode,
    pub(super) fire_mode: super::FireMode,
    pub(super) gatling_damage: Option<u8>,
}

impl From<&super::WeaponUse> for SalvoWeapon {
    fn from(weapon: &super::WeaponUse) -> Self {
        Self {
            weapon: weapon.weapon,
            ammunition_mode: weapon.ammunition_mode,
            fire_mode: weapon.fire_mode,
            gatling_damage: weapon.gatling_damage,
        }
    }
}

impl From<&super::VehicleWeaponUse> for SalvoWeapon {
    fn from(weapon: &super::VehicleWeaponUse) -> Self {
        Self {
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
    Fixed(HitArc),
    Direct {
        aimed: Option<super::aimed_hit::AimedShot>,
        shooter: ObjectId,
        fire_mode: super::FireMode,
        gatling_damage: Option<u8>,
        ams_engaged: bool,
        indirect: bool,
        apollo: bool,
        array: Option<u8>,
        incoming: Option<u8>,
    },
}

impl HitGeometry {
    /// Falls rotate the target and remove standing partial cover before later groups land.
    fn current(self, world: &World, target: ObjectId) -> Result<(HitArc, bool)> {
        let shooter = match self {
            Self::Fixed(arc) => return Ok((arc, false)),
            Self::Direct { shooter, .. } => shooter,
        };
        let arc = super::hit_direction::HitDirection::Direct { shooter }.current(world, target)?;
        Ok((
            arc,
            super::unit_terrain_los(world, shooter, target)?.partial_cover,
        ))
    }
}

/// The building hex that gives a standing target partial cover from this shooter: the hex
/// next to the target along the line of sight (Total Warfare p. 102). Cover from a hill, water
/// or the building the target stands inside is terrain and takes no damage.
fn cover_building(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<Option<super::HexCoordinate>> {
    let Some(from) = super::scanner::scanner_unit(world, shooter).and_then(|unit| unit.position)
    else {
        return Ok(None);
    };
    let Some(to) = world.btech.constructed_units()[&target].position() else {
        return Ok(None);
    };
    if from.map != to.map {
        return Ok(None);
    }
    let hex = |position: super::Position| super::HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let cells = super::los_trace::center_trace(hex(from), hex(to))?;
    let Some(&cover) = cells
        .len()
        .checked_sub(2)
        .and_then(|index| cells.get(index))
    else {
        return Ok(None);
    };
    let map = &world.btech.maps()[&to.map];
    let tile = map.base_hex(i64::from(cover.x), i64::from(cover.y))?;
    Ok(tile.has_building().then_some(cover))
}

/// Shared cluster and hit-location traversal prevents divergence between damage compositions.
fn resolve_salvo_with_effects(
    world: &mut World,
    target: ObjectId,
    weapon: (Weapon, super::AmmunitionMode),
    geometry: HitGeometry,
    effects: SalvoEffects<'_>,
    glancing: bool,
) -> Result<MechSalvoReport> {
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
            Some(super::unit_range(world, shooter, target)?.hexes())
        }
    };
    let artemis_v = match geometry {
        HitGeometry::Fixed(_) => false,
        HitGeometry::Direct { shooter, .. } => super::artemis::artemis_v(world, shooter),
    };
    // Electromagnetic interference over the attacker lowers its Cluster Hits roll; an explicitly
    // supplied direction has no attacker.
    let interference = match geometry {
        HitGeometry::Fixed(_) => false,
        HitGeometry::Direct { shooter, .. } => {
            super::planetary_conditions::under_interference(world, shooter)
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
    // A plasma cannon deals a Mech no damage, only heat, which the direct shot's effects apply
    // (Total Warfare p. 139).
    if weapon == Weapon::ClanPlasmaCannon {
        return Ok(MechSalvoReport {
            building: None,
            collapse: None,
            missiles_before_defense: None,
            missiles_intercepted: None,
            cluster_roll: None,
            inferno: None,
            groups: Vec::new(),
            experience: Vec::new(),
            experience_messages: Vec::new(),
        });
    }
    // Missiles still flying in a Swarm flight, and whether an anti-missile system engaged it.
    let swarm = match geometry {
        HitGeometry::Direct { incoming, .. } => incoming,
        HitGeometry::Fixed(_) => None,
    };
    let ams_engaged = matches!(
        geometry,
        HitGeometry::Direct {
            ams_engaged: true,
            ..
        }
    );
    let unit = world.btech.constructed.get_mut(&target).unwrap();
    let fire_mode = match geometry {
        HitGeometry::Direct { fire_mode, .. } => fire_mode,
        HitGeometry::Fixed(_) => super::FireMode::Normal,
    };
    let gatling_damage = match geometry {
        HitGeometry::Direct { gatling_damage, .. } => gatling_damage,
        HitGeometry::Fixed(_) => None,
    };
    let mut packets = super::weapon_groups::roll_weapon_groups(
        super::weapon_groups::WeaponGroupRequest {
            weapon,
            ammunition: mode,
            fire_mode,
            gatling_damage,
            distance,
            glancing,
            guidance_blocked,
            angel_blocked,
            target_beacon: unit.has_beacon(super::BeaconKind::Narc)
                || unit.has_beacon(super::BeaconKind::Homing),
            artemis_v,
            // A Swarm flight meets an anti-missile system on its own terms below.
            ams_engaged: swarm.is_none() && ams_engaged,
            indirect: matches!(geometry, HitGeometry::Direct { indirect: true, .. }),
            apollo: matches!(geometry, HitGeometry::Direct { apollo: true, .. }),
            array: match geometry {
                HitGeometry::Direct { array, .. } => array,
                HitGeometry::Fixed(_) => None,
            },
            unit_without_heat: false,
            interference,
        },
        &mut unit.dice,
    )?;
    if let Some(incoming) = swarm {
        packets.swarm_flight(incoming, ams_engaged)?;
    }
    let mut report = MechSalvoReport {
        building: None,
        collapse: None,
        missiles_before_defense: None,
        missiles_intercepted: None,
        cluster_roll: packets.cluster_roll,
        inferno: None,
        groups: Vec::new(),
        experience: Vec::new(),
        experience_messages: Vec::new(),
    };
    if weapon.profile().missiles > 0 {
        report.missiles_before_defense = packets.missiles_before_defense;
        report.missiles_intercepted = packets.missiles_intercepted;
        if mode == super::AmmunitionMode::Inferno {
            let surviving = packets.missile_count();
            if surviving > 0 {
                report.inferno = Some(super::resolve_inferno_hit(world, target, surviving)?);
            }
            return Ok(report);
        }
    }
    let building = super::building_interior::absorb(
        world,
        match geometry {
            HitGeometry::Direct { shooter, .. } => Some(shooter),
            HitGeometry::Fixed(_) => None,
        },
        target,
        &mut packets.damage,
    )?;
    let damage_groups = packets.damage;
    // A machine gun array's hits all strike the location its first hit found.
    let mut shared_hit: Option<Hit> = None;
    let weapon_effect = if weapon == Weapon::PlasmaRifle {
        Some(super::impact::WeaponEffect::Plasma)
    } else if mode == super::AmmunitionMode::ArmorPiercing {
        Some(super::impact::WeaponEffect::ArmorPiercing(weapon))
    } else if weapon.is_energy() {
        Some(super::impact::WeaponEffect::Energy)
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
        let hit = if let Some(hit) = shared_hit.filter(|_| packets.single_location) {
            hit
        } else if let Some(super::UnitSection::Mech(section)) = preferred {
            Hit {
                section,
                rear_armor: arc == HitArc::Rear
                    && matches!(
                        section,
                        super::MechSection::LeftTorso
                            | super::MechSection::RightTorso
                            | super::MechSection::CenterTorso
                    ),
                through_armor_critical: false,
            }
        } else {
            let roll = dice.generic_roll();
            super::resolve_hit(unit, arc, roll, &mut dice)?
        };
        if packets.single_location && shared_hit.is_none() {
            shared_hit = Some(hit);
        }
        let covered_legs = partial_cover && unit.chassis().is_leg(hit.section);
        world.btech.constructed.get_mut(&target).unwrap().dice = dice;
        if covered_legs {
            // Partial cover: a leg hit strikes the cover instead (Total Warfare p. 102). A
            // building takes the damage; a hill, water or other terrain simply absorbs it.
            let mut notices = vec![super::Notice {
                unit: target,
                text: "The shot strikes the cover in front of your legs!".into(),
            }];
            if let HitGeometry::Direct { shooter, .. } = geometry
                && let Some(coordinate) = cover_building(world, shooter, target)?
                && let Some(building) =
                    super::building_damage::resolve(world, shooter, coordinate, damage)?
            {
                notices.extend(building.notices);
            }
            report.experience.push(None);
            report.groups.push(SalvoGroup {
                damage,
                hit,
                impact: super::impact::ImpactReport::default(),
                pilot_injuries: Vec::new(),
                pilot_notices: Vec::new(),
                notices,
                balance: Vec::new(),
                flooding: Vec::new(),
            });
            continue;
        }
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
                let result = super::impact::resolve_attack_in_candidate_with_penalty(
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
                    if glancing {
                        super::impact::GLANCING_CRITICAL_PENALTY
                    } else {
                        0
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
        report.groups.push(SalvoGroup {
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
    report.building = building;
    if let Some(cover) = &report.building {
        let rules = tactical_rules.unwrap_or(super::FallRules::STANDARD);
        report.collapse = super::building_interior::collapse(world, target, cover, rules)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::Weapon as W;
    use super::cluster_hits;
    use crate::btech::{GuidedHit, WeaponSalvo};

    #[test]
    fn snub_damage_uses_exact_range_before_glancing_rounding() {
        for (distance, damage, glancing) in [
            (0.0, 10, 5),
            (9.0, 10, 5),
            (9.001, 8, 4),
            (13.0, 8, 4),
            (13.001, 5, 2),
            (15.0, 5, 2),
            (26.0, 5, 2),
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
                .range_modifier(9.001)
                .unwrap()
                .unwrap()
                .modifier,
            0
        );
    }

    /// Glancing LB-X rounds shift the cluster roll down four, never below row 2, while
    /// retaining one damage per pellet.
    #[test]
    fn lbx_glancing_cluster_tables() {
        for (weapon, counts) in [
            (W::Lbx2, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]),
            (W::Lbx5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Lbx10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Lbx20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
        ] {
            for roll in 2..=12 {
                let groups = weapon
                    .damage_groups_for_ammunition_hit(
                        super::super::AmmunitionMode::Cluster,
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
            (0.0, 25, 12),
            (6.0, 25, 12),
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
            (W::ClanLrm10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::ClanLrm15, [5, 5, 5, 5, 5, 5, 6, 9, 9, 9, 9]),
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

    /// A Thunderbolt always lands its single missile; glancing never removes it, and inside the
    /// launcher's minimum range the unarmed warhead does half damage, rounded down
    /// (Tactical Operations: AUE p. 159).
    #[test]
    fn thunderbolt_glancing_clusters_preserve_the_single_missile() {
        for weapon in [
            W::Thunderbolt5,
            W::Thunderbolt10,
            W::Thunderbolt15,
            W::Thunderbolt20,
        ] {
            let minimum = f64::from(weapon.profile().minimum_range);
            assert_eq!(minimum, 5.0);
            for roll in 2..=12 {
                for glancing in [false, true] {
                    let damage = u16::from(weapon.profile().damage);
                    for (distance, expected) in [
                        (0.0, damage / 2),
                        (4.999, damage / 2),
                        (5.0, damage),
                        (12.0, damage),
                    ] {
                        assert_eq!(
                            weapon
                                .damage_groups_for_hit(Some(roll), glancing, Some(distance))
                                .unwrap(),
                            [expected],
                            "{weapon:?} at {distance}"
                        );
                    }
                    assert_eq!(
                        weapon
                            .damage_groups_for_hit(Some(roll), glancing, None)
                            .unwrap(),
                        [damage]
                    );
                }
            }
        }
    }

    /// Inside ten hexes an extended LRM rolls on the Cluster Hits column for half its rack,
    /// rounded up (Tactical Operations: AUE p. 139).
    #[test]
    fn extended_lrms_use_half_the_rack_inside_minimum_range() {
        for (weapon, column) in [
            (W::Elrm5, 3),
            (W::Elrm10, 5),
            (W::Elrm15, 8),
            (W::Elrm20, 10),
        ] {
            assert_eq!(weapon.profile().minimum_range, 10);
            for roll in 2..=12 {
                let close: u16 = weapon
                    .damage_groups_for_hit(Some(roll), false, Some(9.999))
                    .unwrap()
                    .iter()
                    .sum();
                assert_eq!(close, u16::from(cluster_hits(column, roll).unwrap()));
                let far: u16 = weapon
                    .damage_groups_for_hit(Some(roll), false, Some(10.0))
                    .unwrap()
                    .iter()
                    .sum();
                assert_eq!(far, u16::from(weapon.missile_hits(roll).unwrap()));
            }
        }
        assert_eq!(cluster_hits(8, 2).unwrap(), 3);
        assert_eq!(cluster_hits(8, 7).unwrap(), 5);
        assert_eq!(cluster_hits(8, 12).unwrap(), 8);
        // The table has every column from 2 to 30, and 40.
        assert_eq!(cluster_hits(11, 7).unwrap(), 7);
        assert_eq!(cluster_hits(16, 9).unwrap(), 13);
        assert!(cluster_hits(31, 7).is_err());
        assert!(cluster_hits(1, 7).is_err());
    }

    /// An engaged AMS shifts the cluster roll down four, never below 2, and treats a locked
    /// Streak launcher as a roll of 11 (Total Warfare p. 129).
    #[test]
    fn ams_engagement_shifts_cluster_rolls_and_streak_flights() {
        use super::super::AmmunitionMode as Mode;
        let hit = |weapon: W, roll, ams_engaged, glancing| {
            weapon
                .damage_groups_for_guided_hit(GuidedHit {
                    mode: Mode::Normal,
                    cluster_roll: Some(roll),
                    glancing,
                    distance: Some(7.0),
                    artemis_v: false,
                    guidance_blocked: false,
                    ams_engaged,
                    interference: false,
                })
                .unwrap()
                .iter()
                .sum::<u16>()
        };
        for roll in 2..=12u8 {
            let shifted = roll.saturating_sub(4).max(2);
            assert_eq!(
                hit(W::Lrm20, roll, true, false),
                hit(W::Lrm20, shifted, false, false)
            );
            // Glancing and the AMS shift stack, still floored at 2.
            let doubled = roll.saturating_sub(8).max(2);
            assert_eq!(
                hit(W::Srm6, roll, true, true),
                2 * u16::from(W::Srm6.missile_hits(doubled).unwrap())
            );
            assert_eq!(hit(W::StreakSrm6, roll, true, false), 2 * 4);
            assert_eq!(hit(W::StreakSrm2, roll, true, false), 2);
            assert_eq!(hit(W::ClanStreakLrm15, roll, true, false), 9);
            assert_eq!(hit(W::StreakSrm6, roll, false, false), 12);
        }
    }

    /// Electromagnetic interference takes two from every Cluster Hits roll, whatever the
    /// launcher, summed with guidance bonuses, the range shift of a HAG and an AMS engagement
    /// before the roll is floored at 2 (Tactical Operations: Advanced Rules p. 53).
    #[test]
    fn interference_lowers_every_cluster_roll() {
        use super::super::AmmunitionMode as Mode;
        let hit = |weapon: W, mode, roll, distance, ams_engaged, interference| {
            weapon
                .damage_groups_for_guided_hit(GuidedHit {
                    mode,
                    cluster_roll: Some(roll),
                    glancing: false,
                    distance: Some(distance),
                    artemis_v: false,
                    guidance_blocked: false,
                    ams_engaged,
                    interference,
                })
                .unwrap()
                .iter()
                .sum::<u16>()
        };
        for roll in 2..=12u8 {
            let lowered = roll.saturating_sub(2).max(2);
            let hits = |weapon: W, roll| u16::from(weapon.missile_hits(roll).unwrap());
            assert_eq!(
                hit(W::Lrm20, Mode::Normal, roll, 7.0, false, true),
                hits(W::Lrm20, lowered),
                "roll {roll}"
            );
            // LB-X cluster pellets read the same table.
            assert_eq!(
                hit(W::Lbx10, Mode::Cluster, roll, 7.0, false, true),
                hits(W::Lbx10, lowered)
            );
            // Artemis's +2 cancels the -2 before the floor, so even a 2 reads as a 2.
            assert_eq!(
                hit(W::Lrm20, Mode::Artemis, roll, 7.0, false, true),
                hits(W::Lrm20, roll)
            );
            // An AMS engagement and the interference stack.
            assert_eq!(
                hit(W::Lrm20, Mode::Normal, roll, 7.0, true, true),
                hits(W::Lrm20, roll.saturating_sub(6).max(2))
            );
            // A HAG's +2 at short range cancels the interference; at medium range it bites.
            assert_eq!(
                hit(W::ClanHag20, Mode::Normal, roll, 8.0, false, true),
                hits(W::ClanHag20, roll)
            );
            assert_eq!(
                hit(W::ClanHag20, Mode::Normal, roll, 9.0, false, true),
                hits(W::ClanHag20, lowered)
            );
            // An engaged Streak reads 11 - 4 - 2; a locked one rolls no table and lands all.
            assert_eq!(
                hit(W::StreakSrm6, Mode::Normal, roll, 7.0, true, true),
                2 * 3
            );
            assert_eq!(hit(W::StreakSrm6, Mode::Normal, roll, 7.0, false, true), 12);
        }
    }

    /// The ATM's guidance adds two to the cluster roll unless ECM blocks it.
    #[test]
    fn atm_guidance_bonus_is_lost_to_ecm() {
        use super::super::AmmunitionMode as Mode;
        for roll in 2..=12u8 {
            let landed = |guidance_blocked| {
                W::ClanAtm12
                    .damage_groups_for_guided_hit(GuidedHit {
                        mode: Mode::Normal,
                        cluster_roll: Some(roll),
                        glancing: false,
                        distance: None,
                        artemis_v: false,
                        guidance_blocked,
                        ams_engaged: false,
                        interference: false,
                    })
                    .unwrap()
                    .iter()
                    .sum::<u16>()
                    / 2
            };
            let guided = W::ClanAtm12.missile_hits((roll + 2).min(12)).unwrap();
            let unguided = W::ClanAtm12.missile_hits(roll).unwrap();
            assert_eq!(landed(false), u16::from(guided));
            assert_eq!(landed(true), u16::from(unguided));
        }
    }

    /// Glancing direct hits halve damage rounding down (Tactical Operations p. 79); missiles
    /// shift the cluster roll down four, never below row 2, without halving damage.
    #[test]
    fn glancing_damage_rounds_down_and_missiles_shift_clusters_without_halving_damage() {
        assert_eq!(
            W::MediumLaser
                .damage_groups_for_hit(None, true, None)
                .unwrap(),
            [2]
        );
        assert_eq!(
            W::Ac20.damage_groups_for_hit(None, true, None).unwrap(),
            [10]
        );
        for (weapon, expected) in [(W::LightAc2, 1), (W::LightAc5, 2), (W::HeavyMachineGun, 1)] {
            assert_eq!(
                weapon.damage_groups_for_hit(None, true, None).unwrap(),
                [expected]
            );
        }
        for (weapon, expected) in [
            (W::Srm2, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]),
            (W::Lrm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Rocket10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Rocket15, [5, 5, 5, 5, 5, 5, 6, 9, 9, 9, 9]),
            (W::Rocket20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
            (W::Lrm10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Lrm15, [5, 5, 5, 5, 5, 5, 6, 9, 9, 9, 9]),
            (W::Srm4, [1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3]),
            (W::Srm6, [2, 2, 2, 2, 2, 2, 3, 3, 4, 4, 4]),
            (W::Lrm20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
            (W::Mrm10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Mrm20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
            (W::Mrm30, [10, 10, 10, 10, 10, 10, 12, 18, 18, 18, 18]),
            (W::Mrm40, [12, 12, 12, 12, 12, 12, 18, 24, 24, 24, 24]),
            (W::Nlrm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Nlrm10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Nlrm15, [5, 5, 5, 5, 5, 5, 6, 9, 9, 9, 9]),
            (W::Nlrm20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
            (W::Elrm5, [1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3]),
            (W::Elrm10, [3, 3, 3, 3, 3, 3, 4, 6, 6, 6, 6]),
            (W::Elrm15, [5, 5, 5, 5, 5, 5, 6, 9, 9, 9, 9]),
            (W::Elrm20, [6, 6, 6, 6, 6, 6, 9, 12, 12, 12, 12]),
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
    use super::super::{AmmunitionMode, Weapon};
    use crate::btech::WeaponSalvo;

    /// Combine both modifiers before clamping the roll to the table's rows.
    #[test]
    fn artemis_glancing_table_boundaries() {
        for weapon in [Weapon::Srm6, Weapon::Lrm20, Weapon::ClanLrm20, Weapon::Mml9] {
            for roll in 2..=12 {
                let actual = weapon
                    .damage_groups_for_ammunition_hit(
                        AmmunitionMode::Artemis,
                        Some(roll),
                        true,
                        Some(8.0),
                    )
                    .unwrap();
                let hits = weapon.missile_hits((roll - 2).max(2)).unwrap();
                assert_eq!(
                    actual.iter().sum::<u16>(),
                    u16::from(hits) * u16::from(weapon.profile().damage)
                );
            }
        }
        // Streak launchers land every missile and take no Artemis rounds (Total Warfare p. 130).
        assert_eq!(
            Weapon::StreakSrm6
                .damage_groups_for_ammunition_hit(AmmunitionMode::Normal, Some(2), true, Some(8.0))
                .unwrap()
                .iter()
                .sum::<u16>(),
            12
        );
        assert!(
            Weapon::StreakSrm6
                .damage_groups_for_ammunition_hit(AmmunitionMode::Artemis, Some(2), true, Some(8.0))
                .is_err()
        );
    }
}

#[cfg(test)]
mod flechette_tests {
    use super::*;
    /// Odd shell damage is rounded down before a glancing hit halves it again, rounding down;
    /// a one-point flechette round glances off entirely.
    #[test]
    fn flechette_armored_damage_precedes_glancing() {
        for (weapon, normal, glancing) in [
            (Weapon::Ac2, 1, 0),
            (Weapon::Ac5, 2, 1),
            (Weapon::Ac10, 5, 2),
            (Weapon::Ac20, 10, 5),
            (Weapon::LightAc2, 1, 0),
            (Weapon::LightAc5, 2, 1),
        ] {
            for (glance, damage) in [(false, normal), (true, glancing)] {
                let expected = if damage == 0 {
                    Vec::new()
                } else {
                    vec![damage]
                };
                assert_eq!(
                    weapon
                        .damage_groups_for_ammunition_hit(
                            super::super::AmmunitionMode::Flechette,
                            None,
                            glance,
                            Some(1.0)
                        )
                        .unwrap(),
                    expected
                );
            }
        }
    }

    /// A hyper-assault Gauss rifle rolls on its own Cluster Hits column, plus two at short range
    /// and minus two at long range within 2..=12, and lands its damage in five-point groups
    /// (Total Warfare p. 136).
    #[test]
    fn hag_cluster_hits_shift_with_range_in_five_point_groups() {
        let groups = |weapon: Weapon, roll: u8, distance: f64| {
            weapon.damage_groups_at_range(Some(roll), distance).unwrap()
        };
        // HAG/20 rolls of 7: short range reads 9 (16), medium 7 (12), long 5 (12).
        assert_eq!(groups(Weapon::ClanHag20, 7, 8.0), vec![5, 5, 5, 1]);
        assert_eq!(groups(Weapon::ClanHag20, 7, 9.0), vec![5, 5, 2]);
        assert_eq!(groups(Weapon::ClanHag20, 7, 20.0), vec![5, 5, 2]);
        // The shifted roll stays on the table: 11 + 2 reads 12, 3 - 2 reads 2.
        assert_eq!(groups(Weapon::ClanHag40, 11, 2.0).iter().sum::<u16>(), 40);
        assert_eq!(groups(Weapon::ClanHag30, 3, 20.0).iter().sum::<u16>(), 10);
        assert!(Weapon::ClanHag30.damage_groups(None).is_err());
    }

    /// Enhanced LRMs fire the same salvos as the LRMs they extend.
    #[test]
    fn enhanced_lrms_match_lrm_salvos() {
        for (enhanced, standard) in [
            (Weapon::Nlrm5, Weapon::Lrm5),
            (Weapon::Nlrm10, Weapon::Lrm10),
            (Weapon::Nlrm15, Weapon::Lrm15),
            (Weapon::Nlrm20, Weapon::Lrm20),
        ] {
            for roll in 2..=12 {
                assert_eq!(
                    enhanced.damage_groups(Some(roll)).unwrap(),
                    standard.damage_groups(Some(roll)).unwrap()
                );
            }
        }
    }
}
