---
title: map_emit
type: docs
toc_hide: false
---

Broadcasts a message to all or nearby units on a map.

## Function

Broadcasts to every unit on a map.

### Synopsis

```lua
btech.map_emit( map, message )
```

### Arguments

`number map`
: The map dbref.

`string message`
: A non-empty message.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Function

Broadcasts to units within a two-dimensional range of a map coordinate.

### Synopsis

```lua
btech.map_emit( map, x, y, range, message )
```

### Arguments

`number map`
: The map dbref.

`number x, y`
: The origin hex coordinates.

`number range`
: A non-negative range.

`string message`
: A non-empty message.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Function

Broadcasts to units within a three-dimensional range of a map coordinate.

### Synopsis

```lua
btech.map_emit( map, x, y, z, range, message )
```

### Arguments

`number map`
: The map dbref.

`number x, y, z`
: The origin hex coordinates and altitude.

`number range`
: A non-negative range.

`string message`
: A non-empty message.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets,
out-of-bounds coordinates, invalid ranges, empty messages, and legacy error
results raise a Lua error.

## See Also

- [`btech`](../)
