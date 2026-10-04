//! Saved unit markings and cockpit viewing share contact admission across chassis.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Literal player-authored description with the reference large-buffer byte bound.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(super) struct Markings(String);

impl TryFrom<String> for Markings {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        ensure!(value.len() <= 16383, "Markings exceed 16383 bytes");
        ensure!(!value.contains('\0'), "Markings contain a null character");
        Ok(Self(value))
    }
}

impl From<Markings> for String {
    fn from(value: Markings) -> Self {
        value.0
    }
}

/// Read configuration independently of contact disclosure; empty means no markings.
pub fn unit_markings(world: &World, unit: ObjectId) -> Result<&str> {
    if let Some(value) = world
        .btech
        .unit_configuration
        .get(&unit)
        .and_then(|configuration| configuration.markings.as_deref())
    {
        return Ok(value);
    }
    super::with_unit!(
        world.btech.unit(unit).context("Unit is unavailable")?,
        |unit| { Ok(&unit.markings.0) }
    )
}

/// Configure literal markings after administrative authority and validation, before mutation.
pub fn set_unit_markings(
    world: &mut World,
    actor: ObjectId,
    unit: ObjectId,
    value: &str,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&unit)
            .is_some_and(|o| !o.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    let value = Markings::try_from(value.to_owned())?;
    let configured = (!value.0.is_empty()).then(|| value.0.clone());
    crate::btech::with_unit_mut!(
        world.btech.unit_mut(unit).context("Unit is unavailable")?,
        |unit| {
            unit.markings = value;
        }
    );
    super::set_unit_identity_configuration(world, unit, "markings", configured);
    Ok(())
}

/// Inspect an explicit or selected contact without scan-range limits or acquisition side effects.
pub fn view_unit_markings(
    world: &World,
    owner: ObjectId,
    actor: ObjectId,
    target: Option<ObjectId>,
) -> Result<String> {
    let operator = super::combat_operator::admit_running(world, owner, actor)?;
    let selected = target.is_none();
    let target = target
        .or_else(|| match operator.source.selection(world) {
            Some(super::TargetSelection::Unit(lock)) => Some(lock.target),
            _ => None,
        })
        .context("You do not have a default target set!")?;
    ensure!(
        super::scanner::scanner_unit(world, target).is_some(),
        "Invalid default target!"
    );
    let observer = operator.source.unit;
    let seen = observer == target || super::visible_contact(world, observer, target)?.is_some();
    if !selected {
        ensure!(seen, "Target is not in line of sight!");
    }
    let source = super::scanner::scanner_unit(world, observer).context("Unit is unavailable")?;
    let unblocked = seen
        && (observer == target
            || source.visibility.clairvoyant
            || !super::unit_terrain_los(world, observer, target)?.blocked);
    ensure!(
        seen && unblocked,
        "{}",
        if selected {
            "That target isn't seen well enough by the scannfers for viewing!"
        } else {
            "That target isn't seen well enough by the scanners for viewing!"
        }
    );
    let markings = unit_markings(world, target)?;
    Ok(if markings.is_empty() {
        "That target has no markings.".into()
    } else {
        crate::text::escape(markings)
    })
}

/// Dispatch map VIEW in map rooms and contact VIEW in cockpits, retaining map authority checks.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let world = ctx.scripts.world();
    let owner = world.objects.get(&ctx.player).and_then(|o| o.location);
    if owner.is_some_and(|id| world.btech.maps().contains_key(&id)) {
        drop(world);
        return super::map_view::command(ctx, input);
    }
    let result = (|| {
        let owner = owner.context("Enter a unit first")?;
        let operator = super::combat_operator::admit_running(&world, owner, ctx.player)?;
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() <= 1, "Invalid number of arguments to function.");
        let target = args
            .first()
            .map(|token| {
                super::radio_targeted::target(&world, operator.source.unit, token)
                    .map_err(|_| anyhow::anyhow!("Target is not in line of sight!"))
            })
            .transpose()?;
        view_unit_markings(&world, owner, ctx.player, target)
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(text) => crate::CommandReport::Styled(text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restoring configuration enforces the same byte limit as live writes.
    #[test]
    fn saved_markings_keep_the_reference_byte_bound() {
        for value in ["x".repeat(16383), "字".repeat(5461), String::new()] {
            let encoded = serde_json::to_string(&value).unwrap();
            let markings: Markings = serde_json::from_str(&encoded).unwrap();
            assert_eq!(String::from(markings), value);
        }
        for value in ["x".repeat(16384), "字".repeat(5462), "null\0byte".into()] {
            assert!(
                serde_json::from_str::<Markings>(&serde_json::to_string(&value).unwrap()).is_err()
            );
        }
    }
}
