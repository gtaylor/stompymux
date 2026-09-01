---
title: mux.world
linkTitle: mux.world
type: docs
weight: -20
no_list: true
sidebar_root_for: self
---

`mux.world` provides access to database objects and their persistent state.

## Namespaces

| Type | Description |
| --- | --- |
| [`Attribute`](type-attribute/) | Access to an object's supported native attributes. |
| [`Flags`](type-flags/) | Flags that change object behavior. |
| [`Object`](type-object/) | A validated handle for a native database object. |
| [`Powers`](type-powers/) | Bits that bring granularly escalated privileges. |
| [`State`](type-state/) | Typed, persistent state in one object namespace. |

## Functions

| Function | Description |
| --- | --- |
| [`create_object`](create-object/) | Creates a room, thing, or exit. |
| [`destroy_object`](destroy-object/) | Schedules an object for destruction. |
| [`list_objects`](list-objects/) | Lists database objects with optional type and zone filters. |
| [`lock_passes`](lock-passes/) | Tests a native object lock without emitting messages. |
| [`object`](object/) | Creates a validated handle for a database object. |
| [`pemit`](pemit/) | Privately emits a message to an object. |
| [`teleport_object`](teleport-object/) | Teleports a thing or player to a destination. |

## Constants

| Constants package | Description |
| --- | --- |
| [`access`](access/) | Typed constants for Lua command access levels. |
| [`flags`](flags/) | Typed constants for native object flags. |
| [`locks`](locks/) | Typed constants for native object locks. |
| [`powers`](powers/) | Typed constants for native object powers. |
| [`types`](types/) | Typed constants for native object kinds. |
