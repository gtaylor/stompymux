//! Occupant contact reports share physical visibility and independently owned target selection.
use crate::{ObjectId, Scripts};
use anyhow::{Context, Result};

/// Render current contacts with viewer preferences and transactional building identification.
pub(crate) fn report(
    scripts: &Scripts,
    owner: ObjectId,
    viewer: ObjectId,
    arguments: &str,
) -> Result<String> {
    let argument = arguments.trim();
    let target = argument
        .strip_prefix('#')
        .map(|value| value.parse::<i64>().map(ObjectId))
        .transpose()
        .context("Usage: contacts [+ | options | #unit]")?;
    let world = scripts.world.borrow();
    let source = super::brief::display_source(&world, owner, viewer)?;
    let unit = source.unit;
    let selected = source
        .selection(&world)
        .and_then(|selection| match selection {
            super::BattleTargetSelection::Unit(lock) => Some(lock.target),
            super::BattleTargetSelection::Hex(_) => None,
        });
    let observer = super::scanner::scanner_unit(&world, unit)
        .context("Unit construction state is unavailable")?;
    let brief = observer.brief;
    let brief_buildings = brief.includes_buildings();
    let options = if !argument.is_empty() && !argument.starts_with('+') && target.is_none() {
        Some(
            super::contact_preferences::parse_contact_options_for_display(
                argument,
                brief_buildings,
            )?,
        )
    } else {
        None
    };
    let mut contacts = if let Some(target) = target {
        super::visible_contact(&world, unit, target)?
            .into_iter()
            .collect()
    } else if argument.starts_with('+') {
        super::contact_preferences::filtered_for_source(
            &world,
            source,
            super::contact_preferences(&world, viewer)?,
        )?
    } else if let Some(options) = &options {
        super::contact_preferences::filtered_for_source(&world, source, options.preferences)?
    } else {
        super::visible_contacts(&world, unit)?
    };
    let short = brief.contacts != 0;
    contacts.sort_by_key(|contact| {
        super::scanner::scanner_unit(&world, contact.target).and_then(|unit| unit.slot)
    });
    let mut lines = Vec::new();
    for contact in contacts {
        if short && lines.len() >= 250 {
            break;
        }
        let sort_range = contact.range.spatial
            + if super::scanner::scanner_unit(&world, contact.target)
                .is_some_and(|unit| unit.destroyed)
            {
                10_000.0
            } else {
                0.0
            };
        lines.push((
            sort_range,
            if short {
                contact.styled_short_text(selected == Some(contact.target))
            } else {
                crate::text::escape(&contact.verbose_text)
            },
        ));
    }
    let include_buildings = if argument.starts_with('+') {
        super::contact_preferences(&world, viewer)?
            .buildings
            .includes(brief_buildings)
    } else {
        options
            .as_ref()
            .map_or(target.is_none() && brief_buildings, |options| {
                options.buildings
            })
    };
    drop(world);
    if include_buildings {
        for building in super::building_contacts(scripts, source.owner, viewer)? {
            if lines.len() < 250 {
                lines.push((building.range + 20_000.0, building.styled_text()));
            }
        }
    }
    if short {
        lines.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    let lines: Vec<_> = lines.into_iter().map(|(_, text)| text).collect();
    let list = brief.frame_contacts(&lines);
    let mut output: Vec<_> = options
        .into_iter()
        .flat_map(|options| options.ignored)
        .map(|option| crate::text::escape(&format!("Ignoring {option} as contact option.")))
        .collect();
    if !list.is_empty() {
        output.push(list);
    }
    Ok(output.join("\r\n"))
}
