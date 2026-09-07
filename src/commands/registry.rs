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
/// Complete enumerable command definition used by dispatch and discovery.
pub struct CommandDefinition {
    /// Lowercase canonical command name.
    pub name: String,
    /// Native switches available for discovery; handlers validate combinations.
    pub switch_definitions: Vec<super::discovery::SwitchDefinition>,
    /// Whether this entry appears in command listings.
    pub listed: bool,
    /// Required authority.
    pub permission: CommandPermissions,
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
    /// C command cannot be reached through macro expansion.
    pub no_macro: bool,
}
impl CommandDefinition {
    /// Describe a native handler with default exact matching and no switches.
    pub fn native(name: &str, permission: CommandPermissions, handler: NativeHandler) -> Self {
        let permission = crate::access::native_defaults(name, permission);
        Self {
            declared_permission: permission,
            name: name.into(),
            switch_definitions: super::discovery::switches(name),
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
            no_macro: false,
        }
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
                    Some("This command requires an interactive session.")
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
                return Ok(Action::Reply(error.into()));
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
        let base_alias = config.aliases.commands.get(base);
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
            CommandDefinition::native("addcom", P::EVERYONE, crate::communication::addcom),
            CommandDefinition::native("delcom", P::EVERYONE, crate::communication::delcom),
            CommandDefinition::native("clearcom", P::EVERYONE, crate::communication::clearcom),
            CommandDefinition::native("comlist", P::EVERYONE, crate::communication::comlist),
            CommandDefinition::native("allcom", P::EVERYONE, crate::communication::allcom),
            CommandDefinition::native("page", P::EVERYONE, crate::communication::page),
            CommandDefinition::native("@chan", P::WIZARD, crate::communication::admin)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("look", P::EVERYONE, super::objects::look::look)
                .matching(&["l"], None)
                .policy(SwitchPolicy::Handler, true),
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
            entry.requires_session = matches!(
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
                let name = string_field(&command, "name")?;
                ensure!(
                    !name.is_empty()
                        && !name
                            .chars()
                            .any(|c| c.is_whitespace() || c.is_control() || c == '/'),
                    "invalid command name {name:?}"
                );
                let permission = CommandPermissions::parse(&string_field(&command, "permission")?)?;
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
