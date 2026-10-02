//! Autonomous fire spreading, smoke creation and woodland burnout on candidate map state.
use super::{
    BattleDecoration, BattleDecorationKind, BattleHexCoordinate, StoredBattleMap, Terrain,
};
use crate::World;
use anyhow::Result;
use std::{collections::BTreeSet, sync::Arc};

/// Whether any battlefield has a fire requiring simulation service.
pub fn map_fire_pending(world: &World) -> bool {
    world.btech.maps().values().any(|map| {
        map.decorations
            .values()
            .any(|effect| effect.kind == BattleDecorationKind::Fire && effect.remaining != 0)
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
fn advance_fire(map: &mut StoredBattleMap) -> Result<()> {
    if !map
        .decorations
        .values()
        .any(|effect| effect.kind == BattleDecorationKind::Fire && effect.remaining != 0)
    {
        return Ok(());
    }
    map.validate()?;
    let mut due = Vec::new();
    for (&index, effect) in Arc::make_mut(&mut map.decorations).iter_mut() {
        if effect.kind != BattleDecorationKind::Fire || effect.remaining == 0 {
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
            let tile = map.stored_hex(x, y)?;
            if tile.is_woods() {
                let terrain = if map.fire_dice.as_mut().unwrap().d6() < 3 {
                    Terrain::Grassland
                } else {
                    Terrain::Rough
                };
                map.write_hex(x, y, tile.with_terrain(terrain))?;
            }
            continue;
        }
    }
    Ok(())
}

/// Resolve all spread checks before smoke/fire duration draws; fire replaces smoke at shared tiles.
fn spread(map: &mut StoredBattleMap, index: u32, replaced: &mut BTreeSet<u32>) -> Result<()> {
    let origin = BattleHexCoordinate {
        x: (i64::from(index) % map.width) as i32,
        y: (i64::from(index) / map.width) as i32,
    };
    let targets = spread_hexes(map, origin);
    let mut ignite = [false; 4];
    for (slot, threshold) in [9, 11, 11, 12].into_iter().enumerate() {
        let dice = map.fire_dice.as_mut().unwrap();
        ignite[slot] =
            dice.generic_roll() >= threshold && i64::from(dice.die(60)?) <= map.wind_speed;
    }
    for index in targets.iter().flatten().copied() {
        if map.decorations.contains_key(&index)
            || matches!(
                map.hex(i64::from(index) % map.width, i64::from(index) / map.width)?
                    .terrain(),
                Terrain::Building | Terrain::Wall | Terrain::Fire | Terrain::Smoke
            )
        {
            continue;
        }
        let remaining = 89 + map.fire_dice.as_mut().unwrap().die(61)?;
        super::decorations::install_decoration(
            map,
            index,
            BattleDecoration::new(BattleDecorationKind::Smoke, i64::from(remaining), None),
        )?;
        replaced.insert(index);
    }
    for (index, ignite) in targets.into_iter().zip(ignite) {
        let Some(index) = index.filter(|_| ignite) else {
            continue;
        };
        if !map
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
            BattleDecoration::new(
                BattleDecorationKind::Fire,
                i64::from(remaining),
                next_spread,
            ),
        )?;
        replaced.insert(index);
    }
    Ok(())
}

/// Wind-relative candidate cells, including the second cell directly downwind.
fn spread_hexes(map: &StoredBattleMap, origin: BattleHexCoordinate) -> [Option<u32>; 4] {
    let bearing = ((map.wind_direction + 30) / 60 % 6) as usize;
    let neighbor = |origin: BattleHexCoordinate, branch: usize| {
        // Column parity determines offset coordinates; branches retain their distinct spread odds.
        const EVEN: [[(i32, i32); 3]; 6] = [
            [(0, -1), (-1, 0), (1, 0)],
            [(1, 0), (0, -1), (1, 1)],
            [(1, 1), (1, 0), (0, 1)],
            [(0, 1), (1, 1), (-1, 1)],
            [(-1, 1), (0, 1), (1, 0)],
            [(-1, 0), (0, -1), (-1, 1)],
        ];
        const ODD: [[(i32, i32); 3]; 6] = [
            [(0, -1), (1, -1), (-1, -1)],
            [(1, -1), (0, -1), (1, 0)],
            [(1, 0), (1, -1), (0, 1)],
            [(0, 1), (1, 0), (-1, 0)],
            [(-1, 0), (0, 1), (1, -1)],
            [(-1, -1), (-1, 0), (0, -1)],
        ];
        let (dx, dy) = if origin.x.rem_euclid(2) == 0 {
            EVEN[bearing][branch]
        } else {
            ODD[bearing][branch]
        };
        let x = origin.x + dx;
        let y = origin.y + dy;
        (x >= 0 && y >= 0 && i64::from(x) < map.width && i64::from(y) < map.height)
            .then_some(BattleHexCoordinate { x, y })
    };
    let first = neighbor(origin, 0);
    [
        first,
        neighbor(origin, 1),
        neighbor(origin, 2),
        first.and_then(|first| neighbor(first, 0)),
    ]
    .map(|hex| hex.map(|hex| (i64::from(hex.y) * map.width + i64::from(hex.x)) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_targets_follow_parity_rounding_and_map_bounds() {
        let mut map: StoredBattleMap = serde_json::from_value(serde_json::json!({
            "name":"wind", "width":6, "height":6, "gravity":100, "temperature":20,
            "flags":0, "light":2, "visibility":30, "maximum_visibility":60, "cloud_base":200, "sensor_flags":0
        }))
        .unwrap();
        let even = BattleHexCoordinate { x: 2, y: 2 };
        let odd = BattleHexCoordinate { x: 3, y: 2 };
        for bearing in [0, 29, 330, 359] {
            map.wind_direction = bearing;
            assert_eq!(
                spread_hexes(&map, even),
                [Some(8), Some(13), Some(15), Some(2)]
            );
            assert_eq!(
                spread_hexes(&map, odd),
                [Some(9), Some(10), Some(8), Some(3)]
            );
        }
        map.wind_direction = 30;
        assert_eq!(
            spread_hexes(&map, even),
            [Some(15), Some(8), Some(21), Some(10)]
        );
        map.wind_direction = 240;
        assert_eq!(
            spread_hexes(&map, even),
            [Some(19), Some(20), Some(15), Some(18)]
        );
        map.wind_direction = 0;
        assert_eq!(
            spread_hexes(&map, BattleHexCoordinate { x: 0, y: 0 }),
            [None, None, Some(1), None]
        );
    }
}
