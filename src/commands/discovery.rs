//! Read-only discovery of registered commands and their effective native switch metadata.
use super::*;
use crate::{flags, reports::Report, world::ObjectId};

/// One discoverable list topic, with independent effective access.
#[derive(Clone, Debug)]
pub struct ListDefinition {
    /// Canonical C topic name.
    pub name: &'static str,
    /// Minimum accepted abbreviation length.
    pub minimum: usize,
    /// Effective name-table access, separate from the @list command.
    pub permission: CommandPermissions,
    /// Whether a report handler is currently available.
    pub implemented: bool,
}
impl ListDefinition {
    /// Use the C minimum abbreviation without hiding a denied match.
    pub fn accepts(&self, name: &str) -> bool {
        name.len() >= self.minimum && self.name.starts_with(name)
    }
}
/// Defaults from configuration_registry.c LIST_OPTION_TEMPLATES.
pub fn list_definitions() -> Vec<ListDefinition> {
    use CommandPermissions as P;
    [
        ("bad_names", 2, P::WIZARD, true),
        ("commands", 3, P::EVERYONE, true),
        ("config_permissions", 3, P::GOD, true),
        ("default_flags", 1, P::EVERYONE, true),
        ("flags", 2, P::EVERYONE, true),
        ("globals", 1, P::WIZARD, true),
        ("logging", 4, P::GOD, false),
        ("options", 1, P::EVERYONE, true),
        ("permissions", 2, P::WIZARD, true),
        ("powers", 2, P::WIZARD, true),
        ("process", 2, P::WIZARD, false),
        ("site_information", 2, P::WIZARD, true),
        ("switches", 2, P::EVERYONE, true),
        ("logfiles", 4, P::WIZARD, false),
    ]
    .into_iter()
    .map(|(name, minimum, permission, implemented)| ListDefinition {
        name,
        minimum,
        permission,
        implemented,
    })
    .collect()
}
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
        "@destroy" => (&["override"], false),
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
            permission: crate::access::switch_default(name, switch),
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
    let arg = input.args.trim().to_ascii_lowercase();
    let Some(definition) = ctx.scripts.commands.lists.iter().find(|t| t.accepts(&arg)) else {
        let topics = ctx
            .scripts
            .commands
            .lists
            .iter()
            .filter(|t| {
                t.implemented
                    && (ctx.player == ObjectId(1)
                        || t.permission.allows(&ctx.scripts.world.borrow(), ctx.player))
            })
            .map(|t| t.name)
            .collect::<Vec<_>>()
            .join(" ");
        return Ok(Action::Reply(format!(
            "Unknown option. Use one of: {topics}"
        )));
    };
    if !definition
        .permission
        .allows(&ctx.scripts.world.borrow(), ctx.player)
    {
        return Ok(Action::Reply("Permission denied.".into()));
    }
    if !definition.implemented {
        return Ok(Action::Reply(format!(
            "@list {} is not implemented.",
            definition.name
        )));
    }
    let topic = definition.name;
    if topic == "globals" {
        return Ok(Action::GlobalControl(None));
    }
    if let Some(report) = crate::config::administration::report(ctx, topic) {
        return Ok(Action::LiteralReport(report));
    }
    let result = (|| -> Result<String> {
        if topic == "site_information" {
            return ctx
                .config
                .site_policy
                .report(ctx.config.lua.output_byte_limit)?
                .finish();
        }
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
                        .filter(|s| {
                            ctx.player == ObjectId(1) || s.permission.allows(&world, ctx.player)
                        })
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
                            .filter(|sw| {
                                ctx.player == ObjectId(1)
                                    || sw.permission.allows(&world, ctx.player)
                            })
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

/// Toggle the implemented C global control; other known controls remain explicit errors.
pub(super) fn cleaning(_ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let arg = input.args.trim().to_ascii_lowercase();
    if let Some(control) = crate::controls::Control::parse(&arg) {
        return Ok(Action::GlobalControl(Some((
            control,
            input.name == "@enable",
        ))));
    }
    let known = [("checkpointing", 2)];
    Ok(Action::Reply(
        if known
            .iter()
            .any(|(name, min)| arg.len() >= *min && name.starts_with(&arg))
        {
            format!("Global control {arg} is not implemented.")
        } else {
            "I don't know about that flag.".into()
        },
    ))
}

/// File-cache reload is an asynchronous world-owner action.
pub(super) fn readcache(_ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    Ok(if input.args.is_empty() {
        Action::ReadCache
    } else {
        Action::Reply("Usage: @readcache".into())
    })
}
