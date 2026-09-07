# Logging and script logs

Server diagnostics use `[logging.topics]` and `[logging].log_options`. Topics
follow the C registry and compiled defaults. A combined event is printed once if
any of its categories is enabled. `buffer_alloc` is retained for inspection but
has no Rust allocator instrumentation. BattleTech logging is not implemented.

Headers use local time, the game name and C category/subcategory alignment.
`timestamp`, `flags` and `location` control decorations. Styling is stripped and
control characters are escaped in diagnostics. Fatal process errors remain visible
independently of topic configuration. A bounded worker handles blocking output;
queue overload omits diagnostics and reports an omission count when output resumes.

`@admin all_commands=yes` (and the other topic directives) applies immediately.
`@admin log_options=!location flags` applies edits in order, retaining successful
edits when another token is invalid. Runtime edits are lost on restart.
`@list logging` reports effective categories and decorators; its default permission
is GOD. `@list logfiles` is Wizard-only and reports actual cached handles, sorted
lexically, with seconds remaining before idle expiry. Existing topic ACLs apply.

## Explicit file appends

Wizard-only `@log <filename>=<message>` accepts no switches. It replies
`Message logged.` only after the append succeeds; failures reply `Request failed.`.
An empty message replies `Nothing to log!`; an empty filename with a message replies
`Invalid logfile.`. Configured command aliases work normally.

Operators must create readable/writable regular files in `game/logs` beforehand.
Files and that directory cannot be symlinks. Names must be 1–200 UTF-8 bytes,
without `/`, `..` or NUL. Files are never created by the server. Each message is
limited to 4094 UTF-8 bytes plus a newline; truncation keeps valid UTF-8. Files
are appended, with no rotation or automatic retry. Partial failure can leave a
partial append; a timeout never produces a success reply.

Handles expire after 300 idle seconds. The cache uses `runtime.max_connections`
as its maximum and evicts the least recently used handle. Queue capacities and
write/shutdown deadlines reuse existing runtime settings. Shutdown drains accepted
writes and closes handles; failure to finish draining makes shutdown unsuccessful.

## Lua transactions

`mux.log(filename, message)` stages a file request in the current transaction.
`true` means accepted for commit, **not written**. Invalid filenames return false;
invalid argument types or NUL bytes raise a Lua error. Empty messages are successful
no-ops. Checking VMs and calls outside an active transaction reject this API.
Requests share Lua output count/byte budgets with staged messages.

Nested callbacks, commands, flows, schedules and module reload rollbacks discard
staged requests. After successful persistence they are submitted in order. A later
file error is diagnosed and cannot undo an already committed world. Logging-only
operations do not write the database. Queue overload may omit a committed request
with a diagnostic; script logs are not a durable delivery queue.

## Command auditing

`all_commands`, `suspect_commands` and `bad_commands` control stderr auditing.
Attempts are recorded even when execution fails. Actor, cause, session and optional
location/flags are captured before execution. SUSPECT is the object flag.
SUSPECT commands also go to an existing `SuspectsLog` channel independently of
stderr switches. The channel is never created automatically; its persistence is
separate from the command, and audit failure does not prevent execution.

Password arguments are redacted for account creation/reset/change commands,
including aliases and expanded player macros. Queue/force wrappers and macro
definitions are conservatively redacted in full; executed queued commands are
classified again. Unclassifiable expansion also hides arguments. Password prompts
and interactive-flow responses are not command audit inputs. Normal command text
can still contain private information, so enable all-command auditing deliberately.
