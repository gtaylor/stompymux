# Global-removal refactor — structural review feedback

Review of the uncommitted changes against the ownership model described in
`docs/content/en/docs/concepts/source-layout.md`. Scope reviewed: the
composition root (`mux_server.c/.h`), the context/registry headers
(`world_context.h`, `command_context.h`, `command_invocation.h`,
`maintenance.h`, `server_registries.h`), the BTech shim
(`btech_context.c/.h`), and how the contexts thread through the code.

## Overall

This is a genuinely strong refactor. Killing `mudstate`/`mudconf` and replacing
them with named, dependency-ordered owners under an explicit composition root is
the right move, and the doc table in `source-layout.md` is an unusually honest
map of it. `WorldContext` in particular is exemplary — five borrowed pointers,
one narrow initializer, no back-reference to the world. That is the target
shape. The feedback below is mostly about places where the codebase has not
reached that bar yet.

## 1. `btech_context_active()` is a re-globalized singleton — the biggest wart

This is the load-bearing exception. 683 call sites across 62 files reach a
process-global `active_context` through `btech_context_active()`. Functionally
it is `mudstate`/`mudconf` back under a nicer name:

- The single-server assumption the refactor just removed is baked right back in
  by that `static BtechContext *active_context`.
- It is `assert`-guarded, so misuse is a crash rather than a type error — the
  whole point of threading owners is to make "who do I depend on" a compile-time
  fact.
- `btech_context_set_command()` mutates the active context's `command_context`
  with save/restore at three call sites (`command_queue.c:913/959`,
  `netcommon.c:899-916`). That save/restore dance is exactly the reentrancy
  hazard explicit contexts are supposed to eliminate.

The doc frames this as transitional ("older BTech callbacks whose signatures do
not yet carry a command context"), which is fair. But the transitional nature
should be load-bearing in the code, not just the prose:

- Give it a name that advertises the debt — `btech_context_current_UNTHREADED()`
  or similar — so every one of the 683 sites reads as a TODO.
- Track burn-down. 683/62 is the number to drive toward zero; a grep count in CI
  that ratchets down would keep it from becoming permanent.
- The mechanical path is clear: the callbacks that read `->configuration`,
  `->clock`, `->events`, `->database` do not need the *active* context, they need
  *a* context. Most of `mech.damage.c` / `mech.hitloc.c` read config flags —
  those could take a `const BtechContext *` parameter and the global would shrink
  to only the genuinely context-free entry points.

## 2. `MuxServer *` back-reference makes the composition root a god handle

`CommandContext`, `EvaluationContext`, `MaintenanceContext`, and `BtechContext`
all carry a `MuxServer *server`, and the code reaches through it 502 times — 264
in `mux/commands` alone. The top targets:

```
126  ->server->configuration
111  ->server->database
102  ->server->log
 51  ->server->channels
```

This quietly undoes the narrowing `WorldContext` achieves. Anything holding a
`CommandContext` can touch the entire server, so the dependency graph the doc
table describes is not enforced — it is aspirational. `configuration`,
`database`, and `log` are the three most-reached, and they are also the three
most broadly legitimate dependencies, so this is the highest-leverage cleanup:

- `CommandContext` already borrows a `WorldContext *world`, which owns `database`
  and `configuration`. Route `->world->database` / `->world->configuration`
  instead of `->server->...` and a big chunk of the 502 disappears without new
  plumbing.
- `log` is process-wide and effectively ambient; a borrowed `ServerLog *` on the
  contexts (as `MaintenanceContext` and `BtechContext` already have) is more
  honest than reaching through the root for it.
- Once those three are rerouted, whatever `->server->` remains is the genuinely
  broad set worth looking at individually.

Goal: no context should hold `MuxServer *` except the composition/maintenance
layer itself. The presence of that pointer is the tell that a module depends on
"everything."

## 3. Ownership vs. borrowing is invisible in the types

Every member is a raw pointer or an embedded value; the only place
owner-vs-borrow is recorded is the prose column "Passed to" in the doc.
`WorldContext` borrows all five of its members but is initialized identically to
structures that own theirs. A reader cannot tell `descriptors` (owned by
`MuxServer`, borrowed by `WorldContext`) from `database` (embedded, owned)
without cross-referencing the table.

A lightweight convention would carry this in-code: e.g. a `/* borrowed */`
marker on non-owning pointer members, or grouping borrowed pointers under a
comment banner as the initializer already does. Cheap, and it makes the destroy
functions auditable at a glance (a borrowed pointer must never be freed by that
struct).

## 4. The embed-vs-pointer split in `MuxServer` looks unprincipled

Some subsystems are by-value (`clock`, `channels`, `comsys`, `database`,
`events`, `persistence`, `macros`, `command_registry`, `world_indexes`,
`access_control`, `world`, `background_command`, `maintenance`, `log`, `btech`);
others are pointers (`descriptors`, `commands`, `lifecycle`, `login_throttle`,
`players`, `files`, `help`, `vattrs`, `lua`). The rule is not obvious. The
pointer ones get `*_create`/`*_destroy` returning a handle; the embedded ones get
`*_initialize` on a caller-provided slot. If there is a real distinction (opaque
type / heap-internal state vs. POD-ish) it is worth stating; if it is just
historical, converging on one style per category would make `mux_server_create`
far easier to read.

## 5. Construction/teardown robustness

`mux_server_create` interleaves initialization with two separate blocks of
ad-hoc null checks (lines 56-59 and 75-86), and `mux_server_destroy` does not
look symmetric with it:

- `persistence`, `comsys`, `runtime_clock`, `world_indexes`, `command_registry`,
  `access_control` are initialized but there is no matching destroy call. If they
  own no heap that is fine — but the asymmetry means a future field that *does*
  allocate will silently leak. A `*_destroy` per `*_initialize`, even if empty
  today, keeps the pairing enforceable.
- `game_database_destroy` (line 122) runs before `command_context_destroy`
  (line 123); worth confirming the background command's evaluation state holds no
  db references at teardown.
- The two-phase `server_lifecycle_shutdown` (110) ... other teardown ...
  `server_lifecycle_destroy` (126) split is subtle enough to deserve a comment on
  what must happen between the phases.
- `LuaRuntime **lua` in `MaintenanceContext` — a double pointer into the server's
  `lua` slot — is a lazy-init smell. It says "the thing I depend on does not
  exist when I am built." If Lua can be constructed in dependency order like
  everything else, a single `LuaRuntime *` would remove the indirection; if it
  genuinely cannot, that ordering constraint is worth a note in the table.

## Priority

If sequencing the follow-ups:

1. **Reroute `->server->{database,configuration,log}` through the contexts that
   already borrow them** — the cheapest large win, and it directly strengthens
   the documented model.
2. **Name + ratchet `btech_context_active`** — the most important for long-term
   health of the refactor, but a longer burn-down.
3. Items 3–5 are polish that make the invariants self-enforcing rather than
   doc-enforced.
