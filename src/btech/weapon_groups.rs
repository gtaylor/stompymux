//! Target-independent weapon packet sizing after a successful launch and hit.
use super::{AmmunitionMode, Dice, FireMode, FlechetteDamage, RotaryDamage, Weapon, WeaponSalvo};
use anyhow::Result;

/// Damage inputs resolved by the enclosing unit or terrain attack.
pub(super) struct WeaponGroupRequest {
    pub weapon: Weapon,
    pub ammunition: AmmunitionMode,
    pub fire_mode: FireMode,
    pub gatling_damage: Option<u8>,
    pub distance: Option<f64>,
    pub glancing: bool,
    pub guidance_blocked: bool,
    pub angel_blocked: bool,
    pub target_beacon: bool,
    /// The launching unit's Artemis controllers are Artemis V.
    pub artemis_v: bool,
    /// An anti-missile system engaged the flight: -4 on the Cluster Hits roll, never below 2,
    /// with a Streak launcher treated as a roll of 11 (Total Warfare p. 129).
    pub ams_engaged: bool,
    /// Spotter-directed indirect fire, which Artemis and Narc guidance do not improve (Total
    /// Warfare pp. 130, 139).
    pub indirect: bool,
}

/// Shared cluster draw and ordered damage packet sizes, before cover and terrain absorption.
pub(super) struct WeaponGroups {
    /// Retain the ammunition profile through interception and flight truncation.
    missile_damage: u8,
    pub cluster_roll: Option<u8>,
    pub damage: Vec<u16>,
    /// Cluster hits of a missile flight before any anti-missile engagement shifted the roll.
    pub missiles_before_defense: Option<u8>,
    /// Hits an anti-missile engagement removed from the Cluster Hits result.
    pub missiles_intercepted: Option<u8>,
}

/// Consume candidate dice and size packets without assigning hit locations or applying damage.
pub(super) fn roll_weapon_groups(
    request: WeaponGroupRequest,
    dice: &mut Dice,
) -> Result<WeaponGroups> {
    let WeaponGroupRequest {
        weapon,
        ammunition: mode,
        fire_mode,
        gatling_damage,
        distance,
        glancing,
        guidance_blocked,
        angel_blocked,
        target_beacon,
        artemis_v,
        ams_engaged,
        indirect,
    } = request;
    let burst = fire_mode.rounds_per_cycle() > 1;
    let cluster_roll = (burst || weapon.profile().missiles > 0 || mode == AmmunitionMode::Cluster)
        .then(|| {
            if fire_mode == FireMode::Hotload {
                let first = dice.d6();
                let second = dice.d6();
                let third = dice.d6();
                first + second + third - first.max(second).max(third)
            } else {
                dice.generic_roll()
            }
        });
    let missile_damage = if mode == AmmunitionMode::Cluster {
        1
    } else {
        weapon.profile_for_ammunition(mode).damage
    };
    let resolve = |ams_engaged: bool| -> Result<Vec<u16>> {
        let mut damage = if let Some(damage) = gatling_damage {
            vec![if glancing {
                u16::from(damage) / 2
            } else {
                u16::from(damage)
            }]
        } else if weapon.is_streak() && angel_blocked {
            let conventional = match weapon {
                Weapon::StreakSrm2 | Weapon::ClanStreakSrm2 => Weapon::Srm2,
                Weapon::StreakSrm4 | Weapon::ClanStreakSrm4 => Weapon::Srm4,
                Weapon::StreakSrm6 | Weapon::ClanStreakSrm6 => Weapon::Srm6,
                Weapon::ClanStreakLrm5 => Weapon::ClanLrm5,
                Weapon::ClanStreakLrm10 => Weapon::ClanLrm10,
                Weapon::ClanStreakLrm15 => Weapon::ClanLrm15,
                Weapon::ClanStreakLrm20 => Weapon::ClanLrm20,
                _ => unreachable!("Streak family checked"),
            };
            conventional.damage_groups_for_guided_hit(super::GuidedHit {
                mode: AmmunitionMode::Normal,
                cluster_roll,
                glancing,
                distance,
                artemis_v: false,
                guidance_blocked: false,
                ams_engaged,
            })?
        } else if weapon.is_rotary() && burst {
            weapon.rotary_damage_groups(fire_mode, cluster_roll.unwrap(), glancing)?
        } else if burst {
            weapon
                .double_shot_damage_groups(cluster_roll.unwrap(), glancing)?
                .into_iter()
                .map(|damage| mode.armored_damage(damage))
                .collect()
        } else {
            let guided_mode = if mode == AmmunitionMode::Inferno {
                // Nominal missile packets supply the hit count; target resolvers apply burning instead.
                AmmunitionMode::Normal
            } else if (guidance_blocked || indirect)
                && matches!(
                    mode.munition(),
                    AmmunitionMode::Artemis | AmmunitionMode::Narc
                )
            {
                mode.with_munition(AmmunitionMode::Normal)
            } else if mode.munition() == AmmunitionMode::Narc && target_beacon {
                mode.with_munition(AmmunitionMode::Artemis)
            } else {
                mode
            };
            // Artemis V improves only its own guided rounds, not Narc beacon homing.
            let artemis_v =
                artemis_v && guided_mode == mode && mode.munition() == AmmunitionMode::Artemis;
            weapon.damage_groups_for_guided_hit(super::GuidedHit {
                mode: guided_mode,
                cluster_roll,
                glancing,
                distance,
                artemis_v,
                guidance_blocked,
                ams_engaged,
            })?
        };
        // A glancing blow halves damage rounding down (Tactical Operations p. 79), so a one-point
        // round lands without effect and no hit location is rolled for it.
        damage.retain(|packet| *packet > 0);
        Ok(damage)
    };
    let damage = resolve(ams_engaged)?;
    let count = |damage: &[u16]| {
        u8::try_from(damage.iter().sum::<u16>() / u16::from(missile_damage.max(1)))
            .expect("bounded missile rack")
    };
    let missile = weapon.profile().missiles > 0;
    let missiles_before_defense = if !missile {
        None
    } else if ams_engaged {
        Some(count(&resolve(false)?))
    } else {
        Some(count(&damage))
    };
    let missiles_intercepted =
        (missile && ams_engaged).then(|| missiles_before_defense.unwrap_or(0) - count(&damage));
    Ok(WeaponGroups {
        missile_damage,
        cluster_roll,
        damage,
        missiles_before_defense,
        missiles_intercepted,
    })
}

impl WeaponGroups {
    /// A glancing unit hit halves shell damage, rounding down, as well as the already-adjusted
    /// burst count.
    pub(super) fn finish_burst_glancing(&mut self, mode: FireMode, glancing: bool) {
        if !glancing || mode.rounds_per_cycle() <= 1 {
            return;
        }
        for damage in &mut self.damage {
            *damage /= 2;
        }
        self.damage.retain(|damage| *damage > 0);
    }

    /// A retargeted cluster cannot contain more missiles than remain in the flight.
    pub(super) fn limit_missiles(&mut self, incoming: Option<u8>) {
        let Some(incoming) = incoming else {
            return;
        };
        let mut remaining = u16::from(incoming) * u16::from(self.missile_damage);
        for damage in &mut self.damage {
            *damage = (*damage).min(remaining);
            remaining -= *damage;
        }
        self.damage.retain(|damage| *damage > 0);
        self.missiles_before_defense = self.missiles_before_defense.map(|hits| hits.min(incoming));
    }

    /// Missiles represented by the current packets, after any defense and flight cap.
    pub(super) fn missile_count(&self) -> u16 {
        self.damage.iter().sum::<u16>() / u16::from(self.missile_damage.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request with ordinary guidance and no defenses.
    fn request(
        weapon: Weapon,
        ammunition: AmmunitionMode,
        fire_mode: FireMode,
    ) -> WeaponGroupRequest {
        WeaponGroupRequest {
            weapon,
            ammunition,
            fire_mode,
            gatling_damage: None,
            distance: Some(7.0),
            glancing: false,
            guidance_blocked: false,
            angel_blocked: false,
            target_beacon: false,
            artemis_v: false,
            ams_engaged: false,
            indirect: false,
        }
    }

    /// Artemis V adds three to the cluster roll, only for its own guided rounds.
    #[test]
    fn artemis_v_adds_one_more_to_guided_clusters() {
        let weapon = Weapon::Lrm20;
        for roll in 2..=9 {
            assert_eq!(
                weapon
                    .damage_groups_for_guided_hit(super::super::GuidedHit {
                        mode: AmmunitionMode::Artemis,
                        cluster_roll: Some(roll),
                        glancing: false,
                        distance: None,
                        artemis_v: true,
                        guidance_blocked: false,
                        ams_engaged: false,
                    })
                    .unwrap(),
                weapon
                    .damage_groups_for_ammunition_hit(
                        AmmunitionMode::Normal,
                        Some(roll + 3),
                        false,
                        None
                    )
                    .unwrap()
            );
        }
        for seed in 0..=31 {
            let guided =
                |ammunition, artemis_v, target_beacon, guidance_blocked| WeaponGroupRequest {
                    artemis_v,
                    target_beacon,
                    guidance_blocked,
                    ..request(weapon, ammunition, FireMode::Normal)
                };
            let roll = |request| {
                roll_weapon_groups(request, &mut Dice::seeded([seed; 32]))
                    .unwrap()
                    .damage
            };
            // Narc homing keeps the Artemis IV bonus even on an Artemis V chassis.
            assert_eq!(
                roll(guided(AmmunitionMode::Narc, true, true, false)),
                roll(guided(AmmunitionMode::Artemis, false, false, false))
            );
            // ECM blocks Artemis V just as it blocks Artemis IV.
            assert_eq!(
                roll(guided(AmmunitionMode::Artemis, true, false, true)),
                roll(guided(AmmunitionMode::Normal, false, false, false))
            );
        }
    }

    /// Indirect fire strips the Artemis and Narc cluster bonuses but not the ATM's integral one.
    #[test]
    fn indirect_fire_strips_artemis_and_narc_bonuses() {
        for seed in 0..=31 {
            let roll = |request| {
                roll_weapon_groups(request, &mut Dice::seeded([seed; 32]))
                    .unwrap()
                    .damage
            };
            let indirect = |ammunition, target_beacon| WeaponGroupRequest {
                indirect: true,
                target_beacon,
                ..request(Weapon::Lrm20, ammunition, FireMode::Normal)
            };
            let normal = roll(request(
                Weapon::Lrm20,
                AmmunitionMode::Normal,
                FireMode::Normal,
            ));
            assert_eq!(roll(indirect(AmmunitionMode::Artemis, false)), normal);
            assert_eq!(roll(indirect(AmmunitionMode::Narc, true)), normal);
            assert_eq!(
                roll(WeaponGroupRequest {
                    indirect: true,
                    ..request(Weapon::ClanAtm9, AmmunitionMode::Normal, FireMode::Normal)
                }),
                roll(request(
                    Weapon::ClanAtm9,
                    AmmunitionMode::Normal,
                    FireMode::Normal
                ))
            );
        }
    }

    /// An engaged AMS shifts the cluster roll by four (never below 2), treats a Streak as a roll
    /// of 11 and reports the hits it removed; the dice stream is unchanged.
    #[test]
    fn ams_engagement_shifts_the_cluster_roll() {
        for seed in 0..=255 {
            for (weapon, ammunition) in [
                (Weapon::Lrm20, AmmunitionMode::Normal),
                (Weapon::Srm6, AmmunitionMode::Inferno),
                (Weapon::Mml7, AmmunitionMode::MmlLrm),
                (Weapon::Lrm20, AmmunitionMode::Artemis),
            ] {
                let mut dice = Dice::seeded([seed; 32]);
                let plain =
                    roll_weapon_groups(request(weapon, ammunition, FireMode::Normal), &mut dice)
                        .unwrap();
                let mut defended_dice = Dice::seeded([seed; 32]);
                let defended = roll_weapon_groups(
                    WeaponGroupRequest {
                        ams_engaged: true,
                        ..request(weapon, ammunition, FireMode::Normal)
                    },
                    &mut defended_dice,
                )
                .unwrap();
                assert_eq!(dice, defended_dice);
                assert_eq!(plain.cluster_roll, defended.cluster_roll);
                let roll = plain.cluster_roll.unwrap();
                let (table_mode, bonus) = match ammunition {
                    AmmunitionMode::Artemis => (AmmunitionMode::Normal, 2),
                    AmmunitionMode::Inferno => (AmmunitionMode::Normal, 0),
                    mode => (mode, 0),
                };
                let expected = weapon
                    .damage_groups_for_ammunition_hit(
                        table_mode,
                        Some((i16::from(roll) + bonus - 4).clamp(2, 12) as u8),
                        false,
                        Some(7.0),
                    )
                    .unwrap();
                assert_eq!(
                    defended.damage, expected,
                    "{weapon:?} {ammunition:?} roll {roll}"
                );
                let before = defended.missiles_before_defense.unwrap();
                assert_eq!(u16::from(before), plain.missile_count());
                assert_eq!(plain.missiles_before_defense, Some(before));
                assert_eq!(
                    defended.missiles_intercepted,
                    Some(before - u8::try_from(defended.missile_count()).unwrap())
                );
                assert!(plain.missiles_intercepted.is_none());
            }
            // A locked Streak counts as a roll of 11 before the -4.
            let streak = roll_weapon_groups(
                WeaponGroupRequest {
                    ams_engaged: true,
                    ..request(Weapon::StreakSrm6, AmmunitionMode::Normal, FireMode::Normal)
                },
                &mut Dice::seeded([seed; 32]),
            )
            .unwrap();
            assert_eq!(streak.damage, vec![2; 4]);
            assert_eq!(streak.missiles_before_defense, Some(6));
            assert_eq!(streak.missiles_intercepted, Some(2));
            let streak_lrm = roll_weapon_groups(
                WeaponGroupRequest {
                    ams_engaged: true,
                    ..request(
                        Weapon::ClanStreakLrm20,
                        AmmunitionMode::Normal,
                        FireMode::Normal,
                    )
                },
                &mut Dice::seeded([seed; 32]),
            )
            .unwrap();
            assert_eq!(streak_lrm.damage, vec![5, 5, 2]);
            assert_eq!(streak_lrm.missiles_before_defense, Some(20));
            assert_eq!(streak_lrm.missiles_intercepted, Some(8));
        }
        // Non-missile weapons report no defense.
        let laser = roll_weapon_groups(
            WeaponGroupRequest {
                ams_engaged: true,
                ..request(
                    Weapon::MediumLaser,
                    AmmunitionMode::Normal,
                    FireMode::Normal,
                )
            },
            &mut Dice::seeded([1; 32]),
        )
        .unwrap();
        assert_eq!(laser.damage, vec![5]);
        assert!(laser.missiles_before_defense.is_none() && laser.missiles_intercepted.is_none());
    }

    /// Hotloaded clusters use direct dice; ordinary clusters record one generic check.
    #[test]
    fn cluster_accounting_preserves_direct_hotload_and_energy_rolls() {
        for seed in 0..=255 {
            for (weapon, fire_mode, count) in [
                (Weapon::Lrm20, FireMode::Normal, 1),
                (Weapon::Lrm20, FireMode::Hotload, 0),
                (Weapon::MediumLaser, FireMode::Normal, 0),
            ] {
                let mut dice = Dice::seeded([seed; 32]);
                let mut expected = dice.clone();
                let roll = if weapon == Weapon::MediumLaser {
                    None
                } else if fire_mode == FireMode::Hotload {
                    let a = expected.d6();
                    let b = expected.d6();
                    let c = expected.d6();
                    Some(a + b + c - a.max(b).max(c))
                } else {
                    Some(expected.two_d6())
                };
                let groups = roll_weapon_groups(
                    request(weapon, AmmunitionMode::Normal, fire_mode),
                    &mut dice,
                )
                .unwrap();
                assert_eq!(groups.cluster_roll, roll);
                assert_eq!(dice, expected);
                assert_eq!(dice.generic_roll_statistics().total(), count);
                if count == 1 {
                    assert_eq!(
                        dice.generic_roll_statistics().counts()[usize::from(roll.unwrap() - 2)],
                        1
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod streak_lrm_tests {
    use super::*;

    /// Streak LRMs group five-point hits; Angel confusion switches to the Clan LRM tables.
    #[test]
    fn streak_lrm_confusion_and_cluster_packets() {
        for (weapon, conventional) in [
            (Weapon::ClanStreakLrm5, Weapon::ClanLrm5),
            (Weapon::ClanStreakLrm10, Weapon::ClanLrm10),
            (Weapon::ClanStreakLrm15, Weapon::ClanLrm15),
            (Weapon::ClanStreakLrm20, Weapon::ClanLrm20),
        ] {
            for seed in 0..=255 {
                for confused in [false, true] {
                    for glancing in [false, true] {
                        let mut dice = Dice::seeded([seed; 32]);
                        let mut expected_dice = dice.clone();
                        let roll = expected_dice.two_d6();
                        let groups = roll_weapon_groups(
                            WeaponGroupRequest {
                                weapon,
                                ammunition: AmmunitionMode::Normal,
                                fire_mode: FireMode::Normal,
                                gatling_damage: None,
                                distance: Some(7.0),
                                glancing,
                                guidance_blocked: confused,
                                angel_blocked: confused,
                                target_beacon: false,
                                artemis_v: false,
                                ams_engaged: false,
                                indirect: false,
                            },
                            &mut dice,
                        )
                        .unwrap();
                        assert_eq!(groups.cluster_roll, Some(roll));
                        let expected = if confused {
                            conventional
                                .damage_groups_for_hit(Some(roll), glancing, Some(7.0))
                                .unwrap()
                        } else {
                            vec![5; usize::from(weapon.profile().missiles / 5)]
                        };
                        assert_eq!(groups.damage, expected);
                        assert_eq!(dice, expected_dice);
                    }
                }
            }
        }
    }
}
