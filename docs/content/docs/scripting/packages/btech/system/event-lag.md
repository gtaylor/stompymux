---
title: event_lag
type: docs
toc_hide: false
---

Returns the current BattleTech event-processing lag.

## Function

### Synopsis

```lua
btech.system.event_lag(  )
```

### Arguments

None.

### Returns

`integer percent`
: The event-processing lag as a percentage. `0` means processing is on
  schedule; `100` means elapsed wall time is twice the processed tick time.

## See Also

- [`btech`](../../)
- [`btech.system`](../)
