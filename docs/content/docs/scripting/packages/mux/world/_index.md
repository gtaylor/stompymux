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
| [`object`](object/) | Creates a validated handle for a database object. |
| [`pemit`](pemit/) | Privately emits a message to an object. |
