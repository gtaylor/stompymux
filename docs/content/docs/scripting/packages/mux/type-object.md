---
title: Object
type: docs
toc_hide: true
---

An `Object` is a validated handle for a native database object. Create one with
[`mux.object`](../object/). Handles become invalid if their object is destroyed
or its database slot is reused. Two handles for the same live object compare
equal, and `tostring(object)` produces a value such as `object(#123)`.

## Properties

All properties are read-only.

| Property | Type | Description |
| --- | --- | --- |
| `dbref` | number | The native database reference. |
| `name` | string | The object's current name. |
| `type` | string | `room`, `thing`, `exit`, or `player`. |
| `description` | string or nil | The native description, if set. |
| `inside_description` | string or nil | The native inside description, if set. |

## Methods

| Method | Description |
| --- | --- |
| [`Object:contents`](../object-contents/) | Lists directly contained objects. |
| [`Object:contents_visible`](../object-contents-visible/) | Applies native content visibility rules. |
| [`Object:exits`](../object-exits/) | Lists directly attached exits. |
| [`Object:exits_visible`](../object-exits-visible/) | Applies native exit visibility rules. |
| [`Object:enter_lock_passes`](../object-enter-lock-passes/) | Tests an exit's traversal lock. |
| [`Object:attribute`](../object-attribute/) | Opens the native attribute interface. |
| [`Object:state`](../object-state/) | Opens a persistent state namespace. |

## See Also

- [`mux`](../)
- [`mux.object`](../object/)
