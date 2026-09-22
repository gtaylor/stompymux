---
title: "btech.map.emit"
type: docs
linkTitle: "emit"
manualLinkTitle: "emit"
---

Deliver a cockpit message to occupants of running units using the shared transactional emitter.

## Signature

```lua
btech.map.emit(map, message, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `map` | `DbRef\|Object` |  |
| `message` | `string` | One through 8191 bytes; leading spaces are removed. |
| `options?` | `BattleMapEmitOptions` |  |

## Returns

No values.
