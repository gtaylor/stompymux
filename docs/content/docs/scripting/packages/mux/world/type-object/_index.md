---
title: Object
type: docs
toc_hide: false
weight: 30
no_list: true
sidebar_root_for: self
---

An `Object` is a validated handle for a native database object. Create one with
[`mux.world.object`](../object/). Handles become invalid if their object is destroyed
or its database slot is reused. Two handles for the same live object compare
equal, and `tostring(object)` produces a value such as `object(#123)`.

## Sub-handlers

The following methods return nested handlers for managing more complex aspects of the object.

| Method | Description |
| --- | --- |
| [`Object:attributes`](attributes/) | Opens the native attribute interface. |
| [`Object:flags`](flags/) | Opens the object's flag collection. |
| [`Object:powers`](powers/) | Opens the object's power collection. |
| [`Object:state`](state/) | Opens a persistent state namespace. |

## Methods

| Method | Description |
| --- | --- |
| [`Object:affiliation`](affiliation/) | Returns the object's assigned affiliation. |
| [`Object:set_affiliation`](set-affiliation/) | Assigns or clears the object's affiliation. |
| [`Object:contents`](contents/) | Lists and filters directly contained objects and attached exits. |
| [`Object:dbref`](dbref/) | Returns the object's native database reference. |
| [`Object:destination`](destination/) | Returns an exit's destination. |
| [`Object:set_destination`](set-destination/) | Sets or clears an exit's destination. |
| [`Object:home`](home/) | Returns a thing or player's home. |
| [`Object:set_home`](set-home/) | Sets a thing or player's home. |
| [`Object:location`](location/) | Returns a thing or player's location. |
| [`Object:name`](name/) | Returns the object's current name. |
| [`Object:set_name`](set-name/) | Changes the object's name. |
| [`Object:lua_parent`](lua-parent/) | Returns the object's direct Lua parent path. |
| [`Object:set_lua_parent`](set-lua-parent/) | Assigns or clears the object's Lua parent. |
| [`Object:type`](type/) | Returns the object's native object type. |
| [`Object:zone`](zone/) | Returns the object's assigned zone. |
| [`Object:set_zone`](set-zone/) | Assigns or clears the object's zone. |

## See Also

- [`mux`](../../)
- [`mux.world.object`](../object/)
