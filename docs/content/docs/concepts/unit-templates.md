---
title: Unit templates
weight: 30
description: The TOML format for BattleTech mech, vehicle and other unit templates
---

Unit templates describe a unit's construction: its class, movement, tonnage,
per-section armor and the equipment in each critical slot. Stock templates live
in `game/mechs`, the directory named by `database.mech_database`.

## Files and references

Each template is a TOML document named `<reference>.toml`. The file stem is the
unit's reference; the document does not repeat it. Lookups match the whole
reference case-insensitively, and documents may sit in subdirectories of the
template root, so `game/mechs/clan/Daishi-A.toml` answers to `daishi-a`. When
two documents share a reference, the lexically first path wins.

Saving a unit replaces the existing document for its reference, wherever it
lives, or creates `<reference>.toml` at the top of the template root.

## Unit fields

```toml
name = "Zeus"
class = "mech"
movement = "biped"
tons = 80
max_speed = 64.5
heat_sinks = 34
computer = 4
radio = 4
specials = ["DoubleHS"]
```

| Key | Type | Meaning |
| --- | --- | --- |
| `name` | string | Display name. |
| `class` | string | `mech`, `vehicle`, `vtol`, `naval`, `aerofighter`, `spheroid_dropship`, `aerodyne_dropship`, `mechwarrior` or `battlesuit`. |
| `movement` | string | `biped`, `quad`, `track`, `wheel`, `hover`, `vtol`, `hull`, `foil`, `fly`, `sub` or `none`. |
| `tons` | integer | Tonnage. |
| `max_speed`, `jump_speed` | number | Speeds in km/h. |
| `heat_sinks` | integer | Installed heat sinks. |
| `hs_engine_override` | integer | Engine heat sink override. |
| `computer`, `radio`, `radio_type`, `radio_range` | integer | Electronics. |
| `tac_range`, `lrs_range`, `scan_range` | integer | Sensor ranges. |
| `si`, `fuel`, `cargo_space`, `max_suits`, `max_ton`, `carrier_maximum_tonnage` | integer | Class-specific capacities. |
| `template_speed` | number | Movement baseline written when a saved unit's speed was edited. |
| `unit_era`, `unit_tro` | string | Era and technical readout identity. |
| `specials`, `infantry_specials` | array of strings | Feature flags, one word each; repeats are ignored case-insensitively. |

Unknown keys are errors, as are values of the wrong type.

## Sections

Each section is a `[sections.<name>]` table. Section names are the lowercase
headings for the unit's anatomy: `left_arm` through `head` for bipeds,
`front_left_leg` and `rear_right_leg` for quads, `left_side`, `front_side`,
`aft_side`, `turret` and `rotor` for vehicles, and so on.

```toml
[sections.left_torso]
armor = 25
internals = 17
rear = 6
config = "Case"
slots = [
    { at = "1-9", item = "HeatSink", brand = 3 },
    { at = "10-11", item = "IS.ERLargeLaser", brand = 3 },
    { at = 12, item = "IS.MediumPulseLaser", modes = ["RearMount"], brand = 3 },
]
```

`armor`, `internals` and `rear` default to zero. Mech and vehicle internal
structure is recomputed from tonnage on load.

Each slot entry places one item in a run of consecutive critical slots:

| Key | Meaning |
| --- | --- |
| `at` | A one-based slot such as `12`, or an inclusive run such as `"4-6"`, within slots 1 to 12. |
| `item` | Equipment name. |
| `rounds` | Ammunition only: rounds in the bin. |
| `link` | Non-ammunition only: the slot an Artemis IV or similar system controls. |
| `modes` | Ammunition flags or weapon modes, such as `["RearMount"]` or `["Inferno"]`. |
| `brand` | Manufacturer brand number. |

A run fills every slot with the same item, so `{ at = "1-3", item = "HeatSink" }`
is three single-slot heat sinks and `{ at = "4-6", item = "IS.ERPPC" }` is one
three-slot PPC. Overlapping entries are errors.

## Split mounts

A weapon too large for one location can continue into an adjacent section.
Split mounts are listed once, outside the sections:

```toml
[[split_mounts]]
item = "IS.AC/20"
brand = 4
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "1-8" },
]
```

The first placement is the weapon's primary location; the second is the
adjacent section it continues into. Permitted pairs are a side torso with its
arm, its leg or the centre torso, in either direction. Only weapons that support
split mounting (the AC/20 family, Heavy Gauss Rifle and Arrow IV) load, and the
slots across both placements must add up to the weapon's size.
