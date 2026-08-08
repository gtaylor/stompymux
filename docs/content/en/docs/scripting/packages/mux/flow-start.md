---
title: mux.flow_start
linkTitle: mux.flow_start
type: docs
weight: 212
---

# `mux.flow_start`

Attaches an [interactive flow](../../../flows/) to a descriptor and shows its
first prompt.

## Function

### Synopsis

```lua
mux.flow_start( descriptor, module, first_step )
```

### Arguments

`number descriptor`
: A live descriptor ID, normally `ctx.descriptor`.

`string module`
: The flow module path, resolved like `require`.

`string first_step`
: A key in the module's `flows` table.

### Returns

Nothing.

## Examples

```lua
mux.flow_start(ctx.descriptor, "confirm_delete.lua", "confirm")
```

## Notes

Raises an error if the descriptor does not exist, already has a flow, or the module lacks the requested step. This function is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [Flows](../../../flows/)
