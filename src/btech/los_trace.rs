//! Bounded memoization of pure, directed center-to-center hex topology.
//!
//! Entries contain no terrain, sensor facts, unit IDs or world state. Reusing a
//! cell sequence never reuses a LOS result: each caller evaluates live terrain.
use super::HexCoordinate;
use anyhow::Result;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

pub(crate) const MAX_ENTRIES: usize = 32_768;
pub(crate) const MAX_CELLS: usize = 1_000_000;
type Key = (i32, i32, i32, i32);

#[derive(Default)]
struct TraceCache {
    entries: HashMap<Key, Rc<[HexCoordinate]>>,
    fifo: VecDeque<Key>,
    cells: usize,
}
impl TraceCache {
    fn get(
        &mut self,
        from: HexCoordinate,
        to: HexCoordinate,
        max_entries: usize,
        max_cells: usize,
    ) -> Result<Rc<[HexCoordinate]>> {
        let key = (from.x, from.y, to.x, to.y);
        if let Some(cells) = self.entries.get(&key) {
            return Ok(cells.clone());
        }
        let cells: Rc<[HexCoordinate]> = from.center().trace(to.center())?.into();
        if max_entries == 0 || cells.len() > max_cells {
            return Ok(cells);
        }
        while self.entries.len() >= max_entries || self.cells + cells.len() > max_cells {
            let oldest = self.fifo.pop_front().expect("nonempty bounded cache");
            self.cells -= self.entries.remove(&oldest).unwrap().len();
        }
        self.cells += cells.len();
        self.entries.insert(key, cells.clone());
        self.fifo.push_back(key);
        Ok(cells)
    }
}
thread_local! { static CACHE: RefCell<TraceCache> = RefCell::new(TraceCache::default()); }

/// Directed keys preserve exact boundary ownership; reversing a trace is not assumed valid.
pub(super) fn center_trace(from: HexCoordinate, to: HexCoordinate) -> Result<Rc<[HexCoordinate]>> {
    CACHE.with(|cache| cache.borrow_mut().get(from, to, MAX_ENTRIES, MAX_CELLS))
}

/// Each benchmark repetition starts with the same empty topology cache.
pub(crate) fn clear() {
    CACHE.with(|cache| *cache.borrow_mut() = TraceCache::default());
}

/// Retained topology counts, independently bounded from navigation search records.
pub(crate) fn retained() -> (usize, usize) {
    CACHE.with(|cache| {
        let cache = cache.borrow();
        (cache.entries.len(), cache.cells)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_traces_match_directed_geometry_and_remain_bounded() {
        let mut cache = TraceCache::default();
        for x in 0..12 {
            for y in 0..12 {
                for (a, b) in [(0, 0), (1, 5), (5, 1), (11, 11)] {
                    let from = HexCoordinate { x, y };
                    let to = HexCoordinate { x: a, y: b };
                    let expected = from.center().trace(to.center()).unwrap();
                    for _ in 0..2 {
                        assert_eq!(&*cache.get(from, to, 8, 40).unwrap(), expected);
                        assert!(cache.entries.len() <= 8);
                        assert!(cache.cells <= 40);
                        assert_eq!(
                            cache.cells,
                            cache
                                .entries
                                .values()
                                .map(|cells| cells.len())
                                .sum::<usize>()
                        );
                    }
                }
            }
        }
        let from = HexCoordinate { x: 0, y: 0 };
        let to = HexCoordinate { x: 0, y: 100 };
        let before = (cache.entries.len(), cache.cells);
        assert_eq!(
            &*cache.get(from, to, 8, 40).unwrap(),
            from.center().trace(to.center()).unwrap()
        );
        assert_eq!(before, (cache.entries.len(), cache.cells));
    }
}
