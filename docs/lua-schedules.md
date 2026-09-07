# Scheduled Lua callbacks

Both global and object-logic modules can export a `schedules` array:

```lua
return {
  schedules = {
    {
      name = "hourly_maintenance",
      cron = "0 * * * *",
      handler = function(ctx)
        -- Global callbacks have no object, enactor, cause or descriptor.
        assert(ctx.scope == "global")
      end,
    },
  },
}
```

Declarations require a nonempty string name, a valid cron string and a function
handler. Names are case-sensitive and unique within a module, but can repeat
across modules. Metadata and functions are captured at startup; editing files or
module tables does not re-register them. Use `@lua/reload` or restart to apply changes. Module-loading
errors identify the module and declaration. Empty schedule arrays are allowed.

Cron has five numeric fields: minute (0–59), hour (0–23), day of month (1–31),
month (1–12), weekday (0–6, Sunday zero). Matching uses UTC, independent of the
host's local timezone. Fields support `*`, comma lists, inclusive ranges and
positive `/step` values. A step starts at the beginning of its range; `5/10`
matches only 5. Names, macros, seconds fields and weekday 7 are unsupported.
Expressions must be shorter than 256 bytes.

Day-of-month and weekday use OR when neither field is literally `*`; otherwise
both must match. Consequently `*/1` and `*` can have different day-field effects.
Every element is validated, even when an earlier list element already matches.

The first observed minute is skipped. On each newly observed minute, the runtime
collects matching global declarations and declarations attached directly through
objects' Lua parents. New objects and parent changes affect the next collection;
already queued jobs retain their original module. Garbage and GOING objects are
excluded at collection and execution. HALTED and NO_COMMAND do not suppress
schedules. Object incarnation checks reject jobs for invalidated provisional
objects.

Each job receives a deterministic offset from 0 to 54 seconds within its matching
minute. The hash includes relative module path, schedule name, object identity
(or -1 for globals), and UTC minute. A delayed job may run only before the next
minute boundary. Missed minutes are not replayed, backward clock changes do not
repeat collected minutes, and failures are not retried. The queue is not durable.
Jobs with equal due times retain global-module lexical order followed by ascending
object dbrefs, with declaration order within each module/object.

Every callback receives `event = "schedule"`, `schedule`, the original `cron`,
and an empty `args` array. Global callbacks have `scope = "global"` and no
object/enactor/cause. Object callbacks have `scope = "object"`, their object's
dbref, and GOD (#1) as enactor and cause. Neither has a descriptor, command or
subject. Return values are ignored.

Scheduled callbacks run serially on the world owner with the normal Lua memory,
instruction, state and output limits. Each callback has its own transaction:
world validation and database persistence succeed before messages are delivered.
Failure restores world mutations and discards staged messages, while maintaining
session-owned CONNECTED flags. Lua globals are not persistent world state and
are not rolled back. Diagnostics identify the failed module, schedule and object.
Other jobs continue, with a runtime yield between jobs. Shutdown cancels queued
work rather than draining it.

Wizards can inspect registrations without running scripts or writing the database:

```text
@lua/schedule
@lua/schedule #2
@lua/schedule counter.lua
@lua/schedule global_logic/example.lua
```

The overview lists global modules and object modules with attached objects.
Module detail shows names and cron expressions; an object-module path also lists
its attached objects. Object targets use the normal administrative matcher.
Reports are private to the invoking session, escaped and bounded; oversized
reports explicitly indicate truncation. `@lua` lists the implemented switches.
Successful `@lua/reload` clears old queued jobs while retaining the observed
minute high-water mark. Failed reloads leave jobs intact. Neither case backfills
work or recollects a minute.

The implementation separates cron parsing, captured registrations, the transient
queue and read-only inspection under `src/lua/schedules`. The server observes its
schedule clock during maintenance and commits one due job per world-loop turn.
The embedding API `run_with_schedule_clock` supports deterministic TCP tests;
there is no client command or configuration option for changing server time.
