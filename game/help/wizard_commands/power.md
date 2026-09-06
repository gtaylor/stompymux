+++
title = "@power"
description = "Grant or remove an object power"
keywords = ["@power", "grant power", "remove power"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @power

Grant or remove a power:

```text
@power <object>=<power>
@power <object>=!<power>
```

The first form grants the named power and the second removes it. Power names
are case-insensitive. The current MUX server has one registered power,
`IDLE`; use `@list powers` to display the live list.

Only Wizards and God may use `@power`, and the normal control rules apply to
the target. Setting `IDLE` on a player prevents the inactivity timer from
disconnecting that player. Wizards and God already receive that exemption
implicitly.
