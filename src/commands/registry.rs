//! Native and Lua command definitions, immutable registration and access metadata.
use super::{Action, CommandContext, native};
use crate::{
    config::Config,
    flags,
    world::{ObjectId, World},
};
use anyhow::{Context, Result, ensure};
use mlua::{Function, Lua, Table, Value};

/// Minimal required-role bits. Combined restrictions require GOD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandPermissions(u8);
impl CommandPermissions {
    /// Available to every authenticated player.
    pub const EVERYONE: Self = Self(0);
    /// Requires Wizard status; GOD also qualifies.
    pub const WIZARD: Self = Self(1);
    /// Requires dbref #1.
    pub const GOD: Self = Self(2);
    /// Evaluate current world authority, never cached session roles.
    pub fn allows(self, world: &World, player: ObjectId) -> bool {
        if self.0 & Self::GOD.0 != 0 {
            player == ObjectId(1)
        } else {
            self.0 & Self::WIZARD.0 == 0 || flags::is_wizard(world, player)
        }
    }
    /// Human-readable role for command catalogs.
    pub fn name(self) -> &'static str {
        if self.0 & Self::GOD.0 != 0 {
            "god"
        } else if self.0 & Self::WIZARD.0 != 0 {
            "wizard"
        } else {
            "everyone"
        }
    }
    /// Decode an explicit Lua declaration permission.
    fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "everyone" => Self::EVERYONE,
            "wizard" => Self::WIZARD,
            "god" => Self::GOD,
            _ => anyhow::bail!("invalid permission {name:?}"),
        })
    }
}
impl std::ops::BitOr for CommandPermissions {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
/// Where a definition is eligible for dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandScope {
    /// Native exact commands always precede Lua matching.
    Native,
    /// Object-local commands attached via the named Lua parent.
    Object(String),
    /// Global modules in lexical load order.
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
/// Complete enumerable definition for a future command-listing interface.
pub struct CommandDefinition {
    /// Lowercase canonical command name.
    pub name: String,
    /// Required authority.
    pub permission: CommandPermissions,
    /// Exact or pattern-based matching metadata.
    pub matcher: CommandMatcher,
    /// Native or scoped Lua eligibility.
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
}
impl CommandDefinition {
    /// Describe a native handler with default exact matching and no switches.
    pub fn native(name: &str, permission: CommandPermissions, handler: NativeHandler) -> Self {
        Self {
            name: name.into(),
            permission,
            matcher: CommandMatcher::Native {
                aliases: Vec::new(),
                prefix: None,
            },
            scope: CommandScope::Native,
            source: "native".into(),
            declaration: None,
            handler: CommandHandler::Native(handler),
            switches: SwitchPolicy::Reject("Unsupported command switch."),
            private_errors: false,
        }
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
        let error = if !self
            .permission
            .allows(&ctx.scripts.world.borrow(), ctx.player)
        {
            Some("Permission denied.")
        } else if input.switch.is_some() {
            match self.switches {
                SwitchPolicy::Reject(message) => Some(message),
                SwitchPolicy::Handler => None,
            }
        } else {
            None
        };
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
        let definitions = vec![
            CommandDefinition::native("addcom", P::EVERYONE, crate::communication::addcom),
            CommandDefinition::native("delcom", P::EVERYONE, crate::communication::delcom),
            CommandDefinition::native("clearcom", P::EVERYONE, crate::communication::clearcom),
            CommandDefinition::native("comlist", P::EVERYONE, crate::communication::comlist),
            CommandDefinition::native("allcom", P::EVERYONE, crate::communication::allcom),
            CommandDefinition::native("page", P::EVERYONE, crate::communication::page),
            CommandDefinition::native("@chan", P::WIZARD, crate::communication::admin)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("look", P::EVERYONE, native::look).matching(&["l"], None),
            CommandDefinition::native("say", P::EVERYONE, native::say).matching(&[], Some('"')),
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
            CommandDefinition::native("@list", P::WIZARD, native::list),
            CommandDefinition::native("@state", P::WIZARD, crate::state::commands::command)
                .policy(SwitchPolicy::Handler, true),
            CommandDefinition::native("@examine", P::WIZARD, native::examine),
            CommandDefinition::native("@find", P::WIZARD, native::find)
                .policy(SwitchPolicy::Handler, true),
        ];
        Self { definitions }
    }
    /// Stable catalog including commands on currently unattached object modules.
    pub fn definitions(&self) -> impl Iterator<Item = &CommandDefinition> {
        self.definitions.iter()
    }
    /// Match native names before shorthand prefixes.
    pub fn native_match(
        &self,
        mut input: CommandInput,
    ) -> Option<(&CommandDefinition, CommandInput)> {
        for entry in &self.definitions {
            if let CommandMatcher::Native { aliases, .. } = &entry.matcher
                && (entry.name == input.name || aliases.contains(&input.name))
            {
                return Some((entry, input));
            }
        }
        for entry in &self.definitions {
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
                            if not hits[1] then return false end
                            return handler(ctx, unpack(hits, 3)) == true
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
                    permission,
                    matcher: CommandMatcher::LuaPattern(pattern),
                    scope: scope.clone(),
                    source: source.into(),
                    declaration: Some(index),
                    handler: CommandHandler::Lua(invoke),
                    switches: SwitchPolicy::Handler,
                    private_errors: false,
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
