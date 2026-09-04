+++
title = "@flag"
description = "Set or clear an object flag"
keywords = ["@flag", "set flag", "clear flag"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# @flag

Set or clear a flag on an object:

```text
@flag <object>=<flag_name>
@flag <object>=!<flag_name>
```

The first form sets the named flag and the second clears it. Flag names are
case-insensitive; use full flag names rather than compact display letters. Use
`@list flags` to display the live flag list.

Only Wizards and God may use `@flag`, and the normal control rules apply to the
target. Flag-specific privilege rules still apply.
