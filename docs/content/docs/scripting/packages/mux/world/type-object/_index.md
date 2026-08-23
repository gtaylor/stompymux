---
title: Object
type: docs
toc_hide: false
weight: -20
no_list: true
sidebar_root_for: self
---

An `Object` is a validated handle for a native database object. Create one with
[`mux.world.object`](../object/). Handles become invalid if their object is destroyed
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
| [`Object:contents`](contents/) | Lists directly contained objects. |
| [`Object:contents_visible`](contents-visible/) | Applies native content visibility rules. |
| [`Object:exits`](exits/) | Lists directly attached exits. |
| [`Object:exits_visible`](exits-visible/) | Applies native exit visibility rules. |
| [`Object:enter_lock_passes`](enter-lock-passes/) | Tests an exit's traversal lock. |
| [`Object:attribute`](attribute/) | Opens the native attribute interface. |
| [`Object:flags`](flags/) | Opens the object's flag collection. |
| [`Object:powers`](powers/) | Opens the object's power collection. |
| [`Object:state`](state/) | Opens a persistent state namespace. |

## See Also

- [`mux`](../../)
- [`mux.world.object`](../object/)
