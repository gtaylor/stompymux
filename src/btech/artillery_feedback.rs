//! Artillery launch descriptions and visible arrival notices use immutable flight facts.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;

/// Arrow launchers send missiles; other artillery fires rounds.
fn projectile(weapon: BattleWeapon) -> &'static str {
    if matches!(weapon, BattleWeapon::ArrowIv | BattleWeapon::ClanArrowIv) {
        "missile"
    } else {
        "round"
    }
}

/// Select the nearest named direction in catalogue order, retaining first ties and linear heading distance.
fn direction(bearing: i32) -> &'static str {
    [
        (0, "north"),
        (60, "northeast"),
        (90, "east"),
        (120, "southeast"),
        (180, "south"),
        (240, "southwest"),
        (270, "west"),
        (300, "northwest"),
    ]
    .into_iter()
    .min_by_key(|(angle, _)| (bearing - angle).abs())
    .unwrap()
    .1
}

/// Launch direction uses captured hex centers, not the shooter's later continuous position.
pub(super) fn launch_text(
    weapon: BattleWeapon,
    origin: HexCoordinate,
    target: HexCoordinate,
) -> Result<String> {
    let origin = origin.center();
    let target = target.center();
    let heading = if origin.x == target.x {
        if target.y < origin.y { 0 } else { 180 }
    } else {
        let signed = origin.bearing(target)?.unwrap_or(180.0) - 180.0;
        (((signed * 10.0) as i32 + 5) / 10 + 180).rem_euclid(360)
    };
    Ok(format!(
        "shoots a {} towards the {}!",
        projectile(weapon),
        direction(heading)
    ))
}

/// Snapshot observers before smoke, damage or casualties can change their view of the arriving round.
pub(super) fn arrival_notices(
    world: &World,
    map: ObjectId,
    weapon: BattleWeapon,
    mode: BattleArtilleryMode,
    coordinate: HexCoordinate,
) -> Result<Vec<BattleNotice>> {
    let name = weapon.name().split_once('.').expect("catalog namespace").1;
    super::broadcast::hex_notices(
        world,
        map,
        coordinate,
        mode != BattleArtilleryMode::Smoke,
        |location| match mode {
            BattleArtilleryMode::Standard => format!("{name} fire hits {location}!"),
            BattleArtilleryMode::Cluster => {
                format!("A rain of small bomblets hits {location}'s surroundings!")
            }
            BattleArtilleryMode::Mine => format!("A rain of small bomblets hits {location}!"),
            BattleArtilleryMode::Smoke => format!(
                "A {name} {} hits {location}, and smoke starts to billow!",
                projectile(weapon)
            ),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_retain_first_ties_and_nonwrapping_north_boundary() {
        for (heading, expected) in [
            (0, "north"),
            (30, "north"),
            (31, "northeast"),
            (75, "northeast"),
            (76, "east"),
            (105, "east"),
            (106, "southeast"),
            (150, "southeast"),
            (151, "south"),
            (210, "south"),
            (211, "southwest"),
            (255, "southwest"),
            (256, "west"),
            (285, "west"),
            (286, "northwest"),
            (359, "northwest"),
        ] {
            assert_eq!(direction(heading), expected);
        }
        let origin = HexCoordinate { x: 1, y: 1 };
        assert_eq!(
            launch_text(
                BattleWeapon::ClanArrowIv,
                origin,
                HexCoordinate { x: 1, y: 0 }
            )
            .unwrap(),
            "shoots a missile towards the north!"
        );
        assert_eq!(
            launch_text(BattleWeapon::LongTom, origin, origin).unwrap(),
            "shoots a round towards the south!"
        );
    }
}
