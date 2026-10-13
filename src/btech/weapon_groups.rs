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
}

/// Shared cluster draw and ordered damage packet sizes, before target defenses.
pub(super) struct WeaponGroups {
    /// Retain the ammunition profile through interception and flight truncation.
    missile_damage: u8,
    pub cluster_roll: Option<u8>,
    pub damage: Vec<u16>,
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
        conventional.damage_groups_for_hit(cluster_roll, glancing, distance)?
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
        } else if guidance_blocked
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
        weapon.damage_groups_for_guided_hit(
            guided_mode,
            cluster_roll,
            glancing,
            distance,
            artemis_v,
            guidance_blocked,
        )?
    };
    // A glancing blow halves damage rounding down (Tactical Operations p. 79), so a one-point
    // round lands without effect and no hit location is rolled for it.
    damage.retain(|packet| *packet > 0);
    Ok(WeaponGroups {
        missile_damage: if mode == AmmunitionMode::Cluster {
            1
        } else {
            weapon.profile_for_ammunition(mode).damage
        },
        cluster_roll,
        damage,
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
    }

    /// Cap defensive kills and rebuild remaining missile packets for every target anatomy.
    /// Returns cluster size and survivors; non-missile packets remain unchanged.
    pub(super) fn intercept(&mut self, weapon: Weapon, intercepted: u8) -> Option<(u16, u16)> {
        if weapon.profile().missiles == 0 {
            return None;
        }
        let hits = self.damage.iter().sum::<u16>() / u16::from(self.missile_damage);
        let surviving = hits.saturating_sub(u16::from(intercepted));
        if intercepted > 0 {
            let packet = self.damage.first().copied().unwrap_or(1);
            let mut remaining = surviving * u16::from(self.missile_damage);
            self.damage.clear();
            while remaining > 0 {
                let damage = remaining.min(packet);
                self.damage.push(damage);
                remaining -= damage;
            }
        }
        Some((hits, surviving))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Artemis V adds three to the cluster roll, only for its own guided rounds.
    #[test]
    fn artemis_v_adds_one_more_to_guided_clusters() {
        let weapon = Weapon::Lrm20;
        for roll in 2..=9 {
            assert_eq!(
                weapon
                    .damage_groups_for_guided_hit(
                        AmmunitionMode::Artemis,
                        Some(roll),
                        false,
                        None,
                        true,
                        false
                    )
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
            let request =
                |ammunition, artemis_v, target_beacon, guidance_blocked| WeaponGroupRequest {
                    weapon,
                    ammunition,
                    fire_mode: FireMode::Normal,
                    gatling_damage: None,
                    distance: Some(7.0),
                    glancing: false,
                    guidance_blocked,
                    angel_blocked: false,
                    target_beacon,
                    artemis_v,
                };
            let roll = |request| {
                roll_weapon_groups(request, &mut Dice::seeded([seed; 32]))
                    .unwrap()
                    .damage
            };
            // Narc homing keeps the Artemis IV bonus even on an Artemis V chassis.
            assert_eq!(
                roll(request(AmmunitionMode::Narc, true, true, false)),
                roll(request(AmmunitionMode::Artemis, false, false, false))
            );
            // ECM blocks Artemis V just as it blocks Artemis IV.
            assert_eq!(
                roll(request(AmmunitionMode::Artemis, true, false, true)),
                roll(request(AmmunitionMode::Normal, false, false, false))
            );
        }
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
                    WeaponGroupRequest {
                        weapon,
                        ammunition: AmmunitionMode::Normal,
                        fire_mode,
                        gatling_damage: None,
                        distance: Some(7.0),
                        glancing: false,
                        guidance_blocked: false,
                        angel_blocked: false,
                        target_beacon: false,
                        artemis_v: false,
                    },
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
