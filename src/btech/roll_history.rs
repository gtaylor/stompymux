//! Transactional roll history combines live journals with counts retained when their owners retire.
use crate::{BattleRollStatistics, ObjectId, World};
use anyhow::Result;
use std::{collections::BTreeSet, sync::Arc};

impl World {
    /// Read generic roll totals without draining journals, consuming dice or mutating simulation state.
    pub fn battle_roll_statistics(&self) -> Result<BattleRollStatistics> {
        let mut result = self.btech_retired_rolls.clone();
        for unit in self.btech.constructed_units().values() {
            result.merge(unit.dice.generic_roll_statistics())?;
        }
        for vehicle in self.btech.vehicles().values() {
            result.merge(vehicle.dice.generic_roll_statistics())?;
        }
        for map in self.btech.maps().values() {
            if let Some(dice) = &map.fire_dice {
                result.merge(dice.generic_roll_statistics())?;
            }
        }
        Ok(result)
    }

    /// Retain selected owners' journals immediately before removing them inside a world transaction.
    /// Preflight every merge so overflow leaves both history and journals untouched.
    pub(crate) fn retain_battle_rolls(&mut self, ids: &BTreeSet<ObjectId>) -> Result<()> {
        let mut history = self.btech_retired_rolls.clone();
        for id in ids {
            if let Some(unit) = self.btech.constructed_units().get(id) {
                history.merge(unit.dice.generic_roll_statistics())?;
            }
            if let Some(vehicle) = self.btech.vehicles().get(id) {
                history.merge(vehicle.dice.generic_roll_statistics())?;
            }
            if let Some(dice) = self
                .btech
                .maps()
                .get(id)
                .and_then(|map| map.fire_dice.as_ref())
            {
                history.merge(dice.generic_roll_statistics())?;
            }
        }
        for id in ids {
            if let Some(unit) = Arc::make_mut(&mut self.btech.constructed).get_mut(id) {
                unit.dice.take_generic_roll_statistics();
            }
            if let Some(vehicle) = Arc::make_mut(&mut self.btech.vehicles).get_mut(id) {
                vehicle.dice.take_generic_roll_statistics();
            }
            if let Some(dice) = Arc::make_mut(&mut self.btech.maps)
                .get_mut(id)
                .and_then(|map| map.fire_dice.as_mut())
            {
                dice.take_generic_roll_statistics();
            }
        }
        self.btech_retired_rolls = history;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BattleMapAsset, BattleTemplate, BattleVehicleTemplate, Config, Kind};

    /// Live streams, map replacement, retirement and a discarded candidate share one exact total.
    #[test]
    fn journals_survive_owner_retirement_but_not_restart_or_discarded_candidates() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World {
            next_id: 42,
            ..Default::default()
        };
        let mech = world.create(&config, "Mech".into(), Kind::Thing);
        let vehicle = world.create(&config, "Vehicle".into(), Kind::Thing);
        let map = world.create(&config, "Map".into(), Kind::Room);
        crate::create_battle_unit(
            &mut world,
            mech,
            BattleTemplate::parse(include_str!("../../game/mechs/JR7-D")).unwrap(),
        )
        .unwrap();
        crate::create_battle_vehicle(
            &mut world,
            vehicle,
            BattleVehicleTemplate::parse(include_str!("../../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
        crate::create_battle_map(
            &mut world,
            map,
            "test",
            BattleMapAsset::parse("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        let mut expected = BattleRollStatistics::default();
        for (id, count) in [(mech, 2), (vehicle, 3)] {
            let dice = super::super::dice::unit_dice_mut(&mut world, id).unwrap();
            for _ in 0..count {
                expected.record(dice.generic_roll()).unwrap();
            }
            dice.d6();
        }
        let dice = Arc::make_mut(&mut world.btech.maps)
            .get_mut(&map)
            .unwrap()
            .fire_dice
            .as_mut()
            .unwrap();
        for _ in 0..4 {
            expected.record(dice.generic_roll()).unwrap();
        }
        let encoded = serde_json::to_value(&world).unwrap();
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        assert_eq!(serde_json::to_value(&world).unwrap(), encoded);
        super::super::state::replace_map_asset(
            &mut world,
            map,
            "replacement",
            BattleMapAsset::parse("1 1\n~1\n").unwrap(),
        )
        .unwrap();
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);

        let mut candidate = world.clone();
        super::super::dice::unit_dice_mut(&mut candidate, mech)
            .unwrap()
            .generic_roll();
        candidate
            .retain_battle_rolls(&[mech].into_iter().collect())
            .unwrap();
        assert_eq!(candidate.battle_roll_statistics().unwrap().total(), 10);
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        let ids = [mech, vehicle, map].into_iter().collect();
        world.retain_battle_rolls(&ids).unwrap();
        world.retain_battle_rolls(&ids).unwrap();
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        assert_eq!(world.btech_retired_rolls, expected);
        world.btech.purge(&ids);
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        let restored: World =
            serde_json::from_value(serde_json::to_value(&world).unwrap()).unwrap();
        assert_eq!(restored.battle_roll_statistics().unwrap().total(), 0);
    }
}
