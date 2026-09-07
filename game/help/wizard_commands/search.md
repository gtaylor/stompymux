+++
title = "Object searches"
description = "Find controlled objects or search the full database"
keywords = ["@find", "@search", "object search"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# Object searches

```text
@find [name][,low[,high]]
@search [class=criteria[,low[,high]]]
```

Both commands require Wizard access, honor configured aliases and deliver results
automatically in chunks. There is no continuation command. Bounds are inclusive
and accept optional `#`; invalid bounds default to zero and the database maximum.
Reversed bounds find nothing. Searches do not invoke Lua or write the database.

`@find` lists controlled objects except exits and garbage, in dbref order. Names
match case-insensitive prefixes of the name or any word. Wizards cannot list GOD
or other Wizards through this command; GOD can. DARK and GOING are not excluded.
The report ends with `***End of List***`.

`@search` searches all supported stored objects, including other Wizards, exits and
garbage. Classes are `name`, `rooms`, `exits`, `objects`/`things`, `players`, `type`,
`flags`, `power` and `zone`. Class abbreviations follow C ordering: `p` means
players, while `po` means power. Names match prefixes of the entire style-stripped
name, with literal wildcard characters. Bare `@search` matches all objects; an
explicitly empty name criterion such as `name=` matches nothing.

```text
@search rooms=Staff
@search type=players
@search flags=PW
@search power=idle
@search zone=#4
@find ,#10,#100
```

Flags use case-sensitive display letters, optionally mixed with type letters.
Every requested flag must match; the last type letter wins. Power names are
case-insensitive. Zone targets use native object matching without evaluating locks.
Search results are grouped by type, with exit endpoints, player locations and
per-type totals. Empty results report `Nothing found.`

Reports exceeding the aggregate output budget mark omitted rows explicitly.
Search totals still cover every match. Narrow the criteria or dbref range to
retrieve smaller reports. Unallocated gaps do not produce search rows.
