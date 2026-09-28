//! Multi-missile launchers share ammunition profiles, controls and combat with other launchers.
use super::{BattleAmmunitionMode, BattleWeapon, WeaponProfile};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleWeapon {
    /// Multi-missile launchers accept dedicated short- and long-range supplies.
    pub fn is_mml(self) -> bool {
        matches!(self, Self::Mml3 | Self::Mml5 | Self::Mml7 | Self::Mml9)
    }

    /// Resolve ballistic facts once for aim, damage, interception and ammunition hazards.
    /// Normal MML ammunition is SRM; MML_LRM bins carry the long-range family.
    pub fn profile_for_ammunition(self, ammunition: BattleAmmunitionMode) -> WeaponProfile {
        let mut profile = self.profile();
        if self.is_mml() && ammunition.is_mml_lrm() {
            profile.damage = 1;
            profile.minimum_range = 6;
            profile.short_range = 7;
            profile.medium_range = 14;
            profile.long_range = 21;
            profile.ammunition_per_ton = 120 / profile.missiles;
        }
        profile
    }

    /// Observer eligibility follows the loaded family, while equipment eligibility stays weapon-based.
    pub fn supports_indirect_ammunition(self, ammunition: BattleAmmunitionMode) -> bool {
        self.supports_indirect_fire() && (!self.is_mml() || ammunition.is_mml_lrm())
    }

    /// Hotloaded MMLs draw their selected family; other launchers retain ordinary hotload supply.
    pub(super) fn hotload_supply_mode(
        self,
        selected: BattleAmmunitionMode,
    ) -> BattleAmmunitionMode {
        if self.is_mml() {
            selected
        } else {
            BattleAmmunitionMode::Normal
        }
    }

    /// Internal bin damage uses its contents, independent of the launcher's selected supply.
    pub fn ammunition_explosion_damage_for_mode(
        self,
        rounds: u16,
        ammunition: BattleAmmunitionMode,
    ) -> u32 {
        if self.weapon_explosion_damage() > 0 || self == Self::PlasmaRifle {
            return 0;
        }
        let profile = self.profile_for_ammunition(ammunition);
        u32::from(rounds) * u32::from(profile.damage) * u32::from(profile.missiles.max(1))
    }
}

impl BattleAmmunitionMode {
    /// Shared cockpit feedback identifies the selected missile family explicitly.
    pub(crate) fn mml_message(self, index: usize) -> String {
        let family = if self.is_mml_lrm() { "LRM" } else { "SRM" };
        format!("Weapon {index} has been set to fire {family} missiles.")
    }

    /// Whether this supply belongs to the MML long-range family, with or without a special round.
    pub fn is_mml_lrm(self) -> bool {
        matches!(
            self,
            Self::MmlLrm
                | Self::MmlLrmArtemis
                | Self::MmlLrmNarc
                | Self::MmlLrmSwarm
                | Self::MmlLrmSwarm1
                | Self::MmlLrmSemiGuided
                | Self::MmlLrmStinger
        )
    }

    /// The special round independent of MML family; ordinary long-range MML rounds are normal.
    /// Combat rules keyed on a special round consult this instead of the stored supply.
    pub fn munition(self) -> Self {
        match self {
            Self::MmlLrm => Self::Normal,
            Self::MmlLrmArtemis => Self::Artemis,
            Self::MmlLrmNarc => Self::Narc,
            Self::MmlLrmSwarm => Self::Swarm,
            Self::MmlLrmSwarm1 => Self::Swarm1,
            Self::MmlLrmSemiGuided => Self::SemiGuided,
            Self::MmlLrmStinger => Self::Stinger,
            mode => mode,
        }
    }

    /// Combine this round's munition with an MML family. Returns `None` when the family
    /// cannot carry the round: long-range supplies exclude SRM-only rounds such as Inferno,
    /// and short-range supplies exclude LRM-only guidance and Swarm rounds.
    pub fn with_mml_family(self, long_range: bool) -> Option<Self> {
        let munition = self.munition();
        if !long_range {
            return (!matches!(
                munition,
                Self::Swarm | Self::Swarm1 | Self::SemiGuided | Self::Stinger
            ) && munition.supports(BattleWeapon::Mml3))
            .then_some(munition);
        }
        Some(match munition {
            Self::Normal => Self::MmlLrm,
            Self::Artemis => Self::MmlLrmArtemis,
            Self::Narc => Self::MmlLrmNarc,
            Self::Swarm => Self::MmlLrmSwarm,
            Self::Swarm1 => Self::MmlLrmSwarm1,
            Self::SemiGuided => Self::MmlLrmSemiGuided,
            Self::Stinger => Self::MmlLrmStinger,
            _ => return None,
        })
    }

    /// Replace the special round while keeping this supply's MML family, so guidance fallbacks
    /// such as blocked Artemis retain long-range damage and grouping.
    pub(super) fn with_munition(self, munition: Self) -> Self {
        if self.is_mml_lrm() {
            munition.with_mml_family(true).unwrap_or(Self::MmlLrm)
        } else {
            munition
        }
    }
}

/// Toggle an intact recycled MML between its dedicated SRM and LRM ammunition bins.
pub fn toggle_mml_ammunition(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(ready.weapon.is_mml(), "That weapon is not an MML launcher!");
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::MmlLrm,
    ))
}

/// Cockpit lists use the same ordered selection and rollback as other ammunition controls.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_mml_ammunition(world, id, pilot, index).map(|mode| mode.mml_message(index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::weapon_groups::{WeaponGroupRequest, roll_weapon_groups};
    use crate::btech::{AmmunitionBin, BattleDice, BattleFireMode};

    /// Capacity normalization uses the selected family before half-ton rounding.
    #[test]
    fn bins_size_each_family_and_reject_conflicting_modes() {
        for (weapon, srm, lrm) in [
            (BattleWeapon::Mml3, 33, 40),
            (BattleWeapon::Mml5, 20, 24),
            (BattleWeapon::Mml7, 14, 17),
            (BattleWeapon::Mml9, 11, 13),
        ] {
            for (mode, flags, full) in [
                (BattleAmmunitionMode::Normal, vec![], srm),
                (BattleAmmunitionMode::MmlLrm, vec!["MML_LRM".into()], lrm),
            ] {
                assert_eq!(
                    AmmunitionBin::configuration(weapon, &flags).unwrap(),
                    (full, false, mode)
                );
                let mut half = flags.clone();
                half.push("Halfton".into());
                assert_eq!(
                    AmmunitionBin::configuration(weapon, &half).unwrap(),
                    (full / 2, true, mode)
                );
                assert_eq!(
                    BattleAmmunitionMode::initial_selection(weapon, &flags),
                    mode
                );
            }
            assert!(
                AmmunitionBin::configuration(weapon, &["MML_LRM".into(), "Inferno".into()])
                    .is_err()
            );
        }
        assert!(AmmunitionBin::configuration(BattleWeapon::Lrm5, &["MML_LRM".into()]).is_err());
    }

    /// Long-range bins pair the family flag with one LRM-compatible round in either order.
    #[test]
    fn long_range_bins_accept_one_special_round() {
        for (flag, mode) in [
            ("Artemis/Mine", BattleAmmunitionMode::MmlLrmArtemis),
            ("Narc/Smoke", BattleAmmunitionMode::MmlLrmNarc),
            ("Swarm", BattleAmmunitionMode::MmlLrmSwarm),
            ("Swarm1", BattleAmmunitionMode::MmlLrmSwarm1),
            ("Sguided", BattleAmmunitionMode::MmlLrmSemiGuided),
            ("Stinger", BattleAmmunitionMode::MmlLrmStinger),
        ] {
            for flags in [
                vec!["MML_LRM".to_string(), flag.into()],
                vec![flag.into(), "MML_LRM".into()],
            ] {
                assert_eq!(
                    AmmunitionBin::configuration(BattleWeapon::Mml5, &flags).unwrap(),
                    (24, false, mode)
                );
                assert_eq!(
                    BattleAmmunitionMode::initial_selection(BattleWeapon::Mml5, &flags),
                    mode
                );
            }
            assert!(mode.is_mml_lrm());
            assert_eq!(mode.munition().with_mml_family(true), Some(mode));
            assert!(mode.supports(BattleWeapon::Mml5));
            assert!(!mode.supports(BattleWeapon::Lrm5));
            assert!(
                AmmunitionBin::configuration(BattleWeapon::Lrm5, &["MML_LRM".into(), flag.into()])
                    .is_err()
            );
        }
        // SRM-family MML bins keep SRM rounds and still reject LRM-only guidance.
        assert_eq!(
            AmmunitionBin::configuration(BattleWeapon::Mml5, &["Artemis/Mine".into()])
                .unwrap()
                .2,
            BattleAmmunitionMode::Artemis
        );
        for flag in ["Swarm", "Swarm1", "Sguided", "Stinger"] {
            assert!(AmmunitionBin::configuration(BattleWeapon::Mml5, &[flag.into()]).is_err());
        }
        assert!(
            AmmunitionBin::configuration(
                BattleWeapon::Mml5,
                &["MML_LRM".into(), "Swarm".into(), "Artemis/Mine".into()]
            )
            .is_err()
        );
    }

    /// Family changes keep a round both families carry and drop rounds the other family lacks.
    #[test]
    fn family_combination_rules() {
        use BattleAmmunitionMode as Mode;
        assert_eq!(
            Mode::Artemis.with_mml_family(true),
            Some(Mode::MmlLrmArtemis)
        );
        assert_eq!(
            Mode::MmlLrmArtemis.with_mml_family(false),
            Some(Mode::Artemis)
        );
        assert_eq!(Mode::MmlLrmSwarm.with_mml_family(false), None);
        assert_eq!(Mode::Inferno.with_mml_family(true), None);
        assert_eq!(Mode::Inferno.with_mml_family(false), Some(Mode::Inferno));
        assert_eq!(Mode::MmlLrm.with_mml_family(false), Some(Mode::Normal));
        assert_eq!(Mode::MmlLrm.munition(), Mode::Normal);
        assert_eq!(
            Mode::MmlLrmNarc.with_munition(Mode::Artemis),
            Mode::MmlLrmArtemis
        );
        assert_eq!(Mode::MmlLrmNarc.with_munition(Mode::Normal), Mode::MmlLrm);
        assert_eq!(Mode::Narc.with_munition(Mode::Normal), Mode::Normal);
        assert!(Mode::MmlLrmSwarm.bypasses_ams() && Mode::MmlLrmSwarm1.is_swarm());
    }

    /// Long-range special rounds keep LRM damage and grouping; Artemis adds its cluster bonus.
    #[test]
    fn long_range_guidance_uses_lrm_groups() {
        use BattleAmmunitionMode as Mode;
        let weapon = BattleWeapon::Mml9;
        for roll in 2..=10 {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition_hit(Mode::MmlLrmArtemis, Some(roll), false, None)
                    .unwrap(),
                weapon
                    .damage_groups_for_ammunition_hit(Mode::MmlLrm, Some(roll + 2), false, None)
                    .unwrap()
            );
        }
        for (mode, beacon, blocked, expected) in [
            (Mode::MmlLrmArtemis, false, false, Mode::MmlLrmArtemis),
            (Mode::MmlLrmArtemis, false, true, Mode::MmlLrm),
            (Mode::MmlLrmNarc, true, false, Mode::MmlLrmArtemis),
            (Mode::MmlLrmNarc, false, false, Mode::MmlLrmNarc),
            (Mode::MmlLrmNarc, true, true, Mode::MmlLrm),
        ] {
            for seed in 0..=31 {
                let request = |ammunition| WeaponGroupRequest {
                    submerged: false,
                    range_damage: false,
                    damage_penalty: 0,
                    weapon,
                    ammunition,
                    fire_mode: BattleFireMode::Normal,
                    gatling_damage: None,
                    distance: Some(7.0),
                    glancing: false,
                    guidance_blocked: blocked,
                    angel_blocked: false,
                    target_beacon: beacon,
                    artemis_v: false,
                };
                let actual =
                    roll_weapon_groups(request(mode), &mut BattleDice::seeded([seed; 32])).unwrap();
                let mut unguided = request(expected);
                unguided.guidance_blocked = false;
                unguided.target_beacon = false;
                let reference =
                    roll_weapon_groups(unguided, &mut BattleDice::seeded([seed; 32])).unwrap();
                assert_eq!(actual.damage, reference.damage);
                assert!(actual.damage.iter().all(|damage| (1..=5).contains(damage)));
            }
        }
    }

    /// Interception removes missiles, not damage points, after shared hotload and glancing rolls.
    #[test]
    fn interception_uses_ammunition_damage_and_preserves_dice() {
        for weapon in [
            BattleWeapon::Mml3,
            BattleWeapon::Mml5,
            BattleWeapon::Mml7,
            BattleWeapon::Mml9,
        ] {
            for mode in [BattleAmmunitionMode::Normal, BattleAmmunitionMode::MmlLrm] {
                for fire_mode in [BattleFireMode::Normal, BattleFireMode::Hotload] {
                    for glancing in [false, true] {
                        for seed in 0..=255 {
                            let mut dice = BattleDice::seeded([seed; 32]);
                            let mut expected = dice.clone();
                            let first = expected.d6();
                            let second = expected.d6();
                            let roll = if fire_mode == BattleFireMode::Hotload {
                                let third = expected.d6();
                                first + second + third - first.max(second).max(third)
                            } else {
                                first + second
                            };
                            let mut groups = roll_weapon_groups(
                                WeaponGroupRequest {
                                    submerged: false,
                                    range_damage: false,
                                    damage_penalty: 0,
                                    weapon,
                                    ammunition: mode,
                                    fire_mode,
                                    gatling_damage: None,
                                    distance: Some(7.0),
                                    glancing,
                                    guidance_blocked: false,
                                    angel_blocked: false,
                                    target_beacon: false,
                                    artemis_v: false,
                                },
                                &mut dice,
                            )
                            .unwrap();
                            assert_eq!(dice, expected);
                            assert_eq!(groups.cluster_roll, Some(roll));
                            let per_missile = u16::from(weapon.profile_for_ammunition(mode).damage);
                            let hits = groups.damage.iter().sum::<u16>() / per_missile;
                            assert_eq!(
                                groups.intercept(weapon, 2),
                                Some((hits, hits.saturating_sub(2)))
                            );
                            assert_eq!(
                                groups.damage.iter().sum::<u16>(),
                                hits.saturating_sub(2) * per_missile
                            );
                            assert_eq!(dice, expected);
                        }
                    }
                }
            }
        }
    }
}
