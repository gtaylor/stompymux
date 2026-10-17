---
title: "btech.unit.grabclub"
type: docs
linkTitle: "grabclub"
manualLinkTitle: "grabclub"
---

Take up a club in a selected arm (left first by default): a limb or girder lying in the hex,
a tree in woods, or a girder searched out of a collapsed building's rubble, once a turn.
'-' lets the club go; a tree shatters and a limb or girder stays in the hex.

## Signature

```lua
btech.unit.grabclub(dbref, pilot, arm)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `integer` |  |
| `pilot` | `integer` |  |
| `arm?` | `'left'\|'right'\|'-'` |  |

## Returns

- `table notices`
