---
title: "Channel:object"
type: docs
linkTitle: "object"
manualLinkTitle: "object"
---

Returns the object that supplies the channel description and locks.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.object.invalid`.

## Signature

```lua
Channel:object()
```

## Parameters

None.

## Returns

- `Object? object The attached object, or nil when none is attached.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.object.invalid`
