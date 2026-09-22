---
title: "Channel:boot_player"
type: docs
linkTitle: "boot_player"
manualLinkTitle: "boot_player"
---

Announces a God-administered boot and removes a current member's channel
aliases using the native side-effect path.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.object.invalid`.

## Signature

```lua
Channel:boot_player(object)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `object` | `DbRef\|Object` | Current channel member. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.object.invalid`
