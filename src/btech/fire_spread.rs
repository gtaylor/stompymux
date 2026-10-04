//! Autonomous fire spreading, smoke creation and woodland burnout on candidate map state.
use super::{BattleDecoration, DecorationKind, HexCoordinate, StoredMap};
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

/// The hex left when a fire over it burns out, if the fire changed it. Heavy woods thin to
/// light woods, and light woods burn away. Clear ground beneath is left rough two times in
/// three; any other ground, such as a road or sand, keeps its own kind.
fn burn_out(tile: super::Hex, dice: &mut super::BattleDice) -> Option<super::Hex> {
    match tile.woods()? {
        super::Woods::Heavy => Some(tile.with_woods(Some(super::Woods::Light))),
        super::Woods::Light => {
            let ground = match tile.ground() {
                super::Ground::Clear if dice.d6() >= 3 => super::Ground::Rough,
                ground => ground,
            };
            Some(tile.with_woods(None).with_ground(ground))
        }
    }
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
            BattleDecoration::new(DecorationKind::Smoke, i64::from(remaining), None),
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
            BattleDecoration::new(DecorationKind::Fire, i64::from(remaining), next_spread),
        )?;
        replaced.insert(index);
    }
    Ok(())
}

/// Wind-relative candidate cells, including the second cell directly downwind.
///
/// Each row lists, for one wind bearing, the direction of the downwind cell and then the two
/// side branches, as indexes into [`StoredMap::neighbors`] (clockwise from north). The
/// rows follow the reference spread table, irregular rows included: north and north-west winds
/// order their side branches differently on even and odd columns, and a south-west wind's second
/// side branch is north-east rather than north-west.
fn spread_hexes(map: &StoredMap, origin: HexCoordinate) -> Result<[Option<u32>; 4]> {
    const EVEN: [[usize; 3]; 6] = [
        [0, 5, 1],
        [1, 0, 2],
        [2, 1, 3],
        [3, 2, 4],
        [4, 3, 1],
        [5, 0, 4],
    ];
    const ODD: [[usize; 3]; 6] = [
        [0, 1, 5],
        [1, 0, 2],
        [2, 1, 3],
        [3, 2, 4],
        [4, 3, 1],
        [5, 4, 0],
    ];
    let bearing = ((map.wind_direction + 30) / 60 % 6) as usize;
    let neighbor = |origin: HexCoordinate, branch: usize| -> Result<Option<HexCoordinate>> {
        let rows = if origin.x.rem_euclid(2) == 0 {
            &EVEN
        } else {
            &ODD
        };
        Ok(map.neighbors(origin)?[rows[bearing][branch]])
    };
    let first = neighbor(origin, 0)?;
    let second = match first {
        Some(first) => neighbor(first, 0)?,
        None => None,
    };
    Ok([first, neighbor(origin, 1)?, neighbor(origin, 2)?, second]
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
        for bearing in [0, 29, 330, 359] {
            map.wind_direction = bearing;
            assert_eq!(
                spread_hexes(&map, even).unwrap(),
                [Some(8), Some(13), Some(15), Some(2)]
            );
            assert_eq!(
                spread_hexes(&map, odd).unwrap(),
                [Some(9), Some(10), Some(8), Some(3)]
            );
        }
        map.wind_direction = 30;
        assert_eq!(
            spread_hexes(&map, even).unwrap(),
            [Some(15), Some(8), Some(21), Some(10)]
        );
        map.wind_direction = 240;
        assert_eq!(
            spread_hexes(&map, even).unwrap(),
            [Some(19), Some(20), Some(15), Some(18)]
        );
        // North-west winds order their side branches differently on odd columns.
        map.wind_direction = 300;
        assert_eq!(
            spread_hexes(&map, even).unwrap(),
            [Some(13), Some(8), Some(19), Some(6)]
        );
        assert_eq!(
            spread_hexes(&map, odd).unwrap(),
            [Some(8), Some(14), Some(9), Some(7)]
        );
        map.wind_direction = 0;
        assert_eq!(
            spread_hexes(&map, HexCoordinate { x: 0, y: 0 }).unwrap(),
            [None, None, Some(1), None]
        );
    }

    #[test]
    fn burnout_thins_heavy_woods_and_keeps_the_ground_under_light_woods() {
        use crate::btech::{Ground, Hex, Terrain, Woods};
        let mut dice = crate::btech::BattleDice::seeded([7; 32]);
        let heavy = Hex::new(Terrain::HeavyForest, 3);
        assert_eq!(
            burn_out(heavy, &mut dice),
            Some(heavy.with_woods(Some(Woods::Light)))
        );
        for ground in [Ground::Road, Ground::Sand, Ground::Snow, Ground::Mountains] {
            let light = Hex::at_level(2)
                .with_ground(ground)
                .with_woods(Some(Woods::Light));
            assert_eq!(
                burn_out(light, &mut dice),
                Some(Hex::at_level(2).with_ground(ground))
            );
        }
        let mut grounds = std::collections::BTreeSet::new();
        for _ in 0..64 {
            let burnt = burn_out(Hex::new(Terrain::LightForest, 1), &mut dice).unwrap();
            assert_eq!((burnt.woods(), burnt.level()), (None, 1));
            grounds.insert(format!("{:?}", burnt.ground()));
        }
        assert_eq!(
            grounds.len(),
            2,
            "clear ground burns to clear or rough: {grounds:?}"
        );
        assert_eq!(burn_out(Hex::new(Terrain::Rough, 0), &mut dice), None);
    }
}
