---
title: Attribute
linkTitle: Attribute
type: docs
weight: 110
---

# `Attribute`

An `Attribute` handle exposes the same safe native-attribute set as the
Wizard-only `@attribute` command. It is distinct from dynamic [`State`](../type-state/).
Create a handle with [`Object:attribute`](../object-attribute/).

Only descriptions and BattleTech-native attributes are exposed. Names are
matched by the native attribute registry; unsupported names raise an error.

## Methods

| Method | Description |
| --- | --- |
| [`Attribute:get`](../attribute-get/) | Gets an attribute value. |
| [`Attribute:set`](../attribute-set/) | Sets or clears an attribute. |
| [`Attribute:entries`](../attribute-entries/) | Returns every supported attribute. |

## See Also

- [`mux`](../)
- [`Object`](../type-object/)
- [`Object:attribute`](../object-attribute/)
