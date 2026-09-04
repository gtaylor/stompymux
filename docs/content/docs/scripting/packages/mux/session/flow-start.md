---
title: flow_start
type: docs
toc_hide: false
---

Attaches an [interactive flow](../../../../flows/) to a descriptor and shows its
first prompt.

## Function

### Synopsis

```lua
mux.session.flow_start( descriptor, module, first_step )
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
mux.session.flow_start(ctx.descriptor, "confirm_delete.lua", "confirm")
```

## Errors and availability

Raises `mux.connection.invalid` if the descriptor does not exist,
`mux.connection.unavailable` if it already has a flow or flow support is not
available, and `mux.module.invalid` if the module cannot be loaded or lacks the
requested step. This function raises `mux.unavailable.checking` during
`@lua/check`.

## See Also

- [`mux.session`](../)
- [Flows](../../../../flows/)
