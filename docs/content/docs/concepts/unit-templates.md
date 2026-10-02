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

A template states construction choices, not their consequences. The loader
derives the rest: technology flags, the fixed equipment in mech critical slots,
internal structure and speeds.

```toml
name = "Zeus"
class = "mech"
movement = "biped"
tons = 80
walk_mp = 6
heat_sinks = 34
computer = 4
radio = 4
specials = ["SearchLight"]
```

| Key | Type | Meaning |
| --- | --- | --- |
| `name` | string | Display name. |
| `class` | string | `mech`, `vehicle`, `vtol`, `naval`, `aerofighter`, `spheroid_dropship`, `aerodyne_dropship`, `mechwarrior` or `battlesuit`. |
| `movement` | string | `biped`, `quad`, `track`, `wheel`, `hover`, `vtol`, `hull`, `foil`, `fly`, `sub` or `none`. |
| `tons` | integer | Tonnage. |
| `walk_mp`, `jump_mp` | integer | Cruising and jumping movement points; each is 10.75 km/h. |
| `max_speed`, `jump_speed` | number | Speeds in km/h, for the few units whose speed is not whole movement points. Give either the speed or its movement points, not both. |
| `heat_sinks` | integer | Installed heat sinks. |
| `hs_engine_override` | integer | Engine heat sink override. |
| `computer`, `radio`, `radio_type`, `radio_range` | integer | Electronics. |
| `tac_range`, `lrs_range`, `scan_range` | integer | Sensor ranges. |
| `si`, `fuel`, `cargo_space`, `max_suits`, `max_ton`, `carrier_maximum_tonnage` | integer | Class-specific capacities. |
| `template_speed` | number | Movement baseline written when a saved unit's speed was edited. |
| `unit_era`, `unit_tro` | string | Era and technical readout identity. |
| `specials` | array of strings | Feature flags such as `SearchLight`, `CargoTech` or `ECM`, one word each, matched case-insensitively against the known flags. |
| `infantry_specials` | array of strings | Battle suit abilities such as `Swarm_Attack_Tech`. |

Unknown keys, unknown flags and values of the wrong type are errors.
`specials` cannot list a flag that a construction choice owns (for example
`XLEngine_Tech` or `Clan`) or `FlipArms`, which a biped mech has exactly when
neither arm carries a lower or hand actuator.

## Construction

The optional `[construction]` table names the unit's technology. Every key
defaults to the standard choice shown first.

```toml
[construction]
tech_base = "clan"
engine = "xl"
structure = "endo_steel"
armor = "ferro_fibrous"
brand = 3
```

| Key | Choices |
| --- | --- |
| `tech_base` | `inner_sphere`, `clan` |
| `engine` | `standard`, `xl`, `light`, `xxl`, `compact`, `ice` |
| `gyro` | `standard`, `xl`, `compact`, `heavy_duty` |
| `cockpit` | `standard`, `small` |
| `structure` | `standard`, `endo_steel`, `composite`, `reinforced` |
| `armor` | `standard`, `ferro_fibrous`, `light_ferro_fibrous`, `heavy_ferro_fibrous`, `stealth`, `hardened`, `laser_reflective`, `reactive` |
| `heat_sinks` | `single`, `double`, `laser`, `compact`; Clan units default to and require doubles or better |
| `myomer` | `standard`, `triple_strength` |
| `brand` | Manufacturer brand number stamped on the fixed equipment construction places |

Endo steel and ferro-fibrous slots are still listed in the sections: where
they go is a design choice, not a consequence of the type.

## Sections

Each section is a `[sections.<name>]` table. Section names are the lowercase
headings for the unit's anatomy: `left_arm` through `head` for bipeds,
`front_left_leg` and `rear_right_leg` for quads, `left_side`, `front_side`,
`aft_side`, `turret` and `rotor` for vehicles, and so on.

```toml
[sections.left_torso]
armor = 25
rear = 6
config = "Case"
engine_at = 4
slots = [
    { at = "1-3", item = "HeatSink", brand = 3 },
    { at = "10-11", item = "IS.ERLargeLaser", brand = 3 },
    { at = 12, item = "IS.MediumPulseLaser", modes = ["RearMount"], brand = 3 },
]
```

`armor` and `rear` default to zero. Mech internal structure comes from the
tonnage chart and vehicle structure from tonnage, so `internals` appears only
where a unit differs, such as a vehicle location with no structure
(`internals = 0`) or a tonnage the chart does not cover. Other classes list
`internals` for every section.

### Fixed equipment

In biped and quad mechs, construction places the fixed equipment, which
`slots` then leaves out:

- **Head:** life support, sensors and cockpit, laid out for the cockpit type.
- **Centre torso:** engine and gyro slots for the engine and gyro types.
- **Side torsos:** the engine's side slots, starting at slot 1 or at
  `engine_at`.
- **Arms and legs:** shoulder or hip, then upper, lower and hand or foot
  actuators.

A section adjusts that with:

| Key | Meaning |
| --- | --- |
| `omit` | Actuators the section lacks, in any combination: `shoulder`, `upper_actuator`, `lower_actuator`, and `hand_actuator` (biped arms) or `foot_actuator` (legs). The others keep their usual slots. |
| `engine_at` | The side torso slot where the engine's slots begin. |
| `engine_slots` | How many engine slots a torso holds when that differs from the engine type's count. Centre torso slots fill the three ahead of the gyro, then those after it. |
| `brand` | Brand stamped on the section's fixed equipment instead of the construction `brand`. |
| `explicit` | `true` when the section's layout is irregular: nothing is placed, and `slots` lists the fixed equipment too. |

Listing a fixed item in a constructed section, or a slot that collides with
one, is an error.

### Slots

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
