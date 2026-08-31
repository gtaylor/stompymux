---
title: mux.world
linkTitle: mux.world
type: docs
weight: -20
no_list: true
sidebar_root_for: self
---

`mux.world` provides access to database objects and their persistent state.

## Types

| Type | Description |
| --- | --- |
| [`Attribute`](type-attribute/) | Access to an object's supported native attributes. |
| [`Object`](type-object/) | A validated handle for a native database object. |
| [`State`](type-state/) | Typed, persistent state in one object namespace. |

## Functions

| Function | Description |
| --- | --- |
| [`access`](access/) | Typed constants for Lua command access levels. |
| [`create_exit`](create-exit/) | Creates and attaches an exit. |
| [`create_room`](create-room/) | Creates a detached room. |
| [`create_thing`](create-thing/) | Creates and places a thing. |
| [`destroy`](destroy/) | Schedules an object for destruction. |
| [`flags`](flags/) | Typed constants for native object flags. |
| [`link_exit`](link-exit/) | Links or unlinks an exit's destination. |
| [`lock_passes`](lock-passes/) | Tests a native object lock without emitting messages. |
| [`locks`](locks/) | Typed constants for native object locks. |
| [`object`](object/) | Creates a validated handle for a database object. |
| [`pemit`](pemit/) | Privately emits a message to an object. |
| [`powers`](powers/) | Typed constants for native object powers. |
| [`teleport`](teleport/) | Teleports a thing or player to a destination. |
| [`types`](types/) | Typed constants for native object kinds. |
