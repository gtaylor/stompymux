# Reports, logging, and help audit

Audited C baseline `2bbbe6fc` against Rust baseline `74246d9` plus the
corrections below. The scope covers the non-BattleTech `@who` and `@session`
reports, report permissions and delivery, logging categories and live controls,
command auditing, `@log` and Lua log persistence, help discovery, metadata,
visibility, reads, and reloads. Account history and `@last` were audited
separately.

No production database, game tree, log, or running server was changed. Rust
execution used copies of `tests/fixtures/game`, temporary SQLite databases,
temporary help trees, temporary pre-created log files, and loopback TCP. C
evidence here is source tracing through active entry points; the previously run
C baseline suite passed 32 tests, including its help and log coverage, but this
tranche did not launch another C server for a paired protocol transcript.

## Connection reports

The C paths are `dump_users`, `dump_sessions`, `time_format_1`, and
`time_format_2` in `btmux-khi/src/mux/network/connection_commands.c:70-300`,
with the ten-minute constant in `server_config.h:329`. Rust uses
`src/server/operations.rs`, `src/server.rs:687`, `src/operations/mod.rs`, and
`src/telnet/diagnostics.rs`.

The reports retain the selected C behavior: Wizard command permission, active
connection requirement for `@who`, case-insensitive name-prefix filtering,
one row per authenticated session, DARK/SUSPECT markers, location and accepted
command count, private delivery to the invoking session, record-player count,
and duration units. Rust escapes terminal controls, sizes Unicode columns by
display width, uses stable server session identifiers in the administrative
report, and bounds delivery; these are safety and transport adaptations rather
than raw descriptor-memory exposure.

`OP01` corrected idle display. C shows the actual idle duration in `@who`, but
maps values through 600 seconds to zero only in `@session`. Rust previously
applied that hiding inside the shared formatter, causing `@who` to report `0s`
for an actually idle connection. `idle_time` now formats the supplied duration,
and `session_idle` applies the administrative-report rule at its call site.

`OP02` corrected the footer and Lua session maximum. C renders `no maximum`
only when `max_players == -1`; Rust previously treated every negative value as
unlimited in the two text reports and omitted other negatives from Lua session
state. The text reports and Lua projection now preserve non-`-1` values. The
separate admission-policy validation belongs to configuration/admission rather
than this display audit.

## Logging and command audit

The C category catalog and live controls are in
`btmux-khi/src/mux/server/configuration_registry.c:199-221` and
`command_list.c:481`; record generation and arbitrary files are in
`server/log.c:130-340`. Command audit ordering is in
`commands/command_dispatch.c:404-470`. Rust uses `src/logging`,
`src/config/administration.rs`, the server command entry points, and the shared
`commands::executable` predicate.

All sixteen C event switches and the flags/location/timestamp decorators remain
live configurable, visible through `@list logging`, and protected by the same
Wizard/God command and configuration gates. `@list logfiles` is Wizard-only and
reports the worker-owned open-handle snapshot. Buffer allocation remains a
catalogued switch with an explicit notice that Rust has no allocator producer.

`OP03` corrected pre-audit rejection. C rejects invalid, GOING, and disallowed
HALTED executors before emitting CMD audit records or `SuspectsLog` traffic;
interactive halted Players are the exception. Rust server entry points now use
the same executable predicate before auditing, while command execution retains
the player-facing halted-object message. A loopback regression verifies that a
GOING suspect cannot add a channel audit message.

`OP04` corrected queued macro classification. C expands dot macros only for
interactive input. Rust's privacy classifier expanded them for queued work even
after queued dispatch correctly treated the text literally. Audit
classification now receives the input origin, so queued `.pw` remains the
literal redacted command rather than being labelled with an interactive macro
expansion.

`OP06` corrected safe entered-text retention. C emits CMD audit before command
aliases and interactive macros are expanded. Rust still expands a temporary
copy to recognize secret-bearing commands, but nonsecret input now records the
cleaned original line. Thus `@fi Foo` and `.safe` remain those entered forms in
both stderr and `SuspectsLog`; secret classes retain the approved redacted
canonical label.

The existing approved logging protections remain: secrets are conservatively
redacted, cause and session identity decorate audit records, Lua writes stage
until transaction commit, queues and records are bounded, controls and styles
cannot forge physical diagnostic lines, and logfile access is confined to an
existing regular file under the configured log directory. `@log` waits for the
worker result before reporting success. These deliberately strengthen C's raw
command logging and pathname/open behavior.

## Help discovery, visibility, and reload

The C path is `help_locate_frontmatter`, directory traversal, keyword building,
and live body reading in `btmux-khi/src/mux/help/help_index.c:185-715`, metadata
coercion in `help_frontmatter.c:28-132`, visibility and generated indexes in
`help_render.c`, and command dispatch in `help_command.c`. Rust uses
`src/help.rs`, `src/help/render.rs`, and the `Action::Help` and
`Action::HelpReload` branches in `src/server.rs`.

Exact keyword lookup, substring suggestions, default `index.md`, duplicate
first-wins ordering, case folding, Wizard-only lookup and index visibility,
generated tag indexes, weights, the two index styles, live body reads, and
metadata-only reload are covered. The approved relative article-path fallback
remains available after exact keyword lookup. Reload traversal is lexical and
bounded at the C depth of 64. The approved atomic reload behavior also remains:
a missing or unreadable root, unreadable directory entry, or over-depth subtree
rejects the candidate and retains the previously installed index instead of
publishing C's empty or partial rebuild.

`OP05` corrected frontmatter compatibility. Required title and description must
be strings but may be empty; keywords must be a nonempty array, with nonstring
members coerced to empty strings. Malformed optional arrays, weight, and
wizard-only values are ignored, optional-array nonstrings become empty strings,
and an unknown `index_style` warns and defaults. Rust's derived deserializer and
extra nonempty checks had rejected each of those otherwise accepted articles.
Opening delimiters accept C's trailing spaces, tabs, and carriage return, and
body extraction removes only the delimiter's line ending.

Rust continues to validate and render bodies as bounded Markdown with safe
command links and complete terminal/HTML chunks. This audit establishes the
lookup, visibility, reload, and selected rendered-index contracts; it does not
claim byte-for-byte equivalence for every Markdown construct or terminal width.

## Executed Rust evidence

- `cargo test --test help`: seven corpus, index, visibility, live-read/reload,
  permissive-metadata, Markdown-link, and bounded-render tests passed. The
  supplied tree indexed 100 articles and resolved all 246 declared keywords.
- `cargo test --test operations`: four registry/permission, report layout,
  idle/footer, and loopback private-delivery/count tests passed.
- `cargo test --test logging`: five category/configuration, redaction/origin,
  safe-file/cache, transactional Lua, and loopback persistence/failure tests
  passed.
- `cargo test --test telnet_extensions`: eight transport and duration-format
  tests passed.
- `cargo check` and `git diff --check` passed for the combined working tree at
  the end of this tranche.

## Remaining limits

The audit did not inject a wall-clock idle interval into both server binaries in
one paired run, and it did not force every operating-system logfile replacement
race. Rust's asynchronous bounded logger, staged Lua effects, terminal-control
escaping, secret redaction, session/cause decoration, regular-file and path
containment checks, safe Markdown/Unicode rendering, and resource bounds are
approved or established safety adaptations and were not removed. Relative
article-path lookup and atomic retention on fatal help traversal failure are
explicit approved help differences from C.
