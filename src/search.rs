//! C database search criteria and statistics, without callbacks or durable mutations.
use crate::{
    find::{SearchRange, identity},
    flags::{self, FlagSet},
    powers::Power,
    reports::Report,
    world::{Kind, Object, ObjectId, World},
};
use anyhow::{Result, bail, ensure};

/// Native search filters; unused C type letters can never match a supported object.
#[derive(Default, Debug)]
pub struct Criteria {
    pub lower: i64,
    pub upper: i64,
    kind: Option<Kind>,
    impossible: bool,
    name: Option<String>,
    flags: FlagSet,
    power: Option<Power>,
    zone: Option<ObjectId>,
}
impl Criteria {
    pub fn parse(world: &World, player: ObjectId, args: &str) -> Result<Self> {
        let (class, value) = args.split_once('=').unwrap_or((args, ""));
        let class = class.trim().to_ascii_lowercase();
        let range = SearchRange::new(value, world.next_id.saturating_sub(1));
        let value = range.query.as_str();
        let mut c = Self {
            lower: range.lower,
            upper: range.upper,
            ..Default::default()
        };
        let classes = [
            "exits", "flags", "name", "objects", "players", "power", "rooms", "type", "things",
            "zone",
        ];
        let class = if class.is_empty() {
            ""
        } else {
            classes
                .into_iter()
                .find(|s| s.starts_with(&class))
                .ok_or_else(|| anyhow::anyhow!("{class}: unknown class"))?
        };
        match class {
            "" => {}
            "name" | "exits" | "objects" | "things" | "players" | "rooms" => {
                c.name = Some(value.into());
                c.kind = match class {
                    "exits" => Some(Kind::Exit),
                    "objects" | "things" => Some(Kind::Thing),
                    "players" => Some(Kind::Player),
                    "rooms" => Some(Kind::Room),
                    _ => None,
                };
            }
            "type" if value.is_empty() => {}
            "type" => {
                c.kind = Some(
                    [
                        ("rooms", Kind::Room),
                        ("exits", Kind::Exit),
                        ("objects", Kind::Thing),
                        ("things", Kind::Thing),
                        ("garbage", Kind::Garbage),
                        ("players", Kind::Player),
                    ]
                    .into_iter()
                    .find(|(s, _)| s.starts_with(&value.to_ascii_lowercase()))
                    .map(|(_, k)| k)
                    .ok_or_else(|| anyhow::anyhow!("{value}: unknown type"))?,
                );
            }
            "flags" => {
                for letter in value.chars() {
                    let kind = match letter {
                        'R' => Some(Kind::Room),
                        ' ' => Some(Kind::Thing),
                        'E' => Some(Kind::Exit),
                        'P' => Some(Kind::Player),
                        '-' => Some(Kind::Garbage),
                        _ => None,
                    };
                    if kind.is_some() || matches!(letter, '+' | '#') {
                        c.kind = kind;
                        c.impossible = kind.is_none();
                    } else if let Some(flag) = flags::ALL.into_iter().find(|f| f.letter() == letter)
                    {
                        c.flags.insert(flag);
                    } else {
                        bail!("{letter}: Flag unknown or not valid for specified object type");
                    }
                }
            }
            "power" => {
                c.power = Some(
                    Power::parse(value)
                        .map_err(|_| anyhow::anyhow!("{value}: Power not found."))?,
                )
            }
            "zone" => c.zone = Some(crate::commands::target::admin_target(world, player, value)?),
            _ => unreachable!(),
        }
        Ok(c)
    }
    pub fn matches(&self, world: &World, o: &Object) -> bool {
        !self.impossible
            && o.id.0 >= self.lower
            && o.id.0 <= self.upper
            && self.kind.is_none_or(|k| o.kind == k)
            && self.zone.is_none_or(|z| o.zone == Some(z))
            && flags::ALL
                .into_iter()
                .all(|f| !self.flags.contains(f) || o.flags.contains(f))
            && self.power.is_none_or(|p| o.powers.contains(p))
            && self.name.as_ref().is_none_or(|prefix| {
                !prefix.is_empty()
                    && crate::text::plain_with(&world.palette, &o.name)
                        .as_bytes()
                        .get(..prefix.len())
                        .is_some_and(|n| n.eq_ignore_ascii_case(prefix.as_bytes()))
            })
    }
}
/// Match counts are independent of report truncation.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub rooms: usize,
    pub exits: usize,
    pub things: usize,
    pub players: usize,
    pub garbage: usize,
}
impl Counts {
    fn add(&mut self, kind: Kind) {
        match kind {
            Kind::Room => self.rooms += 1,
            Kind::Exit => self.exits += 1,
            Kind::Thing => self.things += 1,
            Kind::Player => self.players += 1,
            Kind::Garbage => self.garbage += 1,
        }
    }
    fn total(&self) -> usize {
        self.rooms + self.exits + self.things + self.players + self.garbage
    }
}
pub fn report(world: &World, player: ObjectId, args: &str, limit: usize) -> Result<String> {
    ensure!(flags::is_wizard(world, player), "Permission denied.");
    let criteria = Criteria::parse(world, player, args)?;
    let mut counts = Counts::default();
    for o in world
        .objects
        .values()
        .filter(|o| criteria.matches(world, o))
    {
        counts.add(o.kind);
    }
    let footer = if counts.total() == 0 {
        "Nothing found.".into()
    } else {
        format!(
            "\nFound:  Rooms...{}  Exits...{}  Objects...{}  Players...{}  Garbage...{}",
            counts.rooms, counts.exits, counts.things, counts.players, counts.garbage
        )
    };
    let mut report = Report::new(limit, &footer)?;
    for (kind, label) in [
        (Kind::Room, "ROOMS"),
        (Kind::Exit, "EXITS"),
        (Kind::Thing, "OBJECTS"),
        (Kind::Garbage, "GARBAGE"),
        (Kind::Player, "PLAYERS"),
    ] {
        let mut heading = false;
        for o in world
            .objects
            .values()
            .filter(|o| o.kind == kind && criteria.matches(world, o))
        {
            if !heading {
                report.row(&format!("\n{label}:"));
                heading = true;
            }
            let mut row = identity(world, Some(o.id));
            if kind == Kind::Exit {
                row.push_str(&format!(
                    " [from {} to {}]",
                    identity(world, o.location),
                    identity(world, o.destination)
                ));
            }
            if kind == Kind::Player {
                row.push_str(&format!(" [location: {}]", identity(world, o.location)));
            }
            report.row(&row);
        }
    }
    report.finish()
}
/// C counts allocated slots, including implicit garbage and GOING non-rooms.
pub fn statistics(world: &World) -> String {
    let mut counts = Counts::default();
    for o in world.objects.values() {
        counts.add(
            if o.kind != Kind::Room && o.flags.contains(flags::Flag::Going) {
                Kind::Garbage
            } else {
                o.kind
            },
        );
    }
    let total = (world.next_id.max(0) as usize).max(world.objects.len());
    counts.garbage += total - world.objects.len();
    format!(
        "{total} objects = {} rooms, {} exits, {} things, {} players. ({} garbage)",
        counts.rooms, counts.exits, counts.things, counts.players, counts.garbage
    )
}
