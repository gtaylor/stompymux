+++
title = "@state/set"
description = "Set or clear a persistent state value"
keywords = ["@state/set"]
article_tags = ["state_switches"]
wizard_only = true
+++

# @state/set

Set a persistent object-state value as a Wizard:

```text
@state/set <object>/<namespace> <attribute_name>=<value>
```

An empty value deletes the attribute. Use `""` to store a zero-length string.
The unquoted values `true` and `false` are booleans; integer and finite decimal
values retain their numeric types. Other unquoted values are strings. Quote a
value to force it to be a string, such as `"123"`.

Quoted strings support `\"`, `\\`, `\n`, `\r`, `\t`, and `\xNN` escapes.
Namespace and attribute names are case-sensitive.
