---
title: load_template
type: docs
toc_hide: false
---

Loads a unit template into a live unit.

## Function

### Synopsis

```lua
btech.unit.load_template( unit, reference )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`string reference`
: The non-empty unit-template reference. It cannot contain `..`, `/`, or `\`.

### Returns

None.

## Notes

A missing template raises `btech.template.not_found`. A template that exists but
is malformed raises `btech.template.invalid`.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
