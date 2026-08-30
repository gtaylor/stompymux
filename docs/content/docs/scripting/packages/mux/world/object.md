---
title: object
type: docs
toc_hide: false
---

Creates a validated handle for a native database [Object](../type-object/).
Since this function raises errors, it's best to wrap its invocation in a
[pcall()](https://www.lua.org/pil/8.4.html) (see below).

Once you've retrieved an [Object](../type-object/), its methods provide access
to native metadata and handles for features such as [State](../type-state/).

## Function

### Synopsis

```lua
mux.world.object( dbref )
```

### Arguments

`number or Object dbref`
: A live database reference or an existing object handle.

### Returns

`Object object`
: A handle for the referenced object.

### Raises

Raises an error with code `mux.object.invalid` when passed an invalid dbref.

## Examples

The simplest form of object retrieval involves naively (without regards to errors)
attempting a retrieval:

```lua
local object = mux.world.object(ctx.object)
mux.world.pemit(ctx.enactor, object:name())
```

`pcall` returns the object handle after the success flag when the database
reference is valid:

```lua
local ok, object = pcall(mux.world.object, ctx.object)

assert(ok)
mux.world.pemit(ctx.enactor, object:name())
```

Invalid database references raise a structured Lua error rather than returning
`nil`:

```lua
local ok, err = pcall(mux.world.object, 999999)

assert(not ok)
assert(err.code == "mux.object.invalid")
assert(err.detail.argument == 1)
```

## See Also

- [`mux`](../../)
- [`Object`](../type-object/)
