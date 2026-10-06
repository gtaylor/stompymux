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
/// `level` (0-35), `ground` (a ground type), `water` (depth 0-9) and `flow` (`still`,
/// `rapids` or `torrent`), `foliage` (a foliage type), `route` (`paved_road`, `gravel_road`,
/// `dirt_road` or `rail`), `condition` (`ice`, `thin_snow`, `deep_snow` or `mud`), and at most
/// one of `bridge` (deck height), `building` or `wall` (height 1-35), optionally with `class`
/// (`light`, `medium`, `heavy` or `hardened`) and `cf`. Unnamed layers are absent, on clear
/// ground at level 0, and structures are medium at full construction factor. The result must
/// pass [`Hex::validate`].
fn layers_argument(words: &[&str]) -> Result<Hex> {
    let mut hex = Hex::at_level(0);
    let mut seen = std::collections::BTreeSet::new();
    let mut flow = None;
    let mut class = None;
    let mut cf = None;
    for word in words {
        let (layer, value) = word
            .split_once('=')
            .with_context(|| format!("Expected layer=value, got {word:?}"))?;
        let layer = layer.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        let number = |limit: u16, least: u16| -> Result<u16> {
            let number: u16 = value
                .parse()
                .with_context(|| format!("Invalid {layer} {value:?}"))?;
            ensure!(
                (least..=limit).contains(&number),
                "{layer} must be from {least} to {limit}"
            );
            Ok(number)
        };
        let named = || serde_json::Value::String(value.clone());
        // The three structures share a layer.
        let slot = match layer.as_str() {
            "bridge" | "building" | "wall" => "structure",
            other => other,
        };
        ensure!(
            seen.insert(slot.to_owned()),
            "Only one {slot} layer is allowed"
        );
        let structure = |kind| -> Result<Option<super::Structure>> {
            Ok(Some(super::Structure::new(
                kind,
                number(u16::from(super::MAX_HEIGHT), 1)? as u8,
                super::ConstructionClass::Medium,
            )))
        };
        hex = match layer.as_str() {
            "level" => hex.with_level(number(u16::from(super::MAX_HEIGHT), 0)? as u8),
            "ground" => hex.with_ground(
                serde_json::from_value(named())
                    .with_context(|| format!("Unknown ground {value:?}"))?,
            ),
            "water" => hex
                .with_water(Some(super::Water::still(
                    number(u16::from(super::MAX_DEPTH), 0)? as u8,
                ))),
            "flow" => {
                flow = Some(
                    serde_json::from_value::<super::Flow>(named())
                        .with_context(|| format!("Unknown flow {value:?}"))?,
                );
                hex
            }
            "foliage" => hex.with_foliage(Some(
                serde_json::from_value(named())
                    .with_context(|| format!("Unknown foliage {value:?}"))?,
            )),
            "route" => hex.with_route(Some(
                serde_json::from_value(named())
                    .with_context(|| format!("Unknown route {value:?}"))?,
            )),
            "condition" => hex.with_condition(Some(
                serde_json::from_value(named())
                    .with_context(|| format!("Unknown condition {value:?}"))?,
            )),
            "bridge" => hex.with_structure(structure(super::StructureKind::Bridge)?),
            "building" => hex.with_structure(structure(super::StructureKind::Building)?),
            "wall" => hex.with_structure(structure(super::StructureKind::Wall)?),
            "class" => {
                class = Some(
                    serde_json::from_value::<super::ConstructionClass>(named())
                        .with_context(|| format!("Unknown construction class {value:?}"))?,
                );
                hex
            }
            "cf" => {
                cf = Some(number(super::MAX_CONSTRUCTION_FACTOR, 1)?);
                hex
            }
            "fire" | "smoke" => bail!("Fire and smoke are not terrain; use ADDFIRE or ADDSMOKE"),
            _ => bail!(
                "Unknown layer {layer:?}; use level, ground, water, flow, foliage, route, \
                 condition, bridge, building, wall, class or cf"
            ),
        };
    }
    if let Some(flow) = flow {
        let water = hex.water().context("flow needs water")?;
        hex = hex.with_water(Some(super::Water { flow, ..water }));
    }
    if class.is_some() || cf.is_some() {
        let mut structure = hex
            .structure()
            .context("class and cf need a bridge, building or wall")?;
        if let Some(class) = class {
            structure.class = class;
            structure.cf = class.construction_factor();
        }
        if let Some(cf) = cf {
            structure.cf = cf;
        }
        hex = hex.with_structure(Some(structure));
    }
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
        use super::super::{
            Condition, ConstructionClass, Flow, Foliage, Ground, Route, Structure, StructureKind,
            Water,
        };
        let (coordinate, hex) =
            parse_hex("3 4 level=5 ground=rough foliage=heavy_jungle condition=deep_snow").unwrap();
        assert_eq!(coordinate, HexCoordinate { x: 3, y: 4 });
        assert_eq!(
            hex,
            Hex::at_level(5)
                .with_ground(Ground::Rough)
                .with_foliage(Some(Foliage::HeavyJungle))
                .with_condition(Some(Condition::DeepSnow))
        );
        let (_, tower) =
            parse_hex("0 0 building=30 level=5 ground=pavement class=hardened cf=99").unwrap();
        assert_eq!(
            tower.structure(),
            Some(Structure {
                kind: StructureKind::Building,
                class: ConstructionClass::Hardened,
                height: 30,
                cf: 99,
            })
        );
        assert_eq!((tower.ground(), tower.top_height()), (Ground::Pavement, 35));
        let (_, lake) = parse_hex("0 0 water=9 condition=ice flow=torrent").unwrap();
        assert_eq!(
            lake.water(),
            Some(Water {
                depth: 9,
                flow: Flow::Torrent
            })
        );
        assert!(lake.is_ice());
        let (_, road) = parse_hex("0 0 route=dirt_road foliage=light_woods").unwrap();
        assert_eq!(road.route(), Some(Route::DirtRoad));
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
            "0 0 water=2 bridge=1 wall=2",
            "0 0 water=2 =1",
            "0 0 water=2 foliage=light_woods",
            "0 0 flow=rapids",
            "0 0 cf=10",
            "0 0 wall=1 class=light cf=40",
            "0 0 ground=mountains",
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
