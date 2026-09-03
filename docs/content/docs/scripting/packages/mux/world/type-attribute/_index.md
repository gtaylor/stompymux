---
title: Attribute
type: docs
toc_hide: false
weight: 10
no_list: true
sidebar_root_for: self
---

An `Attribute` handle exposes the same safe native-attribute set as the
Wizard-only `@attribute` command. It is distinct from dynamic [`State`](../type-state/).
Create a handle with [`Object:attributes`](../type-object/attributes/).

Only BattleTech-native attributes are exposed. Names are matched by the native
attribute registry; unsupported names raise an error. Object descriptions use
dedicated [`Object`](../type-object/) methods instead.

## Methods

| Method | Description |
| --- | --- |
| [`Attribute:get`](get/) | Gets an attribute value. |
| [`Attribute:set`](set/) | Sets or clears an attribute. |
| [`Attribute:entries`](entries/) | Returns every supported attribute. |

## See Also

- [`mux`](../../)
- [`Object`](../type-object/)
- [`Object:attributes`](../type-object/attributes/)
