+++
title = "@attribute"
description = "Inspect and update native object attributes"
keywords = ["@attribute", "@attribute/get", "@attribute/examine", "@attribute/set"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @attribute

Inspect or change supported native attributes. Dynamic Lua-backed values use
`@state` instead.

```text
@attribute/get <object>/<attribute>
@attribute/examine <object>
@attribute/set <object>/<attribute>=<value>
```

`@attribute/examine` shows every supported attribute, including unset ones.
Only BattleTech-native attributes are available. Object descriptions use the
dedicated `@description` and `@internal-description` commands. Set `Xtype`
before enabling XCODE; valid values include `MECH`, `MECHREP`, `MAP`, `DEBUG`,
`AUTOPILOT`, and `TURRET`.
