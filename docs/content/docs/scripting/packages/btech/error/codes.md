---
draft: true
title: btech.error.codes
type: docs
toc_hide: false
---

Provides checked symbols for native `btech.*` error codes.

## Constants

| Symbol | String value | When raised |
| --- | --- | --- |
| `btech.error.codes.unavailable` | `btech.unavailable` | A `btech` operation runs during `@lua/check`. |
| `btech.error.codes.failed` | `btech.failed` | A BattleTech domain check fails, such as a wrong object kind, or a legacy scripting handler reports failure. |

`btech.error` carries codes only. Use [`mux.error`](../../../mux/error/) to
create, raise, wrap, or inspect errors. BattleTech code uses
`mux.error.codes` for generic failures such as invalid arguments.

## Examples

```lua
mux.error.raise(btech.error.codes.failed, "BattleTech operation failed")
```

## See Also

- [`btech.error`](../)
- [`mux.error`](../../../mux/error/)
- [`mux.error.codes`](../../../mux/error/codes/)
