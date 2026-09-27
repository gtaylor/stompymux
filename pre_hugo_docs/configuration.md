# Configuration reference

`Config::load` reads `stompymux.toml` from the game directory. The typed model in
`src/config/model.rs` is the authority for effective defaults. The compatibility
inventory in `tests/fixtures/config/legacy-catalog.json` records all 182 legacy
mappings, their directive names, registry types and compiled defaults. The
annotated game configuration intentionally overrides some defaults.

## Loading and validation

Includes are recursive and resolved relative to the file containing `include`.
Includes are applied in declaration order, followed by the including file. The
merge follows C's tomlc17 behavior: tables merge recursively, scalars replace,
and arrays concatenate when both contain only tables. Other arrays replace.
An empty array therefore does not clear an inherited array of tables. Repeating
an include repeats its table-array contributions. Bootstrap object maps merge
recursively too; partial object fields may be supplied by separate files.
Defaults and shape validation apply after merging. Overridden malformed values
are not validated, but malformed TOML and include declarations always fail.

Table keys retain their first position in the merged document, with new keys
appended. This order determines site-rule precedence. Includes may descend eight
edges from the root file; deeper nesting and cycles are errors. Diagnostics retain
source files for merged values and concatenated rule entries.

Unknown keys warn and are skipped. Malformed known values report the source file
and configuration path. Types, numeric bounds, finite floats, RGB components,
site address/mask families and bootstrap structure are validated. Gameplay
semantics that require deferred systems are not evaluated. Integer-valued
BattleTech switches remain integers. Permissions accept either a whitespace
separated string or an array of strings. Site arrays retain order; aliases,
access tables, colors and OSC presets retain dynamic names.

BattleTech and remaining legacy command-system
options are parsed and retained without enabling those features. A consolidated
capability diagnostic reports this. IPv4 site, command/list and configuration-directive access policies are enforced.
`@admin` supports runtime edits; configuration-file rereading and writing are not provided.

Rust callers use typed fields. Lua `mux.config` lookups accept dotted TOML
names and legacy directive names, and see the same defaulted effective values,
including listener CLI overrides.

## Listener, storage and content

`--listen-address` and `--port` are optional overrides: CLI wins over TOML, which
wins over defaults. `server.listen_address` defaults to `127.0.0.1` and accepts
IPv4 or IPv6 literals. The compiled `server.port` default is 6250; the supplied
file selects 5555. Port zero requests an ephemeral port.

`database.game_database` defaults to `data/stompymux.db` and is the only live
storage path. `serve` loads schema-32 relational tables there directly. Existing Rust JSON snapshots are rejected and never converted or
merged.
Changing configuration never moves files. A missing live database permits fresh
bootstrap; an existing empty or malformed file requires explicit operator action.

Content and database paths are relative to the game directory, unless absolute.
This includes Lua modules, connection/quit files and bootstrap credentials.
Fresh bootstrap uses the configured object map; existing worlds never bootstrap.
The foundational GOD/Wizard IDs and required room references are validated.
Default object flags and Lua parents also come from configuration.

## Additional Rust settings

Counts and byte limits below must be positive. Durations specify their units in
the name. Existing keys remain authoritative for matching features: for example,
`mux.conn_timeout`, `mux.idle_timeout` and `mux.idle_interval` use seconds;
`mux.command_quota_interval` uses milliseconds. Password hashing costs and login
rate/burst settings remain under their legacy `security` names.

| Setting | Default | Meaning |
| --- | --- | --- |
| `runtime.max_connections` | 1024 | Open connections |
| `runtime.event_queue_capacity` | 256 | World events queued |
| `runtime.session_output_queue_capacity` | 128 | Messages queued per session |
| `runtime.input_line_limit` | 8192 | Bytes per input line |
| `runtime.telnet_subnegotiation_limit` | 8192 | Bytes per Telnet subnegotiation |
| `runtime.output_message_limit` | 65536 | Bytes per output message |
| `runtime.write_timeout_ms` | 5000 | Socket write deadline |
| `runtime.shutdown_timeout_ms` | 5000 | Connection drain deadline |
| `runtime.maintenance_interval_ms` | 1000 | Connection/auth housekeeping cadence |
| `security.login_address_limit` | 4096 | Tracked source addresses |
| `security.login_address_retention_seconds` | 86400 | Address throttle retention |
| `security.login_hash_concurrency` | 5 | Concurrent password jobs |
| `security.login_history_limit` | 32 | Login records retained per account |
| `lua.instruction_limit` | 1000000 | Instructions per callback/module |
| `lua.output_entry_limit` | 1024 | Pending Lua output entries |
| `lua.output_byte_limit` | 1048576 | Pending Lua output bytes |
| `database.busy_timeout_ms` | 5000 | SQLite lock wait |
| `database.bootstrap.credentials_file` | `bootstrap-credentials.txt` | Restricted bootstrap credential file |

The other addition is `server.listen_address`, described above. Protocol bytes, schema limits,
cryptographic constants and socket read-buffer sizes remain implementation
constants.

Login history remains bounded by the legacy schema: four successes and three
failures, or fewer when `security.login_history_limit` is lower. The configured
limit remains the total retained-history bound; lifetime counters are not pruned.

## Database cleaning

`mux.check_interval` is the positive number of seconds between automatic database
check attempts (default 600). `mux.check_offset` is the nonnegative initial delay
(default 300); zero uses the interval. Serve-readiness validation rejects invalid
or unrepresentable monotonic deadlines before bootstrap or database writes.
Cleaning starts enabled; runtime `@enable cleaning`/`@disable cleaning` do not
change TOML. Manual `@dbck` does not reset the automatic deadline. No catch-up runs
occur after delayed ticks, and failed attempts consume their interval.

## Admission and cached connection text

`mux.max_players` counts authenticated sessions; negative is unlimited and zero
allows only privileged existing accounts. Wizard/GOD logins bypass this limit and
the runtime logins control. Registration uses the same strict boundary as ordinary
login. Controls default enabled each startup, have no additional TOML keys, and
`@enable`/`@disable` do not persist them.

`mux.connect_file`, `badsite_file`, `down_file`, `full_file`, `quit_file` and optional
`connect_dir` are cached, relative to the game directory. `@readcache` applies
successful replacements together while retaining unreadable/invalid entries.
The existing `lua.output_byte_limit` bounds aggregate cached UTF-8 content; exceeding
it rejects publication. Down/full responses append `mux.down_message` or
`mux.full_message`. Banner discovery is lexical, nonrecursive and capped at 100
regular nonhidden files containing `.txt`; each welcome selects uniformly.

## IPv4 site access and monitoring

`sites.forbid`/`sites.permit` form one ordered list; `sites.suspect`/`sites.trust`
form another. Each array contains `{address="...", mask="..."}` entries. Categories
follow merged TOML declaration order, and entries retain array order. The first
match in each independent list wins. Put exceptions before broader rules.
Unmatched peers are unrestricted and trusted. Matching uses literal
`(peer & mask) == address`; noncontiguous masks are supported. Addresses with bits
outside the mask are not normalized and produce an unmatchable-rule warning.

Site enforcement is IPv4-only. IPv6 rule values remain parseable, but serving
rejects them. Any nonempty site list also requires an IPv4 listener, including
CLI overrides. These checks run before startup side effects. With no site rules,
IPv6 listeners retain their ordinary behavior.

Forbidden peers receive cached `mux.badsite_file` text and close before Telnet
negotiation or login. Empty text falls back to `Connection refused.` There is no
Wizard bypass and no login-history update. A permit does not override connection
limits or disabled logins. Rules are fixed until restart; `@readcache` refreshes
text only.

`@list site_information` (`@list si`) displays effective lists in matching order.
`@telnet <player>` shows each session's peer and captured site classification.
MONITOR players receive connection/reconnection and partial/final disconnection
notices. The existing `Suspect` channel receives per-session notices for SUSPECT
players and suspected sites independently. Trust rules do not clear SUSPECT.
No missing channel is automatically created. MONITOR notices describe actual
session transitions even when callbacks fail; channel history and delivery remain
transactional. Command auditing to `SuspectsLog` is not enabled by this feature.

## Command, switch and list access

`access.commands` edits the declared permissions of native and Lua commands;
`access.lists` independently edits `@list` topic permissions. A value may be a
whitespace-separated string or an array of strings. Tokens apply in order, adding
bits by default and clearing them with `!`. An empty value leaves defaults intact.

```toml
[access.commands]
"@shutdown" = "!wizard god" # GOD only; adding god alone retains Wizard access
"@list" = "!wizard"        # permit ordinary players to request public topics
"pose/nospace" = "god"     # both pose and this switch must permit the caller
"say" = ["no_suspect"]
"@find" = "disabled"       # denies even GOD
"look" = "dark"            # hidden in discovery, still executable

[access.lists]
site_information = "!wizard god"
permissions = "!wizard"
```

The configurable tokens and minimum abbreviations are:

| Token | Minimum | Meaning |
| --- | --- | --- |
| god | go | GOD role alternative |
| wizard | wiz | Wizard role alternative; GOD also qualifies |
| no_suspect | no_su | Deny SUSPECT executors except Wizards/GOD |
| queue_enabled | queue_ | Require enabled runtime queueing |
| disabled | disa | Deny everyone, including GOD |
| need_location | need_l | Invoker type has a location slot |
| need_contents | need_c | Invoker type supports contents |
| need_player | need_p | Invoker is a Player |
| dark | dark | Hide a command from discovery |

Tokens and target names are case-insensitive. `everyone` is Lua declaration
metadata, not a configurable bit; remove role bits to make a command public.
GOD and Wizard bits are alternatives, not cumulative requirements. SUSPECT means
the object flag, not suspected-site status. Type prerequisites do not require a
nonempty inventory or a valid stored location. Queue/type prerequisites apply to
command invocation; they do not add restrictions to list/switch name-table checks.
`dark` hides commands, not list topics or switch names.

Policies follow merged TOML declaration order, including overlapping canonical
names and aliases. Aliases use the existing command resolver. A canonical name
edits every registration with that name across native, global Lua and object Lua
scopes. Switch policies address registered native switches; Lua patterns do not
create switches. Unknown tokens fail configuration loading. Unresolved commands,
switches and list topics fail runtime construction with source diagnostics, before
startup hooks or database writes. Known deferred list topics stay unimplemented.

Object-control rules, locks, session requirements and macro restrictions remain
independent. Exits and Lua handlers retain their current dispatch precedence.
Denied Lua declarations are skipped before pattern evaluation, allowing later
handlers and fallback; a selected native denial stops dispatch. Forced and queued
commands use the executor's authority, never the initiating Wizard's authority.

`@list` still defaults to Wizard access. Command and topic permissions must both
allow the caller. Discovery reports effective bits, hides dark/disabled commands,
and filters switch access. As in C, GOD may inspect disabled switch names and see
disabled topics in topic suggestions, while invocation remains denied. Reports
remain private, bounded and read-only.

Policies compile from declaration defaults, file edits and runtime edits at startup
and Lua reload. Unresolved runtime targets retain the old VM and policies.
Configuration directives have independent effective permissions through
`access.config`. Existing IC/GAGGED domain behavior is retained; no additional
BattleTech permission vocabulary is added.

## Runtime administration

`@admin <directive>=<value>` defaults to Wizard command access. Each directive
also checks its own permissions, usually GOD-only. Directive names are exact C
names (normally lowercase), not dotted TOML names. Switches are rejected.

```text
@admin max_players=50
@admin config_access=max_players !god wizard
@admin access=@shutdown !wizard god
@admin list_access=options !god !wizard
@admin alias=inspect @examine/brief
@admin flag_alias=shiny ansi
@admin default_thing_flags=shiny safe
@admin bad_name=Guest*
@admin good_name=Guest*
@admin forbid_site=192.0.2.0 255.255.255.0
```

Runtime edits never rewrite TOML or touch the database. They survive Lua reload;
restart restores file configuration and CLI overrides. A command's executor is
used for permissions, including `@force` and `@wait`. Successful edits are visible
before `Set.`. Reports and confirmations are private to the invoking session.

`access.config` accepts the same permission tokens as command access:

```toml
[access.config]
max_players = "!god wizard"
```

`@list config_permissions` reports the full C directive catalog with effective
permissions and `live`, `restart-only` or `unsupported` capability. GOD can inspect
disabled directives. Changing a permission cannot activate an unsupported or
restart-only setting. `@list default_flags`, `@list bad_names` and `@list options`
show effective supported settings, with the usual independent topic permissions.

Live settings cover admission capacity/messages, timeouts and intervals, command
quotas and queues, names/password policy and hash parameters, notification depth,
channel lurking, default flags/homes/Lua parents, game name, Lua memory/state/error
limits, and help/Lua directories. The checked-in directive registry and capability
classification are the authoritative inventory. Settings for unused systems,
including BattleTech and dump/cache controls, report unsupported. C-disabled paths/bootstrap/listener/rendering
settings and all Rust-only infrastructure settings remain restart-only.

Existing deadlines remain scheduled; changed intervals determine the next deadline.
The cleaning offset is startup-only and rejects runtime edits. Reduced quota and login-token
maxima clamp current balances without granting tokens. Existing connections observe
new timeout policy at the next check. Reduced queue/state limits do not delete
existing entries. Pending hash jobs retain captured cryptographic parameters;
completion rechecks current admission, name and password-length constraints.

Lua APIs read an effective snapshot for each operation, including previously created
state/channel handles. Lua memory limits update the active VM. Help/Lua directory
changes affect the next explicit reload, not already indexed/loaded content;
failed reloads retain active content. Default flags and parents affect newly created
objects; defaults must name valid destinations and registered parent modules.

Command aliases target registered native or Lua commands, optionally a native
switch. Existing names/aliases cannot be replaced. Resolution stays nonrecursive.
Access edits affect all matching scoped Lua registrations. Runtime aliases and
policy targets are revalidated during Lua reload; removing a referenced target
rejects that reload.

C multi-token access and default-flag edits preserve partial success: invalid tokens
are reported, valid tokens apply in order, and successful/partially successful edits
end with `Set.`. Flag lists replace defaults beginning with the first valid flag;
empty or entirely invalid lists leave defaults unchanged. Structural validation
failure discards the whole candidate. Integers and booleans are checked; malformed
input does not silently turn into zero. C string capacities are retained; oversized
strings report `String truncated` and stop at a valid UTF-8 boundary.

Runtime site rules use address/mask syntax and are prepended ahead of older rules.
New connections use the new policy; existing sessions retain their classification.
IPv6 listeners reject site additions. `good_name` removes a matching bad-name
pattern, case-insensitively; it does not add an allow-list override.

## Login retries and creation zones

`mux.retry_limit` defaults to 3 completed incorrect-credential attempts per connection.
Zero and negative values close after the first incorrect attempt. Each accepted
connection captures its allowance; `@admin retry_limit` affects future connections.
Changing login names does not replenish retries. Registration mistakes, throttling,
stale results and internal hashing failures do not consume them. Failed-login history
is persisted normally, but database rollback does not replenish consumed retries.

`mux.player_zone` defaults to 0. Positive values assign that available room/thing
as the zone of newly registered and administratively created players. Zero or
negative values assign no zone. `@admin player_zone` validates positive targets
before publishing and affects future creation, including registrations still hashing.
Existing objects are not rezoned. Startup validates positive defaults after startup
hooks, and creation rechecks references before allocation.

Non-player objects inherit the invoking creator's zone, including clones and
reverse exits. Lua creation uses GOD as creator; explicit `zone` overrides inheritance,
while omission/nil inherits. Invalid or GOING zones fail creation transactionally.
Foundational bootstrap objects are allocated without a creator or inherited zone.
The policy inventory is recorded in `tests/fixtures/config/mux-policy.json`.
