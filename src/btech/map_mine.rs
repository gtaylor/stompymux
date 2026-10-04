//! Operator mine placement shares record insertion and transactional publication with map gameplay.
use super::{BattleMineKind, BattleMinefield, HexCoordinate};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Requested placement retains full-width strength for confirmation before storage clamping.
#[derive(Debug, Clone, Copy)]
pub struct BattleMinePlacement {
    pub coordinate: HexCoordinate,
    pub kind: BattleMineKind,
    pub strength: i32,
    pub extra: i32,
}

/// Add a newest-first mine owned by the wizard, restoring state and output after any failure.
pub fn add_mine_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    placement: BattleMinePlacement,
) -> Result<u32> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        let record = before.btech.maps().get(&map).context("Map not found")?;
        ensure!(
            placement.coordinate.x >= 0
                && placement.coordinate.y >= 0
                && i64::from(placement.coordinate.x) < record.width
                && i64::from(placement.coordinate.y) < record.height,
            "X,Y out of range!"
        );
        let ordinal = super::insert_minefield(
            &mut scripts.world_mut(),
            map,
            BattleMinefield {
                coordinate: placement.coordinate,
                kind: placement.kind,
                strength: placement
                    .strength
                    .clamp(i32::from(i16::MIN), i32::from(i16::MAX))
                    as i16,
                extra: placement.extra,
                owner: actor,
            },
        )?;
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!(
                "{:?} mine added to ({},{}) (strength: {} / extra: {})",
                placement.kind,
                placement.coordinate.x,
                placement.coordinate.y,
                placement.strength,
                placement.extra,
            ),
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(ordinal)
    })
}

/// Native ADDMINE uses exact case-insensitive type names and four or five arguments.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!((4..=5).contains(&args.len()), "Invalid arguments!");
        let placement = BattleMinePlacement {
            coordinate: HexCoordinate {
                x: args[0].parse().context("Invalid number!")?,
                y: args[1].parse().context("Invalid number!")?,
            },
            strength: args[3].parse().context("Invalid number!")?,
            extra: args
                .get(4)
                .map(|value| value.parse().context("Invalid number!"))
                .transpose()?
                .unwrap_or(0),
            kind: BattleMineKind::parse(args[2])?,
        };
        let map = super::special_dispatch::object(ctx)?;
        add_mine_action(ctx.scripts, ctx.config, ctx.player, map, placement)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
