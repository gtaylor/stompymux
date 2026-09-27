//! Native and Lua command definitions, immutable registration and access metadata.
use super::{Action, CommandContext, native};
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use mlua::{Function, Lua, Table, Value};

pub use crate::access::Permissions as CommandPermissions;
/// Where a definition is eligible for dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandScope {
    /// Object-local commands attached via the named Lua parent.
    Object(String),
    /// Server-wide handlers; Lua modules retain lexical load order.
    Global,
}
/// Matching metadata without inferring command names from Lua patterns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandMatcher {
    /// Exact canonical token plus optional built-in aliases and shorthand prefix.
    Native {
        aliases: Vec<String>,
        prefix: Option<char>,
    },
    /// Existing Lua string.find pattern, with captures passed to the handler.
    LuaPattern(String),
}
/// Native function signature; registration requires no command-name branching.
pub type NativeHandler = fn(&CommandContext<'_>, &CommandInput) -> Result<Action>;
/// Executable implementation retained at registration time.
#[derive(Clone)]
pub enum CommandHandler {
    /// Ordinary Rust function pointer.
    Native(NativeHandler),
    /// Lua closure owned by the world thread.
    Lua(Function),
}
/// Native switch policy is checked after permissions and before invoking handlers.
#[derive(Clone, Copy)]
pub enum SwitchPolicy {
    /// Reject all switches using this diagnostic.
    Reject(&'static str),
    /// Handler validates its supported switch syntax.
    Handler,
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
pub fn native_switches(name: &str) -> Vec<SwitchDefinition> {
    let (names, abbreviated): (&[&str], bool) = match name {
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
        "@btech" => (&["info", "register", "unregister"], true),
        "@destroy" => (&["override"], false),
        "@help" => (&["reload"], false),
        "@state" => (&["examine", "set", "wipe", "copy", "move"], false),
        "@examine" => (&["brief", "debug"], true),
        "@halt" => (&["all"], true),
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
/// Complete enumerable command definition used by dispatch and discovery.
#[derive(Clone)]
pub struct CommandDefinition {
    /// Lowercase canonical command name.
    pub name: String,
    /// Native switches available for discovery; handlers validate combinations.
    pub switch_definitions: Vec<SwitchDefinition>,
    /// Whether this entry appears in command listings.
    pub listed: bool,
    /// Required authority.
    pub permission: CommandPermissions,
    /// Canonical typed Lua access gate, evaluated against the live invoker.
    pub(crate) lua_access: crate::lua::command_access::CommandAccess,
    /// Original declaration, before configuration edits.
    pub declared_permission: CommandPermissions,
    /// Exact or pattern-based matching metadata.
    pub matcher: CommandMatcher,
    /// Object attachment or server-wide eligibility, independent of handler language.
    pub scope: CommandScope,
    /// Native marker or relative Lua module path.
    pub source: String,
    /// One-based Lua command declaration index; absent for native entries.
    pub declaration: Option<usize>,
    /// Registered executable function.
    pub handler: CommandHandler,
    /// Supported native switch handling.
    pub switches: SwitchPolicy,
    /// Session-only errors are required by read-only searches.
    pub private_errors: bool,
    /// Eligible only before interactive macro expansion.
    pub direct_input_only: bool,
    /// Command requires the invoking connection, never a borrowed player session.
    pub requires_session: bool,
    /// Native contextual response when invoked without a descriptor.
    pub session_error: &'static str,
    /// C command cannot be reached through macro expansion.
    pub no_macro: bool,
}
impl CommandDefinition {
    /// Describe a native handler with default exact matching and no switches.
    pub fn native(name: &str, permission: CommandPermissions, handler: NativeHandler) -> Self {
        let permission = crate::access::native_defaults(name, permission);
        Self {
            declared_permission: permission,
            lua_access: Default::default(),
            name: name.into(),
            switch_definitions: native_switches(name),
            listed: !matches!(name, ";" | "\\"),
            permission,
            matcher: CommandMatcher::Native {
                aliases: Vec::new(),
                prefix: None,
            },
            scope: CommandScope::Global,
            source: "native".into(),
            declaration: None,
            handler: CommandHandler::Native(handler),
            switches: SwitchPolicy::Reject("Unsupported command switch."),
            private_errors: false,
            direct_input_only: false,
            requires_session: false,
            session_error: "This command requires an interactive session.",
            no_macro: false,
        }
    }
    /// Customize a descriptor-only command's error while retaining enumerable metadata.
    pub fn requiring_session(mut self, message: &'static str) -> Self {
        self.requires_session = true;
        self.session_error = message;
        self
    }
    /// Attach a native handler to objects using this module identity.
    pub fn object_native(
        name: &str,
        permission: CommandPermissions,
        module: &str,
        handler: NativeHandler,
    ) -> Self {
        let mut definition = Self::native(name, permission, handler);
        definition.scope = CommandScope::Object(module.into());
        definition
    }
    /// Attach built-in shorthand metadata without adding duplicate catalog entries.
    fn matching(mut self, aliases: &[&str], prefix: Option<char>) -> Self {
        self.matcher = CommandMatcher::Native {
            aliases: aliases.iter().map(|s| (*s).into()).collect(),
            prefix,
        };
        self
    }
    /// Set the switch parser and error delivery policy as registration metadata.
    fn policy(mut self, switches: SwitchPolicy, private_errors: bool) -> Self {
        self.switches = switches;
        self.private_errors = private_errors;
        self
    }
    /// Invoke a native function only after central permissions and switch checks.
    pub fn invoke_native(&self, ctx: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
        let error = self
            .permission
            .denial(
                &ctx.scripts.world.borrow(),
                ctx.player,
                crate::access::Context {
                    queue_enabled: ctx.scripts.queue_enabled.get(),
                },
            )
            .or_else(|| {
                if self.requires_session && ctx.session.is_none() {
                    Some(self.session_error)
                } else {
                    None
                }
            })
            .or_else(|| {
                let switch = input.switch.as_deref()?;
                if let SwitchPolicy::Reject(message) = self.switches {
                    return Some(message);
                }
                for part in switch.split('/') {
                    let Some(definition) = self.switch_definitions.iter().find(|s| s.accepts(part))
                    else {
                        // The handler owns command-specific syntax diagnostics.
                        break;
                    };
                    if !definition
                        .permission
                        .allows(&ctx.scripts.world.borrow(), ctx.player)
                    {
                        return Some("Permission denied.");
                    }
                }
                None
            });
        if let Some(error) = error {
            if self.private_errors {
                return Ok(Action::Report(crate::commands::Report::Reply(error.into())));
            }
            ctx.scripts
                .outbox
                .borrow_mut()
                .push((ctx.player, error.into()));
            return Ok(Action::Continue);
        }
        match self.handler {
            CommandHandler::Native(handler) => handler(ctx, input),
            _ => anyhow::bail!("not a native command"),
        }
    }
}
/// Parsed command text shared by exact dispatch and Lua alias matching.
#[derive(Clone, Debug)]
pub struct CommandInput {
    /// Canonical base token for native lookup.
    pub name: String,
    /// Remaining text after the command token.
    pub args: String,
    /// Lowercase switch text, if supplied.
    pub switch: Option<String>,
    /// Original trimmed line, or canonical alias token plus original arguments.
    pub line: String,
}
impl CommandInput {
    /// Apply full-token aliases then base aliases once, without recursive expansion.
    pub fn parse(config: &Config, line: &str) -> Self {
        let line = line.trim();
        let end = line.find(char::is_whitespace).unwrap_or(line.len());
        let verb = &line[..end];
        let tail = &line[end..];
        let args = tail.trim_start().to_string();
        let original = verb.to_ascii_lowercase();
        let full = config.aliases.commands.get(&original);
        let token = full.cloned().unwrap_or(original);
        let (base, switch) = token
            .split_once('/')
            .map_or((token.as_str(), None), |(a, b)| (a, Some(b)));
        let base_alias = if full.is_none() {
            config.aliases.commands.get(base)
        } else {
            None
        };
        let resolved = match base_alias {
            Some(alias) => format!(
                "{alias}{}",
                switch.map_or(String::new(), |s| format!("/{s}"))
            ),
            None => token.clone(),
        }
        .to_ascii_lowercase();
        let (name, switch) = resolved
            .split_once('/')
            .map_or((resolved.as_str(), None), |(a, b)| (a, Some(b.to_string())));
        Self {
            name: name.into(),
            args,
            switch,
            line: if full.is_some() || base_alias.is_some() {
                format!("{resolved}{tail}")
            } else {
                line.into()
            },
        }
    }
}
/// Definitions are registered once at load, in deterministic dispatch order.
#[derive(Clone)]
pub struct CommandRegistry {
    definitions: Vec<CommandDefinition>,
    /// Effective list topic catalog.
    pub lists: Vec<super::discovery::ListDefinition>,
}
impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
impl CommandRegistry {
    /// Build the native table before registering game modules.
    pub fn new() -> Self {
        use CommandPermissions as P;
        let mut definitions = vec![
            CommandDefinition::native("hulldown", P::EVERYONE, crate::btech::hull_down::command)
                .policy(SwitchPolicy::Reject("hulldown takes no switches."), false),
            CommandDefinition::native("dig", P::EVERYONE, crate::btech::dig::command)
                .policy(SwitchPolicy::Reject("dig takes no switches."), false),
            CommandDefinition::native("pickup", P::EVERYONE, crate::btech::tow_actions::command)
                .policy(SwitchPolicy::Reject("pickup takes no switches."), false),
            CommandDefinition::native("dropoff", P::EVERYONE, crate::btech::tow_actions::command)
                .policy(SwitchPolicy::Reject("dropoff takes no switches."), false),
            CommandDefinition::native(
                "enterbase",
                P::EVERYONE,
                crate::btech::building_actions::command,
            )
            .policy(SwitchPolicy::Reject("enterbase takes no switches."), false),
            CommandDefinition::native("addtic", P::EVERYONE, crate::btech::tic::command)
                .policy(SwitchPolicy::Reject("addtic takes no switches."), false),
            CommandDefinition::native("deltic", P::EVERYONE, crate::btech::tic::command)
                .policy(SwitchPolicy::Reject("deltic takes no switches."), false),
            CommandDefinition::native("cleartic", P::EVERYONE, crate::btech::tic::command)
                .policy(SwitchPolicy::Reject("cleartic takes no switches."), false),
            CommandDefinition::native("listtic", P::EVERYONE, crate::btech::tic::command)
                .policy(SwitchPolicy::Reject("listtic takes no switches."), false),
            CommandDefinition::native(
                "autoturret",
                P::EVERYONE,
                crate::btech::automatic_turret::command,
            )
            .policy(SwitchPolicy::Reject("autoturret takes no switches."), false),
            CommandDefinition::native("disable", P::EVERYONE, crate::btech::weapon_power::command)
                .policy(SwitchPolicy::Reject("disable takes no switches."), false),
            CommandDefinition::native(
                "usebin",
                P::EVERYONE,
                crate::btech::ammunition_preference::command,
            )
            .policy(SwitchPolicy::Reject("usebin takes no switches."), false),
            CommandDefinition::native("hide", P::EVERYONE, crate::btech::hiding::command)
                .policy(SwitchPolicy::Reject("hide takes no switches."), false),
            CommandDefinition::native(
                "heatcutoff",
                P::EVERYONE,
                crate::btech::heat_cutoff::command,
            )
            .policy(SwitchPolicy::Reject("heatcutoff takes no switches."), false),
            CommandDefinition::native("explode", P::EVERYONE, crate::btech::self_destruct::command)
                .policy(SwitchPolicy::Reject("explode takes no switches."), false),
            CommandDefinition::native("firetic", P::EVERYONE, crate::btech::tic::fire_command)
                .policy(SwitchPolicy::Reject("firetic takes no switches."), false),
            CommandDefinition::native("target", P::EVERYONE, crate::btech::aimed_target::command)
                .policy(SwitchPolicy::Reject("target takes no switches."), false),
            CommandDefinition::native("sight", P::EVERYONE, crate::btech::sight::command)
                .policy(SwitchPolicy::Reject("sight takes no switches."), false),
            CommandDefinition::native("fire", P::EVERYONE, crate::btech::firing::fire_command)
                .policy(SwitchPolicy::Reject("fire takes no switches."), false),
            CommandDefinition::native("land", P::EVERYONE, crate::btech::landing::command)
                .policy(SwitchPolicy::Reject("land takes no switches."), false),
            CommandDefinition::native("takeoff", P::EVERYONE, crate::btech::vtol_controls::command)
                .policy(SwitchPolicy::Reject("takeoff takes no switches."), false),
            CommandDefinition::native(
                "vertical",
                P::EVERYONE,
                crate::btech::vtol_controls::command,
            )
            .policy(SwitchPolicy::Reject("vertical takes no switches."), false),
            CommandDefinition::native("jump", P::EVERYONE, crate::btech::jumping::command)
                .policy(SwitchPolicy::Reject("jump takes no switches."), false),
            CommandDefinition::native(
                "weapons",
                P::EVERYONE,
                crate::btech::firing::weapons_command,
            )
            .policy(SwitchPolicy::Reject("weapons takes no switches."), false),
            CommandDefinition::native(
                "weaponstatus",
                P::EVERYONE,
                crate::btech::weapon_reports::status_command,
            )
            .policy(
                SwitchPolicy::Reject("weaponstatus takes no switches."),
                false,
            ),
            CommandDefinition::native(
                "weaponspecs",
                P::EVERYONE,
                crate::btech::weapon_reports::specs_command,
            )
            .policy(
                SwitchPolicy::Reject("weaponspecs takes no switches."),
                false,
            ),
            CommandDefinition::native(
                "critstatus",
                P::EVERYONE,
                crate::btech::critical_report::command,
            )
            .policy(SwitchPolicy::Reject("critstatus takes no switches."), false),
            CommandDefinition::native(
                "addstuff",
                P::WIZARD,
                crate::btech::stock_commands::add_command,
            )
            .policy(SwitchPolicy::Reject("addstuff takes no switches."), false),
            CommandDefinition::native(
                "removestuff",
                P::WIZARD,
                crate::btech::stock_commands::remove_command,
            )
            .policy(
                SwitchPolicy::Reject("removestuff takes no switches."),
                false,
            ),
            CommandDefinition::native("setteam", P::WIZARD, crate::btech::scenario_team::command)
                .policy(SwitchPolicy::Reject("SETTEAM takes no switches."), false),
            CommandDefinition::native(
                "@damage",
                P::WIZARD,
                crate::btech::scenario_packets::command,
            )
            .policy(SwitchPolicy::Reject("@DAMAGE takes no switches."), false),
            CommandDefinition::native("@weight", P::WIZARD, crate::btech::weight_report::command)
                .policy(SwitchPolicy::Reject("@WEIGHT takes no switches."), false),
            CommandDefinition::native("setmapindx", P::WIZARD, crate::btech::scenario_map::command)
                .policy(SwitchPolicy::Reject("SETMAPINDX takes no switches."), false),
            CommandDefinition::native(
                "@ood",
                P::WIZARD,
                crate::btech::orbital_drop_launch::command,
            )
            .policy(SwitchPolicy::Reject("@OOD takes no switches."), false),
            CommandDefinition::native("setxy", P::WIZARD, crate::btech::scenario_position::command)
                .policy(SwitchPolicy::Reject("SETXY takes no switches."), false),
            CommandDefinition::native("@losemit", P::WIZARD, crate::btech::losemit::command)
                .policy(SwitchPolicy::Reject("@LOSEMIT takes no switches."), false),
            CommandDefinition::native(
                "@damagesection",
                P::WIZARD,
                crate::btech::scenario_damage::command,
            )
            .policy(
                SwitchPolicy::Reject("@DAMAGESECTION takes no switches."),
                false,
            ),
            CommandDefinition::native(
                "eventstats",
                P::WIZARD,
                crate::btech::runtime_stats::command,
            )
            .policy(SwitchPolicy::Reject("EVENTSTATS takes no switches."), false),
            CommandDefinition::native("memstats", P::WIZARD, crate::btech::runtime_stats::command)
                .policy(SwitchPolicy::Reject("MEMSTATS takes no switches."), false),
            CommandDefinition::native("listforms", P::WIZARD, crate::btech::forms_report::command)
                .policy(SwitchPolicy::Reject("LISTFORMS takes no switches."), false),
            CommandDefinition::native("savedb", P::WIZARD, crate::btech::database_save::command)
                .policy(SwitchPolicy::Reject("SAVEDB takes no switches."), false),
            CommandDefinition::native(
                "+charclear",
                P::WIZARD,
                crate::btech::character_clear::command,
            )
            .policy(SwitchPolicy::Reject("+CHARCLEAR takes no switches."), false),
            CommandDefinition::native("+show", P::WIZARD, crate::btech::character_show::command)
                .policy(SwitchPolicy::Reject("+SHOW takes no switches."), false),
            CommandDefinition::native("xptop", P::WIZARD, crate::btech::xp_ranking::command)
                .policy(SwitchPolicy::Reject("XPTOP takes no switches."), false),
            CommandDefinition::native(
                "setxplevel",
                P::WIZARD,
                crate::btech::skill_catalog::threshold_command,
            )
            .policy(SwitchPolicy::Reject("SETXPLEVEL takes no switches."), false),
            CommandDefinition::native(
                "setvrt",
                P::WIZARD,
                crate::btech::weapon_settings::recycle_command,
            )
            .policy(SwitchPolicy::Reject("SETVRT takes no switches."), false),
            CommandDefinition::native(
                "setwbv",
                P::WIZARD,
                crate::btech::weapon_settings::battle_value_command,
            )
            .policy(SwitchPolicy::Reject("SETWBV takes no switches."), false),
            CommandDefinition::native("@setspecial", P::WIZARD, |ctx, input| {
                crate::btech::special_fields::command(ctx, input, true)
            })
            .policy(
                SwitchPolicy::Reject("@SETSPECIAL takes no switches."),
                false,
            ),
            CommandDefinition::native("@viewspecial", P::WIZARD, |ctx, input| {
                crate::btech::special_fields::command(ctx, input, false)
            })
            .policy(
                SwitchPolicy::Reject("@VIEWSPECIAL takes no switches."),
                false,
            ),
            CommandDefinition::native(
                "@setmech",
                P::WIZARD,
                crate::btech::unit_fields::set_command,
            )
            .policy(SwitchPolicy::Reject("@SETMECH takes no switches."), false),
            CommandDefinition::native("@viewmech", P::WIZARD, crate::btech::unit_fields::command)
                .policy(SwitchPolicy::Reject("@VIEWMECH takes no switches."), false),
            CommandDefinition::native(
                "@viewmap",
                P::WIZARD,
                crate::btech::map_field_report::command,
            )
            .policy(SwitchPolicy::Reject("@VIEWMAP takes no switches."), false),
            CommandDefinition::native("fixmap", P::WIZARD, crate::btech::map_check::command)
                .policy(SwitchPolicy::Reject("FIXMAP takes no switches."), false),
            CommandDefinition::native("@setmap", P::WIZARD, crate::btech::map_fields::command)
                .policy(SwitchPolicy::Reject("@SETMAP takes no switches."), false),
            CommandDefinition::native("view", P::EVERYONE, crate::btech::markings::command)
                .policy(SwitchPolicy::Reject("VIEW takes no switches."), false),
            CommandDefinition::native(
                "updatelinks",
                P::WIZARD,
                crate::btech::map_update_links::command,
            )
            .policy(
                SwitchPolicy::Reject("UPDATELINKS takes no switches."),
                false,
            ),
            CommandDefinition::native("list", P::WIZARD, crate::btech::map_list::command)
                .policy(SwitchPolicy::Reject("LIST takes no switches."), false),
            CommandDefinition::native(
                "delobj",
                P::WIZARD,
                crate::btech::map_object_delete::command,
            )
            .policy(SwitchPolicy::Reject("DELOBJ takes no switches."), false),
            CommandDefinition::native("addfire", P::WIZARD, crate::btech::map_decoration::command)
                .policy(SwitchPolicy::Reject("ADDFIRE takes no switches."), false),
            CommandDefinition::native("addsmoke", P::WIZARD, crate::btech::map_decoration::command)
                .policy(SwitchPolicy::Reject("ADDSMOKE takes no switches."), false),
            CommandDefinition::native("addmine", P::WIZARD, crate::btech::map_mine::command)
                .policy(SwitchPolicy::Reject("ADDMINE takes no switches."), false),
            CommandDefinition::native("addblock", P::WIZARD, crate::btech::map_block::command)
                .policy(SwitchPolicy::Reject("ADDBLOCK takes no switches."), false),
            CommandDefinition::native("setlinked", P::WIZARD, crate::btech::map_link::command)
                .policy(SwitchPolicy::Reject("SETLINKED takes no switches."), false),
            CommandDefinition::native("loadmap", P::WIZARD, crate::btech::map_load::command)
                .policy(SwitchPolicy::Reject("LOADMAP takes no switches."), false),
            CommandDefinition::native("savemap", P::WIZARD, crate::btech::map_save::command)
                .policy(SwitchPolicy::Reject("SAVEMAP takes no switches."), false),
            CommandDefinition::native("setmapsize", P::WIZARD, crate::btech::map_resize::command)
                .policy(SwitchPolicy::Reject("SETMAPSIZE takes no switches."), false),
            CommandDefinition::native("clearmechs", P::WIZARD, crate::btech::map_clear::command)
                .policy(SwitchPolicy::Reject("CLEARMECHS takes no switches."), false),
            CommandDefinition::native("@mapemit", P::WIZARD, crate::btech::map_emit::command)
                .policy(SwitchPolicy::Reject("@mapemit takes no switches."), false),
            CommandDefinition::native("addhex", P::WIZARD, crate::btech::terrain_edit::command)
                .policy(SwitchPolicy::Reject("addhex takes no switches."), false),
            CommandDefinition::native("addice", P::WIZARD, crate::btech::map_ice::add_command)
                .policy(SwitchPolicy::Reject("addice takes no switches."), false),
            CommandDefinition::native("delice", P::WIZARD, crate::btech::map_ice::remove_command)
                .policy(SwitchPolicy::Reject("delice takes no switches."), false),
            CommandDefinition::native("setcond", P::WIZARD, crate::btech::map_environment::command)
                .policy(SwitchPolicy::Reject("setcond takes no switches."), false),
            CommandDefinition::native(
                "fixstuff",
                P::WIZARD,
                crate::btech::inventory_cleanup::command,
            )
            .policy(SwitchPolicy::Reject("fixstuff takes no switches."), false),
            CommandDefinition::native(
                "clearstuff",
                P::WIZARD,
                crate::btech::stock_commands::clear_command,
            )
            .policy(SwitchPolicy::Reject("clearstuff takes no switches."), false),
            CommandDefinition::native(
                "manifest",
                P::EVERYONE,
                crate::btech::cargo::manifest_command,
            )
            .policy(SwitchPolicy::Reject("manifest takes no switches."), false),
            CommandDefinition::native("stores", P::EVERYONE, crate::btech::cargo::stores_command)
                .policy(SwitchPolicy::Reject("stores takes no switches."), false),
            CommandDefinition::native("loadcargo", P::EVERYONE, crate::btech::cargo::load_command)
                .policy(SwitchPolicy::Reject("loadcargo takes no switches."), false),
            CommandDefinition::native(
                "unloadcargo",
                P::EVERYONE,
                crate::btech::cargo::unload_command,
            )
            .policy(
                SwitchPolicy::Reject("unloadcargo takes no switches."),
                false,
            ),
            CommandDefinition::native("status", P::EVERYONE, crate::btech::status::command)
                .policy(SwitchPolicy::Reject("status takes no switches."), false),
            CommandDefinition::native(
                "charge",
                P::EVERYONE,
                crate::btech::physical::charge_command,
            )
            .policy(SwitchPolicy::Reject("charge takes no switches."), false),
            CommandDefinition::native("club", P::EVERYONE, crate::btech::physical::club_command)
                .policy(SwitchPolicy::Reject("club takes no switches."), false),
            CommandDefinition::native(
                "grabclub",
                P::EVERYONE,
                crate::btech::physical::grabclub_command,
            )
            .policy(SwitchPolicy::Reject("grabclub takes no switches."), false),
            CommandDefinition::native("claw", P::EVERYONE, crate::btech::physical::claw_command)
                .policy(SwitchPolicy::Reject("claw takes no switches."), false),
            CommandDefinition::native("saw", P::EVERYONE, crate::btech::physical::saw_command)
                .policy(SwitchPolicy::Reject("saw takes no switches."), false),
            CommandDefinition::native("mace", P::EVERYONE, crate::btech::physical::mace_command)
                .policy(SwitchPolicy::Reject("mace takes no switches."), false),
            CommandDefinition::native("axe", P::EVERYONE, crate::btech::physical::axe_command)
                .policy(SwitchPolicy::Reject("axe takes no switches."), false),
            CommandDefinition::native("chop", P::EVERYONE, crate::btech::physical::sword_command)
                .policy(SwitchPolicy::Reject("chop takes no switches."), false),
            CommandDefinition::native("sword", P::EVERYONE, crate::btech::physical::sword_command)
                .policy(SwitchPolicy::Reject("sword takes no switches."), false),
            CommandDefinition::native("punch", P::EVERYONE, crate::btech::physical::punch_command)
                .policy(SwitchPolicy::Reject("punch takes no switches."), false),
            CommandDefinition::native("trip", P::EVERYONE, crate::btech::physical::trip_command)
                .policy(SwitchPolicy::Reject("trip takes no switches."), false),
            CommandDefinition::native("kick", P::EVERYONE, crate::btech::physical::command)
                .policy(SwitchPolicy::Reject("kick takes no switches."), false),
            CommandDefinition::native("prone", P::EVERYONE, crate::btech::prone::command)
                .policy(SwitchPolicy::Reject("prone takes no switches."), false),
            CommandDefinition::native("stand", P::EVERYONE, crate::btech::commands::stand_command)
                .policy(SwitchPolicy::Reject("stand takes no switches."), false),
            CommandDefinition::native("lock", P::EVERYONE, crate::btech::commands::lock_command)
                .policy(SwitchPolicy::Reject("lock takes no switches."), false),
            CommandDefinition::native("tag", P::EVERYONE, crate::btech::commands::tag_command)
                .policy(SwitchPolicy::Reject("tag takes no switches."), false),
            CommandDefinition::native("spot", P::EVERYONE, crate::btech::commands::spot_command)
                .policy(SwitchPolicy::Reject("spot takes no switches."), false),
            CommandDefinition::native(
                "contacts",
                P::EVERYONE,
                crate::btech::commands::contacts_command,
            )
            .policy(SwitchPolicy::Reject("contacts takes no switches."), false),
            CommandDefinition::native(
                "sensor",
                P::EVERYONE,
                crate::btech::commands::sensor_command,
            )
            .policy(SwitchPolicy::Reject("sensor takes no switches."), false),
            CommandDefinition::native("nss", P::EVERYONE, crate::btech::commands::facing_command)
                .policy(SwitchPolicy::Reject("nss takes no switches."), false),
            CommandDefinition::native(
                "stealth",
                P::EVERYONE,
                crate::btech::commands::facing_command,
            )
            .policy(SwitchPolicy::Reject("stealth takes no switches."), false),
            CommandDefinition::native("snipe", P::EVERYONE, crate::btech::snipe::command)
                .policy(SwitchPolicy::Reject("snipe takes no switches."), false),
            CommandDefinition::native("safety", P::EVERYONE, crate::btech::safety::command)
                .policy(SwitchPolicy::Reject("safety takes no switches."), false),
            CommandDefinition::native("slite", P::EVERYONE, crate::btech::commands::facing_command)
                .policy(SwitchPolicy::Reject("slite takes no switches."), false),
            CommandDefinition::native(
                "rottorso",
                P::EVERYONE,
                crate::btech::commands::facing_command,
            )
            .policy(SwitchPolicy::Reject("rottorso takes no switches."), false),
            CommandDefinition::native(
                "hotload",
                P::EVERYONE,
                crate::btech::fire_mode::hotload_command,
            )
            .policy(SwitchPolicy::Reject("hotload takes no switches."), false),
            CommandDefinition::native("gattling", P::EVERYONE, crate::btech::gatling::command)
                .policy(SwitchPolicy::Reject("gattling takes no switches."), false),
            CommandDefinition::native("flechette", P::EVERYONE, crate::btech::flechette::command)
                .policy(SwitchPolicy::Reject("flechette takes no switches."), false),
            CommandDefinition::native(
                "armorpiercing",
                P::EVERYONE,
                crate::btech::armor_piercing::command,
            )
            .matching(&["ap"], None)
            .policy(
                SwitchPolicy::Reject("armorpiercing takes no switches."),
                false,
            ),
            CommandDefinition::native("caseless", P::EVERYONE, crate::btech::caseless::command)
                .policy(SwitchPolicy::Reject("caseless takes no switches."), false),
            CommandDefinition::native("vector", P::EVERYONE, crate::btech::vector_display::command)
                .policy(SwitchPolicy::Reject("vector takes no switches."), false),
            CommandDefinition::native("range", P::EVERYONE, crate::btech::range_display::command)
                .policy(SwitchPolicy::Reject("range takes no switches."), false),
            CommandDefinition::native("bearing", P::EVERYONE, crate::btech::bearing::command)
                .policy(SwitchPolicy::Reject("bearing takes no switches."), false),
            CommandDefinition::native("eta", P::EVERYONE, crate::btech::eta::command)
                .policy(SwitchPolicy::Reject("eta takes no switches."), false),
            CommandDefinition::native("bootlegger", P::EVERYONE, crate::btech::bootlegger::command)
                .policy(SwitchPolicy::Reject("bootlegger takes no switches."), false),
            CommandDefinition::native(
                "c3targets",
                P::EVERYONE,
                crate::btech::network_targets::c3_command,
            )
            .policy(SwitchPolicy::Reject("c3targets takes no switches."), false),
            CommandDefinition::native(
                "c3network",
                P::EVERYONE,
                crate::btech::network_status::c3_command,
            )
            .policy(SwitchPolicy::Reject("c3network takes no switches."), false),
            CommandDefinition::native(
                "c3itargets",
                P::EVERYONE,
                crate::btech::network_targets::command,
            )
            .policy(SwitchPolicy::Reject("c3itargets takes no switches."), false),
            CommandDefinition::native(
                "c3inetwork",
                P::EVERYONE,
                crate::btech::network_status::command,
            )
            .policy(SwitchPolicy::Reject("c3inetwork takes no switches."), false),
            CommandDefinition::native(
                "c3message",
                P::EVERYONE,
                crate::btech::network_message::c3_command,
            )
            .policy(SwitchPolicy::Reject("c3message takes no switches."), false),
            CommandDefinition::native(
                "c3imessage",
                P::EVERYONE,
                crate::btech::network_message::command,
            )
            .policy(SwitchPolicy::Reject("c3imessage takes no switches."), false),
            CommandDefinition::native("c3", P::EVERYONE, crate::btech::command_network::c3_command)
                .policy(SwitchPolicy::Reject("c3 takes no switches."), false),
            CommandDefinition::native("c3i", P::EVERYONE, crate::btech::command_network::command)
                .policy(SwitchPolicy::Reject("c3i takes no switches."), false),
            CommandDefinition::native(
                "scharge",
                P::EVERYONE,
                crate::btech::booster_control::supercharger_command,
            )
            .policy(SwitchPolicy::Reject("scharge takes no switches."), false),
            CommandDefinition::native(
                "masc",
                P::EVERYONE,
                crate::btech::booster_control::masc_command,
            )
            .policy(SwitchPolicy::Reject("masc takes no switches."), false),
            CommandDefinition::native("dump", P::EVERYONE, crate::btech::dumping::command)
                .policy(SwitchPolicy::Reject("dump takes no switches."), false),
            CommandDefinition::native("lateral", P::EVERYONE, crate::btech::lateral::command)
                .policy(SwitchPolicy::Reject("lateral takes no switches."), false),
            CommandDefinition::native("brief", P::EVERYONE, crate::btech::brief::command)
                .policy(SwitchPolicy::Reject("brief takes no switches."), false),
            CommandDefinition::native(
                "mapdisplay",
                P::EVERYONE,
                crate::btech::view_preferences::command,
            )
            .policy(SwitchPolicy::Reject("mapdisplay takes no switches."), false),
            CommandDefinition::native(
                "findcenter",
                P::EVERYONE,
                crate::btech::find_center::command,
            )
            .policy(SwitchPolicy::Reject("findcenter takes no switches."), false),
            CommandDefinition::native("navigate", P::EVERYONE, crate::btech::navigation::command)
                .policy(SwitchPolicy::Reject("navigate takes no switches."), false),
            CommandDefinition::native("tactical", P::EVERYONE, crate::btech::tactical_map::command)
                .policy(SwitchPolicy::Reject("tactical takes no switches."), false),
            CommandDefinition::native("lrs", P::EVERYONE, crate::btech::long_range_map::command)
                .matching(&["lrsmap"], None)
                .policy(SwitchPolicy::Reject("lrs takes no switches."), false),
            CommandDefinition::native("report", P::EVERYONE, crate::btech::report::command)
                .policy(SwitchPolicy::Reject("report takes no switches."), false),
            CommandDefinition::native("scan", P::EVERYONE, crate::btech::scan::command)
                .policy(SwitchPolicy::Reject("scan takes no switches."), false),
            CommandDefinition::native("radio", P::EVERYONE, crate::btech::radio_targeted::command)
                .policy(SwitchPolicy::Reject("radio takes no switches."), false),
            CommandDefinition::native(
                "sendchannel",
                P::EVERYONE,
                crate::btech::radio_transmission::command,
            )
            .policy(
                SwitchPolicy::Reject("sendchannel takes no switches."),
                false,
            ),
            CommandDefinition::native("setchannelfreq", P::EVERYONE, crate::btech::radio::command)
                .policy(
                    SwitchPolicy::Reject("setchannelfreq takes no switches."),
                    false,
                ),
            CommandDefinition::native("setchanneltitle", P::EVERYONE, crate::btech::radio::command)
                .policy(
                    SwitchPolicy::Reject("setchanneltitle takes no switches."),
                    false,
                ),
            CommandDefinition::native("setchannelmode", P::EVERYONE, crate::btech::radio::command)
                .policy(
                    SwitchPolicy::Reject("setchannelmode takes no switches."),
                    false,
                ),
            CommandDefinition::native("listchannels", P::EVERYONE, crate::btech::radio::command)
                .policy(
                    SwitchPolicy::Reject("listchannels takes no switches."),
                    false,
                ),
            CommandDefinition::native("listfreqs", P::EVERYONE, crate::btech::radio::command)
                .policy(SwitchPolicy::Reject("listfreqs takes no switches."), false),
            CommandDefinition::native(
                "inferno",
                P::EVERYONE,
                crate::btech::inferno_ammunition::command,
            )
            .policy(SwitchPolicy::Reject("inferno takes no switches."), false),
            CommandDefinition::native("incendiary", P::EVERYONE, crate::btech::incendiary::command)
                .policy(SwitchPolicy::Reject("incendiary takes no switches."), false),
            CommandDefinition::native("precision", P::EVERYONE, crate::btech::precision::command)
                .policy(SwitchPolicy::Reject("precision takes no switches."), false),
            CommandDefinition::native("fireswarm", P::EVERYONE, crate::btech::swarm::command)
                .policy(SwitchPolicy::Reject("fireswarm takes no switches."), false),
            CommandDefinition::native("fireswarm1", P::EVERYONE, crate::btech::swarm::command)
                .policy(SwitchPolicy::Reject("fireswarm1 takes no switches."), false),
            CommandDefinition::native("sguided", P::EVERYONE, crate::btech::semiguided::command)
                .policy(SwitchPolicy::Reject("sguided takes no switches."), false),
            CommandDefinition::native("mml", P::EVERYONE, crate::btech::mml::command)
                .policy(SwitchPolicy::Reject("mml takes no switches."), false),
            CommandDefinition::native("atmrange", P::EVERYONE, crate::btech::atm::command)
                .policy(SwitchPolicy::Reject("atmrange takes no switches."), false),
            CommandDefinition::native("atmexplosive", P::EVERYONE, crate::btech::atm::command)
                .policy(
                    SwitchPolicy::Reject("atmexplosive takes no switches."),
                    false,
                ),
            CommandDefinition::native("stinger", P::EVERYONE, crate::btech::stinger::command)
                .policy(SwitchPolicy::Reject("stinger takes no switches."), false),
            CommandDefinition::native("rac", P::EVERYONE, crate::btech::rotary::command)
                .policy(SwitchPolicy::Reject("rac takes no switches."), false),
            CommandDefinition::native("rapidfire", P::EVERYONE, crate::btech::rapid::command)
                .policy(SwitchPolicy::Reject("rapidfire takes no switches."), false),
            CommandDefinition::native("ultra", P::EVERYONE, crate::btech::ultra::command)
                .policy(SwitchPolicy::Reject("ultra takes no switches."), false),
            CommandDefinition::native("ecm", P::EVERYONE, crate::btech::electronics::command)
                .policy(SwitchPolicy::Reject("ecm takes no switches."), false),
            CommandDefinition::native("eccm", P::EVERYONE, crate::btech::electronics::command)
                .policy(SwitchPolicy::Reject("eccm takes no switches."), false),
            CommandDefinition::native("angelecm", P::EVERYONE, crate::btech::electronics::command)
                .policy(SwitchPolicy::Reject("angelecm takes no switches."), false),
            CommandDefinition::native("angeleccm", P::EVERYONE, crate::btech::electronics::command)
                .policy(SwitchPolicy::Reject("angeleccm takes no switches."), false),
            CommandDefinition::native(
                "extinguish",
                P::EVERYONE,
                crate::btech::vehicle_burning::command,
            )
            .policy(SwitchPolicy::Reject("extinguish takes no switches."), false),
            CommandDefinition::native("pods", P::EVERYONE, crate::btech::pods::command)
                .policy(SwitchPolicy::Reject("pods takes no switches."), false),
            CommandDefinition::native(
                "removepods",
                P::EVERYONE,
                crate::btech::vehicle_pods::command,
            )
            .policy(SwitchPolicy::Reject("removepods takes no switches."), false),
            CommandDefinition::native("removepod", P::EVERYONE, crate::btech::pods::command)
                .policy(SwitchPolicy::Reject("removepod takes no switches."), false),
            CommandDefinition::native("inarc", P::EVERYONE, crate::btech::inarc::command)
                .policy(SwitchPolicy::Reject("inarc takes no switches."), false),
            CommandDefinition::native("narc", P::EVERYONE, crate::btech::narc::command)
                .policy(SwitchPolicy::Reject("narc takes no switches."), false),
            CommandDefinition::native("explosive", P::EVERYONE, crate::btech::narc::command)
                .policy(SwitchPolicy::Reject("explosive takes no switches."), false),
            CommandDefinition::native("ams", P::EVERYONE, crate::btech::ams::command)
                .policy(SwitchPolicy::Reject("ams takes no switches."), false),
            CommandDefinition::native("unjam", P::EVERYONE, crate::btech::unjam::command)
                .policy(SwitchPolicy::Reject("unjam takes no switches."), false),
            CommandDefinition::native(
                "artemis",
                P::EVERYONE,
                crate::btech::ammunition_mode::artemis_command,
            )
            .policy(SwitchPolicy::Reject("artemis takes no switches."), false),
            CommandDefinition::native(
                "cluster",
                P::EVERYONE,
                crate::btech::ammunition_mode::cluster_command,
            )
            .policy(SwitchPolicy::Reject("cluster takes no switches."), false),
            CommandDefinition::native(
                "firesmoke",
                P::EVERYONE,
                crate::btech::special_rounds::command,
            )
            .policy(SwitchPolicy::Reject("firesmoke takes no switches."), false),
            CommandDefinition::native(
                "firemine",
                P::EVERYONE,
                crate::btech::special_rounds::command,
            )
            .policy(SwitchPolicy::Reject("firemine takes no switches."), false),
            CommandDefinition::native(
                "firecluster",
                P::EVERYONE,
                crate::btech::ammunition_mode::cluster_command,
            )
            .policy(
                SwitchPolicy::Reject("firecluster takes no switches."),
                false,
            ),
            CommandDefinition::native("lbx", P::EVERYONE, crate::btech::ammunition_mode::command)
                .policy(SwitchPolicy::Reject("lbx takes no switches."), false),
            CommandDefinition::native("flamerheat", P::EVERYONE, crate::btech::fire_mode::command)
                .matching(&["heat"], None)
                .policy(SwitchPolicy::Reject("flamerheat takes no switches."), false),
            CommandDefinition::native(
                "fliparms",
                P::EVERYONE,
                crate::btech::commands::facing_command,
            )
            .policy(SwitchPolicy::Reject("fliparms takes no switches."), false),
            CommandDefinition::native(
                "heading",
                P::EVERYONE,
                crate::btech::commands::motion_command,
            ),
            CommandDefinition::native(
                "mechprefs",
                P::EVERYONE,
                crate::btech::commands::preferences_command,
            )
            .policy(SwitchPolicy::Reject("mechprefs takes no switches."), false),
            CommandDefinition::native(
                "turret",
                P::EVERYONE,
                crate::btech::commands::motion_command,
            )
            .policy(SwitchPolicy::Reject("turret takes no switches."), false),
            CommandDefinition::native(
                "fixturret",
                P::EVERYONE,
                crate::btech::commands::motion_command,
            )
            .policy(SwitchPolicy::Reject("fixturret takes no switches."), false),
            CommandDefinition::native("speed", P::EVERYONE, crate::btech::commands::motion_command),
            CommandDefinition::native(
                "startup",
                P::EVERYONE,
                crate::btech::commands::power_command,
            ),
            CommandDefinition::native(
                "shutdown",
                P::EVERYONE,
                crate::btech::commands::power_command,
            ),
            CommandDefinition::native("pilot", P::EVERYONE, crate::btech::commands::pilot_command)
                .policy(SwitchPolicy::Reject("pilot takes no switches."), false),
            CommandDefinition::native(
                "unpilot",
                P::EVERYONE,
                crate::btech::commands::pilot_command,
            )
            .policy(SwitchPolicy::Reject("unpilot takes no switches."), false),
            CommandDefinition::native("@btech", P::WIZARD, crate::btech::commands::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("+rolls", P::WIZARD, crate::btech::commands::rolls_command)
                .policy(
                    SwitchPolicy::Reject("Command +rolls does not take switches."),
                    true,
                ),
            CommandDefinition::native("@who", P::WIZARD, crate::operations::who_command)
                .requiring_session("@who is only available from an active connection.")
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("version", P::EVERYONE, crate::operations::version_command)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@log", P::WIZARD, crate::logging::command)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@admin", P::WIZARD, crate::config::administration::command)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("addcom", P::EVERYONE, crate::communication::addcom),
            CommandDefinition::native("delcom", P::EVERYONE, crate::communication::delcom),
            CommandDefinition::native("clearcom", P::EVERYONE, crate::communication::clearcom),
            CommandDefinition::native("comlist", P::EVERYONE, crate::communication::comlist),
            CommandDefinition::native("allcom", P::EVERYONE, crate::communication::allcom),
            CommandDefinition::native("page", P::EVERYONE, crate::communication::page),
            CommandDefinition::native("@chan", P::WIZARD, crate::communication::admin)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("goto", P::EVERYONE, native::goto),
            CommandDefinition::native("look", P::EVERYONE, super::objects::look::look)
                .matching(&["l"], None)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("say", P::EVERYONE, crate::communication::speech::say)
                .matching(&[], Some('"')),
            CommandDefinition::native("pose", P::EVERYONE, crate::communication::speech::pose)
                .matching(&[], Some(':'))
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native(";", P::EVERYONE, crate::communication::speech::pose)
                .matching(&[], Some(';')),
            CommandDefinition::native(
                "\\",
                P::EVERYONE,
                crate::communication::speech::emit_command,
            )
            .matching(&[], Some('\\')),
            CommandDefinition::native(
                "@emit",
                P::WIZARD,
                crate::communication::speech::emit_command,
            )
            .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@pemit", P::WIZARD, crate::communication::speech::pemit)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@npemit", P::WIZARD, crate::communication::speech::pemit)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@oemit", P::WIZARD, crate::communication::speech::oemit)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@fsay", P::WIZARD, crate::communication::speech::fsay)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@fpose", P::WIZARD, crate::communication::speech::fpose)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@femit", P::WIZARD, crate::communication::speech::femit)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@wall", P::WIZARD, crate::communication::speech::wall)
                .policy(SwitchPolicy::Handler, false),
            CommandDefinition::native("@session", P::WIZARD, native::sessions)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@telnet", P::WIZARD, native::telnet)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@shutdown", P::WIZARD, native::shutdown)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@dbck", P::WIZARD, native::dbck)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("color", P::EVERYONE, native::color)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("help", P::EVERYONE, native::help)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@pcreate", P::WIZARD, crate::account_admin::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@newpassword", P::WIZARD, crate::account_admin::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@boot", P::WIZARD, crate::account_admin::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@last", P::WIZARD, crate::account_admin::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@lua", P::WIZARD, native::lua_admin)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@help", P::WIZARD, native::help_admin)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("quit", P::EVERYONE, native::quit),
            CommandDefinition::native("home", P::WIZARD, native::home).policy(
                SwitchPolicy::Reject("Movement command switches are not supported."),
                false,
            ),
            CommandDefinition::native("@teleport", P::WIZARD, native::teleport).policy(
                SwitchPolicy::Reject("Movement command switches are not supported."),
                false,
            ),
            CommandDefinition::native("@flag", P::WIZARD, native::flag),
            CommandDefinition::native("@power", P::WIZARD, native::power),
            CommandDefinition::native("@readcache", P::WIZARD, super::discovery::readcache)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@enable", P::WIZARD, super::discovery::cleaning)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@disable", P::WIZARD, super::discovery::cleaning)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@list", P::WIZARD, super::discovery::list)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@search", P::WIZARD, super::discovery::search)
                .policy(SwitchPolicy::Reject("Unsupported @search switch."), true),
            CommandDefinition::native("@stats", P::WIZARD, super::discovery::stats)
                .policy(SwitchPolicy::Reject("Unsupported @stats switch."), true),
            CommandDefinition::native("@state", P::WIZARD, crate::state::commands::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@examine", P::WIZARD, super::inspection::examine)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@entrances", P::WIZARD, super::inspection::entrances)
                .policy(SwitchPolicy::Reject("Unsupported command switch."), true),
            CommandDefinition::native("@find", P::WIZARD, native::find)
                .policy(SwitchPolicy::Reject("Unsupported @find switch."), true),
        ];
        for (name, handler) in [
            ("@force", super::queue::force as NativeHandler),
            ("@wait", super::queue::wait),
            ("@halt", super::queue::halt),
        ] {
            let mut entry = CommandDefinition::native(name, P::WIZARD, handler);
            entry.private_errors = true;
            entry.no_macro = name != "@halt";
            if name == "@halt" {
                entry.switches = SwitchPolicy::Handler;
            }
            definitions.push(entry);
        }
        definitions.extend(crate::macros::commands::definitions());
        definitions.extend(super::objects::definitions());
        for entry in &mut definitions {
            entry.requires_session |= matches!(
                entry.name.as_str(),
                "color"
                    | "quit"
                    | "help"
                    | "@help"
                    | "@session"
                    | "@telnet"
                    | "@lua"
                    | "@pcreate"
                    | "@newpassword"
                    | "@boot"
                    | "@last"
            );
        }
        Self {
            definitions,
            lists: super::discovery::list_definitions(),
        }
    }
    /// Resolve ordered configuration edits against the completed module registry.
    pub fn configure_access(&mut self, config: &Config) -> Result<()> {
        for alias in &config.runtime_aliases {
            let input = CommandInput::parse(config, alias);
            ensure!(
                self.definitions.iter().any(|d| d.name == input.name
                    && input
                        .switch
                        .as_ref()
                        .is_none_or(|sw| d.switch_definitions.iter().any(|s| s.accepts(sw)))),
                "Runtime alias {alias} has an unknown command or switch target"
            );
        }
        let mut masks: Vec<_> = self
            .definitions
            .iter()
            .map(|d| d.declared_permission)
            .collect();
        let mut switches: Vec<_> = self
            .definitions
            .iter()
            .map(|d| {
                d.switch_definitions
                    .iter()
                    .map(|s| crate::access::switch_default(&d.name, s.name))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut lists = super::discovery::list_definitions();
        for rule in &config.access_rules {
            let result = (|| -> Result<()> {
                if rule.list {
                    let topic = lists
                        .iter_mut()
                        .find(|t| t.accepts(&rule.target))
                        .ok_or_else(|| anyhow::anyhow!("unknown list topic {:?}", rule.target))?;
                    topic.permission.edit(&rule.edits);
                    return Ok(());
                }
                let input = CommandInput::parse(config, &rule.target);
                let canonical = self
                    .definitions
                    .iter()
                    .find(|d| d.name == input.name)
                    .or_else(|| {
                        self.definitions.iter().find(|d| match &d.matcher {
                            CommandMatcher::Native { aliases, prefix } => {
                                aliases.contains(&input.name)
                                    || prefix.is_some_and(|p| input.name == p.to_string())
                            }
                            _ => false,
                        })
                    })
                    .map(|d| d.name.as_str())
                    .ok_or_else(|| anyhow::anyhow!("unknown command {:?}", rule.target))?;
                let mut applied = false;
                for (index, definition) in self
                    .definitions
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.name == canonical)
                {
                    if let Some(switch) = &input.switch {
                        if let Some((i, _)) = definition
                            .switch_definitions
                            .iter()
                            .enumerate()
                            .find(|(_, s)| s.accepts(switch))
                        {
                            switches[index][i].edit(&rule.edits);
                            applied = true;
                        }
                    } else {
                        masks[index].edit(&rule.edits);
                        applied = true;
                    }
                }
                ensure!(applied, "unknown native switch {:?}", rule.target);
                Ok(())
            })();
            result.with_context(|| rule.origin.clone())?;
        }
        for (i, d) in self.definitions.iter_mut().enumerate() {
            d.permission = masks[i];
            d.listed = !d.permission.contains(CommandPermissions::DARK);
            for (sw, mask) in d.switch_definitions.iter_mut().zip(&switches[i]) {
                sw.permission = *mask;
            }
        }
        self.lists = lists;
        Ok(())
    }
    /// Stable catalog including commands on currently unattached object modules.
    pub fn definitions(&self) -> impl Iterator<Item = &CommandDefinition> {
        self.definitions.iter()
    }
    /// Register a native handler without changing Lua module registrations.
    pub fn register_native(&mut self, definition: CommandDefinition) -> Result<()> {
        ensure!(
            matches!(definition.handler, CommandHandler::Native(_)),
            "Expected native handler"
        );
        ensure!(
            !self.definitions.iter().any(|d| d.scope == definition.scope
                && d.name == definition.name
                && matches!(d.handler, CommandHandler::Native(_))),
            "Duplicate native command"
        );
        self.definitions.push(definition);
        Ok(())
    }
    /// Match global native names before shorthand prefixes.
    pub fn native_match(&self, input: CommandInput) -> Option<(&CommandDefinition, CommandInput)> {
        self.native_match_scope(input, &CommandScope::Global)
    }
    /// Match a native command in one explicit scope.
    pub fn native_match_scope(
        &self,
        mut input: CommandInput,
        scope: &CommandScope,
    ) -> Option<(&CommandDefinition, CommandInput)> {
        for entry in self.definitions.iter().filter(|d| &d.scope == scope) {
            if let CommandMatcher::Native { aliases, prefix } = &entry.matcher
                && (entry.name == input.name || aliases.contains(&input.name))
            {
                if let Some(prefix) = prefix
                    && let Some(args) = input.line.strip_prefix(*prefix)
                {
                    input.args = args.into();
                }
                return Some((entry, input));
            }
        }
        for entry in self.definitions.iter().filter(|d| &d.scope == scope) {
            if let CommandMatcher::Native {
                prefix: Some(prefix),
                ..
            } = &entry.matcher
                && let Some(args) = input.line.strip_prefix(*prefix)
            {
                input.args = args.into();
                input.name = entry.name.clone();
                input.switch = None;
                return Some((entry, input));
            }
        }
        None
    }
    /// Validate and capture declarations; caller publishes the registry only after load succeeds.
    pub fn register_lua(
        &mut self,
        lua: &Lua,
        module: &Table,
        source: &str,
        scope: CommandScope,
    ) -> Result<()> {
        let Some(commands) = checked(module.get::<Option<Table>>("commands"))
            .with_context(|| format!("{source}: commands"))?
        else {
            return Ok(());
        };
        let count = checked(
            commands
                .clone()
                .pairs::<Value, Value>()
                .collect::<mlua::Result<Vec<_>>>(),
        )?
        .len();
        for key in commands.clone().pairs::<Value, Value>() {
            let (key, _) = checked(key)?;
            ensure!(
                matches!(key,Value::Integer(n) if n>=1 && n as usize<=count),
                "{source}: commands must be a contiguous array"
            );
        }
        for index in 1..=count {
            let definition = (|| -> Result<CommandDefinition> {
                let command = checked(commands.raw_get::<Table>(index))?;
                let name = match checked(command.get::<Option<Value>>("name"))? {
                    None | Some(Value::Nil) => String::new(),
                    Some(Value::String(value)) => checked(value.to_str())?.to_string(),
                    Some(_) => anyhow::bail!("name must be a string"),
                };
                ensure!(
                    name.is_empty()
                        || !name
                            .chars()
                            .any(|c| c.is_whitespace() || c.is_control() || c == '/'),
                    "invalid command name {name:?}"
                );
                let permission = match checked(command.get::<Option<Value>>("permission"))? {
                    None | Some(Value::Nil) => CommandPermissions::EVERYONE,
                    Some(Value::String(value)) => {
                        CommandPermissions::parse(checked(value.to_str())?.as_ref())?
                    }
                    Some(_) => anyhow::bail!("permission must be a string"),
                };
                let lua_access = crate::lua::command_access::read(&command).map_err(|_| {
                    anyhow::anyhow!(
                        "command access in {source} must be a mux.world.access constant"
                    )
                })?;
                let pattern = string_field(&command, "pattern")?;
                validate_pattern(&pattern)?;
                let find: Function =
                    checked(checked(lua.globals().get::<Table>("string"))?.get("find"))?;
                let handler: Function = checked(command.get("handler"))?;
                let factory = checked(
                    lua.load(
                        r#"
                    return function(find, pattern, handler, unpack)
                        return function(ctx, line)
                            local hits = {find(line, pattern)}
                            if not hits[1] then return false, false end
                            return handler(ctx, unpack(hits, 3)) == true, true
                        end
                    end
                "#,
                    )
                    .eval::<Function>(),
                )?;
                let unpack: Function = checked(lua.globals().get("unpack"))?;
                let invoke: Function =
                    checked(factory.call((find, pattern.clone(), handler, unpack)))?;
                Ok(CommandDefinition {
                    name: name.to_ascii_lowercase(),
                    switch_definitions: Vec::new(),
                    listed: true,
                    permission,
                    lua_access,
                    declared_permission: permission,
                    matcher: CommandMatcher::LuaPattern(pattern),
                    scope: scope.clone(),
                    source: source.into(),
                    declaration: Some(index),
                    handler: CommandHandler::Lua(invoke),
                    switches: SwitchPolicy::Handler,
                    private_errors: false,
                    direct_input_only: false,
                    requires_session: false,
                    session_error: "This command requires an interactive session.",
                    no_macro: false,
                })
            })()
            .with_context(|| format!("{source}: command {index}"))?;
            self.definitions.push(definition);
        }
        Ok(())
    }
}
/// Validate all Lua pattern syntax, including suffixes unreachable on a sample input.
fn validate_pattern(pattern: &str) -> Result<()> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut captures = Vec::new();
    let mut open = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            b'[' => {
                i = class_end(bytes, i)?;
            }
            b'%' => {
                i += 1;
                ensure!(i < bytes.len(), "malformed pattern: trailing %");
                match bytes[i] {
                    b'b' => {
                        ensure!(
                            i + 2 < bytes.len(),
                            "malformed pattern: %b requires two delimiters"
                        );
                        i += 2;
                    }
                    b'f' => {
                        ensure!(
                            bytes.get(i + 1) == Some(&b'['),
                            "malformed pattern: %f requires a class"
                        );
                        i = class_end(bytes, i + 1)?;
                    }
                    b'0'..=b'9' => {
                        let n = (bytes[i] - b'0') as usize;
                        ensure!(
                            n > 0 && captures.get(n - 1) == Some(&true),
                            "invalid capture reference"
                        );
                    }
                    _ => {}
                }
            }
            b'(' => {
                ensure!(captures.len() < 32, "too many captures");
                let index = captures.len();
                captures.push(false);
                open.push(index);
            }
            b')' => {
                let index = open.pop().context("invalid pattern capture")?;
                captures[index] = true;
            }
            _ => {}
        }
        i += 1;
    }
    ensure!(open.is_empty(), "unfinished pattern capture");
    Ok(())
}
/// Locate a Lua bracket class terminator, honoring escapes and a literal leading ].
fn class_end(bytes: &[u8], mut i: usize) -> Result<usize> {
    i += 1;
    if bytes.get(i) == Some(&b'^') {
        i += 1;
    }
    loop {
        ensure!(i < bytes.len(), "malformed pattern: missing ]");
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
        }
        i += 1;
        if bytes.get(i) == Some(&b']') {
            return Ok(i);
        }
    }
}

/// Convert thread-local Lua errors without enabling cross-thread Lua values.
fn checked<T>(result: mlua::Result<T>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!(error.to_string()))
}
/// Require actual strings rather than Lua's automatic number-to-string coercion.
fn string_field(table: &Table, field: &str) -> Result<String> {
    match checked(table.get::<Value>(field))? {
        Value::String(value) => Ok(checked(value.to_str())?.to_string()),
        _ => anyhow::bail!("{field} must be a string"),
    }
}
