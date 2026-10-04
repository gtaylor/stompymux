//! Object-scoped BattleTech help and restricted-command admission before general lookup.
use super::{CommandClass as Class, SpecialType as Special};
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, Flag, Kind, ObjectId};
use anyhow::{Context, Result};

/// Selection is detached from the world borrow before an existing native handler can mutate it.
enum Selection {
    Reply(CommandAction),
    Object(Special, ObjectId, CommandInput),
}

/// Special handlers use the selected object without moving the actor or copying domain state.
pub(crate) fn object(ctx: &CommandContext<'_>) -> Result<ObjectId> {
    ctx.object
        .or_else(|| {
            ctx.scripts
                .world()
                .objects
                .get(&ctx.player)
                .and_then(|actor| actor.location)
        })
        .context("Player has no location")
}

/// Admit catalogue commands through shared services, retaining explicitly inert catalogue handlers.
pub(crate) fn admit(ctx: &CommandContext<'_>, line: &str) -> Result<Option<CommandAction>> {
    match select(ctx, line) {
        None => Ok(None),
        Some(Selection::Reply(action)) => Ok(Some(action)),
        Some(Selection::Object(kind, object, input)) => {
            let selected = CommandContext {
                object: Some(object),
                ..ctx.clone()
            };
            let action = match (kind, input.name.as_str()) {
                (Special::Map, "view") => super::map_view::command(&selected, &input),
                // Map STORES is the actor-location manifest, unlike cockpit STORES.
                (Special::Map, "stores") => super::cargo::manifest_command(&selected, &input),
                (Special::Debug, "setwbv" | "setvrt") => {
                    super::weapon_settings::debug_command(&selected, &input)
                }
                (Special::Debug, "shutdown") => super::map_clear::debug_command(&selected, &input),
                _ => {
                    let (definition, input) = ctx
                        .scripts
                        .commands
                        .native_match(input)
                        .context("Special command lacks a native handler")?;
                    definition.invoke_native(&selected, &input)
                }
            }?;
            Ok(Some(action))
        }
    }
}

/// Handle special HELP and restricted matches using actor/location/contents candidate order.
/// Map, debug and turret field/lifecycle matches retain the selected object; other handlers use native dispatch.
fn select(ctx: &CommandContext<'_>, line: &str) -> Option<Selection> {
    if ctx.scripts.world().btech.registrations().is_empty() {
        return None;
    }
    let compressed;
    let line = if ctx.config.mux.space_compress {
        compressed = line.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        compressed.as_str()
    } else {
        line.trim_start_matches(|c: char| c.is_ascii_whitespace())
    };
    // The reference reserves MBUF_SIZE bytes below its 16 KiB command buffer.
    if line.len() > 14_336 {
        return None;
    }
    let (word, topic) = line.split_once(' ').unwrap_or((line, ""));
    let is_help = word == "HELP";
    let world = ctx.scripts.world();
    let actor = world.objects.get(&ctx.player)?;
    let candidates = std::iter::once(ctx.player).chain(actor.location).chain(
        std::iter::once_with(|| {
            crate::dbck::ordered_members(&world, &world.links, ctx.player, false)
        })
        .flatten(),
    );
    for id in candidates {
        let Some(object) = world.objects.get(&id) else {
            continue;
        };
        if object.kind == Kind::Garbage || object.flags.contains(Flag::Zombie) {
            continue;
        }
        let kind = match world.btech.registrations().get(&id).map(String::as_str) {
            Some("MECH") => Special::Unit,
            Some("MAP") => Special::Map,
            Some("DEBUG") => Special::Debug,
            Some("AUTOPILOT") => Special::Autopilot,
            _ => continue,
        };
        let class = world
            .btech
            .units()
            .get(&id)
            .map(|unit| match unit.class_code {
                0 => Class::Mech,
                1 => Class::Ground,
                2 => Class::Vtol,
                3 => Class::Naval,
                4 | 7 => Class::Dropship,
                5 => Class::Aero,
                6 => Class::MechWarrior,
                8 => Class::BattleSuit,
                _ => Class::Unknown,
            });
        let privileged = crate::authority::is_wizard(&world, ctx.player);
        if let Some((command, arguments)) = kind.find_command(line, class) {
            // A permitted first match also ends the search; later objects cannot deny it.
            if !command.visible(privileged) {
                return Some(Selection::Reply(CommandAction::Report(
                    CommandReport::Reply("Sorry, that command is restricted!".into()),
                )));
            }
            let routed = matches!(kind, Special::Map | Special::Debug);
            return routed.then(|| {
                Selection::Object(
                    kind,
                    id,
                    CommandInput {
                        name: command.name().to_ascii_lowercase(),
                        args: arguments.into(),
                        switch: None,
                        line: line.into(),
                    },
                )
            });
        }
        if is_help {
            return Some(Selection::Reply(CommandAction::Report(
                CommandReport::Styled(kind.help(class, privileged, topic.trim_start_matches(' '))),
            )));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every DEBUG command resolves before general exit and alias lookup.
    #[test]
    fn debug_catalogue_has_complete_native_bindings() {
        let registry = crate::commands::CommandRegistry::new();
        for command in Special::Debug.commands() {
            let name = command.name().to_ascii_lowercase();
            let input = CommandInput {
                name: name.clone(),
                args: String::new(),
                switch: None,
                line: name.clone(),
            };
            assert!(
                registry.native_match(input).is_some(),
                "Unbound debug command: {name}"
            );
        }
    }

    /// Every MAP catalogue entry has a handler; overloaded VIEW and STORES have explicit adapters.
    #[test]
    fn map_catalogue_has_complete_native_bindings() {
        let registry = crate::commands::CommandRegistry::new();
        for command in Special::Map.commands() {
            let name = command.name().to_ascii_lowercase();
            if matches!(name.as_str(), "view" | "stores") {
                continue;
            }
            let input = CommandInput {
                name: name.clone(),
                args: String::new(),
                switch: None,
                line: name.clone(),
            };
            assert!(
                registry.native_match(input).is_some(),
                "Unbound map command: {name}"
            );
        }
    }
}
