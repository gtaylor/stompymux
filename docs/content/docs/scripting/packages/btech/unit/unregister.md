---
title: "btech.unit.unregister"
type: docs
linkTitle: "unregister"
manualLinkTitle: "unregister"
---

Remove the object's BattleTech registration and forget its configuration
references. Rust extension without a C Lua counterpart: the reference exposes
teardown only through the native wizard command, and this binding shares that
command's teardown exactly. Succeeds silently for an already-plain object and
never moves or destroys the container thing; mutations join the surrounding
callback transaction.

## Signature

```lua
btech.unit.unregister(unit)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `unit` | `DbRef\|Object` | Live thing to tear down. |

## Returns

- `boolean true`
