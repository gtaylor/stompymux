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

Hexadecimal numeric forms such as `0x1.8p2` are also accepted. Binary strings,
including NUL and non-UTF-8 bytes, survive saving and restart. Inspection displays
those bytes as escapes and does not interpret bracket markup.

Namespaces have a 127-byte limit and keys a 255-byte limit. Both start with an
ASCII letter; subsequent characters may be letters, digits, `_`, `-`, `.`, or
`/`. State limits also apply to values, entry counts and total per-object bytes.
Failed changes leave the previous state intact.
