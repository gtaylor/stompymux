//! Chassis-independent fall damage, hit direction and heading change.
use super::{BattleHex, BattleHitArc};
use anyhow::{Context, Result, ensure};

/// Select the supporting surface for descent, including ice and passage below a bridge.
pub(super) fn surface(tile: BattleHex, elevation: i32) -> i16 {
    let upper = tile.standing_height();
    if i32::from(upper) <= elevation {
        return upper;
    }
    if tile.has_bridge() {
        return tile.water_line() - 1;
    }
    tile.surface_height()
}

/// Water halves structural damage; special gravity reduces it but never increases it.
pub(super) fn damage(tons: u32, levels: i32, wet: bool, gravity: Option<i64>) -> Result<u32> {
    let mut damage =
        u64::from(levels.max(0) as u32) * (u64::from(tons) + 5) / if wet { 20 } else { 10 };
    if let Some(gravity) = gravity {
        damage = damage * gravity.clamp(0, 100) as u64 / 100;
    }
    u32::try_from(damage).context("Fall damage exceeds supported range")
}

/// One d6 chooses the struck side and rotates heading in sixty-degree steps.
pub(super) fn direction(roll: u8) -> Result<(BattleHitArc, u16)> {
    ensure!((1..=6).contains(&roll), "Invalid fall direction roll");
    let arc = match roll {
        1 => BattleHitArc::Front,
        2 | 3 => BattleHitArc::Right,
        4 => BattleHitArc::Rear,
        _ => BattleHitArc::Left,
    };
    Ok((arc, u16::from(roll - 1) * 60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    #[test]
    fn descent_surface_preserves_decks_ice_and_submerged_passage() {
        for (terrain, altitude, expected) in [
            (Terrain::Grassland, 5, 3),
            (Terrain::Grassland, 1, 3),
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
            assert_eq!(surface(BattleHex::new(terrain, 3), altitude), expected);
        }
    }
}
