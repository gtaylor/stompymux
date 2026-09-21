---
title: "Channel:set_object"
type: docs
---

Attaches an object that supplies channel locks and description, or detaches
the current object when passed nil.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid` when `object` is omitted, `mux.error.codes.channel.invalid`, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
Channel:set_object(object)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `object` | `DbRef\|Object\|nil` |  |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
