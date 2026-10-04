//! Live terrain edits preserve unit positions and share their mutation with seasonal ice growth.
use super::{Hex, HexCoordinate};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::sync::Arc;

/// A base-terrain edit; temporary overlays and map objects remain independently owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MapHexChange {
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub before: Hex,
    pub after: Hex,
}

impl super::StoredMap {
    /// The single write path for base terrain: checks bounds and elevation, then stores `hex`.
    /// Unit altitude, overlays and map objects stay with the caller; see [`replace_hex`].
    pub(crate) fn write_hex(&mut self, x: i64, y: i64, hex: Hex) -> Result<()> {
        hex.validate()?;
        ensure!(
            hex.overlay().is_none(),
            "Fire and smoke are not terrain; add them as fire or smoke instead"
        );
        self.base_hex(x, y)?;
        let index = (y * self.width + x) as usize;
        Arc::make_mut(
            self.terrain
                .as_mut()
                .context("Map terrain is unavailable")?,
        )[index] = hex;
        Ok(())
    }
}

/// Replace a tile while retaining physical altitude. Authority and atomic publication belong to the caller.
pub(super) fn replace_hex(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    after: Hex,
) -> Result<MapHexChange> {
    after.validate()?;
    ensure!(
        after.overlay().is_none(),
        "Fire and smoke are not terrain; add them as fire or smoke instead"
    );
    let before = world
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let report = MapHexChange {
        map,
        coordinate,
        before,
        after,
    };
    if world.btech.maps()[&map].base_hex(i64::from(coordinate.x), i64::from(coordinate.y))? == after
    {
        return Ok(report);
    }
    let on_tile = |position: Option<super::Position>| {
        position.is_some_and(|position| {
            position.map == map
                && i32::from(position.x) == coordinate.x
                && i32::from(position.y) == coordinate.y
        })
    };
    for unit in world.btech.constructed.values_mut() {
        if on_tile(unit.position) && unit.retained_altitude().is_none() {
            unit.ground_elevation = Some(unit.altitude(before));
        }
    }
    for unit in world.btech.vehicles.values_mut() {
        if !on_tile(unit.position()) {
            continue;
        }
        if unit.ground_elevation.is_none()
            && unit.vtol_flight().is_none()
            && unit.free_fall().is_none()
            && unit.orbital_drop().is_none()
        {
            unit.ground_elevation = Some(unit.altitude(before));
        }
        if after.deck_clearance().is_none_or(|deck| deck < 2) {
            unit.under_bridge = false;
        }
    }
    world.btech.maps.get_mut(&map).unwrap().write_hex(
        i64::from(coordinate.x),
        i64::from(coordinate.y),
        after,
    )?;
    Ok(report)
}

/// Wizard terrain change replacing one hex's layers.
/// Editing changes terrain facts without moving units or applying a combat fracture.
/// Fire and smoke are not terrain and are rejected; they are added as map decorations.
pub fn set_map_hex_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    coordinate: HexCoordinate,
    hex: Hex,
) -> Result<MapHexChange> {
    ensure!(
        hex.overlay().is_none(),
        "Fire and smoke are not terrain; use ADDFIRE or ADDSMOKE"
    );
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| object.kind != crate::Kind::Garbage
                    && !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let report = replace_hex(&mut scripts.world_mut(), map, coordinate, hex)?;
        super::notify_message(scripts, super::MessageTarget::Player(actor), "Hex set!")?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// Parse `<x> <y> <layer>=<value> ...` into the coordinate and hex to write.
fn parse_hex(arguments: &str) -> Result<(HexCoordinate, Hex)> {
    let args: Vec<_> = arguments.split_whitespace().collect();
    ensure!(args.len() >= 3, "Expected x y layer=value ...");
    let coordinate = HexCoordinate {
        x: args[0].parse().context("Invalid x coordinate")?,
        y: args[1].parse().context("Invalid y coordinate")?,
    };
    Ok((coordinate, layers_argument(&args[2..])?))
}

/// Build a hex from `layer=value` words, the way `btech.map.set_hex` takes a hex's layers:
/// `level` (0-35), `ground` (a ground type), `woods` (`light` or `heavy`), `water` or `ice`
/// (depth 1-9), and at most one of `bridge` (deck height), `building` or `wall` (height 1-35).
/// Unnamed layers are absent, on clear ground at level 0. A bridge must span water or ice.
fn layers_argument(words: &[&str]) -> Result<Hex> {
    let mut hex = Hex::at_level(0);
    let mut seen = std::collections::BTreeSet::new();
    for word in words {
        let (layer, value) = word
            .split_once('=')
            .with_context(|| format!("Expected layer=value, got {word:?}"))?;
        let layer = layer.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        let height = |limit: u8, least: u8| -> Result<u8> {
            let height: u8 = value
                .parse()
                .with_context(|| format!("Invalid {layer} {value:?}"))?;
            ensure!(
                (least..=limit).contains(&height),
                "{layer} must be from {least} to {limit}"
            );
            Ok(height)
        };
        // Water and ice share a layer, as do the three structures.
        let slot = match layer.as_str() {
            "water" | "ice" => "water",
            "bridge" | "building" | "wall" => "structure",
            other => other,
        };
        ensure!(
            seen.insert(slot.to_owned()),
            "Only one {slot} layer is allowed"
        );
        hex = match layer.as_str() {
            "level" => hex.with_level(height(super::MAX_HEIGHT, 0)?),
            "ground" => hex.with_ground(
                serde_json::from_value(serde_json::Value::String(value.clone()))
                    .with_context(|| format!("Unknown ground {value:?}"))?,
            ),
            "woods" => hex.with_woods(Some(
                serde_json::from_value(serde_json::Value::String(value.clone()))
                    .with_context(|| format!("Unknown woods {value:?}"))?,
            )),
            "water" | "ice" => hex.with_water(Some(super::Water {
                depth: height(super::MAX_DEPTH, 1)?,
                frozen: layer == "ice",
            })),
            "bridge" => hex.with_structure(Some(super::Structure::Bridge {
                deck: height(super::MAX_HEIGHT, 1)?,
            })),
            "building" => hex.with_structure(Some(super::Structure::Building {
                height: height(super::MAX_HEIGHT, 1)?,
            })),
            "wall" => hex.with_structure(Some(super::Structure::Wall {
                height: height(super::MAX_HEIGHT, 1)?,
            })),
            "fire" | "smoke" => bail!("Fire and smoke are not terrain; use ADDFIRE or ADDSMOKE"),
            _ => bail!(
                "Unknown layer {layer:?}; use level, ground, woods, water, ice, bridge, building or wall"
            ),
        };
    }
    ensure!(
        hex.deck_clearance().is_none() || hex.water().is_some(),
        "A bridge must span water or ice"
    );
    hex.validate()?;
    Ok(hex)
}

/// Native operators edit the map containing their player object.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let (coordinate, hex) = parse_hex(&input.args)?;
        let map = super::special_dispatch::object(ctx)?;
        set_map_hex_action(ctx.scripts, ctx.config, ctx.player, map, coordinate, hex)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// ADDHEX names each layer, so a hex can hold several at once.
    #[test]
    fn terrain_edit_layers() {
        use super::super::{Ground, Structure, Water, Woods};
        let (coordinate, hex) = parse_hex("3 4 level=5 ground=snow woods=heavy").unwrap();
        assert_eq!(coordinate, HexCoordinate { x: 3, y: 4 });
        assert_eq!(
            hex,
            Hex::at_level(5)
                .with_ground(Ground::Snow)
                .with_woods(Some(Woods::Heavy))
        );
        let (_, tower) = parse_hex("0 0 building=30 level=5 ground=road").unwrap();
        assert_eq!(tower.structure(), Some(Structure::Building { height: 30 }));
        assert_eq!((tower.ground(), tower.top_height()), (Ground::Road, 35));
        let (_, lake) = parse_hex("0 0 ice=9").unwrap();
        assert_eq!(
            lake.water(),
            Some(Water {
                depth: 9,
                frozen: true
            })
        );
        // Water, bridges and structures stand on raised ground.
        let (_, bridge) = parse_hex("0 0 level=5 water=1 bridge=3").unwrap();
        assert_eq!((bridge.water_line(), bridge.deck_height()), (5, Some(8)));
        for args in [
            "0 0 level=",
            "0 0 =3",
            "0 0 level=-1",
            "0 0 building=0",
            "0 0 bridge=1",
            "0 0 water=2 wall=1 wall=2",
            "0 0 water=2 =1",
            "0 0 smoke=1",
            "0 0 level=1 x",
            "x 0 level=1",
            "0 0",
            "0 0 ~ 2",
            "0 0 = 4",
        ] {
            assert!(parse_hex(args).is_err(), "{args}");
        }
    }

    /// The shared write path stores in-bounds tiles and leaves the map untouched on rejection.
    #[test]
    fn write_hex_checks_bounds_and_elevation() {
        let mut map = super::super::state::map_from_asset(
            "write",
            super::super::MapAsset::from_cells("2 1\n.0.0\n").unwrap(),
        )
        .unwrap();
        let rough = Hex::new(Terrain::Rough, 3);
        map.write_hex(1, 0, rough).unwrap();
        assert_eq!(map.base_hex(1, 0).unwrap(), rough);
        let before = map.clone();
        for (x, y, elevation) in [(2, 0, 0), (0, 1, 0), (-1, 0, 0), (0, 0, 36)] {
            assert!(
                map.write_hex(x, y, Hex::new(Terrain::Road, elevation))
                    .is_err()
            );
        }
        assert_eq!(map, before);
    }
}
