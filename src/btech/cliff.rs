//! Shared speed-based cliff avoidance and vehicle impact severity.
/// Cliff avoidance retains the signed-speed offset used by the ordinary rules.
pub(super) fn modifier(speed: f64, skid: bool) -> i16 {
    if !skid {
        return ((speed + 10.75).abs() / 10.75) as i16 / 3;
    }
    let mp = speed.abs() / 10.75;
    if mp < 2.1 {
        return -1;
    }
    if mp < 4.1 {
        return 0;
    }
    if mp < 7.1 {
        return 1;
    }
    if mp < 10.1 {
        return 2;
    }
    4
}

/// Vehicle crashes truncate signed speed; skid rules retain a zero-damage fall.
pub(super) fn vehicle_levels(speed: f64, skid: bool) -> u8 {
    if skid {
        return 0;
    }
    (speed / 10.75 / 4.0).clamp(0.0, 255.0) as u8
}

/// Pilotless units stop automatically; auto-fall suppresses only downhill avoidance.
pub(super) fn avoids(
    world: &mut crate::World,
    id: crate::ObjectId,
    change: i16,
    speed: f64,
    rules: super::BattleMovementRules,
) -> anyhow::Result<super::terrain_control::TerrainControl> {
    let (pilot, auto_fall) = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        (unit.pilot(), unit.auto_fall())
    });
    if pilot.is_some() && change < 0 && auto_fall {
        return Ok(super::terrain_control::TerrainControl::automatic(false));
    }
    super::terrain_control::check(
        world,
        id,
        modifier(speed, rules.skid_cliff),
        rules.fall.extended_piloting,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicle_crash_severity_preserves_signed_speed_and_skid_zero() {
        for (mp, levels) in [
            (-8.0, 0),
            (0.0, 0),
            (3.99, 0),
            (4.0, 1),
            (7.99, 1),
            (8.0, 2),
        ] {
            assert_eq!(vehicle_levels(mp * 10.75, false), levels);
            assert_eq!(vehicle_levels(mp * 10.75, true), 0);
        }
    }
}
