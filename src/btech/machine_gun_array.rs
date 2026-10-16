//! Machine gun array volleys: firing a gun its array links fires every ready linked gun the
//! ammunition feed can supply as one attack, rolled on the Cluster Hits Table column for the
//! number of guns with every hit in one location (Total Warfare p. 137). Mechs and vehicles
//! share the rule; each reads availability from its own critical model. Unit shots and hex
//! shots on both chassis spend the linked guns through [`fire_linked_guns`].
use super::{AmmunitionDraw, FireMode, Mech, Notice, Vehicle};
use crate::{ObjectId, World};
use anyhow::Result;
use std::collections::HashMap;

impl Mech {
    /// The guns, in weapon order, that fire together when the gun at `index` fires: every
    /// linked gun that is ready, set to normal fire and fed a round, while the array module
    /// survives and at least two linked guns are ready. A destroyed array leaves its guns
    /// firing alone (Total Warfare p. 137), and a gun in rapid fire fires alone, since an
    /// array never rapid-fires (Tactical Operations p. 100).
    ///
    /// The selected gun is fed first and the rest in weapon order; a gun the shared bins
    /// cannot supply stays silent, so the list may hold the selected gun alone when its
    /// round was the last one the array could draw.
    pub fn array_volley(&self, index: usize) -> Result<Option<Vec<usize>>> {
        let loadout = self.loadout()?;
        let Some(array) = loadout
            .machine_gun_arrays()
            .into_iter()
            .find(|array| array.weapon_indices.contains(&index))
        else {
            return Ok(None);
        };
        if self.critical_unavailable(array.location)
            || self.effective_fire_mode(index)? != FireMode::Normal
        {
            return Ok(None);
        }
        let mut ready = Vec::new();
        for &gun in &array.weapon_indices {
            if self.weapon_readiness_with_loadout(&loadout, gun)?.ready
                && self.effective_fire_mode(gun)? == FireMode::Normal
            {
                ready.push(gun);
            }
        }
        ready.sort_unstable();
        if ready.len() < 2 || !ready.contains(&index) {
            return Ok(None);
        }
        let guns = fed_guns(index, &ready, |gun| {
            let mount = &loadout.weapons[gun];
            if mount.one_shot || mount.weapon.profile().ammunition_per_ton == 0 {
                return Ok(None);
            }
            self.ammunition_feed_with_loadout(&loadout, gun, u16::MAX)
                .map(Some)
        })?;
        Ok((!guns.is_empty()).then_some(guns))
    }
}

impl Vehicle {
    /// The guns, in weapon order, that fire together when the gun at `index` fires, under the
    /// rule [`Mech::array_volley`] describes: every ready linked gun in normal fire that the
    /// ammunition feed can supply, while the array module survives and at least two linked
    /// guns are ready (Total Warfare p. 137; Tactical Operations p. 100).
    pub fn array_volley(&self, index: usize) -> Result<Option<Vec<usize>>> {
        let loadout = self.loadout()?;
        let Some(array) = loadout
            .machine_gun_arrays()
            .into_iter()
            .find(|array| array.weapon_indices.contains(&index))
        else {
            return Ok(None);
        };
        if self.critical_unavailable(array.location)
            || self.effective_fire_mode(index)? != FireMode::Normal
        {
            return Ok(None);
        }
        let mut ready = Vec::new();
        for &gun in &array.weapon_indices {
            if self.weapon_readiness_with_loadout(&loadout, gun)?.ready
                && self.effective_fire_mode(gun)? == FireMode::Normal
            {
                ready.push(gun);
            }
        }
        ready.sort_unstable();
        if ready.len() < 2 || !ready.contains(&index) {
            return Ok(None);
        }
        let guns = fed_guns(index, &ready, |gun| {
            let mount = &loadout.weapons[gun];
            if mount.one_shot || mount.weapon.profile().ammunition_per_ton == 0 {
                return Ok(None);
            }
            self.ammunition_feed_with_loadout(&loadout, gun, u16::MAX)
                .map(Some)
        })?;
        Ok((!guns.is_empty()).then_some(guns))
    }
}

/// The ready guns, in weapon order, that the live bins can each supply one normal-fire
/// cycle, drawing for `selected` first and then for the rest in weapon order, as the volley
/// spends them. `feed` lists every bin a gun may draw from, in feed-priority order, with its
/// live rounds, or `None` for a gun that needs no ammunition.
fn fed_guns(
    selected: usize,
    ready: &[usize],
    feed: impl Fn(usize) -> Result<Option<Vec<AmmunitionDraw>>>,
) -> Result<Vec<usize>> {
    let rounds = FireMode::Normal.rounds_per_cycle();
    let mut taken: HashMap<usize, u16> = HashMap::new();
    let mut guns = Vec::new();
    let order =
        std::iter::once(selected).chain(ready.iter().copied().filter(|gun| *gun != selected));
    for gun in order {
        let Some(bins) = feed(gun)? else {
            guns.push(gun);
            continue;
        };
        let mut draws = Vec::new();
        let mut remaining = rounds;
        for bin in bins {
            if remaining == 0 {
                break;
            }
            let available = bin.rounds - taken.get(&bin.bin_index).copied().unwrap_or(0);
            let draw = available.min(remaining);
            if draw > 0 {
                draws.push((bin.bin_index, draw));
                remaining -= draw;
            }
        }
        if remaining > 0 {
            continue;
        }
        for (bin, draw) in draws {
            *taken.entry(bin).or_default() += draw;
        }
        guns.push(gun);
    }
    guns.sort_unstable();
    Ok(guns)
}

/// Spend and recycle the linked guns of `volley` beside the gun at `selected`, which the
/// caller has already launched, and tell the shooter how many guns fired. Returns the number
/// of guns for the Cluster Hits Table column, or `None` when fewer than two guns fired and
/// the attack resolves as the selected gun's own.
pub(super) fn fire_linked_guns(
    world: &mut World,
    shooter: ObjectId,
    pilot: ObjectId,
    selected: usize,
    volley: Option<&[usize]>,
    launched: bool,
    notices: &mut Vec<Notice>,
) -> Result<Option<u8>> {
    let Some(guns) = volley else {
        return Ok(None);
    };
    let vehicle = world.btech.vehicles().contains_key(&shooter);
    for &gun in guns.iter().filter(|gun| **gun != selected) {
        if vehicle {
            // Vehicles shed weapon heat, so only the spent rounds and recycle matter here.
            let _linked = super::vehicle_readiness::reserve_prepared_weapon(
                world, shooter, pilot, gun, launched, None,
            )?;
        } else {
            let _linked = super::readiness::use_weapon(world, shooter, pilot, gun, launched, None)?;
        }
    }
    let text = if guns.len() >= 2 {
        format!("Your machine gun array fires {} guns together.", guns.len())
    } else {
        "Your machine gun array has ammunition for only one gun.".to_owned()
    };
    notices.push(Notice {
        unit: shooter,
        text,
    });
    Ok((guns.len() >= 2).then_some(guns.len() as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One bin holding `rounds` rounds.
    fn bin(rounds: u16) -> Option<Vec<AmmunitionDraw>> {
        Some(vec![AmmunitionDraw {
            bin_index: 0,
            rounds,
        }])
    }

    #[test]
    fn a_shared_bin_feeds_the_selected_gun_first() {
        let guns = fed_guns(5, &[3, 4, 5], |_| Ok(bin(1))).unwrap();
        assert_eq!(guns, vec![5]);
        let guns = fed_guns(5, &[3, 4, 5], |_| Ok(bin(2))).unwrap();
        assert_eq!(guns, vec![3, 5]);
        let guns = fed_guns(4, &[3, 4, 5], |_| Ok(bin(200))).unwrap();
        assert_eq!(guns, vec![3, 4, 5]);
    }

    #[test]
    fn guns_draw_across_bins_and_skip_empty_feeds() {
        let feed = |gun: usize| {
            Ok(match gun {
                3 => Some(Vec::new()),
                _ => Some(vec![
                    AmmunitionDraw {
                        bin_index: 0,
                        rounds: 1,
                    },
                    AmmunitionDraw {
                        bin_index: 1,
                        rounds: 1,
                    },
                ]),
            })
        };
        assert_eq!(fed_guns(4, &[3, 4, 5, 6], feed).unwrap(), vec![4, 5]);
        assert_eq!(fed_guns(4, &[3, 4], |_| Ok(None)).unwrap(), vec![3, 4]);
    }
}
