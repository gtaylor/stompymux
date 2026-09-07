+++
title = "Destruction and cleaning"
description = "Schedule object destruction and control automatic database cleaning"
keywords = ["@destroy", "@enable", "@disable", "@list globals", "cleaning"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# Destruction and cleaning

Wizards use `@destroy <target>` to mark a controlled object GOING. The object
begins to crumble; the next successful database check completes destruction.
`@destroy/override <target>` bypasses SAFE. It cannot bypass protection for #0,
GOD, Wizard players or configured starting/home destinations. Other switches,
including `/recursive`, are unsupported.

Cleaning evacuates surviving occupants, removes owned records and disconnects
all sessions of destroyed players after the transaction commits. Purged dbrefs
remain Garbage tombstones and are never reused. GOING survives restart. Where
flag permissions allow, `@flag <target>=!going` cancels pending destruction.

Automatic cleaning starts enabled on every restart. The initial delay is
`mux.check_offset` seconds (default 300), or `mux.check_interval` when the offset
is zero. Subsequent attempts use `mux.check_interval` seconds (default 600).
Failed checks roll back and wait until the next interval.

- `@disable cleaning` pauses automatic checks.
- `@enable cleaning` resumes them; an overdue check becomes eligible immediately.
- `@list globals` shows whether cleaning is enabled.
- `@dbck` checks immediately, even when automatic cleaning is disabled.

`cl` abbreviates `cleaning`. Controls are runtime-only, do not edit TOML, and
have no switches. The same commands support `idlechecking`, `queueing`, and
`logins`; see `help runtime controls`. Checkpointing is unsupported. Automatic checks
log their summaries to server diagnostics; ordinary destruction notifications
and relocation callbacks still run. No prompt requests confirmation.
