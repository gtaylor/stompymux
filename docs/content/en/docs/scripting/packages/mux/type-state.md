---
title: State
linkTitle: State
type: docs
weight: 120
---

# `State`

A `State` handle accesses persistent values belonging to one named subsystem on
one object. Create a handle with [`Object:state`](../object-state/).

Namespaces and keys are exact and case-sensitive. They begin with an ASCII
letter and may contain letters, digits, `_`, `-`, `.`, and `/`; keys are at most
255 bytes. Values retain their Lua scalar type across reloads and restarts.
Supported values are strings, booleans, integers, and finite numbers. Empty
strings are values; `nil` deletes a value.

Every callback has an implicit transaction. Reads observe earlier writes from
that callback, including writes to multiple objects. A successful callback
commits them together; a Lua error or memory-limit error discards them. The
configured per-value, per-object entry, and per-object byte limits are enforced
before data leaves the Lua VM.

## Methods

| Method | Description |
| --- | --- |
| [`State:get`](../state-get/) | Gets a value or a caller-supplied default. |
| [`State:has`](../state-has/) | Tests whether a key exists. |
| [`State:set`](../state-set/) | Sets or deletes a value. |
| [`State:delete`](../state-delete/) | Deletes a value and reports whether it existed. |
| [`State:keys`](../state-keys/) | Lists keys in sorted order. |
| [`State:entries`](../state-entries/) | Lists key/value records in sorted order. |
| [`State:get_many`](../state-get-many/) | Gets all present keys from a requested set. |
| [`State:set_many`](../state-set-many/) | Applies a table of updates. |

## Notes

Do not attempt to hold a transaction across flow steps or other player input;
persist an explicit reservation instead.

## See Also

- [`mux`](../)
- [`Object:state`](../object-state/)
