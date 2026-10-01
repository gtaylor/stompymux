//! Hit direction samples shared unit geometry at each material packet boundary.
use super::{BattleHitArc, scanner::scanner_unit, unit_range};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Fixed impacts retain their authored direction; direct fire follows the live combatants.
#[derive(Clone, Copy)]
pub(super) enum HitDirection {
    Fixed(BattleHitArc),
    Direct {
        shooter: ObjectId,
        mode: i64,
    },
    /// Self-inflicted scenario damage also permits an unplaced unit's default heading.
    SelfHit {
        mode: i64,
    },
}

impl HitDirection {
    /// Recompute after earlier damage effects have changed a combatant's position or heading.
    pub(super) fn current(self, world: &World, target: ObjectId) -> Result<BattleHitArc> {
        if let Self::Fixed(arc) = self {
            return Ok(arc);
        }
        let unit = scanner_unit(world, target).context("Target is unavailable")?;
        let (bearing, heading, mode) = match self {
            Self::Fixed(_) => unreachable!("fixed direction returned above"),
            Self::Direct { shooter, mode } => (
                unit_range(world, target, shooter)?.bearing.unwrap_or(180.0),
                unit.heading.context("Target is not placed")?,
                mode,
            ),
            Self::SelfHit { mode } => (180.0, unit.heading.unwrap_or(0.0), mode),
        };
        let classify = if world.btech.vehicles().contains_key(&target) {
            BattleHitArc::from_vehicle_bearing
        } else {
            BattleHitArc::from_bearing
        };
        classify(bearing, heading, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    /// Both combatants may use any supported chassis; direct direction observes each new state sample.
    #[test]
    fn direct_direction_tracks_heading_and_position_across_chassis() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let tracked = include_str!("../../game/mechs/Demolisher.toml");
        let templates = [
            include_str!("../../game/mechs/JR7-D.toml").to_owned(),
            include_str!("../../game/mechs/GOL-1H.toml").to_owned(),
            tracked.to_owned(),
            tracked.replace("movement = \"track\"", "movement = \"wheel\""),
            tracked.replace("movement = \"track\"", "movement = \"hover\""),
            tracked
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("max_speed = 53.75", "max_speed = 0"),
            include_str!("../../game/mechs/Kestrel.toml").to_owned(),
        ];
        for source in &templates {
            for recipient in &templates {
                let mut world = World::default();
                let map = world.create(&config, "Direction lane".into(), Kind::Room);
                create_battle_map(
                    &mut world,
                    map,
                    "direction",
                    BattleMapAsset::parse("1 3\n.0\n.0\n.0\n").unwrap(),
                )
                .unwrap();
                let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
                let target = world.create(&config, "Target".into(), Kind::Thing);
                BattleUnitTemplate::parse("shooter", source)
                    .unwrap()
                    .create(&mut world, shooter)
                    .unwrap();
                BattleUnitTemplate::parse("target", recipient)
                    .unwrap()
                    .create(&mut world, target)
                    .unwrap();
                place_battle_unit(&mut world, shooter, map, 0, 2).unwrap();
                place_battle_unit(&mut world, target, map, 0, 1).unwrap();
                let direction = HitDirection::Direct { shooter, mode: 1 };
                for (heading, expected) in [
                    (0.0, BattleHitArc::Rear),
                    (90.0, BattleHitArc::Right),
                    (180.0, BattleHitArc::Front),
                    (270.0, BattleHitArc::Left),
                ] {
                    let motion = if world.btech.vehicles().contains_key(&target) {
                        world
                            .btech
                            .vehicles
                            .get_mut(&target)
                            .unwrap()
                            .motion
                            .as_mut()
                            .unwrap()
                    } else {
                        world
                            .btech
                            .constructed
                            .get_mut(&target)
                            .unwrap()
                            .motion
                            .as_mut()
                            .unwrap()
                    };
                    motion.heading = heading;
                    motion.desired_heading = heading;
                    assert_eq!(direction.current(&world, target).unwrap(), expected);
                    assert_eq!(
                        HitDirection::Fixed(BattleHitArc::Front)
                            .current(&world, target)
                            .unwrap(),
                        BattleHitArc::Front
                    );
                }
                // Sample every degree and both sides of each boundary for all rule modes.
                let vehicle = world.btech.vehicles().contains_key(&target);
                for mode in 0..=2 {
                    let (front, rear) = match (mode, vehicle) {
                        (0, true) => (30.0, 30.0),
                        (1, _) => (45.0, 45.0),
                        _ => (90.0, 30.0),
                    };
                    for degree in 0..360 {
                        for offset in [-0.001, 0.0, 0.001] {
                            let angle = (f64::from(degree) + offset).rem_euclid(360.0);
                            let expected = if angle <= front || angle >= 360.0 - front {
                                BattleHitArc::Front
                            } else if angle >= 180.0 - rear && angle <= 180.0 + rear {
                                BattleHitArc::Rear
                            } else if angle > 180.0 {
                                BattleHitArc::Left
                            } else {
                                BattleHitArc::Right
                            };
                            let motion = if vehicle {
                                world
                                    .btech
                                    .vehicles
                                    .get_mut(&target)
                                    .unwrap()
                                    .motion
                                    .as_mut()
                                    .unwrap()
                            } else {
                                world
                                    .btech
                                    .constructed
                                    .get_mut(&target)
                                    .unwrap()
                                    .motion
                                    .as_mut()
                                    .unwrap()
                            };
                            motion.heading = (180.0 - angle).rem_euclid(360.0);
                            motion.desired_heading = motion.heading;
                            assert_eq!(
                                HitDirection::Direct { shooter, mode }
                                    .current(&world, target)
                                    .unwrap(),
                                expected,
                                "vehicle={vehicle}, mode={mode}, angle={angle}"
                            );
                            assert_eq!(
                                HitDirection::SelfHit { mode }
                                    .current(&world, target)
                                    .unwrap(),
                                expected,
                                "self hit: vehicle={vehicle}, mode={mode}, angle={angle}"
                            );
                        }
                    }
                }
                place_battle_unit(&mut world, target, map, 0, 1).unwrap();
                place_battle_unit(&mut world, shooter, map, 0, 0).unwrap();
                assert_eq!(
                    direction.current(&world, target).unwrap(),
                    BattleHitArc::Front
                );
            }
        }
    }
}
