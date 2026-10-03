//! Target-independent weapon packet sizing after a successful launch and hit.
use super::{BattleAmmunitionMode, BattleDice, BattleFireMode, BattleWeapon};
use anyhow::{Context, Result, ensure};

/// Damage inputs resolved by the enclosing unit or terrain attack.
pub(super) struct WeaponGroupRequest {
    pub range_damage: bool,
    /// Whether the launching mount is below the waterline.
    pub submerged: bool,
    pub damage_penalty: u8,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub fire_mode: BattleFireMode,
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
    dice: &mut BattleDice,
) -> Result<WeaponGroups> {
    let WeaponGroupRequest {
        range_damage,
        submerged,
        damage_penalty,
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
    ensure!(
        damage_penalty == 0
            || (weapon.gunnery_skill(true) == "Gunnery-Laser"
                && damage_penalty <= weapon.profile().critical_slots),
        "Invalid weapon damage penalty"
    );
    let range_energy = range_damage && weapon.gunnery_skill(true) == "Gunnery-Laser";
    let defer_glancing = damage_penalty > 0 || range_energy;
    let burst = fire_mode.rounds_per_cycle() > 1;
    let cluster_roll = (burst
        || weapon.profile().missiles > 0
        || mode == BattleAmmunitionMode::Cluster)
        .then(|| {
            if fire_mode == BattleFireMode::Hotload {
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
            u16::from(damage).div_ceil(2)
        } else {
            u16::from(damage)
        }]
    } else if weapon.is_streak() && angel_blocked {
        let conventional = match weapon {
            BattleWeapon::StreakSrm2 | BattleWeapon::ClanStreakSrm2 => BattleWeapon::Srm2,
            BattleWeapon::StreakSrm4 | BattleWeapon::ClanStreakSrm4 => BattleWeapon::Srm4,
            BattleWeapon::StreakSrm6 | BattleWeapon::ClanStreakSrm6 => BattleWeapon::Srm6,
            BattleWeapon::ClanStreakLrm5 => BattleWeapon::ClanLrm5,
            BattleWeapon::ClanStreakLrm10 => BattleWeapon::ClanLrm10,
            BattleWeapon::ClanStreakLrm15 => BattleWeapon::ClanLrm15,
            BattleWeapon::ClanStreakLrm20 => BattleWeapon::ClanLrm20,
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
        let guided_mode = if mode == BattleAmmunitionMode::Inferno {
            // Nominal missile packets supply the hit count; target resolvers apply burning instead.
            BattleAmmunitionMode::Normal
        } else if guidance_blocked
            && matches!(
                mode.munition(),
                BattleAmmunitionMode::Artemis | BattleAmmunitionMode::Narc
            )
        {
            mode.with_munition(BattleAmmunitionMode::Normal)
        } else if mode.munition() == BattleAmmunitionMode::Narc && target_beacon {
            mode.with_munition(BattleAmmunitionMode::Artemis)
        } else {
            mode
        };
        // Artemis V improves only its own guided rounds, not Narc beacon homing.
        let artemis_v =
            artemis_v && guided_mode == mode && mode.munition() == BattleAmmunitionMode::Artemis;
        weapon.damage_groups_for_guided_hit(
            guided_mode,
            cluster_roll,
            glancing && !defer_glancing,
            distance,
            artemis_v,
            guidance_blocked,
        )?
    };
    if defer_glancing {
        for packet in &mut damage {
            *packet = packet.saturating_sub(u16::from(damage_penalty));
            if range_energy {
                let distance = distance.context("Energy range damage requires attack distance")?;
                let (medium, long) = if submerged {
                    let water = weapon
                        .water_ranges()
                        .context("This weapon may not be fired underwater.")?;
                    (
                        f64::from(water.medium_range),
                        water.long_range.map_or(-1.0, f64::from),
                    )
                } else {
                    let profile = weapon.profile();
                    (
                        f64::from(profile.medium_range),
                        f64::from(profile.long_range),
                    )
                };
                *packet = if distance <= 1.0 {
                    packet.saturating_add(1)
                } else if distance > long {
                    *packet / 2
                } else if distance > medium {
                    packet.saturating_sub(1)
                } else {
                    *packet
                }
                .max(1);
            }
            if glancing {
                *packet = packet.div_ceil(2);
            }
        }
        damage.retain(|packet| *packet > 0);
    }
    Ok(WeaponGroups {
        missile_damage: if mode == BattleAmmunitionMode::Cluster {
            1
        } else {
            weapon.profile_for_ammunition(mode).damage
        },
        cluster_roll,
        damage,
    })
}

impl WeaponGroups {
    /// A glancing unit hit reduces shell damage as well as the already-adjusted burst count.
    /// Occupied-woods callers defer this step until after per-shell absorption instead.
    pub(super) fn finish_burst_glancing(&mut self, mode: BattleFireMode, glancing: bool) {
        if !glancing || mode.rounds_per_cycle() <= 1 {
            return;
        }
        for damage in &mut self.damage {
            *damage = damage.div_ceil(2);
        }
    }

    /// Remove cover damage in whole missiles while retaining the ordered packet boundaries.
    /// The returned totals precede and follow absorption, after any interception or flight cap.
    pub(super) fn absorb_missiles(&mut self, reduction: u16) -> (u16, u16) {
        let damage = u16::from(self.missile_damage);
        let before = self.damage.iter().sum::<u16>();
        let surviving = before.saturating_sub(reduction) / damage;
        self.limit_missiles(Some(u8::try_from(surviving).expect("bounded missile rack")));
        (before, surviving * damage)
    }

    /// Per-missile payload includes the launcher's selected ammunition family.
    pub(super) fn missile_payload(&self) -> u16 {
        u16::from(self.missile_damage)
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
    pub(super) fn intercept(
        &mut self,
        weapon: BattleWeapon,
        intercepted: u8,
    ) -> Option<(u16, u16)> {
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
        let weapon = BattleWeapon::Lrm20;
        for roll in 2..=9 {
            assert_eq!(
                weapon
                    .damage_groups_for_guided_hit(
                        BattleAmmunitionMode::Artemis,
                        Some(roll),
                        false,
                        None,
                        true,
                        false
                    )
                    .unwrap(),
                weapon
                    .damage_groups_for_ammunition_hit(
                        BattleAmmunitionMode::Normal,
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
                    submerged: false,
                    range_damage: false,
                    damage_penalty: 0,
                    weapon,
                    ammunition,
                    fire_mode: BattleFireMode::Normal,
                    gatling_damage: None,
                    distance: Some(7.0),
                    glancing: false,
                    guidance_blocked,
                    angel_blocked: false,
                    target_beacon,
                    artemis_v,
                };
            let roll = |request| {
                roll_weapon_groups(request, &mut BattleDice::seeded([seed; 32]))
                    .unwrap()
                    .damage
            };
            // Narc homing keeps the Artemis IV bonus even on an Artemis V chassis.
            assert_eq!(
                roll(request(BattleAmmunitionMode::Narc, true, true, false)),
                roll(request(BattleAmmunitionMode::Artemis, false, false, false))
            );
            // ECM blocks Artemis V just as it blocks Artemis IV.
            assert_eq!(
                roll(request(BattleAmmunitionMode::Artemis, true, false, true)),
                roll(request(BattleAmmunitionMode::Normal, false, false, false))
            );
        }
    }

    /// Hotloaded clusters use direct dice; ordinary clusters record one generic check.
    #[test]
    fn cluster_accounting_preserves_direct_hotload_and_energy_rolls() {
        for seed in 0..=255 {
            for (weapon, fire_mode, count) in [
                (BattleWeapon::Lrm20, BattleFireMode::Normal, 1),
                (BattleWeapon::Lrm20, BattleFireMode::Hotload, 0),
                (BattleWeapon::MediumLaser, BattleFireMode::Normal, 0),
            ] {
                let mut dice = BattleDice::seeded([seed; 32]);
                let mut expected = dice.clone();
                let roll = if weapon == BattleWeapon::MediumLaser {
                    None
                } else if fire_mode == BattleFireMode::Hotload {
                    let a = expected.d6();
                    let b = expected.d6();
                    let c = expected.d6();
                    Some(a + b + c - a.max(b).max(c))
                } else {
                    Some(expected.two_d6())
                };
                let groups = roll_weapon_groups(
                    WeaponGroupRequest {
                        range_damage: false,
                        submerged: false,
                        damage_penalty: 0,
                        weapon,
                        ammunition: BattleAmmunitionMode::Normal,
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

    /// Cover follows defensive interception and removes whole projectiles for every payload size.
    #[test]
    fn missile_cover_preserves_payload_grouping_and_consumes_no_dice() {
        for (weapon, mode) in [
            (BattleWeapon::Lrm5, BattleAmmunitionMode::Normal),
            (BattleWeapon::Lrm20, BattleAmmunitionMode::Normal),
            (BattleWeapon::Srm2, BattleAmmunitionMode::Normal),
            (BattleWeapon::Srm6, BattleAmmunitionMode::Normal),
            (BattleWeapon::StreakSrm6, BattleAmmunitionMode::Normal),
            (BattleWeapon::Thunderbolt20, BattleAmmunitionMode::Normal),
            (BattleWeapon::Mml5, BattleAmmunitionMode::Normal),
            (BattleWeapon::Mml5, BattleAmmunitionMode::MmlLrm),
        ] {
            for seed in 0..=255 {
                for glancing in [false, true] {
                    for intercepted in [0, 1, 255] {
                        for reduction in [2, 4] {
                            let mut dice = BattleDice::seeded([seed; 32]);
                            let mut groups = roll_weapon_groups(
                                WeaponGroupRequest {
                                    submerged: false,
                                    range_damage: false,
                                    damage_penalty: 0,
                                    weapon,
                                    ammunition: mode,
                                    fire_mode: BattleFireMode::Normal,
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
                            let before_dice = dice.clone();
                            let (_, survivors) = groups.intercept(weapon, intercepted).unwrap();
                            let payload = groups.missile_payload();
                            let expected_before = survivors * payload;
                            let expected_after =
                                expected_before.saturating_sub(reduction) / payload * payload;
                            assert_eq!(
                                groups.absorb_missiles(reduction),
                                (expected_before, expected_after)
                            );
                            assert_eq!(groups.damage.iter().sum::<u16>(), expected_after);
                            assert!(
                                groups
                                    .damage
                                    .iter()
                                    .all(|damage| *damage > 0 && damage % payload == 0)
                            );
                            assert_eq!(dice, before_dice);
                        }
                    }
                }
            }
        }
    }

    /// Water damage uses raw distance and applies focusing damage before range and glancing.
    #[test]
    fn underwater_energy_damage_uses_water_bands_and_missing_long_range() {
        for (weapon, distance, penalty, expected) in [
            (BattleWeapon::Ppc, 1.0, 0, 11),
            (BattleWeapon::Ppc, 1.01, 0, 10),
            (BattleWeapon::Ppc, 7.0, 0, 10),
            (BattleWeapon::Ppc, 7.01, 0, 9),
            (BattleWeapon::Ppc, 10.0, 0, 9),
            (BattleWeapon::Ppc, 10.01, 0, 5),
            (BattleWeapon::Ppc, 10.01, 2, 4),
            (BattleWeapon::SmallLaser, 1.0, 0, 4),
            (BattleWeapon::SmallLaser, 1.01, 0, 1),
            (BattleWeapon::SmallLaser, 2.0, 0, 1),
        ] {
            for glancing in [false, true] {
                let mut dice = BattleDice::seeded([3; 32]);
                let before = dice.clone();
                let groups = roll_weapon_groups(
                    WeaponGroupRequest {
                        submerged: true,
                        range_damage: true,
                        damage_penalty: penalty,
                        weapon,
                        ammunition: BattleAmmunitionMode::Normal,
                        fire_mode: BattleFireMode::Normal,
                        gatling_damage: None,
                        distance: Some(distance),
                        glancing,
                        guidance_blocked: false,
                        angel_blocked: false,
                        target_beacon: false,
                        artemis_v: false,
                    },
                    &mut dice,
                )
                .unwrap();
                let expected: u16 = expected;
                assert_eq!(
                    groups.damage,
                    vec![if glancing {
                        expected.div_ceil(2)
                    } else {
                        expected
                    }]
                );
                assert_eq!(dice, before);
            }
        }
    }

    /// Actual distance controls the optional bonus, penalty and extended-range halving.
    #[test]
    fn configured_energy_range_damage_precedes_glancing_and_preserves_dice() {
        for (distance, expected) in [
            (0.0, 11),
            (1.0, 11),
            (1.01, 10),
            (12.0, 10),
            (12.01, 9),
            (18.0, 9),
            (18.01, 5),
        ] {
            for enabled in [false, true] {
                for glancing in [false, true] {
                    let mut dice = BattleDice::seeded([3; 32]);
                    let before = dice.clone();
                    let groups = roll_weapon_groups(
                        WeaponGroupRequest {
                            submerged: false,
                            range_damage: enabled,
                            damage_penalty: 0,
                            weapon: BattleWeapon::Ppc,
                            ammunition: BattleAmmunitionMode::Normal,
                            fire_mode: BattleFireMode::Normal,
                            gatling_damage: None,
                            distance: Some(distance),
                            glancing,
                            guidance_blocked: false,
                            angel_blocked: false,
                            target_beacon: false,
                            artemis_v: false,
                        },
                        &mut dice,
                    )
                    .unwrap();
                    let expected: u16 = if enabled { expected } else { 10 };
                    assert_eq!(
                        groups.damage,
                        vec![if glancing {
                            expected.div_ceil(2)
                        } else {
                            expected
                        }]
                    );
                    assert_eq!(dice, before);
                }
            }
        }
        // A ballistic weapon keeps its own range profile, and a degraded energy hit floors at one.
        for (weapon, penalty, distance, expected) in [
            (BattleWeapon::HeavyGaussRifle, 0, 1.0, 25),
            (BattleWeapon::Ppc, 1, 19.0, 4),
            (BattleWeapon::SmallLaser, 1, 4.0, 1),
        ] {
            let groups = roll_weapon_groups(
                WeaponGroupRequest {
                    submerged: false,
                    range_damage: true,
                    damage_penalty: penalty,
                    weapon,
                    ammunition: BattleAmmunitionMode::Normal,
                    fire_mode: BattleFireMode::Normal,
                    gatling_damage: None,
                    distance: Some(distance),
                    glancing: false,
                    guidance_blocked: false,
                    angel_blocked: false,
                    target_beacon: false,
                    artemis_v: false,
                },
                &mut BattleDice::seeded([0; 32]),
            )
            .unwrap();
            assert_eq!(groups.damage, vec![expected]);
        }
    }

    /// Focus loss reduces energy packets before glancing rounding, for every damage recipient.
    #[test]
    fn energy_degradation_precedes_glancing_packet_rounding() {
        for (glancing, expected) in [(false, 9), (true, 5)] {
            let mut dice = BattleDice::seeded([0; 32]);
            let before = dice.clone();
            let groups = roll_weapon_groups(
                WeaponGroupRequest {
                    submerged: false,
                    range_damage: false,
                    weapon: BattleWeapon::Ppc,
                    ammunition: BattleAmmunitionMode::Normal,
                    fire_mode: BattleFireMode::Normal,
                    gatling_damage: None,
                    damage_penalty: 1,
                    distance: Some(4.0),
                    glancing,
                    guidance_blocked: false,
                    angel_blocked: false,
                    target_beacon: false,
                    artemis_v: false,
                },
                &mut dice,
            )
            .unwrap();
            assert_eq!(groups.damage, vec![expected]);
            assert_eq!(dice, before);
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
            (BattleWeapon::ClanStreakLrm5, BattleWeapon::ClanLrm5),
            (BattleWeapon::ClanStreakLrm10, BattleWeapon::ClanLrm10),
            (BattleWeapon::ClanStreakLrm15, BattleWeapon::ClanLrm15),
            (BattleWeapon::ClanStreakLrm20, BattleWeapon::ClanLrm20),
        ] {
            for seed in 0..=255 {
                for confused in [false, true] {
                    for glancing in [false, true] {
                        let mut dice = BattleDice::seeded([seed; 32]);
                        let mut expected_dice = dice.clone();
                        let roll = expected_dice.two_d6();
                        let groups = roll_weapon_groups(
                            WeaponGroupRequest {
                                submerged: false,
                                range_damage: false,
                                damage_penalty: 0,
                                weapon,
                                ammunition: BattleAmmunitionMode::Normal,
                                fire_mode: BattleFireMode::Normal,
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
