---
title: Scheduled events
linkTitle: Scheduled events
description: How to run object-attached and global Lua logic on UTC cron schedules
type: docs
weight: 23
---

Object and global Lua modules may define a `schedules` array. Each entry runs a
handler according to a five-field UTC cron expression:

```lua
return {
  schedules = {
    {
      name = "hourly_maintenance",
      cron = "0 * * * *",
      handler = function(ctx)
        -- Scheduled work goes here.
      end,
    },
  },
}
```

Every entry must have a nonempty `name`, a valid `cron` string, and a
`handler` function. Names must be unique within the module. A module may mix
schedules with commands or flows; object modules may also provide their
object-specific hooks.

## Cron expressions

The five numeric fields are minute, hour, day of month, month, and day of week:

```text
minute hour day-of-month month day-of-week
```

The accepted ranges are `0-59`, `0-23`, `1-31`, `1-12`, and `0-6`, with
Sunday represented by `0`. Fields accept `*`, individual values, comma-separated
lists, inclusive ranges, and `/` steps. For example, `*/15 * * * *` matches
every fifteen minutes and `0 9 * * 1-5` matches 09:00 UTC Monday through
Friday.

When both day of month and day of week are restricted, a match in either field
is sufficient, following traditional cron behavior. All other fields must
match.

Matching jobs are spread deterministically across seconds 0 through 54 of the
minute. The offset is derived from the module path, schedule name, target
object, and matching minute. This avoids running every handler at once while
keeping execution predictable. A missed minute is not replayed after downtime
or a delayed scheduler tick.

## Object-attached schedules

Put an object schedule in a module under `game/lua/object_logic` and attach the
module with `@lua/parent`. A matching entry runs once for every live object
directly attached to that module path. Attachments are not inherited, so only
direct users of the module create jobs.

```lua
return {
  schedules = {
    {
      name = "hourly_notice",
      cron = "0 * * * *",
      handler = function(ctx)
        -- ctx.scope == "object"
        mux.world.pemit(ctx.object, "Another hour has passed.")
      end,
    },
  },
}
```

The handler receives `ctx.scope == "object"`, the attached object in
`ctx.object`, the entry name in `ctx.schedule`, and its expression in
`ctx.cron`. There is no live descriptor. `ctx.enactor` and `ctx.cause`
identify God because the work is initiated by the server rather than a player.
If the object is destroyed or marked for destruction before the delayed job
runs, that invocation is skipped.

## Global schedules

Put a global schedule in any discovered module under `game/lua/global_logic`.
A matching entry runs once for that module, independent of how many objects
exist:

```lua
return {
  schedules = {
    {
      name = "nightly_cleanup",
      cron = "0 3 * * *",
      handler = function(ctx)
        -- ctx.scope == "global"
      end,
    },
  },
}
```

The handler receives `ctx.scope == "global"`, the entry name in
`ctx.schedule`, and its expression in `ctx.cron`. `ctx.object`, `ctx.enactor`,
`ctx.cause`, and `ctx.descriptor` are `nil` because no object or player
initiated the job.

## Errors, validation, and inspection

A handler error is logged with the module path and schedule name; it does not
stop other scheduled jobs. Use the wizard-only [`@lua/check`](validating-and-reloading/)
command to catch invalid entries, cron expressions, and duplicate names before
reloading.

Use `@lua/schedule` to list object modules that define schedules, their directly
attached object counts, and global modules that define schedules. Pass an
object to inspect its direct module, an `object_logic`-relative path to inspect
that module and its attached objects, or `global_logic/<path>.lua` to inspect a
global module.
