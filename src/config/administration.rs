//! Validated runtime directive candidates; publication belongs to the serial world owner.
use super::{
    Config, catalog,
    directives::{DIRECTIVES, Directive},
};
use crate::{
    access::{Edit, Permissions, Rule},
    commands::{Action, CommandContext, CommandInput, CommandRegistry},
    world::{Kind, ObjectId, World},
};
use anyhow::{Context, Result, bail, ensure};

/// Runtime capability is independent of editable access bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Live,
    Restart,
    Deferred,
}
impl Support {
    /// Stable diagnostic label used in directive discovery.
    pub fn name(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Restart => "restart-only",
            Self::Deferred => "unsupported",
        }
    }
}
/// Implemented consumers, never silently accepted storage-only edits.
pub fn support(d: &Directive) -> Support {
    if d.name == "log_options"
        || crate::logging::CATEGORIES
            .iter()
            .any(|(_, name, _)| *name == d.name)
    {
        return Support::Live;
    }
    if d.permission.contains(Permissions::DISABLED) || d.name == "check_offset" {
        return Support::Restart;
    }
    if matches!(
        d.name,
        "access"
            | "alias"
            | "bad_name"
            | "good_name"
            | "flag_alias"
            | "config_access"
            | "list_access"
            | "forbid_site"
            | "permit_site"
            | "suspect_site"
            | "trust_site"
            | "check_interval"
            | "command_quota_increment"
            | "command_quota_max"
            | "command_quota_interval"
            | "conn_timeout"
            | "default_home"
            | "down_message"
            | "full_message"
            | "default_exit_flags"
            | "default_player_flags"
            | "default_room_flags"
            | "default_thing_flags"
            | "help_directory"
            | "idle_interval"
            | "idle_timeout"
            | "lua_directory"
            | "lua_memory_limit"
            | "lua_error_reporting"
            | "lua_state_value_limit"
            | "lua_state_entry_limit"
            | "lua_state_object_limit"
            | "default_thing_lua_parent"
            | "default_room_lua_parent"
            | "default_exit_lua_parent"
            | "default_player_lua_parent"
            | "max_players"
            | "mud_name"
            | "notify_recursion_limit"
            | "password_hash_memlimit"
            | "password_hash_opslimit"
            | "player_name_length_limit"
            | "player_password_length_limit"
            | "player_name_spaces"
            | "command_queue_limit"
            | "player_starting_home"
            | "player_starting_room"
            | "command_queue_active_chunk"
            | "command_queue_idle_chunk"
            | "login_attempt_burst"
            | "login_attempt_refill"
            | "login_hash_limit"
            | "space_compress"
            | "allow_chanlurking"
    ) {
        Support::Live
    } else {
        Support::Deferred
    }
}
/// Exact C directive identity; command spelling itself remains case insensitive.
fn directive(name: &str) -> Result<&'static Directive> {
    if catalog::KEYS
        .iter()
        .any(|k| k.legacy.is_empty() && k.path == name)
    {
        bail!("{name} is restart-only.");
    }
    DIRECTIVES
        .iter()
        .find(|d| d.name == name)
        .ok_or_else(|| anyhow::anyhow!("Set: Config directive {name} not found."))
}
/// Server action, shared by interactive and queued dispatch.
#[derive(Debug)]
pub struct Request {
    pub directive: String,
    pub value: String,
}
/// Parse only syntax here; the owner checks live directive access before preparing changes.
pub fn command(_: &CommandContext<'_>, input: &CommandInput) -> Result<Action> {
    let Some((name, value)) = input.args.split_once('=') else {
        return Ok(Action::Reply("Usage: @admin <directive>=<value>".into()));
    };
    Ok(Action::ConfigAdmin(Request {
        directive: name.trim().into(),
        value: value.trim().into(),
    }))
}
/// Unpublished configuration and C partial-success diagnostics.
pub struct Candidate {
    pub config: Config,
    pub diagnostics: Vec<String>,
}
impl Config {
    /// Compile file access declarations before any startup side effects.
    pub(super) fn compile_directive_permissions(&mut self) -> Result<()> {
        self.directive_permissions = DIRECTIVES
            .iter()
            .map(|d| (d.name.into(), d.permission))
            .collect();
        for (name, tokens) in &self.settings.access.config {
            directive(name).with_context(|| {
                format!(
                    "{}: access.config.{name}",
                    self.origins
                        .get(&format!("access.config.{name}"))
                        .unwrap_or(&self.root)
                        .display()
                )
            })?;
            let edits = tokens
                .0
                .iter()
                .map(|t| Edit::parse(t))
                .collect::<Result<Vec<_>>>()
                .with_context(|| {
                    format!(
                        "{}: access.config.{name}",
                        self.origins
                            .get(&format!("access.config.{name}"))
                            .unwrap_or(&self.root)
                            .display()
                    )
                })?;
            self.directive_permissions
                .get_mut(name)
                .unwrap()
                .edit(&edits);
        }
        Ok(())
    }
    /// Keep serde and Lua effective lookup synchronized after supported edits.
    fn refresh(&mut self) -> Result<()> {
        self.effective = toml::Value::try_from(&self.settings)?;
        self.validate_values(false)?;
        self.validate_for_serve()
    }
    /// Parse a directive into a candidate; no active configuration or world is mutated.
    pub fn administer(
        &self,
        request: &Request,
        world: &World,
        player: ObjectId,
        registry: &CommandRegistry,
    ) -> Result<Candidate> {
        let d = directive(&request.directive)?;
        ensure!(
            self.directive_permissions[d.name].allows(world, player),
            "Permission denied."
        );
        ensure!(
            support(d) == Support::Live,
            "{} is {}.",
            d.name,
            support(d).name()
        );
        let mut c = self.clone();
        let mut diagnostics = Vec::new();
        let value = request.value.as_str();
        match d.name {
            "log_options" => {
                let mut success = false;
                for word in value.split_whitespace() {
                    let (set, name) = word.strip_prefix('!').map_or((true, word), |s| (false, s));
                    let name = name.to_ascii_lowercase();
                    let option = [
                        ("flags", super::LogOption::Flags),
                        ("location", super::LogOption::Location),
                        ("timestamp", super::LogOption::Timestamp),
                    ]
                    .into_iter()
                    .find(|(n, _)| !name.is_empty() && n.starts_with(&name));
                    if let Some((_, option)) = option {
                        success = true;
                        c.settings.logging.log_options.retain(|o| o != &option);
                        if set {
                            c.settings.logging.log_options.push(option);
                        }
                    } else {
                        diagnostics.push(format!("Unknown logging option {word}"));
                    }
                }
                ensure!(
                    success,
                    "{}",
                    if diagnostics.is_empty() {
                        "Nothing to set".into()
                    } else {
                        diagnostics.join("\n")
                    }
                );
            }
            "access" | "list_access" | "config_access" => {
                let (target, words) = value
                    .split_once(char::is_whitespace)
                    .context("Missing access target or permissions.")?;
                let mut edits = Vec::new();
                let mut accepted = Vec::new();
                for token in words.split_whitespace() {
                    match Edit::parse(token) {
                        Ok(edit) => {
                            edits.push(edit);
                            accepted.push(token.to_string());
                        }
                        Err(e) => diagnostics.push(e.to_string()),
                    }
                }
                ensure!(
                    !edits.is_empty(),
                    "{}",
                    if diagnostics.is_empty() {
                        "Nothing to set".into()
                    } else {
                        diagnostics.join("\n")
                    }
                );
                if d.name == "config_access" {
                    directive(target)?;
                    c.directive_permissions
                        .get_mut(target)
                        .unwrap()
                        .edit(&edits);
                    c.settings
                        .access
                        .config
                        .entry(target.into())
                        .or_default()
                        .0
                        .extend(accepted);
                } else {
                    let table = if d.name == "list_access" {
                        &mut c.settings.access.lists
                    } else {
                        &mut c.settings.access.commands
                    };
                    table
                        .entry(target.to_ascii_lowercase())
                        .or_default()
                        .0
                        .extend(accepted);
                    c.access_rules.push(Rule {
                        target: target.to_ascii_lowercase(),
                        list: d.name == "list_access",
                        edits,
                        origin: format!("@admin {}", d.name),
                    });
                }
            }
            "alias" | "flag_alias" => {
                let parts = value
                    .split(|ch: char| ch.is_whitespace() || ch == '=' || ch == ',')
                    .filter(|v| !v.is_empty())
                    .collect::<Vec<_>>();
                ensure!(parts.len() == 2, "Expected alias and target.");
                let alias = parts[0].to_ascii_lowercase();
                let target = parts[1].to_ascii_lowercase();
                ensure!(!alias.is_empty(), "Invalid alias.");
                if d.name == "flag_alias" {
                    let flag = crate::flags::Flag::resolve(&target, &c.aliases.flags)?;
                    ensure!(
                        crate::flags::Flag::parse(&alias).is_err()
                            && !c
                                .aliases
                                .flags
                                .keys()
                                .any(|k| k.eq_ignore_ascii_case(&alias)),
                        "Invalid or conflicting flag alias: {alias}"
                    );
                    c.settings.aliases.flags.insert(alias, flag.world_name());
                } else {
                    ensure!(
                        !c.aliases
                            .commands
                            .keys()
                            .any(|k| k.eq_ignore_ascii_case(&alias))
                            && !registry.definitions().any(|d| matches_command(d, &alias)),
                        "Unable to add command alias: {alias}"
                    );
                    let input = CommandInput::parse(&c, &target);
                    let definition = registry
                        .definitions()
                        .find(|d| matches_command(d, &input.name))
                        .context("Command not found.")?;
                    if let Some(switch) = &input.switch {
                        ensure!(
                            definition
                                .switch_definitions
                                .iter()
                                .any(|s| s.accepts(switch) && s.permission.allows(world, player)),
                            "Switch not found."
                        );
                    }
                    let canonical = format!(
                        "{}{}",
                        definition.name,
                        input.switch.map_or(String::new(), |s| format!("/{s}"))
                    );
                    c.runtime_aliases.push(alias.clone());
                    c.settings.aliases.commands.insert(alias, canonical);
                }
            }
            "bad_name" => {
                if !c.names.bad.iter().any(|p| p.eq_ignore_ascii_case(value)) {
                    c.settings.names.bad.push(value.into());
                }
            }
            "good_name" => {
                c.settings
                    .names
                    .bad
                    .retain(|p| !p.eq_ignore_ascii_case(value));
            }
            "forbid_site" | "permit_site" | "suspect_site" | "trust_site" => {
                let fields = value
                    .split(|ch: char| ch.is_whitespace() || ch == '=' || ch == ',')
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                ensure!(fields.len() == 2, "Missing host address or mask.");
                let rule = crate::sites::Rule {
                    address: fields[0].parse().context("Bad host address")?,
                    mask: fields[1].parse().context("Bad mask")?,
                    marked: matches!(d.name, "forbid_site" | "suspect_site"),
                    origin: format!("@admin {}", d.name),
                };
                ensure!(
                    rule.address != std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST),
                    "Bad host address."
                );
                let site = super::SiteRule {
                    address: rule.address,
                    mask: rule.mask,
                };
                match d.name {
                    "forbid_site" => c.settings.sites.forbid.insert(0, site),
                    "permit_site" => c.settings.sites.permit.insert(0, site),
                    "suspect_site" => c.settings.sites.suspect.insert(0, site),
                    _ => c.settings.sites.trust.insert(0, site),
                }
                if matches!(d.name, "forbid_site" | "permit_site") {
                    c.site_policy.access.insert(0, rule);
                } else {
                    c.site_policy.suspicion.insert(0, rule);
                }
            }
            _ => {
                let spec = catalog::KEYS
                    .iter()
                    .find(|k| k.legacy == d.name)
                    .context("Directive mapping missing")?;
                let old = c
                    .effective_value(spec.path)
                    .context("Directive value missing")?;
                let parsed = if d.parser == "cf_set_flags" {
                    let mut flags = Vec::new();
                    for word in value.split_whitespace() {
                        match crate::flags::Flag::resolve(word, &c.aliases.flags) {
                            Ok(f) => {
                                flags.push(toml::Value::String(f.world_name().to_ascii_lowercase()))
                            }
                            Err(e) => diagnostics.push(e.to_string()),
                        }
                    }
                    ensure!(
                        !flags.is_empty(),
                        "{}",
                        if diagnostics.is_empty() {
                            "Nothing to set".into()
                        } else {
                            diagnostics.join("\n")
                        }
                    );
                    toml::Value::Array(flags)
                } else {
                    match old {
                        toml::Value::Integer(_) => {
                            toml::Value::Integer(value.parse().context("Expected integer")?)
                        }
                        toml::Value::Float(_) => {
                            toml::Value::Float(value.parse().context("Expected number")?)
                        }
                        toml::Value::Boolean(_) => {
                            toml::Value::Boolean(match value.to_ascii_lowercase().as_str() {
                                v if ["yes", "true", "1"]
                                    .iter()
                                    .any(|s| !v.is_empty() && s.starts_with(v)) =>
                                {
                                    true
                                }
                                v if ["no", "false", "0"]
                                    .iter()
                                    .any(|s| !v.is_empty() && s.starts_with(v)) =>
                                {
                                    false
                                }
                                _ => bail!("Expected boolean"),
                            })
                        }
                        toml::Value::String(_) => {
                            toml::Value::String(string_value(d, value, &mut diagnostics))
                        }
                        _ => bail!("Unsupported directive shape"),
                    }
                };
                let mut effective = c.effective.clone();
                let mut slot = &mut effective;
                for part in spec.path.split('.') {
                    slot = slot.get_mut(part).context("Missing configuration path")?;
                }
                *slot = parsed;
                c.settings = effective.try_into().context("Invalid directive value")?;
                if matches!(
                    d.name,
                    "default_home" | "player_starting_home" | "player_starting_room"
                ) {
                    let id = ObjectId(value.parse()?);
                    let o = world
                        .objects
                        .get(&id)
                        .filter(|o| {
                            o.kind != Kind::Garbage && !o.flags.contains(crate::flags::Flag::Going)
                        })
                        .context("Invalid configured object")?;
                    ensure!(
                        if d.name != "default_home" {
                            o.kind == Kind::Room
                        } else {
                            matches!(o.kind, Kind::Room | Kind::Thing | Kind::Player)
                        },
                        "Invalid configured destination type"
                    );
                }
            }
        }
        c.refresh()?;
        let mut retained = std::collections::BTreeMap::new();
        for object in world.objects.values().filter(|o| o.kind != Kind::Garbage) {
            if crate::state::validate(&object.state, &c).is_err() {
                ensure!(
                    self.retains_state(object)
                        || crate::state::validate(&object.state, self).is_ok(),
                    "Existing state for #{} is invalid",
                    object.id.0
                );
                retained.insert(object.id, (object.generation, object.state.clone()));
            }
        }
        c.retained_state = std::sync::Arc::new(retained);
        Ok(Candidate {
            config: c,
            diagnostics,
        })
    }
}

/// Configuration reports describe effective behavior without implying deferred C services exist.
pub fn report(ctx: &CommandContext<'_>, topic: &str) -> Option<String> {
    let c = ctx.config;
    let text = match topic {
        "config_permissions" => DIRECTIVES
            .iter()
            .filter(|d| {
                ctx.player == ObjectId(1)
                    || c.directive_permissions[d.name]
                        .allows(&ctx.scripts.world.borrow(), ctx.player)
            })
            .map(|d| {
                format!(
                    "{}: {} ({})",
                    d.name,
                    c.directive_permissions[d.name].name(),
                    support(d).name()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "default_flags" => [
            ("Players", &c.mux.default_player_flags),
            ("Rooms", &c.mux.default_room_flags),
            ("Things", &c.mux.default_thing_flags),
            ("Exits", &c.mux.default_exit_flags),
        ]
        .into_iter()
        .map(|(name, flags)| {
            format!(
                "{name}: {}",
                flags
                    .iter()
                    .map(|f| f.world_name())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n"),
        "bad_names" => format!("Disallowed names: {}", c.names.bad.join(" ")),
        "options" => {
            let mut rows = vec![
                format!(
                    "Player names {} contain spaces.",
                    if c.mux.player_name_spaces {
                        "may"
                    } else {
                        "may not"
                    }
                ),
                "Player names and aliases use printable ASCII; other text uses UTF-8.".into(),
                format!(
                    "Players may have at most {} commands in the queue at one time.",
                    c.mux.command_queue_limit
                ),
            ];
            if crate::flags::is_wizard(&ctx.scripts.world.borrow(), ctx.player) {
                rows.extend([
                    format!(
                        "Queue chunks: Active...{} Idle...{}",
                        c.mux.command_queue_active_chunk, c.mux.command_queue_idle_chunk
                    ),
                    format!(
                        "Maximum authenticated sessions: {}",
                        if c.mux.max_players < 0 {
                            "unlimited".into()
                        } else {
                            c.mux.max_players.to_string()
                        }
                    ),
                    format!(
                        "Intervals: Clean...{} Idlecheck...{}",
                        c.mux.check_interval, c.mux.idle_interval
                    ),
                    format!(
                        "Timeouts: Idle...{} Connect...{}",
                        c.mux.idle_timeout, c.mux.conn_timeout
                    ),
                    format!(
                        "Scheduling: Timeslice...{} Max_Quota...{} Increment...{}",
                        c.mux.command_quota_interval,
                        c.mux.command_quota_max,
                        c.mux.command_quota_increment
                    ),
                    format!(
                        "Spaces...{}",
                        if c.mux.space_compress {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                ]);
            }
            rows.join("\n")
        }
        _ => return None,
    };
    Some(text)
}

/// Include native built-ins in C alias collision and source lookup behavior.
fn matches_command(d: &crate::commands::CommandDefinition, name: &str) -> bool {
    d.name == name
        || matches!(&d.matcher, crate::commands::CommandMatcher::Native { aliases, prefix } if aliases.iter().any(|a| a == name) || prefix.is_some_and(|p| p.to_string() == name))
}

impl Config {
    /// Exempt only unchanged state on the same object incarnation from reduced live quotas.
    pub(crate) fn retains_state(&self, object: &crate::world::Object) -> bool {
        self.retained_state
            .get(&object.id)
            .is_some_and(|(generation, state)| {
                *generation == object.generation && state == &object.state
            })
    }
}

/// C string capacities exclude their terminating NUL; preserve valid UTF-8 at truncation.
fn string_value(d: &Directive, value: &str, diagnostics: &mut Vec<String>) -> String {
    const NAME_BYTES: usize = 31;
    const DIRECTORY_BYTES: usize = 127;
    const MESSAGE_BYTES: usize = 4095;
    let limit = match d.name {
        "mud_name" => NAME_BYTES,
        "down_message" | "full_message" => MESSAGE_BYTES,
        "help_directory"
        | "lua_directory"
        | "default_player_lua_parent"
        | "default_room_lua_parent"
        | "default_thing_lua_parent"
        | "default_exit_lua_parent" => DIRECTORY_BYTES,
        _ => return value.into(),
    };
    if value.len() <= limit {
        return value.into();
    }
    diagnostics.push("String truncated".into());
    value[..value.floor_char_boundary(limit)].into()
}
