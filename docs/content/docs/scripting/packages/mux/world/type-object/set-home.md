---
title: set_home
type: docs
---

Sets a thing or player's home.

## Method

```lua
object:set_home(new_home)
```

The receiver must be a live thing or player. `new_home` is a dbref or
[Object](../) handle for a live room, thing, or player capable of containing
objects. The receiver cannot be its own home.

This trusted world mutation validates the same object relationships used when
assigning a home with `@link`, but it does not perform ownership or `SET_HOME`
lock checks. The method returns no values.

It raises `mux.arg.invalid` when `new_home` is omitted,
`mux.object.invalid` for invalid object kinds, references, or self-home, and
`mux.object.unavailable` when the receiver or destination is being destroyed.
It raises `mux.unavailable.checking` during `@lua/check`.

## Example

```lua
object:set_home(destination)
object:set_home(destination:dbref())
```
