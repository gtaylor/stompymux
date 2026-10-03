//! Live terrain edits preserve unit positions and share their mutation with seasonal ice growth.
use super::{BattleHex, BattleHexCoordinate, Terrain};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::sync::Arc;

/// A base-terrain edit; temporary overlays and map objects remain independently owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleMapHexChange {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub before: BattleHex,
    pub after: BattleHex,
}

impl super::StoredBattleMap {
    /// The single write path for base terrain: checks bounds and elevation, then stores `hex`.
    /// Unit altitude, overlays and map objects stay with the caller; see [`replace_hex`].
    pub(crate) fn write_hex(&mut self, x: i64, y: i64, hex: BattleHex) -> Result<()> {
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
    coordinate: BattleHexCoordinate,
    after: BattleHex,
) -> Result<BattleMapHexChange> {
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
    let report = BattleMapHexChange {
        map,
        coordinate,
        before,
        after,
    };
    if world.btech.maps()[&map].base_hex(i64::from(coordinate.x), i64::from(coordinate.y))? == after
    {
        return Ok(report);
    }
    let on_tile = |position: Option<super::BattlePosition>| {
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
    coordinate: BattleHexCoordinate,
    hex: BattleHex,
) -> Result<BattleMapHexChange> {
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
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Hex set!",
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// Native terrain arguments use the first symbol, including the operator's grassland shorthand.
fn terrain_argument(value: &str) -> Result<Terrain> {
    match value.chars().next().context("Expected a terrain symbol")? {
        '.' => Ok(Terrain::Grassland),
        symbol => Terrain::from_symbol(symbol),
    }
}

/// Parsed compact ADDHEX arguments: where, what, its signed height and an optional ground
/// level.
type HexArguments = (BattleHexCoordinate, Terrain, i32, Option<u8>);

/// Parse ADDHEX arguments into the hex to write: either the compact symbol form or, when the
/// third argument names a layer, the layer form. A bare `=` is the wall symbol, not a layer.
fn parse_hex(arguments: &str) -> Result<(BattleHexCoordinate, BattleHex)> {
    let args: Vec<_> = arguments.split_whitespace().collect();
    let names_layer = |arg: &&str| {
        arg.split_once('=')
            .is_some_and(|(layer, _)| !layer.is_empty())
    };
    if args.get(2).is_some_and(names_layer) {
        let coordinate = BattleHexCoordinate {
            x: args[0].parse().context("Invalid x coordinate")?,
            y: args[1].parse().context("Invalid y coordinate")?,
        };
        return Ok((coordinate, layers_argument(&args[2..])?));
    }
    let (coordinate, terrain, elevation, level) = parse(arguments)?;
    Ok((coordinate, hex_argument(terrain, elevation, level)?))
}

/// Build a hex from `layer=value` words, the way `btech.map.set_hex` takes a hex's layers:
/// `level` (0-35), `ground` (a ground type), `woods` (`light` or `heavy`), `water` or `ice`
/// (depth 1-9), and at most one of `bridge` (deck height), `building` or `wall` (height 1-35).
/// Unnamed layers are absent, on clear ground at level 0. A bridge must span water or ice.
fn layers_argument(words: &[&str]) -> Result<BattleHex> {
    let mut hex = BattleHex::at_level(0);
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
            "level" => hex.with_level(height(super::hex::MAX_HEIGHT, 0)?),
            "ground" => hex.with_ground(
                serde_json::from_value(serde_json::Value::String(value.clone()))
                    .with_context(|| format!("Unknown ground {value:?}"))?,
            ),
            "woods" => hex.with_woods(Some(
                serde_json::from_value(serde_json::Value::String(value.clone()))
                    .with_context(|| format!("Unknown woods {value:?}"))?,
            )),
            "water" | "ice" => hex.with_water(Some(super::Water {
                depth: height(super::hex::MAX_DEPTH, 1)?,
                frozen: layer == "ice",
            })),
            "bridge" => hex.with_structure(Some(super::Structure::Bridge {
                deck: height(super::hex::MAX_HEIGHT, 1)?,
            })),
            "building" => hex.with_structure(Some(super::Structure::Building {
                height: height(super::hex::MAX_HEIGHT, 1)?,
            })),
            "wall" => hex.with_structure(Some(super::Structure::Wall {
                height: height(super::hex::MAX_HEIGHT, 1)?,
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

/// Parse one coordinate, symbol, signed magnitude and optional ground level before
/// authorizing an edit.
fn parse(arguments: &str) -> Result<HexArguments> {
    let args: Vec<_> = arguments.split_whitespace().take(6).collect();
    ensure!(
        matches!(args.len(), 4 | 5),
        "Expected x y terrain elevation [level], or x y layer=value ..."
    );
    let level = match args.get(4) {
        Some(level) => {
            let level: u8 = level.parse().context("Invalid level")?;
            ensure!(level <= super::hex::MAX_HEIGHT, "Invalid level");
            Some(level)
        }
        None => None,
    };
    Ok((
        BattleHexCoordinate {
            x: args[0].parse().context("Invalid x coordinate")?,
            y: args[1].parse().context("Invalid y coordinate")?,
        },
        terrain_argument(args[2])?,
        args[3].parse().context("Invalid elevation")?,
        level,
    ))
}

/// Build the hex ADDHEX describes. The magnitude's absolute value is the notation's height,
/// capped by [`height_cap`]. Water, ice, bridges, buildings and walls may stand on ground at
/// `level`; other terrain takes its level from the height itself.
fn hex_argument(terrain: Terrain, elevation: i32, level: Option<u8>) -> Result<BattleHex> {
    ensure!(
        !matches!(terrain, Terrain::Fire | Terrain::Smoke),
        "Fire and smoke are not terrain; use ADDFIRE or ADDSMOKE"
    );
    let hex = BattleHex::new(
        terrain,
        elevation.unsigned_abs().min(u32::from(height_cap(terrain))) as u8,
    );
    let Some(level) = level else {
        return Ok(hex);
    };
    ensure!(
        matches!(
            terrain,
            Terrain::Water | Terrain::Ice | Terrain::Bridge | Terrain::Building | Terrain::Wall
        ),
        "Only water, ice, bridges, buildings and walls take a separate level"
    );
    Ok(hex.with_level(level))
}

/// The largest height ADDHEX gives a terrain: water and ice depth, otherwise a ground height,
/// structure height or bridge deck.
fn height_cap(terrain: Terrain) -> u8 {
    match terrain {
        Terrain::Water | Terrain::Ice => super::hex::MAX_DEPTH,
        _ => super::hex::MAX_HEIGHT,
    }
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

    /// Canonical symbols, grassland shorthand and signed magnitudes share one bounded grammar.
    #[test]
    fn terrain_edit_arguments() {
        for terrain in [
            Terrain::Grassland,
            Terrain::Road,
            Terrain::LightForest,
            Terrain::HeavyForest,
            Terrain::Water,
            Terrain::Ice,
            Terrain::Bridge,
            Terrain::Rough,
            Terrain::Mountains,
            Terrain::Fire,
            Terrain::Smoke,
            Terrain::Snow,
            Terrain::Building,
            Terrain::Wall,
            Terrain::Sand,
        ] {
            let symbol = if terrain == Terrain::Grassland {
                '.'
            } else {
                terrain.symbol()
            };
            assert_eq!(
                parse(&format!("1 2 {symbol} -9")).unwrap(),
                (BattleHexCoordinate { x: 1, y: 2 }, terrain, -9, None)
            );
        }
        assert_eq!(parse("0 0 .ignored -2147483648").unwrap().2, i32::MIN);
        assert_eq!(parse("0 0 ~ 2 3").unwrap().3, Some(3));
        for args in [
            "",
            "0 0 .",
            "0 0 . 1 extra",
            "0 0 ~ 2 36",
            "0 0 ~ 2 -1",
            "0 0 ~ 2 3 4",
            "x 0 . 1",
            "0 y . 1",
            "0 0 X 1",
            "0 0 . 2147483648",
        ] {
            assert!(parse(args).is_err(), "{args}");
        }
    }

    /// The layer form names each layer, so a hex can hold several at once.
    #[test]
    fn terrain_edit_layers() {
        use super::super::{Ground, Structure, Water, Woods};
        let (coordinate, hex) = parse_hex("3 4 level=5 ground=snow woods=heavy").unwrap();
        assert_eq!(coordinate, BattleHexCoordinate { x: 3, y: 4 });
        assert_eq!(
            hex,
            BattleHex::at_level(5)
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
        // The symbol form still reads the compact notation, including the wall symbol `=`.
        assert_eq!(
            parse_hex("0 0 ~ 2 3").unwrap().1,
            BattleHex::new(Terrain::Water, 2).with_level(3)
        );
        assert_eq!(
            parse_hex("0 0 = 4").unwrap().1,
            BattleHex::new(Terrain::Wall, 4)
        );
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
        ] {
            assert!(parse_hex(args).is_err(), "{args}");
        }
    }

    /// A level raises water, bridges and structures; ground terrain takes its level as height.
    #[test]
    fn terrain_edit_levels() {
        let lake = hex_argument(Terrain::Water, 2, Some(4)).unwrap();
        assert_eq!((lake.water_line(), lake.water_depth()), (4, 2));
        let bridge = hex_argument(Terrain::Bridge, -3, Some(5)).unwrap();
        assert_eq!(bridge.deck_height(), Some(8));
        let tower = hex_argument(Terrain::Building, 30, Some(5)).unwrap();
        assert_eq!(tower.top_height(), 35);
        assert_eq!(
            hex_argument(Terrain::Road, 7, None).unwrap(),
            BattleHex::new(Terrain::Road, 7)
        );
        assert!(hex_argument(Terrain::Road, 1, Some(2)).is_err());
        assert!(hex_argument(Terrain::Fire, 1, None).is_err());
    }

    /// The shared write path stores in-bounds tiles and leaves the map untouched on rejection.
    #[test]
    fn write_hex_checks_bounds_and_elevation() {
        let mut map = super::super::state::map_from_asset(
            "write",
            super::super::BattleMapAsset::from_cells("2 1\n.0.0\n").unwrap(),
        )
        .unwrap();
        let rough = BattleHex::new(Terrain::Rough, 3);
        map.write_hex(1, 0, rough).unwrap();
        assert_eq!(map.base_hex(1, 0).unwrap(), rough);
        let before = map.clone();
        for (x, y, elevation) in [(2, 0, 0), (0, 1, 0), (-1, 0, 0), (0, 0, 36)] {
            assert!(
                map.write_hex(x, y, BattleHex::new(Terrain::Road, elevation))
                    .is_err()
            );
        }
        assert_eq!(map, before);
    }
}
