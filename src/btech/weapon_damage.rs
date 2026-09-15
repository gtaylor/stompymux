//! Persistent weapon degradation and derived combat effects, shared by every firing target path.
use super::{BattleRangeBracket, BattleUnit, BattleWeapon, CriticalLocation};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The consequence carried by an occupied, damaged weapon slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleWeaponDamageKind {
    Superficial,
    Moderate,
    Focus,
    Crystal,
    Ranging,
    Barrel,
    Feed,
}

/// A damaged slot can accumulate distinct component effects through linked extension hits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleWeaponDamage {
    pub location: CriticalLocation,
    /// Empty means damaged without a component penalty; repeated effects do not stack.
    pub effects: BTreeSet<BattleWeaponDamageKind>,
}

impl BattleWeaponDamage {
    /// Create one damaged slot, representing superficial damage without a component flag.
    pub fn new(location: CriticalLocation, kind: BattleWeaponDamageKind) -> Self {
        Self {
            location,
            effects: (kind != BattleWeaponDamageKind::Superficial)
                .then_some(kind)
                .into_iter()
                .collect(),
        }
    }
}

/// Derived penalties; these are never persisted separately from the damaged slots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BattleWeaponDamageEffects {
    pub moderate: u8,
    pub ranging: u8,
    pub heat: u8,
    pub damage: u8,
    pub explosion: u8,
    pub jam: u8,
    pub feed_locked: bool,
}

impl BattleWeaponDamageEffects {
    /// Ranging and focus penalties apply outside short range; minimum-range penalties retain the short bracket.
    pub fn accuracy(self, bracket: Option<BattleRangeBracket>) -> u8 {
        self.moderate
            + if matches!(
                bracket,
                Some(BattleRangeBracket::Short | BattleRangeBracket::Minimum)
            ) {
                0
            } else {
                self.ranging
            }
    }

    /// A nonzero failure count gets a one-point threshold bonus against the attack roll.
    pub(super) fn explodes(self, roll: u8) -> bool {
        self.explosion > 0 && roll <= self.explosion + 1
    }

    /// Barrel damage jams the loader separately from a manually cleared feed jam.
    pub(super) fn jams(self, roll: u8) -> bool {
        self.jam > 0 && roll <= self.jam + 1
    }
}

/// Distinguish the energy family using the same catalogue classification as character gunnery.
fn energy(weapon: BattleWeapon) -> bool {
    weapon.gunnery_skill(true) == "Gunnery-Laser"
}

impl BattleUnit {
    /// Installed weapon damage records, including records retained on subsequently destroyed mounts.
    pub fn weapon_damage(&self) -> &[BattleWeaponDamage] {
        &self.weapon_damage
    }

    /// Aggregate one mount's slots, including any split sections, without consuming dice.
    pub fn weapon_damage_effects(&self, index: usize) -> Result<BattleWeaponDamageEffects> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        let mut effects = BattleWeaponDamageEffects::default();
        for damage in self
            .weapon_damage
            .iter()
            .filter(|damage| mount.criticals.contains(&damage.location))
        {
            use BattleWeaponDamageKind as Kind;
            for kind in &damage.effects {
                match kind {
                    Kind::Superficial => (),
                    Kind::Moderate => effects.moderate += 1,
                    Kind::Focus => {
                        effects.ranging += 1;
                        effects.damage += 1;
                    }
                    Kind::Crystal => {
                        effects.heat += 1;
                        effects.explosion += 1;
                    }
                    Kind::Ranging => effects.ranging += 1,
                    Kind::Barrel => effects.jam += 1,
                    Kind::Feed => {
                        effects.feed_locked = true;
                        effects.explosion += 1;
                    }
                }
            }
        }
        Ok(effects)
    }

    /// Classify an ordinary weapon critical. None requires destruction by the enclosing impact.
    /// The caller draws the two-dice roll even when the slot-count threshold forces destruction.
    pub(super) fn degrade_weapon(
        &mut self,
        location: CriticalLocation,
        roll: u8,
    ) -> Result<Option<BattleWeaponDamageKind>> {
        ensure!((2..=12).contains(&roll), "Invalid weapon critical roll");
        let loadout = self.loadout()?;
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, mount)| mount.criticals.contains(&location))
            .context("Slot is not a weapon")?;
        let primary = mount.criticals[0];
        let extension = location.section != primary.section;
        ensure!(
            extension
                || !self
                    .weapon_damage
                    .iter()
                    .any(|damage| damage.location == location),
            "Weapon slot is already damaged"
        );
        let location = if extension { primary } else { location };
        if !self.weapon_intact(index)? {
            return Ok(None);
        }
        let previous = self
            .weapon_damage
            .iter()
            .filter(|damage| mount.criticals.contains(&damage.location))
            .count();
        if (previous + 1) * 2 > mount.criticals.len() {
            return Ok(None);
        }
        let adjusted = usize::from(roll) + previous + 1;
        use BattleWeaponDamageKind as Kind;
        let kind = match adjusted {
            0..=3 => Kind::Superficial,
            4..=5 => Kind::Moderate,
            6..=7 if energy(mount.weapon) => Kind::Focus,
            8..=9 if energy(mount.weapon) => Kind::Crystal,
            6..=7 if mount.weapon.gunnery_skill(true) == "Gunnery-Missile" => Kind::Ranging,
            8..=9 if mount.weapon.gunnery_skill(true) == "Gunnery-Missile" => Kind::Feed,
            6..=7
                if mount.weapon.gunnery_skill(true) == "Gunnery-Ballistic"
                    || mount.weapon.is_artillery() =>
            {
                Kind::Barrel
            }
            8..=9
                if mount.weapon.gunnery_skill(true) == "Gunnery-Ballistic"
                    || mount.weapon.is_artillery() =>
            {
                Kind::Feed
            }
            _ => return Ok(None),
        };
        if let Some(record) = self
            .weapon_damage
            .iter_mut()
            .find(|record| record.location == location)
        {
            if kind != Kind::Superficial {
                record.effects.insert(kind);
            }
        } else {
            self.weapon_damage
                .push(BattleWeaponDamage::new(location, kind));
            self.weapon_damage.sort_by_key(|damage| damage.location);
        }
        Ok(Some(kind))
    }

    /// A destructive table result removes all damaged slots and spends one loss on an undamaged slot.
    /// Other slots remain installed on the broken mount and can take later critical hits.
    pub(super) fn destroy_degraded_weapon(
        &mut self,
        index: usize,
    ) -> Result<super::BattleCriticalLoss> {
        let loadout = self.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        ensure!(
            self.weapon_intact(index)?,
            "Weapon is already nonfunctional"
        );
        ensure!(
            matches!(
                self.critical_loss(mount.criticals[0])?,
                Some(super::BattleCriticalLoss::Weapon {
                    explosion_damage: 0,
                    ..
                })
            ),
            "Explosive critical requires the explosion handler"
        );
        let mut additional = true;
        let locations: Vec<_> = mount
            .criticals
            .iter()
            .copied()
            .filter(|location| {
                if self
                    .weapon_damage
                    .iter()
                    .any(|damage| damage.location == *location)
                {
                    return true;
                }
                if additional {
                    additional = false;
                    return true;
                }
                false
            })
            .collect();
        for location in locations {
            let _loss = self.destroy_critical(location)?;
        }
        Ok(super::BattleCriticalLoss::Weapon {
            index,
            explosion_damage: 0,
        })
    }

    /// Reject duplicate, nonweapon and family-incompatible saved damage.
    pub(super) fn validate_weapon_damage(&self) -> Result<()> {
        let loadout = self.loadout()?;
        let mut seen = std::collections::BTreeSet::new();
        for damage in &self.weapon_damage {
            ensure!(seen.insert(damage.location), "Duplicate weapon damage slot");
            let mount = loadout
                .weapons
                .iter()
                .find(|mount| mount.criticals.contains(&damage.location))
                .context("Damage on a nonweapon slot")?;
            use BattleWeaponDamageKind as Kind;
            for kind in &damage.effects {
                ensure!(
                    match kind {
                        Kind::Superficial => false,
                        Kind::Moderate => true,
                        Kind::Focus | Kind::Crystal => energy(mount.weapon),
                        Kind::Ranging => mount.weapon.gunnery_skill(true) == "Gunnery-Missile",
                        Kind::Barrel =>
                            mount.weapon.gunnery_skill(true) == "Gunnery-Ballistic"
                                || mount.weapon.is_artillery(),
                        Kind::Feed => matches!(
                            mount.weapon.gunnery_skill(true),
                            "Gunnery-Missile" | "Gunnery-Ballistic" | "Gunnery-Artillery"
                        ),
                    },
                    "Weapon damage does not match its family"
                );
            }
        }
        for mount in &loadout.weapons {
            let count = self
                .weapon_damage
                .iter()
                .filter(|damage| mount.criticals.contains(&damage.location))
                .count();
            ensure!(
                count * 2 <= mount.criticals.len(),
                "Too many damaged slots on one weapon"
            );
        }
        for &index in &self.weapon_damage_jams {
            ensure!(index < loadout.weapons.len(), "Invalid critical loader jam");
            ensure!(
                self.weapon_damage_effects(index)?.jam > 0,
                "Critical loader jam without barrel damage"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BattleSection, BattleTemplate, CriticalDefinition};

    /// A single test mount in a free torso, retaining normal engines and crew equipment.
    fn unit(weapon: BattleWeapon) -> (BattleUnit, usize, Vec<CriticalLocation>) {
        let mut template = BattleTemplate::parse(include_str!("../../game/mechs/JR7-D")).unwrap();
        let section = template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap();
        section.criticals.clear();
        for slot in 0..weapon.profile().critical_slots {
            section.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    modes: Vec::new(),
                    brand: None,
                },
            );
        }
        let unit = BattleUnit::from_template(template).unwrap();
        let loadout = unit.loadout().unwrap();
        let index = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals[0].section == BattleSection::LeftTorso)
            .unwrap();
        (unit, index, loadout.weapons[index].criticals.clone())
    }

    /// Family thresholds distinguish moderate damage, degraded components and outright loss.
    #[test]
    fn critical_family_thresholds_and_durable_effects() {
        use BattleWeaponDamageKind as Kind;
        for (weapon, first, second) in [
            (BattleWeapon::Ppc, Kind::Focus, Kind::Crystal),
            (BattleWeapon::Lrm20, Kind::Ranging, Kind::Feed),
            (BattleWeapon::Ac10, Kind::Barrel, Kind::Feed),
        ] {
            let (base, index, slots) = unit(weapon);
            for (roll, expected) in [
                (2, Some(Kind::Superficial)),
                (3, Some(Kind::Moderate)),
                (4, Some(Kind::Moderate)),
                (5, Some(first)),
                (6, Some(first)),
                (7, Some(second)),
                (8, Some(second)),
                (9, None),
                (12, None),
            ] {
                let mut unit = base.clone();
                assert_eq!(unit.degrade_weapon(slots[0], roll).unwrap(), expected);
                assert!(unit.weapon_intact(index).unwrap());
                assert_eq!(unit.mass().unwrap(), base.mass().unwrap());
                assert_eq!(
                    unit.critical_candidates(BattleSection::LeftTorso)
                        .contains(&slots[0]),
                    expected.is_none()
                );
                unit.validate().unwrap();
                let restored: BattleUnit =
                    serde_json::from_str(&serde_json::to_string(&unit).unwrap()).unwrap();
                restored.validate().unwrap();
                assert_eq!(
                    restored.weapon_damage_effects(index).unwrap(),
                    unit.weapon_damage_effects(index).unwrap()
                );
                if let Some(kind) = expected {
                    let effects = unit.weapon_damage_effects(index).unwrap();
                    assert_eq!(effects.feed_locked, kind == Kind::Feed);
                    assert_eq!(
                        effects.accuracy(Some(BattleRangeBracket::Minimum)),
                        effects.accuracy(Some(BattleRangeBracket::Short))
                    );
                    assert_eq!(effects.heat, u8::from(kind == Kind::Crystal));
                    assert_eq!(effects.damage, u8::from(kind == Kind::Focus));
                    assert_eq!(
                        effects.accuracy(Some(BattleRangeBracket::Short)),
                        u8::from(kind == Kind::Moderate)
                    );
                    assert_eq!(
                        effects.accuracy(Some(BattleRangeBracket::Long)),
                        u8::from(matches!(kind, Kind::Moderate | Kind::Focus | Kind::Ranging))
                    );
                    assert_eq!(
                        effects.explodes(2),
                        matches!(kind, Kind::Crystal | Kind::Feed)
                    );
                    assert!(!effects.explodes(3));
                    assert_eq!(effects.jams(2), kind == Kind::Barrel);
                    let before = unit.clone();
                    assert!(unit.degrade_weapon(slots[0], roll).is_err());
                    assert_eq!(unit, before);
                }
            }
        }
    }

    /// A hit beyond half the mount's slots forces destruction even with the lowest roll.
    #[test]
    fn accumulated_damage_forces_destruction_and_rejects_bad_records() {
        let (mut unit, index, slots) = unit(BattleWeapon::Ppc);
        assert!(unit.degrade_weapon(slots[0], 2).unwrap().is_some());
        assert!(unit.degrade_weapon(slots[1], 2).unwrap().is_none());
        assert_eq!(unit.weapon_damage().len(), 1);
        let mut duplicate = unit.clone();
        duplicate
            .weapon_damage
            .push(duplicate.weapon_damage[0].clone());
        assert!(duplicate.validate().is_err());
        let mut wrong_family = unit.clone();
        wrong_family.weapon_damage[0].effects = BTreeSet::from([BattleWeaponDamageKind::Barrel]);
        assert!(wrong_family.validate().is_err());
        assert!(unit.weapon_damage_effects(usize::MAX).is_err());
        unit.destroy_critical(slots[1]).unwrap();
        assert!(!unit.weapon_intact(index).unwrap());
        unit.validate().unwrap();
    }
    /// Destruction spends its one additional loss in mount order, independently of the selected slot.
    #[test]
    fn destructive_criticals_remove_damaged_slots_and_one_additional_slot() {
        let (base, index, slots) = unit(BattleWeapon::Ppc);
        for degraded in [false, true] {
            let mut unit = base.clone();
            if degraded {
                assert!(unit.degrade_weapon(slots[2], 5).unwrap().is_some());
            }
            assert!(unit.degrade_weapon(slots[1], 12).unwrap().is_none());
            assert!(matches!(
                unit.destroy_degraded_weapon(index).unwrap(),
                super::super::BattleCriticalLoss::Weapon {
                    explosion_damage: 0,
                    ..
                }
            ));
            assert!(unit.critical_destroyed(slots[0]));
            assert!(!unit.critical_destroyed(slots[1]));
            assert_eq!(unit.critical_destroyed(slots[2]), degraded);
            assert!(!unit.weapon_intact(index).unwrap());
            unit.validate().unwrap();
        }
    }
}
