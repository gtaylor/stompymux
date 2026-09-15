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
        if self.is_mml() && ammunition == BattleAmmunitionMode::MmlLrm {
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
        self.supports_indirect_fire()
            && (!self.is_mml() || ammunition == BattleAmmunitionMode::MmlLrm)
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
        let family = if self == Self::MmlLrm { "LRM" } else { "SRM" };
        format!("Weapon {index} has been set to fire {family} missiles.")
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
