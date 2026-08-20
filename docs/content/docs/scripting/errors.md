---
title: Lua errors
description: Structured errors for Lua scripts and native bindings
type: docs
weight: 34
---

Lua errors use values with a stable `code`, readable `message`, optional
`detail`, and optional `cause`. The complete API, value type, and native code
catalogue are in the [`mux.error` package reference](packages/mux/error/).

Use raised errors for contract violations such as invalid arguments, dead
handles, and limits. For a failure the caller is expected to recover from,
return `nil, err` instead.

```lua
local ok, err = mux.error.pcall(function()
  mux.error.raise(
    mux.error.codes.state.value_too_large,
    "state value exceeds 4096 bytes",
    { limit = 4096 }
  )
end)
if not ok and err:is(mux.error.codes.state) then
  mux.notify(ctx.enactor, err.message)
end
```

Plain strings remain supported for existing scripts. Authors can build their
own checked symbols with [`mux.error.namespace`](packages/mux/error/namespace/):

```lua
local codes = mux.error.namespace("cargo", { "full", "bay.locked" })
mux.error.raise(codes.full, "the cargo bay is full")
```

See [`Error`](packages/mux/error/type-error/) for fields, matching, cause chains,
and callback reporting. See [`mux.error.codes`](packages/mux/error/codes/) for the
native constants, including `mux.unavailable.checking` for operations that
cannot run during `@lua/check` and `btech.failed` for BattleTech script
failures.
