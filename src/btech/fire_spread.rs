//! Autonomous fire spreading, smoke creation and woodland burnout on candidate map state.
use super::{Decoration, DecorationKind, HexCoordinate, StoredMap};
use crate::World;
use anyhow::Result;
use std::{collections::BTreeSet, sync::Arc};

/// Whether any battlefield has a fire requiring simulation service.
pub fn map_fire_pending(world: &World) -> bool {
    world.btech.maps().values().any(|map| {
        map.decorations
            .values()
            .any(|effect| effect.kind == DecorationKind::Fire && effect.remaining != 0)
    })
}

/// Advance fire by one second; the host owns persistence rollback and sensor refresh.
pub fn advance_map_fire(world: &mut World) -> Result<()> {
    if !map_fire_pending(world) {
        return Ok(());
    }
    world.attempt(|world| {
        for map in world.btech.maps.values_mut() {
            advance_fire(map)?;
        }
        world.btech.validate_action(world)?;
        Ok(())
    })
}

/// Step existing clocks before processing events, so new markers receive their full lifetime.
fn advance_fire(map: &mut StoredMap) -> Result<()> {
    if !map
        .decorations
        .values()
        .any(|effect| effect.kind == DecorationKind::Fire && effect.remaining != 0)
    {
        return Ok(());
    }
    map.validate()?;
    let mut due = Vec::new();
    for (&index, effect) in Arc::make_mut(&mut map.decorations).iter_mut() {
        if effect.kind != DecorationKind::Fire || effect.remaining == 0 {
            continue;
        }
        if let Some(seconds) = &mut effect.next_spread {
            *seconds = seconds.saturating_sub(1);
        } else {
            effect.remaining = effect.remaining.saturating_sub(1);
        }
        if effect.remaining == 0 || effect.next_spread == Some(0) {
            due.push(index);
        }
    }
    let mut replaced = BTreeSet::new();
    for index in due {
        if replaced.contains(&index) {
            continue;
        }
        let effect = map.decorations[&index];
        if effect.next_spread == Some(0) {
            spread(map, index, &mut replaced)?;
            if replaced.contains(&index) {
                continue;
            }
            // A scheduled spread keeps its deadline when wind changes. The current
            // interval spends the fire budget and determines only the next event.
            let interval = map.fire_spread_interval();
            let effect = Arc::make_mut(&mut map.decorations).get_mut(&index).unwrap();
            effect.object_duration = effect.object_duration.wrapping_sub(interval as i16);
            effect.remaining = i64::from(effect.object_duration).max(1);
            effect.next_spread = (effect.remaining > i64::from(interval)).then_some(interval);
            continue;
        }
        if effect.remaining == 0 {
            Arc::make_mut(&mut map.decorations).remove(&index);
            let (x, y) = (i64::from(index) % map.width, i64::from(index) / map.width);
            let tile = map.base_hex(x, y)?;
            if let Some(burnt) = burn_out(tile, map.fire_dice.as_mut().unwrap()) {
                map.write_hex(x, y, burnt)?;
            }
            continue;
        }
    }
    Ok(())
}

/// The hex left when a fire over it burns out, if the fire changed it. Woods and jungle thin
/// one density, light woods and jungle burn away, and planted fields burn to stubble. Clear
/// ground beneath burnt-away woods or jungle is left rough two times in three; any other ground,
/// such as sand, keeps its own kind. Ice and snow melt to mud under a fire.
fn burn_out(tile: super::Hex, dice: &mut super::Dice) -> Option<super::Hex> {
    let melted =
        match tile.condition() {
            Some(
                super::Condition::Ice | super::Condition::ThinSnow | super::Condition::DeepSnow,
            ) if tile.water().is_none() => Some(tile.with_condition(Some(super::Condition::Mud))),
            Some(super::Condition::Ice) => Some(tile.thawed()),
            _ => None,
        };
    let tile = melted.unwrap_or(tile);
    let Some(foliage) = tile.foliage() else {
        return melted;
    };
    if let Some(thinner) = foliage.thinned() {
        return Some(tile.with_foliage(Some(thinner)));
    }
    let ground = match tile.ground() {
        super::Ground::Clear if foliage.density().is_some() && dice.d6() >= 3 => {
            super::Ground::Rough
        }
        ground => ground,
    };
    Some(tile.with_foliage(None).with_ground(ground))
}

/// Resolve all spread checks before smoke/fire duration draws; fire replaces smoke at shared tiles.
fn spread(map: &mut StoredMap, index: u32, replaced: &mut BTreeSet<u32>) -> Result<()> {
    let origin = HexCoordinate {
        x: (i64::from(index) % map.width) as i32,
        y: (i64::from(index) / map.width) as i32,
    };
    let targets = spread_hexes(map, origin)?;
    let mut ignite = [false; 4];
    for (slot, threshold) in [9, 11, 11, 12].into_iter().enumerate() {
        let dice = map.fire_dice.as_mut().unwrap();
        ignite[slot] =
            dice.generic_roll() >= threshold && i64::from(dice.die(60)?) <= map.wind_speed;
    }
    for index in targets.iter().flatten().copied() {
        if map.decorations.contains_key(&index) {
            continue;
        }
        let remaining = 89 + map.fire_dice.as_mut().unwrap().die(61)?;
        super::decorations::install_decoration(
            map,
            index,
            Decoration::new(DecorationKind::Smoke, i64::from(remaining), None),
        )?;
        replaced.insert(index);
    }
    for (index, ignite) in targets.into_iter().zip(ignite) {
        let Some(index) = index.filter(|_| ignite) else {
            continue;
        };
        let burning = map
            .decorations
            .get(&index)
            .is_some_and(|effect| effect.kind == DecorationKind::Fire);
        if burning
            || !map
                .base_hex(i64::from(index) % map.width, i64::from(index) / map.width)?
                .is_woods()
        {
            continue;
        }
        let remaining = 59 + map.fire_dice.as_mut().unwrap().die(121)?;
        let next_spread = Some(map.fire_spread_interval());
        super::decorations::install_decoration(
            map,
            index,
            Decoration::new(DecorationKind::Fire, i64::from(remaining), next_spread),
        )?;
        replaced.insert(index);
    }
    Ok(())
}

/// Wind-relative candidate cells: the downwind neighbor, the neighbors on either side of it
/// (counter-clockwise, then clockwise), and the second cell directly downwind.
fn spread_hexes(map: &StoredMap, origin: HexCoordinate) -> Result<[Option<u32>; 4]> {
    // A direction index into `StoredMap::neighbors`, clockwise from north.
    let downwind = ((map.wind_direction + 30) / 60 % 6) as usize;
    let around = map.neighbors(origin)?;
    let first = around[downwind];
    let second = match first {
        Some(first) => map.neighbors(first)?[downwind],
        None => None,
    };
    Ok([
        first,
        around[(downwind + 5) % 6],
        around[(downwind + 1) % 6],
        second,
    ]
    .map(|hex| hex.map(|hex| (i64::from(hex.y) * map.width + i64::from(hex.x)) as u32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_targets_follow_parity_rounding_and_map_bounds() {
        let mut map: StoredMap = serde_json::from_value(serde_json::json!({
            "name":"wind", "width":6, "height":6, "gravity":100, "temperature":20,
            "flags":0, "light":2, "visibility":30, "maximum_visibility":60, "cloud_base":200, "sensor_flags":0
        }))
        .unwrap();
        let even = HexCoordinate { x: 2, y: 2 };
        let odd = HexCoordinate { x: 3, y: 2 };
        // Downwind, counter-clockwise side, clockwise side, second downwind; index y * 6 + x.
        let expected = [
            (0, [8, 13, 15, 2], [9, 8, 10, 3]),
            (60, [15, 8, 21, 10], [10, 9, 16, 11]),
            (120, [21, 15, 20, 22], [16, 10, 21, 23]),
            (180, [20, 21, 19, 26], [21, 16, 14, 27]),
            (240, [19, 20, 13, 18], [14, 21, 8, 19]),
            (300, [13, 19, 8, 6], [8, 14, 9, 7]),
        ];
        for (direction, from_even, from_odd) in expected {
            map.wind_direction = direction;
            assert_eq!(
                spread_hexes(&map, even).unwrap(),
                from_even.map(Some),
                "{direction}"
            );
            assert_eq!(
                spread_hexes(&map, odd).unwrap(),
                from_odd.map(Some),
                "{direction}"
            );
        }
        // Bearings round to the nearest of the six directions.
        for direction in [29, 330, 359] {
            map.wind_direction = direction;
            assert_eq!(spread_hexes(&map, even).unwrap(), [8, 13, 15, 2].map(Some));
        }
        map.wind_direction = 30;
        assert_eq!(spread_hexes(&map, even).unwrap(), [15, 8, 21, 10].map(Some));
        map.wind_direction = 0;
        assert_eq!(
            spread_hexes(&map, HexCoordinate { x: 0, y: 0 }).unwrap(),
            [None, None, Some(1), None]
        );
    }

    #[test]
    fn burnout_thins_heavy_woods_and_keeps_the_ground_under_light_woods() {
        use crate::btech::{Foliage, Ground, Hex, Terrain};
        let mut dice = crate::btech::Dice::seeded([7; 32]);
        let heavy = Hex::new(Terrain::HeavyWoods, 3);
        assert_eq!(
            burn_out(heavy, &mut dice),
            Some(heavy.with_foliage(Some(Foliage::LightWoods)))
        );
        let jungle = Hex::new(Terrain::UltraHeavyJungle, 0);
        assert_eq!(
            burn_out(jungle, &mut dice),
            Some(jungle.with_foliage(Some(Foliage::HeavyJungle)))
        );
        for ground in [Ground::Swamp, Ground::Sand, Ground::Tundra, Ground::Rough] {
            let light = Hex::at_level(2)
                .with_ground(ground)
                .with_foliage(Some(Foliage::LightWoods));
            assert_eq!(
                burn_out(light, &mut dice),
                Some(Hex::at_level(2).with_ground(ground))
            );
        }
        let mut grounds = std::collections::BTreeSet::new();
        for _ in 0..64 {
            let burnt = burn_out(Hex::new(Terrain::LightWoods, 1), &mut dice).unwrap();
            assert_eq!((burnt.foliage(), burnt.level()), (None, 1));
            grounds.insert(format!("{:?}", burnt.ground()));
        }
        assert_eq!(
            grounds.len(),
            2,
            "clear ground burns to clear or rough: {grounds:?}"
        );
        assert_eq!(
            burn_out(Hex::new(Terrain::PlantedFields, 0), &mut dice),
            Some(Hex::new(Terrain::Clear, 0))
        );
        assert_eq!(burn_out(Hex::new(Terrain::Rough, 0), &mut dice), None);
    }

    /// Fire melts ice and snow on land to mud and thaws frozen water.
    #[test]
    fn burnout_melts_ice_and_snow() {
        use crate::btech::{Condition, Hex, Terrain};
        let mut dice = crate::btech::Dice::seeded([7; 32]);
        let snowy = Hex::new(Terrain::DeepSnow, 0);
        assert_eq!(
            burn_out(snowy, &mut dice),
            Some(snowy.with_condition(Some(Condition::Mud)))
        );
        let ice = Hex::new(Terrain::Ice, 2);
        assert_eq!(burn_out(ice, &mut dice), Some(Hex::new(Terrain::Water, 2)));
    }
}
