---
title: Unit templates
weight: 30
description: The TOML format for BattleTech mech, vehicle and other unit templates
---

Unit templates describe a unit's construction: its class, movement, tonnage,
per-section armor and the equipment in each critical slot. Stock templates live
in `game/units`, the directory named by `database.unit_database`.

## Files and references

Each template is a TOML document named `<reference>.toml`. The file stem is the
unit's reference; the document does not repeat it. Lookups match the whole
reference case-insensitively, and documents may sit in subdirectories of the
template root, so `game/units/clan/Daishi-A.toml` answers to `daishi-a`. When
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
walk_mp = 4
heat_sinks = 17
radio = 4
specials = ["SearchLight"]
```

| Key | Type | Meaning |
| --- | --- | --- |
| `name` | string | Display name. |
| `class` | string | `mech`, `vehicle`, `vtol`, `naval`, `aerofighter`, `spheroid_dropship`, `aerodyne_dropship`, `mechwarrior` or `battlesuit`. |
| `movement` | string | `biped`, `quad`, `track`, `wheel`, `hover`, `vtol`, `hull`, `foil`, `fly`, `sub` or `none`. |
| `tons` | integer | Tonnage. |
| `walk_mp` | integer | Walking movement points, as the record sheet states them; each is 10.75 km/h. For a mech it is the walking MP and the running MP is walking x 1.5 rounded up (TechManual p. 48); for a vehicle, VTOL or naval unit it is the cruising MP and the flanking MP is cruising x 1.5 rounded up (TechManual p. 100); for an aerospace unit it is the safe thrust and the maximum thrust follows the same way. Infantry and battle armor have no faster gait. The engine rating is tonnage x `walk_mp`. |
| `jump_mp` | integer | Jumping movement points. Vehicles cannot jump. |
| `max_speed`, `jump_speed` | number | Top and jumping speeds in km/h, for the few unconstructed units (aerospace, battle armor) whose speed is not whole movement points. Give either the speed or its movement points, not both. Mechs and vehicles must state `walk_mp`. |
| `heat_sinks` | integer | Installed heat sinks, counted as the record sheet counts them ("Heat Sinks: 17 (34)" is 17), not the heat they dissipate. The `[construction]` `heat_sinks` kind sets the dissipation: one heat per single sink, two per double or laser sink (TechManual p. 52; Tactical Operations: Advanced Units & Equipment). Without it a mech carries the engine's ten free sinks. Vehicles mount single heat sinks only, at least one per point of energy weapon heat. |
| `radio`, `radio_type`, `radio_range` | integer | Electronics. |
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
engine_rating = 300
structure = "endo_steel"
armor = "ferro_fibrous"
```

| Key | Choices |
| --- | --- |
| `tech_base` | `inner_sphere`, `clan`; the `.mtf` `TechBase:` line (a `Mixed (IS Chassis)` design is `inner_sphere` with `mixed_tech`, `Mixed (Clan Chassis)` is `clan` with `mixed_tech`). Saved mechs and vehicles always state it, and every shipped one declares it. |
| `engine` | `standard`, `xl`, `light`, `xxl`, `compact`, `ice` |
| `engine_rating` | Integer: the engine rating the record sheet states. For a mech it must be tonnage x `walk_mp` (TechManual p. 48); for a vehicle or VTOL it must be the Final Engine Rating, tonnage x cruising MP less the suspension factor, rounded up to a multiple of 5 and at least 10 (TechManual p. 100). A template without it derives the rating from its movement points; every shipped mech and vehicle declares it. |
| `gyro` | `standard`, `xl`, `compact`, `heavy_duty` |
| `cockpit` | `standard`, `small`, `torso_mounted` (Tactical Operations: Advanced Units & Equipment p. 112: four tons in the center torso with a sensor package, a life support slot in each side torso and two sensors left in the head) |
| `structure` | `standard`, `endo_steel`, `composite`, `reinforced` |
| `armor` | `standard`, `ferro_fibrous`, `light_ferro_fibrous`, `heavy_ferro_fibrous`, `stealth`, `hardened`, `laser_reflective` |
| `heat_sinks` | `single`, `double`, `laser`; Clan units default to doubles, and laser heat sinks are Clan only |
| `myomer` | `standard`, `triple_strength` |
| `engine_tech`, `structure_tech`, `armor_tech` | `inner_sphere`, `clan`; default to `tech_base`, and may differ from it only on a `mixed_tech` design (TechManual p. 46) |
| `mixed_tech` | `true` for a mixed-technology design, whose components, weapons and equipment may come from either technology base |

Endo steel and ferro-fibrous slots are still listed in the sections: where
they go is a design choice, not a consequence of the type. Inner Sphere endo
steel and ferro-fibrous armor fill exactly fourteen slots and Clan versions seven;
light ferro-fibrous fills seven, heavy ferro-fibrous twenty-one and stealth armor
twelve, two in each arm, leg and side torso. A mech lists those slots only when it
declares the matching choice.

A mixed-technology unit may take its engine, structure or armor from the other
technology base. An Inner Sphere Mech with a Clan XL engine sets
`engine_tech = "clan"`, giving the engine two slots in each side torso and the
Clan XL engine's mass; a Clan Mech with Inner Sphere ferro-fibrous armor sets
`armor_tech = "inner_sphere"`; both must also set `mixed_tech = true`, since
only a mixed-technology design builds components from both bases (TechManual p.
46).

Without `mixed_tech`, construction refuses anything from the other technology
base, on mechs and vehicles alike: a weapon of the other base, and equipment
only the other base builds. The Bloodhound probe, C3, the null signature system,
Angel ECM, triple strength myomer, stealth armor and light and heavy
ferro-fibrous armor are Inner Sphere equipment; the light active probe, light
TAG, Artemis V, laser heat sinks and the Watchdog suite are Clan equipment.
CASE is an Inner Sphere item that goes in a torso; Clan CASE is built into
every location of a Clan-structure unit rather than installed (TechManual p.
210).

CASE II, ECM suites, active probes, MASC, targeting computers, Artemis IV, jump
jets and machine gun arrays (`MGA`) are built by both bases to their own
specifications under one name.
An unprefixed slot is built from the chassis's base; a slot may instead state
its base with an `IS.` or `CL.` prefix, as `CL.Ecm` (the one-slot, one-ton Clan
ECM suite rather than the Guardian), `CL.Masc` (4% of the tonnage rather than
5%) or `IS.TargetingComputer` (a ton per four tons of weapons rather than
five). A stated base other than the chassis's needs `mixed_tech`.

### Construction rules

A mech template constructs only as the TechManual builds a BattleMech, and the
`template-check` command, the template loader and every live unit apply the same
checks:

- The components weigh the tonnage: nothing over it, and no more than half a ton
  under it (TechManual pp. 46, 57). Every item weighs its whole catalogue mass,
  and the engine, gyro, structure, armor and supercharger round up to the half ton.
- `walk_mp` is at least 1, and the engine rating, tonnage x `walk_mp`, lies
  between 10 and 500 and equals `engine_rating` when the template declares one; ratings above 400 are large engines with eight
  centre torso slots (TechManual p. 48; Tactical Operations: Advanced Units &
  Equipment).
- The engine fills its own layout: six centre torso slots (three for a compact
  engine), plus three per side torso for an Inner Sphere XL, two for a Clan XL or
  light engine, six for an Inner Sphere XXL and four for a Clan XXL (TechManual
  p. 50).
- `jump_mp` equals the installed jump jets (improved jets: pairs of slots), is at
  most the walking MP (improved: running MP), and jets sit only in torsos and
  legs (TechManual p. 51).
- At least ten heat sinks; the engine holds up to its rating / 25 of them and the
  rest fill critical slots (TechManual pp. 52-53).
- Each location's armor, front and rear together, is at most twice its internal
  structure, nine on the head (TechManual p. 54).
- MASC fills 5% (Inner Sphere) or 4% (Clan) of the tonnage, rounded to the nearest
  slot, in one run; triple strength myomer fills six slots, is never combined with
  MASC and is not available to Clan mechs unless they are mixed technology (TechManual p. 52). A supercharger is
  one slot beside engine slots together with `SuperCharger_Tech`.
- A targeting computer fills one slot per ton of direct-fire weapons / 4 (Clan:
  / 5), rounded up, in one run (TechManual p. 238).
- Hatchets, swords, maces, claws, lances and the other melee weapons sit in biped
  arms with the actuators they need, one per arm, at their tonnage-dependent slot
  counts (TechManual pp. 220, 237; Tactical Operations: Advanced Units &
  Equipment).
- Artemis IV serves every standard LRM, SRM and MML launcher from its location or
  none; Inner Sphere CASE sits in a torso; rear-firing weapons sit in torsos, legs
  or the head; a Heavy Gauss Rifle sits in the torsos (TechManual pp. 46, 206,
  210, 218).
- `OneShot` applies only to LRM, SRM, MRM and Streak SRM launchers and Narc and
  iNarc beacons (TechManual p. 342).

### Combat vehicles

Vehicle and VTOL templates follow the TechManual combat vehicle rules (pp.
95-107), and construction refuses a template that breaks them:

- Hovercraft weigh at most 50 tons, VTOLs 30, wheeled vehicles 80 and tracked
  vehicles 100.
- Structure is standard; endo steel, composite and reinforced structure are
  Mech technologies.
- The Engine Rating is the tonnage times the cruising MP, less the suspension
  factor, at least 10 and rounded up to a multiple of 5; it may not exceed 400,
  and a declared `engine_rating` must equal it.
  Any fusion engine weighs half again its listed weight; an `ice` engine weighs
  twice a standard fusion engine and needs power amplifiers (a tenth of the
  energy weapons' mass) for lasers and PPCs.
- A Clan engine is an ICE, a standard fusion or a Clan XL engine, never a light
  or compact one, and Clan armor is standard or Clan ferro-fibrous, never light
  or heavy ferro-fibrous (TechManual pp. 100, 105). An Inner Sphere XL engine on
  a Clan vehicle needs `mixed_tech` and `engine_tech = "inner_sphere"`.
- Hardened armor is refused on hovercraft and VTOLs. Stealth armor is Inner
  Sphere only, needs an ECM suite and generates 10 heat that the heat sinks must
  cover besides the energy weapons (TacOps AUE pp. 92-93).
- Heat sinks are single, at a ton each beyond the ten a fusion engine carries
  free, and must cover the heat of every energy weapon firing at once.
- Armor may not exceed 3.5 points per ton plus 40, and a rotor carries at most
  2 points. Locations have a single armor value, so `rear` is refused.
- The vehicle has 5 item slots plus 1 per 5 tons, less 2 for an Inner Sphere XL
  engine (1 for Clan XL or light), 2 for Inner Sphere ferro-fibrous armor (1
  for Clan or light, 3 for heavy), 1 for hardened or laser-reflective armor, 2
  for stealth armor, and plus 1 for a compact engine (TacOps AUE p. 92 for the
  advanced armors). Each weapon
  and item takes a slot, as does each ammunition type, the cargo bay and each
  type of transport bay (see below); heat
  sinks and power amplifiers take none. The item slots are the whole
  vehicle's budget: a location has no slot limit of its own, so a turret may
  list more than twelve entries.
- Nothing is mounted in the rotor, and vehicles carry no jump jets, MASC or
  triple-strength myomer. A VTOL's turret is a chin turret (TacOps AUE p. 160).
- Everything together may not weigh more than the vehicle. Unspent tonnage is
  cargo space or wasted weight and is only reported. Controls weigh 5 percent of
  the vehicle and the turret 10 percent of the items in it, both rounded up to
  the half ton.

`@btech template-check <name>` reports a vehicle's verdict with its cruising
MP, engine rating, mass, item slots, armor and energy weapon heat against these
limits. Stationary
installations (`movement = "none"`) are not combat vehicles and only have their
mass calculated.

### Cargo and transport bays

A vehicle carries cargo in two ways (TechManual p. 239, Transport Bays):

- `cargo_space` is the standard cargo bay, the only bay the game loads stock
  into. With the `CargoTech` special it is counted in hundredths of a ton
  (`cargo_space = 400` is a 4-ton bay); it weighs what it holds and takes one
  item slot.
- A `[transport_bays]` table lists every other bay. The game has no infantry
  units and does not load anything into these bays, so they are construction
  data only: they add mass, item slots and C-bill cost, and no Battle Value.
  Only vehicles and VTOLs may have one.

```toml
[transport_bays]
infantry_compartment = 4   # tons
foot_infantry_bay = 1      # bays
```

| Key | Size | Weight | Cost per ton |
| --- | --- | --- | --- |
| `infantry_compartment` | tons | as stated | 0 |
| `foot_infantry_bay` | bays | 5 tons each | 15,000 |
| `jump_infantry_bay` | bays | 6 tons each | 15,000 |
| `motorized_infantry_bay` | bays | 7 tons each | 15,000 |
| `mechanized_infantry_bay` | bays | 8 tons each | 15,000 |
| `battle_armor_bay` | bays | 8 tons each (4 troopers) | 15,000 |
| `clan_battle_armor_bay` | bays | 10 tons each (5 troopers) | 15,000 |
| `six_trooper_battle_armor_bay` | bays | 12 tons each (6 troopers) | 15,000 |
| `liquid_cargo` | tons | as stated | 100 |
| `insulated_cargo` | tons | as stated | 250 |
| `refrigerated_cargo` | tons | as stated | 200 |
| `livestock_cargo` | tons | as stated | 2,500 |
| `cargo_containers` | containers | 10 tons each | 0 |

Weights are given in half-ton steps. Each type of bay takes one item slot, as
all infantry bays of one type combine into a single slot, except cargo
containers, which take a slot each. The standard cargo bay takes its own slot
beside them.

## Sections

Each section is a `[sections.<name>]` table. Section names are the lowercase
headings for the unit's anatomy: `left_arm` through `head` for bipeds,
`front_left_leg` and `rear_right_leg` for quads, `left_side`, `front_side`,
`aft_side`, `turret` and `rotor` for vehicles, and so on.

```toml
[sections.left_torso]
armor = 25
rear = 6
engine_at = 4
slots = [
    { at = "1-3", item = "HeatSink" },
    { at = "10-11", item = "IS.ERLargeLaser" },
    { at = 12, item = "IS.MediumPulseLaser", modes = ["RearMount"] },
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

- **Head:** life support, sensors and cockpit, laid out for the cockpit type;
  a torso-mounted cockpit leaves only the two sensors.
- **Centre torso:** engine and gyro slots for the engine and gyro types, then a
  torso-mounted cockpit and its sensor package.
- **Side torsos:** a torso-mounted cockpit's life support slot first, then the
  engine's side slots, starting at the next slot or at `engine_at`.
- **Arms and legs:** shoulder or hip, then upper, lower and hand or foot
  actuators.

A section adjusts that with:

| Key | Meaning |
| --- | --- |
| `omit` | Actuators a biped arm lacks: `hand_actuator`, or `lower_actuator` with `hand_actuator`. These are the only actuators a Mech may remove; shoulders, upper arm actuators and every leg actuator are mandatory (TechManual p. 45). The others keep their usual slots. |
| `engine_at` | The side torso slot where the engine's slots begin. |
| `explicit` | `true` when the section's layout is irregular: nothing is placed, and `slots` lists the fixed equipment too. |

Listing a fixed item in a constructed section, or a slot that collides with
one, is an error.

### Slots

Each slot entry places one item in a run of consecutive critical slots:

| Key | Meaning |
| --- | --- |
| `at` | A one-based slot such as `12`, or an inclusive run such as `"4-6"`. Mech sections hold slots 1 to 12; vehicle and VTOL locations have no per-location limit (slots 1 to 255), since only the vehicle's item slots limit its equipment. |
| `item` | Equipment name. |
| `rounds` | Ammunition only: rounds in the bin. |
| `link` | Non-ammunition only: the slot an Artemis IV or similar system controls. |
| `modes` | Ammunition flags or weapon modes, such as `["RearMount"]` or `["Inferno"]`. |

A run fills every slot with the same item, so `{ at = "1-3", item = "HeatSink" }`
is three single-slot heat sinks and `{ at = "4-6", item = "IS.ERPPC" }` is one
three-slot PPC. Overlapping entries are errors.

## Split mounts

A weapon too large for one location can continue into an adjacent section.
Split mounts are listed once, outside the sections:

```toml
[[split_mounts]]
item = "IS.AC/20"
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "1-8" },
]
```

The first placement is the weapon's primary location; the second is the
adjacent section it continues into. Permitted pairs are a side torso with its
arm or the centre torso, in either direction; legs never take a split mount
(TechManual p. 47). Only ranged weapons of eight or more critical slots split, a
Heavy Gauss Rifle only between a side torso and the centre torso (TechManual
p. 218), and the slots across both placements must add up to the weapon's size.

## Converting MegaMek units

The `megamek-convert` tool, in `crates/megamek-convert`, turns MegaMek BattleMech
(`.mtf`) and combat vehicle (`.blk`) files into templates. It validates its output
through `crates/unit-construction` alone, so it builds without the server:

```sh
# Print one template.
just megamek-convert "Atlas AS7-D.mtf"

# Write <reference>.toml files; --force replaces existing ones.
just megamek-convert --output-dir game/units units/*.mtf units/*.blk
```

The reference is the mech's model when that is a designation such as `AS7-D`,
or its chassis and model otherwise (`NightGyr-Prime`, `PackHunter-2`); vehicles
use their name. When two units in one run would share a reference, the second
takes the chassis-qualified one instead. Pass `--reference` to choose one for a
single file.

OmniMechs get the `OmniMech_Tech` special and a searchlight design quirk the
`SearchLight` special; other quirks have no stompymux counterpart and are not
converted. Mixed-technology units set `mixed_tech = true` and keep a Clan or
Inner Sphere engine, structure or armor through the `[construction]` technology
keys. A CASE II, ECM suite, active probe, MASC, targeting computer, Artemis IV
or jump jet of the other technology base keeps its base as an `IS.` or `CL.`
prefix on its item. A machine gun array is written after the machine guns it
links, which it finds in the slots directly above it.

Every converted unit must load and construct exactly as the game would, so the
tool refuses anything stompymux cannot field instead of dropping it: other unit
types (LAMs, QuadVees, tripods, support and superheavy vehicles, naval and
aerospace units, infantry), construction technology without a `[construction]`
choice (such as fuel cell engines or industrial structure), and weapons, equipment or ammunition munitions with no stompymux
item. A refused file is reported on standard error and writes nothing; the
other files still convert, and the exit status is non-zero.

MegaMek's vehicle body location has no counterpart, so body ammunition is
placed beside a weapon it feeds and other body equipment in the first location
with room. Vehicle heat sinks are set to cover energy weapon heat with single heat
sinks, and vehicles with endo steel, composite or reinforced structure are
refused. A `cargobay` becomes the standard cargo bay (`cargo_space` in
hundredths of a ton, with `CargoTech`); `troopspace` becomes an
`infantry_compartment`, `infantrybay` lines become infantry bays counted in
platoon bays, and liquid, insulated, refrigerated and livestock bays, which
MegaMek sizes by capacity, become `[transport_bays]` weights. Battle armor bays
are refused; seats, quarters, vehicle cubicles and battle armor handles are
reported and left out. Radios are left at their defaults.
