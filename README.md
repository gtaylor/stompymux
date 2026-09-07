# StompyMUX Rust foundation

An independent Rust implementation of the MUX foundation. The copied game world,
configuration and Lua modules are the compatibility fixtures; no C server code is
linked or invoked. LuaJIT and SQLite are built from vendored dependencies.

## Run the copied world

From this directory, with Rust and a C compiler installed:

```sh
cargo run -- serve --game-dir game
```

The server reads and updates `game/data/stompymux.db` directly. Existing
`stompymux-rs.db` JSON snapshots are left untouched and are not merged or loaded.
Use only one server writer for a database; do not run legacy and Rust servers
against the same file simultaneously.

The supplied configuration listens on `127.0.0.1:5555`. Without a configured
port, the compiled legacy default is 6250.
Override with `--listen-address 0.0.0.0 --port 5556` when appropriate. Connect
using a Telnet/MUD client. Enter an existing player name, alias, or dbref (such
as `#2`) and password, or enter
a new name and follow the registration prompts. Registration connects the new
player immediately. The server requests password hiding using Telnet ECHO negotiation; this
milestone uses plain TCP and does not provide transport encryption.

Players can use `look`/`l`, `say <message>`/`"<message>`, `WHO`, exit names or
aliases, `global-hello`, and `quit`. The starter-room exit remains wizard-only.
Accounts and locations survive restart. Multiple sessions are supported;
connection hooks receive `reconnect`, and disconnect hooks run only for the
last session. Ctrl-C and SIGTERM stop the listener and finish accepted writes.

## Storage

The live format is **schema 32**. The supplied fixture has 16 objects, two
accounts and two channels. Supported fields are read directly from the relational
tables. SQLx updates only changed supported columns in a single transaction;
existing rows are never replaced. Failed writes restore the in-memory world and
discard pending success output.

Unknown columns, deferred BattleTech data, indexes
and triggers remain in place. Unchanged Lua values retain their original SQLite
storage types, including binary strings in TEXT or BLOB cells. Explicit state-key removals and expired
login-history entries remove only those owned rows. Unsupported required columns
on new rows cause an atomic failure instead of invented defaults.

Legacy containment lists (`contents`, `exits`, `next`) are updated for movement
and creation, preserving unaffected lists and the relative order of retained
members. Login history keeps at most four successes and three failures, newest
first within each outcome, additionally respecting a smaller configured total
limit. Lifetime login counters are independent of retained history.

Persistence uses SQLx 0.9 with Tokio and bundled SQLite, operation-scoped
connections and `database.busy_timeout_ms`. Database loads use read-only connections.
Normal writes do not change journal mode, foreign-key policy or schema metadata.
Older schemas and Rust JSON snapshots are rejected without conversion.

For a **new game**, use a separate game-directory copy with the configured live
database absent. `serve` exclusively creates schema-32 storage with mode 0600,
runs bootstrap Lua, and commits initialization atomically. Empty or malformed
existing files are rejected. Random administrator credentials are written to
`bootstrap-credentials.txt` with mode 0600; stale credentials are not overwritten.
Existing worlds never run first-startup hooks.

## Configuration

All 182 legacy TOML mappings are typed and retained, including options for
features not yet implemented. Twenty-one additional settings configure the Rust
runtime. See [configuration semantics and runtime defaults](docs/configuration.md)
and the annotated `game/stompymux.toml`.

Listener precedence is CLI → TOML → centralized defaults. Both IPv4 and IPv6
addresses are supported. Content paths resolve relative to `--game-dir` (default
`game`); recursive include paths resolve relative to the including file.

`database.game_database` names live schema-32 storage, defaulting to
`data/stompymux.db`. `database.legacy_game_database` is deprecated and ignored.
Changing configuration never moves files or merges data from another database.

## Wizard movement

Wizards and GOD can use `home` to return to their stored home, or
`@teleport <destination>` to move inside a room, player or thing.
`@teleport <object>=<destination>` moves another player, thing or exit; configured
aliases such as `@tel` work too. Ordinary players cannot use these commands.
Wizards may teleport other Wizards. Use dbrefs for remote targets, or `me`,
`here` and exact visible nearby names. Ambiguous names are rejected.

Containment cycles and invalid destinations are rejected. Moving an occupied
container carries its contents; relocating an exit preserves its linked
destination. Teleporting through exits and command switches remain unavailable.
Teleportation checks the destination's `teleport` lock and each enclosing
source container's `teleport_out` lock; errors deny movement. `home` bypasses
these locks, but still validates its stored destination.

Movement calls source `on_exit` and destination `on_enter` hooks with the moved
object as `enactor`, initiator as `cause`, immediate `source` and `destination`,
and the location hosting the callback as `object`. A descriptor is supplied
only when the moved player initiated the command. Moving an exit or remaining
in the same location does not fire occupant transition hooks. Containers without
an appearance callback use native description rendering for `look` and the generic Lua renderer on arrival.
Connected moved players receive the appearance on all their sessions.

Location changes and callback effects persist together before success output.
Failures roll back the move and discard its pending messages. No home fallback
is selected when the stored home is missing or invalid.

## Object flags

Objects use the shared 19-flag MUX catalog. Wizards can inspect flags with
`@list flags` and `@examine <target>` (including configured aliases such as
`@ex`). This examination command currently shows identity, type, flags and powers.
Use `@flag <target>=DARK` to set a flag and `@flag <target>=!DARK` to clear it.
Targets support `me`, `here`, dbrefs and exact visible nearby names or exit
aliases. Ambiguous names are rejected. Flag names and configured aliases are
case-insensitive; one flag is changed per command.

GOD controls all live objects. Wizards control themselves and non-Wizard
objects, but cannot edit another Wizard or grant Wizard status. Only GOD can
change WIZARD, and cannot clear its own WIZARD flag. GOING follows the legacy
special clearing policy; destruction itself remains deferred. Ordinary players
cannot use these administrative commands. Successful durable changes are saved
before acknowledgement.

**CONNECTED is session-owned.** It becomes true after a successful login or
registration and stays true until the last session disconnects. Lua lifecycle
hooks observe the updated value. Callback errors and database failures cannot
restore stale connection state. CONNECTED is stored as zero and
ignored in stored/default object flags; commands and Lua cannot override it.

Lua uses immutable constants such as `mux.world.flags.DARK` and
`object:flags():has/add/remove`. Mutation returns whether the flag changed;
unknown names and raw strings are rejected. Trusted Lua retains GOD-level
mutation authority, subject to GOD's WIZARD protection and session-owned
CONNECTED. Flags for deferred systems remain available as data without enabling
those systems.

## Object powers

The current legacy catalog contains one power, `IDLE`. It is persistent metadata
only: granting it does **not** bypass idle timeouts yet. New objects have no
powers, and imported powers survive restart.

Wizards can use `@power <target>=idle`, `@power <target>=!idle`, and `@list powers`.
`@examine` also shows a `Powers:` line. Target resolution and control permissions
match flag administration: GOD controls all live objects; Wizards control
themselves and non-Wizard objects. Names are case-insensitive, and each command
changes one power. Configured command aliases apply; power aliases are not added.
Changes are saved before success is reported.

Lua exposes immutable `mux.world.powers.IDLE` and
`object:powers():has/add/remove`. Add/remove return whether the set changed.
Use power constants rather than strings or flag constants. Trusted Lua can
change powers on any live object, with the same transactional rollback used by
other callback mutations.

## Implementation boundaries

- Tokio handles sockets and timers. A single owner serializes world and Lua
  work, using bounded channels. SQLx provides async SQLite access through its
  own worker threads; Argon2id runs on bounded blocking workers. Lua values
  never cross threads. A database commit serializes command processing while
  networking continues.
- LuaJIT runs the copied packages, object parents and global modules in lexical
  path order. World/state/flag handles, appearance and traversal callbacks,
  lifecycle hooks, local/global command matching, basic channel creation,
  configuration lookups, text formatting and WHO session APIs are available.
- Lua memory and persistent-state limits are enforced. Callback execution has
  a one-million-instruction budget. LuaJIT tracing is disabled so traces cannot
  bypass that budget; the LuaJIT runtime and language remain in use. OS, FFI,
  debugging and native module loading are unavailable to game scripts.
- Lifecycle callback failures are isolated; startup errors prevent listening.
  Failed commands roll back mutations. Invalid traversal policies deny entry.
  Lua output is bounded independently of Lua heap allocations.
- Telnet supports fragmented framing, ECHO, TTYPE, NAWS, UTF-8 CHARSET,
  NEW-ENVIRON, MSSP, MCCP2 output compression and GMCP Core.Ping,
  with plain, ANSI 16/256 and truecolor output, plus capability-gated OSC 8 links. Unsupported options are declined. Input
  lines are bounded at 8 KiB; output messages at 64 KiB, with 128 queued
  messages per connection. Slow clients are disconnected. Login throttles,
  hash concurrency/rate limits, command quotas and idle timeouts are active.

BattleTech simulation, additional GMCP packages, a webserver and browser-side
action handling remain deferred. Legacy help/type files describe a
larger API than this milestone implements. Deferred configuration is reported in one capability diagnostic. Site/access
rules parse and pass configuration checks, but block serving before writes or
listening because enforcement is not implemented.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests use `tests/fixtures/game` copied into temporary directories, ephemeral
TCP ports and controlled credentials. They cover relational data preservation and schema
rejection, bootstrap idempotence, Lua limits/locks, real registration and login,
duplicate registration, password mismatch, throttling, speech, WHO, movement,
multiple sessions, restart durability, injected write failures, stale auth
results, bounded output and graceful shutdown. They do not mutate `game/` or
`../btmux-khi/`.

The original game assets and data were copied intact. Their inherited license
is retained in `LEGACY-LICENSE.md`.

## Object search

Wizards and GOD can use `@find [name][,low[,high]]` (also `@fi` or `@fin`).
Names match case-insensitive prefixes at the start of a name or word; wildcard
characters are literal. Bounds are inclusive, accept optional `#`, and default
to zero and the current database maximum when omitted or invalid.
Searches list controlled live objects except exits, ordered by dbref, including
DARK and GOING objects. Wizards cannot list GOD or other Wizards; GOD can list all.
Results use `Name(#dbref:type-and-flag-letters)`.

Results are delivered automatically in bounded chunks and end with
`***End of List***`. There is no continuation command or session search cursor.
`runtime.find_page_size` is removed; old settings receive an unknown-key warning.
Searches never write the database or run callbacks. Reports exceeding the aggregate
Lua output-byte budget explicitly mark omitted rows; use narrower criteria or dbref
ranges to reduce the result set.

`@search [class=criteria[,low[,high]]]` searches the entire supported database,
including exits, other Wizards and Garbage tombstones. Classes are `name`,
`rooms`, `exits`, `objects`/`things`, `players`, `type`, `flags`, `power` and `zone`.
C abbreviations apply (`p` means players, `po` means power). Name criteria match
case-insensitive whole-name prefixes; an empty name criterion matches nothing,
while bare `@search` matches everything. Flags use case-sensitive display letters,
including optional type letters; all specified flags must match. `power=idle`
uses the existing power catalog. Reports group results by type with relationship
annotations and totals covering all matches, including any omitted rows.

`@stats` reports allocated object slots and counts by type. GOING non-room objects
and allocation gaps count as garbage; GOING rooms still count as rooms. Reports do
not create objects for gaps. Search results enumerate actual stored objects.

`@list commands`, `@list permissions` and `@list switches` describe active command
registrations alongside existing flags/powers listings. Commands are grouped into
built-in, global Lua and reachable object Lua entries, with aliases, permissions,
patterns and source identity. Listings share current dispatch scope; inventory and
command-zone expansion remain deferred. Switch listings show minimum accepted
abbreviation lengths; handlers still enforce valid combinations. Successful Lua
reload updates listings; editing runtime tables does not re-register declarations.

## Command registration

Native and Lua commands share an enumerable registry of names, permissions,
matchers and sources. Native definitions in `src/commands/registry.rs` register
function pointers; command handlers live separately from dispatch and target
matching. Permission bits are `EVERYONE`, `WIZARD` and `GOD`; combining restricted
bits requires GOD. Object control, flag policies and movement locks still apply.

Lua declarations now require explicit metadata:

```lua
return {
  commands = {
    {
      name = "greet",
      permission = "everyone", -- also "wizard" or "god"
      pattern = "^greet%s+(.*)$",
      handler = function(ctx, name)
        mux.world.pemit(ctx.enactor, "Hello, " .. name)
        return true
      end,
    },
  },
}
```

Missing metadata or malformed patterns fail module loading with the source and
command index. Names are single tokens without `/`; the registry normalizes
names to lowercase. Lua patterns retain their original case behavior. Patterns,
permissions and handlers are captured at load time; editing the returned module
table does not change registration.

Configured aliases work for both native and Lua commands, preserving arguments.
Full-token aliases take precedence, followed by base-token aliases for switches;
there is no recursive alias expansion. Unaliased Lua commands see original input.

Native commands run first and reject unauthorized access or unsupported switches
without falling through. Lua tries eligible local objects by dbref, then global
modules in lexical order, preserving declaration order within modules. Restricted
Lua entries are skipped; a handler returning false/nil allows later handlers and
then exits to match. NO_COMMAND and HALTED still exclude object-local commands.
The metadata API prepares for `@list commands`; that command is not added yet.

## Shutdown and database maintenance

`@shutdown` and `@dbck` require Wizard status (or GOD), honor configured command
aliases, and accept no arguments or switches. `@dump` is not implemented: ordinary
mutations already commit to SQLite before success is reported.

`@shutdown` validates and saves first. A failed initial save cancels the command
and leaves sessions online. Once accepted, it broadcasts `Game: Shutdown by <name>`
and stops accepting connections, commands and authentication results, including
remaining commands in the same input batch. SIGINT and SIGTERM enter this same
shutdown coordinator, but proceed with best-effort cleanup if the initial save
fails. Repeated requests do not repeat cleanup. All paths detach sessions, run the
last-session disconnect hooks, reconcile CONNECTED, finish persistence and drain
connection output for up to `runtime.shutdown_timeout_ms` before aborting remaining
connection tasks. Persistence failures are logged and produce an unsuccessful
process exit.

`@dbck` checks SQLite integrity and repairs supported references, homes, containment
lists and exit sources. It preserves valid list order, resolves competing lists
using the object's claimed location when that list contains it, and appends newly
attached members by dbref. Replacement homes use a safe controlled location, safe
controlled home, configured default home, starting home, then starting room.
Room droptos are stored separately from containment and applied after ordinary drops.
Unreachable rooms without FLOATING, Wizard objects and ordinary occupants inside
Wizard containers are diagnostic findings, not reasons to change flags.

GOD and configured starting/home destinations are protected from GOING purges by
clearing GOING. Other doomed objects become retained Garbage tombstones; surviving
occupants are evacuated and doomed exits are removed. Dbrefs are never reused.
The explicit ownership map in `src/persistence/maintenance.rs` removes owned
accounts/history, Lua state, channel memberships, communication macros, character,
economy and BattleTech records, and clears known dependent references. Unknown
columns on retained rows and unrelated tables remain untouched. An unknown declared
dependency that prevents safe cleanup aborts the transaction.

Repair relocations bypass movement locks and run applicable exit/enter callbacks,
with the moved player's session when available. Callback failures, invalid callback
results and database errors roll back the whole repair and discard pending output.
Destroyed players are notified and disconnected only after commit; their pending
authentication results cannot reattach them. Successful checks return a bounded
summary ending with `Done.`; detailed findings go to server diagnostics.

Startup remains strict and does not repair malformed worlds automatically. This
is an online maintenance command, not an offline recovery tool. It does not invent
missing foundational objects or guess repairs for duplicate accounts or undecodable
records. As with ordinary persistence, only one server may own the database.


## Telnet negotiation

Each session owns an RFC 1143 Q-method state machine, with independent local and
remote state for all option numbers. Pending enable/disable negotiations retain
one opposite request: repeated requests are suppressed, reversals wait for the
outstanding acknowledgement, and refusals do not automatically trigger retries.
Only the confirmed YES state enables option-specific subnegotiation effects.

The compatibility reference is the C server's `mux/network/telnet_handler.c`,
followed by its bundled libtelnet. Startup sends `DO TTYPE`, `DO NAWS`,
`DO NEW-ENVIRON`, `WILL MSSP`, `WILL MCCP2`, `WILL CHARSET` and `WILL GMCP`. This corrects the earlier Rust implementation's CHARSET direction.
Unsupported options/directions and unsolicited ECHO requests are declined.
Password prompts explicitly request local ECHO changes; their acknowledgements
are accepted even though unsolicited ECHO is not supported.

TTYPE discovery requests up to three initial responses, including MTTS, following
the C handler's connection-wide response counter. Duplicate WILL messages do not
restart discovery. Disabling TTYPE resets the terminal name to `vt100`, retaining
observed ANSI/MTTS metadata; disabling NAWS restores 80×25. Unnegotiated payloads
cannot alter terminal state, but still count against subnegotiation size limits.

After CHARSET enablement, the server requests UTF-8 once. Acceptance, rejection
or disablement clears the pending request; competing requests are rejected while
it is pending. Only UTF-8 offers are accepted. Input validation always remains
UTF-8, regardless of a client's CHARSET response; no transcoding is performed.

Login and registration do not wait for negotiation acknowledgements, preserving
plain TCP client compatibility. Clients that refuse or ignore ECHO negotiation
may display passwords locally; queued echo restoration is sent when the outstanding
negotiation completes. No negotiation timeout or automatic retry is introduced.

The Rust API separates `telnet::q::Negotiator` transitions from decoder framing
and option handling. Mutable `Decoder::initial`, `echo` and `negotiate` methods
return ordered output/state-change events. `feed_byte` lets the world owner finish
a login action before processing later bytes in the same packet; bulk `feed`
remains available to callers without interleaved application actions. Every wire
reply uses the existing bounded session output queue.


## Extended protocols and session inspection

`@session [player-name prefix]` and `@telnet <player>` require Wizard status or
GOD. Both honor command aliases, reject switches, and send read-only diagnostics
only to the invoking session. `@telnet` accepts names, account aliases and dbrefs.
Multiple connections are separate rows/blocks in ascending Rust session-ID order;
IDs replace C file descriptors. Reports are bounded by `runtime.output_message_limit`
and explicitly mark truncation. No database writes or Lua callbacks are performed.

`@session` shows connection time, idle time (C's ten-minute idle suppression), and
input/output pending, lost and total byte counters. Input totals count socket
bytes; pending input includes queued socket bytes and unterminated text. Lost input
counts undeliverable input batches and rejected invalid, oversized or quota-limited
text. Output totals count attempted logical, Telnet-encoded output bytes before
compression; pending counts accepted message bytes until the writer completes the
message, and lost counts rejected or failed messages. A partially written failed
message is counted as lost in full because compressed bytes cannot be mapped back
to a precise logical prefix. Protocol startup control markers are excluded from
logical output totals. `@telnet` additionally reports actual wire bytes written,
including the MCCP2 marker and zlib overhead. Counters are live samples.

`@telnet` groups information under TTYPE/MTTS, NAWS, CHARSET, NEW-ENVIRON, GMCP,
MSSP, MCCP2 and ECHO. It shows both Q directions, queued reversals, requested echo
suppression, and compression transport state independently. Advertised terminal
color depth is metadata; rendering remains plain/16-color ANSI. Client-controlled
values are escaped before display, including non-ASCII bytes and terminal controls.

NEW-ENVIRON IS replaces the session environment; INFO patches it. VAR and USERVAR
are separate byte-string namespaces, absent values delete entries, and empty values
remain present. Updates are atomic with C's limits: 64 entries, 256-byte names,
4,096-byte values and 65,536 aggregate bytes. The configured subnegotiation bound
also applies. Invalid or oversized environment updates are logged and discarded;
previous values survive. Values never modify the server's process environment.

MSSP reports configured NAME, authenticated session count PLAYERS, server-start
Unix timestamp UPTIME, CODEBASE `stompymux-rs`, and the actual listening PORT when
enabled. GMCP currently handles only case-sensitive `Core.Ping` (with optional
space-delimited payload), replying with bare `Core.Ping`, as the C handler does.

MCCP2 uses one persistent zlib stream per connection. Queued plaintext precedes an
uncompressed activation marker; all subsequent bytes, including Telnet replies,
are compressed and sync-flushed for prompt delivery. Duplicate negotiation cannot
start another stream. Matching the C server, DONT changes Q state but does not
stop an already active compression stream. Graceful closure finishes the stream
within existing output/shutdown deadlines. Compression or socket failures close
the connection; the writer never switches back to plaintext. Buffers, queues and
logical message limits remain bounded. Inbound compression is unsupported.


## Styled text, Markdown and help

Game output retains the C bracket language. The parser supports grouped and
nested foreground/background colors, bold, italic, blink, underline, overline,
strikethrough, inverse, reset and literal `[[` escaping. Colors resolve through
CSS/X11 names, configured `[colors]`, `#RRGGBB` and `rgb(R,G,B)`.

`mux.text.markup()` validates and returns the original markup. `mux.text.style()`
wraps its input using foreground/background, bold, underline and inverse options.
Invalid strict input raises a Lua error. Ordinary output uses permissive parsing:
invalid markup stays visible, legacy SGR is adapted to the terminal, and other
raw escape sequences are removed. `say` strips user styling, matching C.
Object name/description setters validate new markup; existing stored text is
preserved and rendered permissively.

OSC 8 supports C's external/send/prompt actions and all Tier 1–6 metadata:
base/state styles, tooltips, titles, menus, visibility/expiry, spoilers, disabled
links, selection, compact encoding and configured presets. Each capability requires
its corresponding NEW-ENVIRON USERVAR to equal `1`. Presets are sent once per
connection before use. Unsupported actions retain their labels and applicable
ANSI styling; `osc8demo` exercises the copied demonstration module.

`color` reports the current session's mode. Use `color auto`, `off`, `16`, `256`
or `truecolor`; overrides affect only that connection and disappear at disconnect.
The ANSI player flag is still required. Auto mode respects screen-reader detection;
an explicit override can enable colors for a screen-reader client. OSC capability
selection is independent of color. `@telnet <player>` reports effective settings.

Conventional Markdown is opt-in, parsed by pulldown-cmark with CommonMark, tables,
task lists and strikethrough. It is never inferred from names or ordinary messages:

```lua
mux.world.pemit(ctx.enactor, mux.text.markdown(
  "## Notice\n\n**Welcome!** Read [help](help:about)."
))
```

The immutable document stays typed through callback transactions and renders for
each receiving session. Markdown code and C bracket examples stay literal.
Telnet output wraps prose to NAWS width, preserves code whitespace, and lays out
tables using display columns, falling back to labeled rows on narrow terminals.
External links display their destination when OSC links are unavailable. Relative
help paths and `help:topic` links become help actions.

`help [topic]` and configured aliases browse `mux.help_directory`. Bare `help`
opens `index.md`; topics match keywords case-insensitively, with substring
suggestions when no exact match exists. Root-relative article paths also work.
Wizard-only articles are excluded from unauthorized lookups, suggestions and
indexes. Index links issue `help <primary keyword>` when OSC send links are
available; plain clients see the same topic names and typing instructions.

Metadata is indexed at startup. Article bodies are read on demand, so editing a
body is immediately visible. Wizards use `@help/reload` to refresh metadata,
keywords, visibility and added/deleted files; bare `@help` lists the switch.
Malformed articles are skipped and duplicate keywords retain the first lexical
file's declaration, with contextual diagnostics and reload counts. A fatal
traversal failure retains the previous index. Missing help content is tolerated
at initial startup. These commands never change the world database.

Help uses complete output chunks rather than truncating at one message. Each
chunk honors `runtime.output_message_limit` and closes ANSI/OSC sequences;
the complete encoded response is bounded by `lua.output_byte_limit`. Queue
admission shares one `runtime.write_timeout_ms` deadline for the response, with
existing slow-client eviction and compression behavior. A response that cannot
fit these budgets produces an explicit error instead of silent truncation.

Help articles use complete-line `+++` delimiters around TOML front matter:

```toml
title = "Movement"
description = "Getting around the world"
keywords = ["movement", "travel"]
article_tags = ["show_in_index"]
# Optional: wizard_only = true, weight = 10
# For an index: show_index_for_article_tags = ["movement_topics"]
# index_style = "list_with_description" or "columnar"
```

The body after the closing delimiter is Markdown. Tagged indexes exclude
themselves, sort weighted articles first, and adapt topic/description rows or
three-column layouts to the terminal width. Relative Markdown links resolve
against the containing article, with `..` allowed only inside the help root;
`help:topic` links address keywords directly. Code examples and bracket markup
remain literal. Lists and quotes retain their indentation when prose wraps.

Text width, wrapping and truncation use unicode-width and unicode-segmentation.
This deliberately improves on C's byte counts: CJK, combining marks and emoji
clusters remain intact, even across style boundaries. Rendered messages fit the
existing encoded output limit, including closing OSC links and ANSI resets.
Rendering precedes queue accounting and MCCP2 compression. Lua document sources
are bounded by the message limit; retained document memory additionally shares a
budget capped by the configured Lua memory and output-byte limits.

The library exposes `text::Document::html(palette, limit)` for future web use.
Markdown uses pulldown-cmark's HTML renderer with raw HTML suppressed. Legacy
styles produce escaped spans and links with controlled CSS; game actions and
advanced link metadata use `data-mux-*` attributes, never inline JavaScript.
Hosts may supply presentation for `.mux-inverse` and `.mux-blink` and implement
action handling. No HTTP server, browser interaction or network image fetching
is included. HTML exceeding its byte limit returns an error rather than a broken
fragment.

### Server-owned Lua code

`src/lua` owns the LuaJIT runtime. Runtime construction installs the shared
instruction/memory budgets, registers built-ins, seals the existing sandbox,
then loads object and global game modules in lexical order. Loading, callbacks
and scoped command invocation live in separate modules; command metadata stays
in the command registry. Rust callers use `stompymux_rs::lua::Scripts`.

`src/lua/packages` contains one directory per built-in API area: `world`,
`session`, `config`, `text` and `comsys`. Each directory owns its Rust bindings
and embedded Lua facade. World bindings also own flag/power userdata; text
bindings own immutable Markdown userdata and its allocation accounting. Domain
models and text renderers remain independent of those Lua adapters. Registration
passes shared dependencies explicitly and assembles the same `mux` table returned
by `require("mux")`; it adds no new require paths.

The Lua facades and sandbox script are compiled into the binary. They do not
require the source tree at runtime. Editable object/global scripts and helper
packages such as `access_policy` and `object_appearances` remain in the configured
game Lua directory. Dotted helper imports still resolve through
`packages/?.lua`, independently of the built-in source layout.


## Channels and paging

The shared `communication` service owns channel memberships, per-player aliases,
listening preferences, twenty-message histories and last-page recipients.
`addcom`, `delcom`, `clearcom`, `comlist`, `allcom`, channel aliases and `page`
are available to players; `@chan` administration requires Wizard access.
See `help channels`, `help page` and `help @chan` for syntax.

Channel aliases match case-insensitively before the native/Lua command registry.
Configured aliases still select registered communication commands normally.
Channel speech strips styling from message bodies; administrative emits retain
styled markup. All output uses the existing session rendering, Telnet encoding,
bounded queues and MCCP2 writer. Active recipients come from authenticated
sessions, so multiple sessions receive the same player-directed message without
duplicating channel history or first/final connection announcements.

Storage directly owns `comsys_channels`, `comsys_channel_users`,
`comsys_channel_messages`, `commac_aliases` and `player_last_page_recipients`.
Updates preserve unknown columns on retained rows and untouched position lists.
Channel deletion removes related aliases, members and history. `commac_entries`
is initialized only when needed; existing macro slots remain unchanged. Unknown
foreign-key dependencies block destruction instead of being guessed. No schema
migration or reinterpretation of stored channel bits occurs: PUBLIC is `0x200`,
LOUD `0x100`, TRANSPARENT `0x400`, and a new channel starts with mask `127`.

Lua exposes `mux.comsys.channel(name)`, `create_channel(name)`,
`destroy_channel(channel)` and `list_channels()`. Channel handles provide
`name`, `object`, `set_object`, `user_count`, `max_user_count`, `message_count`,
`emit(message, {no_header=true})`, `who({all=true})`,
`add_player(player, alias, quiet)` and `boot_player(object)`.
`channel:flags()` supports `list`, `has`, `add` and `remove` with immutable
`mux.comsys.flags.PUBLIC`, `.LOUD` and `.TRANSPARENT` constants. Mutators return
whether the flag changed. Handles retain identity across transactions and reject
use after destruction, including provisional channels removed by rollback.

Attached objects supply `CHANNEL_JOIN`, `CHANNEL_TRANSMIT` and `CHANNEL_RECEIVE`
locks. As in the C fork, a passing lock grants access independently of the
player/object access bits. Failed lock callbacks roll back their mutations;
explicit access bits still apply. Channel things receive `on_leave` callbacks.
Native commands, Lua calls and lifecycle delivery share transaction-staged output.
Pages are online-only, obey in-character/GAGGED restrictions with Wizard endpoint
exceptions, and persist the ordered successful recipients for repeat paging.
## Player macros

`help macros` documents the complete dot-command system: `.create`, `.add`,
`.del`, `.chslot`, `.list`, `.glist`, `.ex`, `.gex`, `.name`, `.chmod`, `.chown`,
`.clear`, `.def` and `.undef`. For example, `.create Shortcuts`, then
`.def hi=say Hello, *!`, makes `.hi everyone` say “Hello, everyone!”.

Players attach up to five shared sets. Lookup checks slots 0–4 independently
of the selected editing slot. Aliases are 1–4 printable ASCII bytes; matching,
duplicate detection and removal are case-insensitive. `*` substitutes the
remaining arguments and `%*` is literal. Expansion runs once before channel,
native and Lua dispatch, preserving permissions and configured command aliases.
Expanded text cannot invoke macro management. The expansion limit is the smaller
of `runtime.input_line_limit` and 8,191 bytes; overflow rejects the entire command.

The `macros` domain owns sets and attachments; registered management handlers
stage private confirmations until commit. SQLite uses `macro_sets`,
`macro_entries` and the existing macro fields of `commac_entries`, with no schema
change. Channel operations preserve these fields. Explicit row identities track
set/entry reindexing so unknown columns move with retained records; identities
advance only after a successful transaction. `.clear` and `@dbck` compact global
set numbers and update every affected slot. Unknown declared dependencies prevent
unsafe reindexing rather than being rewritten speculatively.

L/R/W permissions follow the C model, including no implicit Wizard write bypass
and continued use of previously attached sets after R is removed. Two deliberate
corrections are case-insensitive invocation/removal and requiring an unlocked,
writable set for `.name`. No macro Lua API or recursive command evaluation is
provided.

## Object state and exit policies

Wizard-only `@state` manages persistent, case-sensitive namespaces on any live
object (including other Wizards and GOD). Configured command aliases apply.
`@examine` includes namespace counts; `@state` lists the available switches.

```text
@state/examine here
@state/examine #13/locks.traverse
@state/set #13/locks.traverse flag/WIZARD=true
@state/set #13/locks.traverse message/enactor="Staff only."
@state/set me/access pass="\x00\xFF"
@state/copy me/access pass=backup pass
@state/move me/backup pass=archive pass
@state/set me/access pass=
@state/wipe me/archive
```

`/examine` defaults to `here`; `/wipe` requires an object and optionally a
namespace. Copy and move operate on one object, overwrite their destination,
and preserve the value's type. An empty assignment deletes; `""` stores an empty
string. Unquoted `true`/`false`, signed decimal integers and finite numbers are
parsed as scalars; other text is a string. Quoted strings support `\"`, `\\`,
`\n`, `\r`, `\t`, and `\xNN`, including NUL and non-UTF-8 bytes. Inspection escapes
these bytes and displays bracket examples literally.

Namespaces start with an ASCII letter and are at most 127 bytes; keys follow the
same rule with a 255-byte limit. Subsequent characters may be ASCII letters,
digits, `_`, `-`, `.`, or `/`. Empty namespaces disappear after their last key is
removed. The existing Lua limits apply to all state mutations: value payloads
count string bytes, one byte per boolean, or eight bytes per integer/number;
object totals also include namespace and key bytes plus one terminator each.

Lua `object:state(namespace)` returns an immutable handle with `get(key, default)`,
`has(key)`, `set(key, value)`, `delete(key)`, `keys()`, `entries()`,
`get_many(keys)` and `set_many(values)`. Keys and entries are sorted. Missing
values return the original default, including tables and `false`; setting `nil`
deletes. Mutations and enumeration require a game callback. Reads can also occur
during module loading. Handles to purged or rolled-back provisional objects are
invalid. Lua strings preserve every byte; integers remain signed 64-bit in
storage, subject to LuaJIT's numeric precision when read into Lua.

`set_many` validates its entire final candidate before publishing any updates,
even when Lua catches an error with `pcall`. This intentionally improves on C's
partial batch behavior. Callback errors and persistence failures roll back state
and staged output. New or changed strings use SQLite BLOB values; unchanged
values retain their original SQLite storage class and extension columns.

The copied `default_exit.lua` and `access_policy.lua` run unchanged. Policies in
`locks.traverse` combine flag, affiliation and typed state requirements with AND;
malformed entries fail closed. `message/enactor` and `message/others` customize
denials, with empty strings suppressing the corresponding message. Neighbor
messages include the traveler's name. Silent Dark Wizards suppress denial
notifications and failure hooks. Valid denials invoke `on_fail`; teleport policy
denials use their corresponding teleport failure events. Lock errors and invalid
return tables cannot retain callback mutations. Channel permission bits retain
their independent grant behavior.

## Scheduled Lua events

Global and object-logic modules can declare named `schedules` with a five-field
UTC `cron` expression and a `handler(ctx)`. The supplied `example.lua` hourly
schedule is registered at startup. Definitions are captured at load time; apply changes with `@lua/reload` or
a restart. See [Lua schedule authoring](docs/lua-schedules.md) for syntax and
callback contexts.

Matching jobs run at a deterministic offset within the first 55 seconds of their
minute. Startup and missed minutes are skipped, expired jobs are dropped, and
failed jobs are not retried. Each callback uses the normal resource limits and
commits its world changes before delivering output. Shutdown cancels pending
jobs. Garbage and GOING objects do not run schedules; HALTED and NO_COMMAND do
not suppress them.

Wizards use `@lua/schedule [object or module]` to inspect captured declarations.
For example, `@lua/schedule global_logic/example.lua` shows the supplied hourly
job. Inspection is private to the invoking session and performs no writes.
Bare `@lua` lists `/parent`, `/viewparent`, `/check`, `/reload`, `/schedule` and `/test`.
Reload validates a separate runtime and persists initialization mutations before
publication. Failure preserves active code and pending jobs. Success resets Lua
globals and cancels old jobs without rerunning startup hooks or recollecting the
current minute. See [Lua object APIs and administration](docs/lua-world.md).


## Object locks and operations

All 17 non-BattleTech locks share `LockType`, `LockInvocation` and `LockOutcome`.
`mux.world.locks` exposes immutable typed constants; `lock_passes` validates
identities/options, supplies a silent context and returns false on callback
failure. Declarations are validated at startup and handlers are looked up live
on the object's current module. Missing attachments/handlers allow checks;
malformed handlers fail closed. Failure rollback uses the existing world/output
transaction. See `help locks` and the checked-in `tests/fixtures/lock_catalog.tsv`
for native callers, identities and failure-event choices.

Players can `get`/`take`, `drop`, `give`, `use`, `enter`, `leave` and inspect
`inventory`. Configured aliases and player macros use the same handlers.
MATCH is a silent matching preference, followed by action-specific policies.
AUDITORIUM speech now checks SPEAK; Wizards retain the C GAGGED exception.
Channel policies retain independent flag grants and Wizard bypasses.

Wizards can `@open`, `@link`, `@unlink` and `@clone`, including homes and room
droptos. Optional link denial may leave a new exit/clone unlinked. Callback or
persistence errors restore the entire command. Cloning copies state,
descriptions and Lua parent into a fresh object; it does not duplicate contents,
accounts, channel/macro membership or deferred BattleTech records. See
`help objects` and `help object building` for syntax and quiet/placement switches.

The `lua::ObjectAction` service evaluates message handlers and operation-specific
events. Ordinary relocation uses leave/move/enter and cross-location messages;
existing teleport/home transition interfaces remain supported. Quiet switches
follow their operation's C policy and never bypass lock checks. Output stays
staged until the final world validates and SQLite commits. Schema-32 storage,
unknown-column preservation and session-owned CONNECTED are unchanged.

### Basic building and inspection

Wizards can build and edit objects with `@create`, `@dig[/teleport]`, `@name`,
`@alias`, `@description`, `@internal-description`, and `@chzone`, alongside
`@open`, `@link`, `@unlink`, and `@clone`. Configured command aliases apply.
Creation uses configured defaults and the existing schema-32 database. Player
renames and aliases share case-insensitive login-name uniqueness; `@alias` is
player-only. Zoning a non-player clears WIZARD and its powers.

`look <target>` supports local objects, exit aliases and possessive names.
`look/outside` looks out of a player/thing container. Internal and external Lua
appearances are selected separately; native fallback includes descriptions,
describe callbacks and visible contents. Looking through a transparent exit can
show its destination. Callback mutations and output share the command transaction.

`@examine` defaults to `here` and shows descriptions as literal editable markup,
relationships, Lua metadata and state namespace counts. `/brief` omits namespace
counts; `/debug` includes actual stored containment pointers. `@entrances` lists
incoming exits, homes and droptos with optional inclusive dbref bounds. Inspection
reports are private, chunked through the bounded transport, and perform no writes
or Lua callbacks. BattleTech fields remain outside these commands' current coverage. See `help object building`, `help look` and
`help @examine` for syntax and partial-success behavior.


## Lua test suites

Wizards use `@lua/test [filter]`, `/test/unit`, `/test/integration` and the
`/verbose` modifier. Suites use the supplied `testing` helper under `lua/tests`;
filters are case-sensitive literal substrings of `module_path:test_name`.

The test VM is separate, but its world is live: assertions and runtime errors do
not undo valid mutations. Every module initialization, hook and test validates
and saves its changes before sending output. Invalid or failed writes roll back
that invocation and count as an error. Normal gameplay callbacks retain their
existing rollback behavior. Use a scratch database when authoring tests.

`@lua/check` also validates test declarations without running hooks or tests.
Reports are private, with bounded failure details, tracebacks, optional passing
names and totals. See [Lua testing](game/help/wizard_commands/lua/test.md).

## Player and account administration

Wizards can use `@pcreate <name>=<password>`, `@newpassword <player>=<password>`,
`@boot <player>` and `@last [player]`, including configured aliases.

Creation shares registration defaults but leaves the player offline. Password
hashing shares existing bounded workers and global limits. Creation and reset
confirmations follow persistence; the caller must remain connected and authorized
through completion. Empty passwords are rejected, and creation confirmations omit
the supplied password. GOD's password cannot be reset by this command.

A successful reset preserves existing sessions but invalidates pending login
results checked against old credentials. Only one reset per account can be
pending. Passwords are not included in administrative diagnostics.

`@boot` closes all sessions for another player, protecting GOD. `/port` selects
one stable Rust session ID (including unauthenticated sessions), and `/quiet`
suppresses the victim message. Only GOD can boot a GOD session by ID. Booting uses
the existing disconnect hooks and session-owned CONNECTED reconciliation.

`@last` defaults to the caller and reports lifetime success/failure counts and
retained login records in newest-first order, formatted in UTC. Reports remain
private and do not write the database. See the corresponding command help articles.

## Speech and notification routing

Public `say`, `pose` and the `"`, `:`, `;`, and backslash shorthands now use the
C notification graph. Wizards also have `@emit`, `@pemit`, `@npemit`, `@oemit`,
`@fsay`, `@fpose`, `@femit` and `@wall`, including their C switch catalogs and
configured aliases. See `help speech` and `help message commands`.

Routing supports direct recipients, selected contents, AUDIBLE containers and
AUDIBLE exits, with C forwarding prefixes and exclusions. Distinct routes can
produce repeated delivery. The existing `mux.notify_recursion_limit` limits path
depth; existing Lua output entry/byte limits also bound native fan-out. Budget
failure discards staged output. Listener-only forwarding remains inactive, as in
this C fork. No listener callbacks or command queues are added.

`mux.world.pemit` shares this routing while retaining explicit string and Markdown
document formats. Forwarding adds typed prefixes without reparsing Markdown as
bracket markup. Every recipient session renders before Telnet encoding and MCCP2.
Pure messages do not write the database; SPEAK-lock mutations commit before output.

Compatibility details: say formatting has no comma before its quote; public
backslash emit remains available although `@emit` is Wizard-only; `: ` selects a
no-space pose; and C's `@fpose/nospace` switch retains default spacing. Wall `/admin`
has the same Wizard audience/authority as C and adds no new role.

## Command queues

Wizard-only `@force <target>=<commands>`, `@wait <seconds>=<commands>` and
`@halt [target]` use a bounded runtime queue. `@halt/all` removes every ready and
delayed list, including the unexecuted tail of a list. Configured aliases work;
`#<dbref> command` is not a native shorthand. See `help command queues`.

Force uses controlled-object matching and executes with the target's authority.
The causal actor is retained separately. Wait retains that cause and uses a
monotonic deadline; nonpositive delays are ready immediately. Literal command
lists split at unescaped, unnested semicolons using C brace/bracket/parenthesis
rules and `mux.space_compress`. `@wait` strips an outer pair of braces. There is
no expression evaluation. `@force` and `@wait` reject player-macro invocation;
queued commands themselves use the executor's current macros and permissions.

Every executor has `mux.command_queue_limit` outstanding lists (default 100),
including delayed and partially executed lists. Overflow cancels its pending
work and transactionally sets HALTED. This corrects the C zero-limit behavior
for non-player objects. `/all` also corrects C's incomplete ready-queue cleanup.
Each list is bounded by `runtime.input_line_limit`; retained command text across
all queues has a 16 MiB safety ceiling. Exceeding the byte ceiling rejects the
new list without halting its executor.

`mux.command_queue_active_chunk` and `mux.command_queue_idle_chunk` default to
10. They limit commands after network service and at maintenance/delayed wakeups,
respectively; each command yields a world-loop turn and executors rotate fairly.
The supplied game selects 100/200. Queue settings must be nonnegative. Zero
chunks disable the corresponding processing opportunity; a zero entry limit
rejects admission and triggers the executor's overflow halt.

Queued execution never borrows a player's connection. Connection-specific
commands (color, quit, help/reload, session diagnostics, `@lua` and
account-administration tooling) require interactive input. World commands,
including `@dbck` and `@shutdown`, and ordinary Lua commands run without a
descriptor. Background replies follow object notification and existing output
budgets. Ordinary player output is rendered separately for each connected session.

Admission, cancellation and overflow effects publish only after the originating
operation commits. Each executed command has its own transaction and callback
budgets; failures discard its mutations/output and consume the attempt, without
retrying or rolling back earlier commands. Purge, GOING, HALTED and incarnation
checks prevent stale execution. Queues survive disconnects and Lua reloads,
but remain outside database snapshots and are discarded on shutdown/restart.

## Interactive Lua flows

`mux.session.flow_start(descriptor, module, first_step)` starts an authenticated
session's interactive conversation. Try `flow-demo confirm`, `flow-demo menu`
or `flow-demo signup`; the supplied example runs unchanged. Prompts are private
to the selected connection, even when the player has multiple sessions.

Active flows consume all lines before normal commands, including blank lines and
`quit`. Each step decides how to finish or cancel. There is no implicit escape or
password/echo control. Disconnecting ends the flow.

Flow state and output are staged with world mutations. Successful saves precede
prompts; failed saves retain the prior step for retry. Script or validation errors
cancel the flow after rollback. Flow-only input does not write the database.
Successful Lua reload retains scratch data and resolves subsequent steps against
new code. Restart does not retain flows. See [Lua flow authoring](docs/lua-flows.md)
for contexts, limits, explicit targeting and hosted test behavior.
