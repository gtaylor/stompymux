//! Operator landing restrictions reuse the shared circular exclusion and selective persistence.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};

/// Add a stable restriction slot and publish confirmation in one host transaction.
/// A nonzero team is exempt; zero grants no exemption.
pub fn add_landing_exclusion_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    coordinate: super::HexCoordinate,
    radius: i32,
    team: i32,
) -> Result<u32> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        let record = before.btech.maps().get(&map).context("Map not found")?;
        ensure!(
            coordinate.x >= 0
                && coordinate.y >= 0
                && i64::from(coordinate.x) < record.width
                && i64::from(coordinate.y) < record.height,
            "X,Y out of range!"
        );
        let zones = record.landing_exclusions();
        let ordinal = (0..=u32::try_from(zones.len())?)
            .find(|slot| !zones.contains_key(slot))
            .context("No landing restriction slot available")?;
        super::set_landing_exclusion(
            &mut scripts.world_mut(),
            map,
            ordinal,
            Some(super::LandingExclusion {
                coordinate,
                radius: i64::from(radius),
                exempt_team: team,
                owner: actor,
                data_short: 0,
            }),
        )?;
        {
            let mut world = scripts.world_mut();
            let record = world.btech.maps.get_mut(&map).unwrap();
            let order = std::sync::Arc::make_mut(&mut record.landing_exclusion_order);
            order.retain(|slot| *slot != ordinal);
            order.insert(0, ordinal);
        }
        super::notify_message(
            scripts,
            super::MessageTarget::Player(actor),
            &format!(
                "Landingzone-block added to {},{} (distance: {radius})",
                coordinate.x, coordinate.y
            ),
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(ordinal)
    })
}

/// Native ADDBLOCK uses the selected map and an optional exempt team.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let args: Vec<_> = input
            .args
            .split([' ', '\t'])
            .filter(|token| !token.is_empty())
            .take(4)
            .collect();
        ensure!(args.len() >= 3, "Invalid arguments!");
        let number = |token: &str| {
            token
                .trim_matches(|c: char| matches!(c, ' ' | '\t'..='\r'))
                .parse::<i32>()
                .map_err(|_| anyhow::anyhow!("Invalid number!"))
        };
        let coordinate = super::HexCoordinate {
            x: number(args[0])?,
            y: number(args[1])?,
        };
        let radius = number(args[2])?;
        let team = args
            .get(3)
            .map(|team| number(team))
            .transpose()?
            .unwrap_or(0);
        let map = super::special_dispatch::object(ctx)?;
        add_landing_exclusion_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            map,
            coordinate,
            radius,
            team,
        )?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
