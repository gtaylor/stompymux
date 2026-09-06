+++
title = "@state/examine"
description = "Inspect an object's persistent state"
keywords = ["@state/examine"]
article_tags = ["state_switches"]
wizard_only = true
+++

# @state/examine

Inspect persistent state as a Wizard:

```text
@state/examine
@state/examine <object>
@state/examine <object>/<namespace>
```

With no argument, the command lists each state namespace on your current
location and the number of values in it. Supply an object to show that object's
summary instead. Summary output includes the syntax for drilling into a
namespace.

Append a namespace to the object with `/` to list its keys, scalar types, and
values. Strings are quoted and control bytes are escaped. Namespace and key
names are case-sensitive.

Only Wizards can use `@state/examine`. Wizards may inspect any object.
