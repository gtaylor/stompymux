---
title: "mux.names.person"
type: docs
linkTitle: "person"
manualLinkTitle: "person"
---

Generates one person's name in the spec's culture. For the same spec it does not
overlap with `mux.names.roster`, so a commander and their
roster can share a seed.

Raises `mux.error.codes.arg.invalid` for a
malformed spec.

## Signature

```lua
mux.names.person(spec)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `spec?` | `NameSpec` |  |

## Returns

- `PersonName person`

## Related errors

- `mux.error.codes.arg.invalid`
