---
title: "mux.comsys.list_channels"
type: docs
linkTitle: "list_channels"
manualLinkTitle: "list_channels"
---

Lists every live communication channel in case-insensitive name order, with
original spelling used as the tie-breaker.

Raises `mux.error.codes.unavailable.checking`
or `mux.error.codes.internal` if the native
registry count changes while it is copied.

## Signature

```lua
mux.comsys.list_channels()
```

## Parameters

None.

## Returns

- `Channel[] channels`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.internal`
