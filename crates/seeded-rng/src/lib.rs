//! A small, dependency-free random number generator for seeded generation.
//!
//! Generators such as mapgen and namegen must reproduce their output from a seed forever, so
//! the algorithm is pinned here rather than borrowed from a crate whose algorithms may change
//! between releases. Each generation stage draws from its own [`Rng::stream`], so adding a
//! stage does not reshuffle the output of the stages beside it.

/// SplitMix64's finalizer: a fast, well-distributed 64-bit hash.
pub const fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Hash a stage label into a seed offset.
const fn label_hash(label: &str) -> u64 {
    let bytes = label.as_bytes();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash = (hash ^ bytes[index] as u64).wrapping_mul(0x0100_0000_01b3);
        index += 1;
    }
    hash
}

/// A seed from the clock and process, for specs that leave the seed out.
pub fn random_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    mix(nanos ^ u64::from(std::process::id()).rotate_left(32))
}

/// A SplitMix64 random number stream.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// The stream for one generation stage of a seeded generation.
    pub const fn stream(seed: u64, label: &str) -> Self {
        Self {
            state: mix(seed ^ label_hash(label)),
        }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.state)
    }

    /// A uniform value in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A uniform integer in `low..=high`.
    pub fn between(&mut self, low: i32, high: i32) -> i32 {
        if high <= low {
            return low;
        }
        let span = (i64::from(high) - i64::from(low) + 1) as u64;
        low + (self.next_u64() % span) as i32
    }

    /// A uniform index below `len`, which must be non-zero.
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }

    /// A uniform choice from a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    /// A choice from a non-empty slice weighted by `weight`, which must be positive for at
    /// least one item. Items with zero weight are never chosen.
    pub fn pick_weighted<'a, T>(&mut self, items: &'a [T], weight: impl Fn(&T) -> u32) -> &'a T {
        let total: u64 = items.iter().map(|item| u64::from(weight(item))).sum();
        assert!(total > 0, "weighted pick needs a positive total weight");
        let mut roll = self.next_u64() % total;
        for item in items {
            let weight = u64::from(weight(item));
            if roll < weight {
                return item;
            }
            roll -= weight;
        }
        unreachable!("roll is below the total weight")
    }

    /// True with probability `chance`.
    pub fn chance(&mut self, chance: f64) -> bool {
        self.unit() < chance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_independent() {
        let draw = |label| {
            let mut rng = Rng::stream(7, label);
            (0..4).map(|_| rng.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(draw("terrain"), draw("terrain"));
        assert_ne!(draw("terrain"), draw("roads"));
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut rng = Rng::stream(1, "test");
        for _ in 0..1000 {
            assert!((0.0..1.0).contains(&rng.unit()));
            assert!((-2..=3).contains(&rng.between(-2, 3)));
            assert!(rng.index(5) < 5);
            assert!([1, 2, 3].contains(rng.pick(&[1, 2, 3])));
        }
        assert_eq!(rng.between(4, 4), 4);
    }

    #[test]
    fn weighted_pick_skips_zero_weights_and_follows_proportions() {
        let mut rng = Rng::stream(3, "weights");
        let items = [("never", 0), ("rare", 1), ("common", 9)];
        let mut counts = [0; 3];
        for _ in 0..10_000 {
            let chosen = rng.pick_weighted(&items, |item| item.1);
            counts[items.iter().position(|item| item == chosen).unwrap()] += 1;
        }
        assert_eq!(counts[0], 0);
        assert!((700..1300).contains(&counts[1]), "{counts:?}");
        assert!((8700..9300).contains(&counts[2]), "{counts:?}");
    }
}
