//! Chassis-independent fall damage, hit direction and heading change.
use super::{Hex, HitArc};
use anyhow::{Context, Result, ensure};

/// Select the supporting surface for descent, including ice, passage below a bridge and the
/// floor a unit inside a building stands on.
pub(super) fn surface(tile: Hex, elevation: i32) -> i16 {
    let upper = tile.standing_height();
    if i32::from(upper) <= elevation {
        return upper;
    }
    if tile.has_bridge() {
        return tile.water_line() - 1;
    }
    if super::building_interior::inside(tile, elevation) {
        return i16::try_from(elevation).unwrap_or(upper);
    }
    tile.surface_height()
}

/// Fall damage: tonnage divided by ten, rounded up, times the levels fallen plus one (Total
/// Warfare p. 68). Water halves it, rounding the final damage down (p. 57), and unusual
/// gravity scales it by the world's G-rating in either direction (Tactical Operations p. 53).
/// `levels` is the multiplier, levels fallen plus one.
pub(super) fn damage(tons: u32, levels: i32, wet: bool, gravity: Option<i64>) -> Result<u32> {
    let mut damage = u64::from(levels.max(0) as u32) * u64::from(tons.div_ceil(10));
    if wet {
        damage /= 2;
    }
    if let Some(gravity) = gravity {
        damage = damage * gravity.max(0) as u64 / 100;
    }
    u32::try_from(damage).context("Fall damage exceeds supported range")
}

/// One d6 chooses the struck side and rotates heading in sixty-degree steps.
pub(super) fn direction(roll: u8) -> Result<(HitArc, u16)> {
    ensure!((1..=6).contains(&roll), "Invalid fall direction roll");
    let arc = match roll {
        1 => HitArc::Front,
        2 | 3 => HitArc::Right,
        4 => HitArc::Rear,
        _ => HitArc::Left,
    };
    Ok((arc, u16::from(roll - 1) * 60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// Water halves, and gravity scales the result in both directions.
    #[test]
    fn damage_scales_with_levels_water_and_gravity() {
        assert_eq!(damage(55, 1, false, None).unwrap(), 6);
        assert_eq!(damage(55, 3, false, None).unwrap(), 18);
        assert_eq!(damage(55, 2, true, None).unwrap(), 6);
        assert_eq!(damage(50, 1, false, Some(50)).unwrap(), 2);
        assert_eq!(damage(50, 1, false, Some(200)).unwrap(), 10);
        assert_eq!(damage(50, 1, false, Some(-5)).unwrap(), 0);
    }

    /// Total Warfare p. 68 divides the tonnage by ten and rounds up before multiplying by the
    /// levels fallen plus one; p. 57 halves a water fall and rounds the result down.
    #[test]
    fn damage_rounds_tonnage_up_before_multiplying() {
        // A 50-ton 'Mech falling one level: 5 x 2.
        assert_eq!(damage(50, 2, false, None).unwrap(), 10);
        // A 40-ton 'Mech missing a death from above falls two levels' worth: 4 x 3.
        assert_eq!(damage(40, 3, false, None).unwrap(), 12);
        // A 100-ton 'Mech falling three levels: 10 x 4.
        assert_eq!(damage(100, 4, false, None).unwrap(), 40);
        // A 50-ton 'Mech tumbling into Depth 2 water: 5 x 3 / 2, rounded down.
        assert_eq!(damage(50, 3, true, None).unwrap(), 7);
        // A 21-ton vehicle's same-level fall: 2.1 rounds up to 3.
        assert_eq!(damage(21, 1, false, None).unwrap(), 3);
        assert_eq!(damage(8, 1, false, None).unwrap(), 1);
        assert_eq!(damage(21, 0, false, None).unwrap(), 0);
    }

    #[test]
    fn descent_surface_preserves_decks_ice_and_submerged_passage() {
        for (terrain, altitude, expected) in [
            (Terrain::Clear, 5, 3),
            (Terrain::Clear, 1, 3),
            (Terrain::Water, 5, -3),
            (Terrain::Water, -4, -3),
            (Terrain::Ice, 5, 0),
            (Terrain::Ice, 0, 0),
            (Terrain::Ice, -1, -3),
            (Terrain::Bridge, 5, 3),
            (Terrain::Bridge, 3, 3),
            (Terrain::Bridge, 2, -1),
            (Terrain::Bridge, -2, -1),
        ] {
            assert_eq!(surface(Hex::new(terrain, 3), altitude), expected);
        }
    }
}
