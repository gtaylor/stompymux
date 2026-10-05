---
title: "btech.unit.create"
type: docs
linkTitle: "create"
manualLinkTitle: "create"
---

Construct a persistent Mech or ground vehicle on an unused live thing. Transactional.

## Signature

```lua
btech.unit.create(dbref, name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `name` | `string` | Template reference: the file stem of a `.toml` document anywhere under database.unit_database. |

## Returns

- `boolean`
