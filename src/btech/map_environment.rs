//! Live map environment controls feed shared movement, heat and flight rules without per-unit copies.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// SETCOND values; omitted vacuum and underground arguments are false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleMapEnvironment {
    pub gravity: u8,
    pub temperature: i8,
    #[serde(default)]
    pub vacuum: bool,
    #[serde(default)]
    pub underground: bool,
}

impl super::StoredBattleMap {
    /// Current environment is read from the shared map, including its durable underground flag.
    pub fn environment(&self) -> BattleMapEnvironment {
        BattleMapEnvironment {
            gravity: self.gravity as u8,
            temperature: self.temperature as i8,
            vacuum: self.flags & 4 != 0,
            underground: self.flags & 16 != 0,
        }
    }
}

/// Change environmental state without advancing time, moving units or rewriting their state.
/// SETCOND clears omitted vacuum; underground is retained once enabled, including for an explicit false.
pub fn set_map_environment(
    world: &mut World,
    actor: ObjectId,
    id: ObjectId,
    conditions: BattleMapEnvironment,
) -> Result<BattleMapEnvironment> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| object.kind != crate::Kind::Garbage
                && !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let mut map = world
        .btech
        .maps()
        .get(&id)
        .context("Map not found")?
        .clone();
    map.gravity = i64::from(conditions.gravity);
    map.temperature = i64::from(conditions.temperature);
    map.flags &= !(2 | 4);
    if conditions.vacuum {
        map.flags |= 4;
    }
    if conditions.underground {
        map.flags |= 16;
    }
    if conditions.vacuum
        || conditions.gravity != 100
        || !(-30..=50).contains(&conditions.temperature)
    {
        map.flags |= 2;
    }
    map.validate()?;
    let actual = map.environment();
    world.btech.maps.insert(id, map);
    Ok(actual)
}

/// Parse the real handler's four fields rather than the catalogue's outdated cloud-base description.
fn parse(arguments: &str) -> Result<BattleMapEnvironment> {
    // The reference tokenizer reads at most four space/tab-delimited fields.
    let args: Vec<_> = arguments
        .split([' ', '\t'])
        .filter(|value| !value.is_empty())
        .take(4)
        .collect();
    ensure!(
        args.len() >= 2,
        "(At least) 2 options required (gravity + temperature)"
    );
    let field = |index: usize, minimum, maximum, message: &'static str| -> Result<i32> {
        let Some(value) = args.get(index) else {
            return Ok(0);
        };
        let value = value
            .trim_matches(|c: char| c.is_ascii_whitespace())
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!(message))?;
        ensure!((minimum..=maximum).contains(&value), message);
        Ok(value)
    };
    Ok(BattleMapEnvironment {
        gravity: field(
            0,
            0,
            255,
            "Invalid gravity (must be integer in range of 0 to 255)",
        )? as u8,
        temperature: field(
            1,
            -128,
            127,
            "Invalid temperature (must be integer in range of -128 to 127",
        )? as i8,
        vacuum: field(2, 0, 1, "Invalid vacuum flag (must be integer, 0 or 1)")? == 1,
        underground: field(
            3,
            0,
            1,
            "Invalid underground flag (must be integer, 0 or 1)",
        )? == 1,
    })
}

/// Share native/Lua publication and rollback for one environment transition.
pub fn set_map_environment_action(
    scripts: &crate::Scripts,
    actor: ObjectId,
    id: ObjectId,
    conditions: BattleMapEnvironment,
) -> Result<BattleMapEnvironment> {
    scripts.atomic(|_| {
        let actual = set_map_environment(&mut scripts.world_mut(), actor, id, conditions)?;
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Conditions set!",
        )?;
        scripts.effects.validate()?;
        Ok(actual)
    })
}

/// Native map operators use their current location and the same transaction as Lua.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let id = super::special_dispatch::object(ctx)?;
        set_map_environment_action(ctx.scripts, ctx.player, id, parse(&input.args)?)?;
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

    /// Bound numeric fields and decode the optional flags independently.
    #[test]
    fn setcond_argument_contract() {
        for input in ["-0 +0 -0 +0", "0 0 0 0 ignored arguments", "\t0\t0\t"] {
            assert_eq!(
                parse(input).unwrap(),
                BattleMapEnvironment {
                    gravity: 0,
                    temperature: 0,
                    vacuum: false,
                    underground: false,
                }
            );
        }
        assert_eq!(
            parse("0 -128").unwrap(),
            BattleMapEnvironment {
                gravity: 0,
                temperature: -128,
                vacuum: false,
                underground: false
            }
        );
        assert_eq!(
            parse("255 127 1 1").unwrap(),
            BattleMapEnvironment {
                gravity: 255,
                temperature: 127,
                vacuum: true,
                underground: true
            }
        );
        for input in [
            "",
            "100",
            "-1 20",
            "256 20",
            "100 -129",
            "100 128",
            "100 20 2",
            "100 20 0 2",
            "100.5 20",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}
