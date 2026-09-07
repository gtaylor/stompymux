//! Read-only discovery of registered commands and their effective native switch metadata.
use super::*;
use crate::{flags, reports::Report, world::ObjectId};

/// Accepted native switch spelling and minimum unambiguous abbreviation.
#[derive(Clone, Debug)]
pub struct SwitchDefinition {
    pub name: &'static str,
    pub minimum: usize,
    pub permission: CommandPermissions,
}
impl SwitchDefinition {
    pub fn accepts(&self, spelling: &str) -> bool {
        spelling.len() >= self.minimum && self.name.starts_with(spelling)
    }
}
/// Native switch inventory; handlers retain operation-specific combination validation.
pub fn switches(name: &str) -> Vec<SwitchDefinition> {
    let (names, abbreviated): (&[&str], bool) = match name {
        "look" => (&["outside"], true),
        "pose" | "@fpose" => (&["default", "nospace"], true),
        "@emit" | "@femit" => (&["here", "room"], true),
        "@pemit" | "@npemit" => (&["contents", "object", "silent", "list"], true),
        "@wall" => (&["emit", "pose", "wizard", "admin", "no_prefix"], true),
        "@boot" => (&["port", "quiet"], true),
        "@lua" => (
            &[
                "parent",
                "viewparent",
                "check",
                "reload",
                "schedule",
                "test",
                "unit",
                "integration",
                "verbose",
            ],
            false,
        ),
        "@help" => (&["reload"], false),
        "@state" => (&["examine", "set", "wipe", "copy", "move"], false),
        "@examine" => (&["brief", "debug"], true),
        "@halt" => (&["all"], true),
        "get" | "drop" | "give" | "enter" | "leave" => (&["quiet"], true),
        "@dig" => (&["teleport"], true),
        "@open" | "@clone" => (&["inventory", "location"], true),
        "@chan" => (
            &[
                "boot", "create", "destroy", "emit", "list", "object", "oflags", "pflags", "flags",
                "status", "who", "full", "noheader",
            ],
            false,
        ),
        _ => (&[], false),
    };
    names
        .iter()
        .map(|&switch| SwitchDefinition {
            name: switch,
            minimum: if name == "@clone" && switch == "inventory" {
                3
            } else if abbreviated {
                1
            } else {
                switch.len()
            },
            permission: if name.starts_with('@') || name == "give" {
                CommandPermissions::WIZARD
            } else {
                CommandPermissions::EVERYONE
            },
        })
        .collect()
}
/// Search an object database without invoking matching locks or Lua handlers.
pub(super) fn search(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(crate::search::report(
        &ctx.scripts.world.borrow(),
        ctx.player,
        &input.args,
        ctx.config.lua.output_byte_limit,
    )
    .map(Action::LiteralReport)
    .unwrap_or_else(|e| Action::Reply(e.to_string())))
}
pub(super) fn stats(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    if !input.args.trim().is_empty() {
        return Ok(Action::Reply("Usage: @stats".into()));
    }
    Ok(Action::LiteralReport(crate::search::statistics(
        &ctx.scripts.world.borrow(),
    )))
}
/// Only implemented topics are advertised, while C topic names retain explicit diagnostics.
pub(super) fn list(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let result = (|| -> Result<String> {
        let arg = input.args.trim().to_ascii_lowercase();
        let topics = [
            ("bad_names", 2),
            ("commands", 3),
            ("config_permissions", 3),
            ("default_flags", 1),
            ("flags", 2),
            ("globals", 1),
            ("logging", 4),
            ("options", 1),
            ("permissions", 2),
            ("powers", 2),
            ("process", 2),
            ("site_information", 2),
            ("switches", 2),
            ("logfiles", 4),
        ];
        let topic = topics
            .into_iter()
            .find(|(name, min)| arg.len() >= *min && name.starts_with(&arg))
            .map(|(name, _)| name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown option. Use one of: commands flags permissions powers switches"
                )
            })?;
        if topic == "flags" {
            return Ok(format!(
                "Flags: {}",
                flags::ALL
                    .into_iter()
                    .map(|f| format!("{}({})", f.world_name(), f.letter()))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        if topic == "powers" {
            return Ok(format!(
                "Powers: {}",
                crate::powers::ALL
                    .into_iter()
                    .map(|p| p.display_name())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        anyhow::ensure!(
            ["commands", "permissions", "switches"].contains(&topic),
            "@list {topic} is not implemented."
        );
        let world = ctx.scripts.world.borrow();
        let mut report = Report::new(ctx.config.lua.output_byte_limit, "")?;
        for (native, title) in [
            (true, "Built-in commands (global native):"),
            (false, "Global commands (global Lua):"),
        ] {
            if topic == "switches" && !native {
                continue;
            }
            report.row(title);
            let mut count = 0;
            for d in ctx.scripts.commands.definitions().filter(|d| {
                d.scope == CommandScope::Global
                    && matches!(d.handler, CommandHandler::Native(_)) == native
                    && d.listed
                    && d.permission.allows(&world, ctx.player)
            }) {
                if topic == "switches" {
                    let switches = d
                        .switch_definitions
                        .iter()
                        .filter(|s| s.permission.allows(&world, ctx.player))
                        .map(|s| {
                            format!("/{} [{}; min {}]", s.name, s.permission.name(), s.minimum)
                        })
                        .collect::<Vec<_>>();
                    if switches.is_empty() {
                        continue;
                    }
                    report.row(&format!("  {}: {}", d.name, switches.join(" ")));
                } else {
                    report.row(&row(ctx, d, None, topic == "permissions"));
                }
                count += 1;
            }
            if count == 0 {
                report.row("  (none)");
            }
        }
        {
            report.row("Object commands:");
            let mut count = 0;
            for source in super::sources::sources(&world, ctx.player) {
                let id = source.object;
                let object = &world.objects[&id];
                for d in ctx.scripts.commands.definitions().filter(|d| {
                    d.scope == CommandScope::Object(object.lua_parent.clone())
                        && d.listed
                        && d.permission.allows(&world, ctx.player)
                }) {
                    if topic == "switches" {
                        if !matches!(d.handler, CommandHandler::Native(_))
                            || d.switch_definitions.is_empty()
                        {
                            continue;
                        }
                        let switches = d
                            .switch_definitions
                            .iter()
                            .filter(|sw| sw.permission.allows(&world, ctx.player))
                            .map(|sw| {
                                format!(
                                    "/{} [{}; min {}]",
                                    sw.name,
                                    sw.permission.name(),
                                    sw.minimum
                                )
                            })
                            .collect::<Vec<_>>();
                        if switches.is_empty() {
                            continue;
                        }
                        report.row(&format!(
                            "  {}: {}; object: {}; local native / {}",
                            d.name,
                            switches.join(" "),
                            crate::find::identity(&world, Some(id)),
                            source.stage
                        ));
                        count += 1;
                        continue;
                    }
                    report.row(&format!(
                        "{}; {} / {}",
                        row(ctx, d, Some(id), topic == "permissions"),
                        if matches!(d.handler, CommandHandler::Native(_)) {
                            "local native"
                        } else {
                            "local Lua"
                        },
                        source.stage
                    ));
                    count += 1;
                }
            }
            if count == 0 {
                report.row("  (none)");
            }
        }
        report.finish()
    })();
    Ok(result
        .map(Action::LiteralReport)
        .unwrap_or_else(|e| Action::Reply(e.to_string())))
}
fn row(
    ctx: &CommandContext<'_>,
    d: &CommandDefinition,
    object: Option<ObjectId>,
    permissions: bool,
) -> String {
    let mut row = format!("  {}: {}", d.name, d.permission.name());
    if permissions {
        if d.requires_session {
            row.push_str("; requires_session");
        }
        if d.no_macro {
            row.push_str("; no_macro");
        }
        if d.direct_input_only {
            row.push_str("; direct_input_only");
        }
    }
    match &d.matcher {
        CommandMatcher::Native { aliases, prefix } => {
            let mut aliases = aliases.clone();
            for alias in ctx.config.aliases.commands.keys() {
                let resolved = CommandInput::parse(ctx.config, alias);
                if resolved.name == d.name
                    && resolved
                        .switch
                        .as_ref()
                        .is_none_or(|s| d.switch_definitions.iter().any(|d| d.accepts(s)))
                {
                    aliases.push(alias.clone());
                }
            }
            aliases.sort();
            aliases.dedup();
            if !aliases.is_empty() {
                row.push_str(&format!("; aliases: {}", aliases.join(", ")));
            }
            if let Some(prefix) = prefix {
                row.push_str(&format!("; prefix: {prefix}"));
            }
        }
        CommandMatcher::LuaPattern(pattern) => {
            row.push_str(&format!(
                "; {} declaration {}; pattern: {}",
                d.source,
                d.declaration.unwrap_or(0),
                pattern
            ));
            let aliases = ctx
                .config
                .aliases
                .commands
                .keys()
                .filter(|alias| CommandInput::parse(ctx.config, alias).name == d.name)
                .cloned()
                .collect::<Vec<_>>();
            if !aliases.is_empty() {
                row.push_str(&format!("; aliases: {}", aliases.join(", ")));
            }
        }
    }
    if let Some(id) = object {
        row.push_str(&format!(
            "; object: {}",
            crate::find::identity(&ctx.scripts.world.borrow(), Some(id))
        ));
    }
    row.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
