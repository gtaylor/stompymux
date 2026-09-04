+++
title = "Exit access policies"
description = "Restrict default exits with persistent state"
keywords = ["exit access", "exit policy", "traverse lock", "locks.traverse"]
wizard_only = true
+++

# Exit access policies

Exits using the bundled `default_exit.lua` parent can be restricted through
the `locks.traverse` persistent-state namespace. An exit without requirements
allows traversal. When several requirements are present, the traversing object
must satisfy all of them. Wizards do not implicitly bypass these policies.

Use `@state/set` to add requirements:

```text
@state/set <exit>/locks.traverse flag/WIZARD=true
@state/set <exit>/locks.traverse affiliation=123
@state/set <exit>/locks.traverse state/access/member=true
```

`flag/<FLAG>` takes a boolean and requires the named flag to have that state.
This is the only requirement that can express a negative condition: for
example, `flag/SUSPECT=false`. Flag names must be canonical uppercase names.

`affiliation` takes the integer dbref of a live object and compares native
affiliation object identity. Do not include the `#` prefix.

`state/<namespace>/<key>` compares a persistent state value on the traversing
object. Boolean, number, and string types remain distinct, so `7` does not
match `"7"`. Policy-referenced state namespaces and keys cannot contain `/`;
use letters, digits, dots, dashes, or underscores instead.

Customize failure messages with string values:

```text
@state/set <exit>/locks.traverse message/enactor="The checkpoint denies you."
@state/set <exit>/locks.traverse message/others="is stopped at the checkpoint."
```

Messages alone do not restrict an exit. Empty strings suppress the
corresponding message. Delete any policy entry by assigning an empty value, or
remove the complete policy with `@state/wipe <exit>/locks.traverse`.

Malformed entries, unsupported flag names, invalid affiliation dbrefs, and
incorrectly typed values fail closed and are reported as Lua
callback errors.
