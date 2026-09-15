//! Wizard fire and smoke commands share installation, duration admission and transactional publication.
use super::{BattleDecoration, BattleDecorationKind, BattleHexCoordinate};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Install an operator marker. Off-map requests confirm without changing terrain.
/// Zero is permanent; negative smoke expires next tick and short fire lasts through its first spread.
pub fn add_map_decoration_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    kind: BattleDecorationKind,
    duration: i32,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        ensure!(
            crate::authority::is_wizard(&before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let record = before.btech.maps().get(&map).context("Map not found")?;
        ensure!(record.terrain_ready(), "Map terrain is unavailable");
        if coordinate.x >= 0
            && coordinate.y >= 0
            && i64::from(coordinate.x) < record.width
            && i64::from(coordinate.y) < record.height
        {
            let remaining = match (kind, duration) {
                (_, 0) => 0,
                (BattleDecorationKind::Smoke, seconds) => i64::from(seconds.max(1)),
                (BattleDecorationKind::Fire, seconds) => {
                    // The first spread is scheduled independently of this budget.
                    i64::from(seconds.clamp(i32::from(i16::MIN), i32::from(i16::MAX)))
                }
            };
            let mut effect = BattleDecoration::new(
                kind,
                remaining,
                (remaining < 0).then_some(record.fire_spread_interval()),
            );
            effect.object_duration =
                duration.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            super::set_map_decoration(&mut scripts.world_mut(), map, coordinate, Some(effect))?;
        }
        let label = match kind {
            BattleDecorationKind::Fire => "Fire",
            BattleDecorationKind::Smoke => "Smoke",
        };
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!(
                "Added: {label} at ({},{}) with duration of {duration}s.",
                coordinate.x, coordinate.y
            ),
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Parse both native marker commands through the same operator action.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let kind = if input.name.eq_ignore_ascii_case("addfire") {
            BattleDecorationKind::Fire
        } else {
            BattleDecorationKind::Smoke
        };
        let (coordinate, duration) = parse(&input.args, kind)?;
        let map = super::special_dispatch::object(ctx)?;
        add_map_decoration_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            map,
            coordinate,
            kind,
            duration,
        )
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Consume the reference's three space/tab-delimited fields and checked signed integers.
fn parse(arguments: &str, kind: BattleDecorationKind) -> Result<(BattleHexCoordinate, i32)> {
    let name = match kind {
        BattleDecorationKind::Fire => "addfire",
        BattleDecorationKind::Smoke => "addsmoke",
    };
    let args: Vec<_> = arguments
        .split([' ', '\t'])
        .filter(|token| !token.is_empty())
        .take(3)
        .collect();
    ensure!(
        args.len() == 3,
        "Error: Invalid number of attributes to {name} command."
    );
    let number = |token: &str| {
        token
            .trim_matches(|c: char| matches!(c, ' ' | '\t'..='\r'))
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Error: Invalid numeric {name} argument."))
    };
    Ok((
        BattleHexCoordinate {
            x: number(args[0])?,
            y: number(args[1])?,
        },
        number(args[2])?,
    ))
}
