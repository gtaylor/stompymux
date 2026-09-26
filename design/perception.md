# Perception design

Internal engineering note for `src/btech/perception/`. Player-facing rules live in
`game/help/line_of_sight.md`.

## Why

The reference game gave every unit a primary/secondary sensor pair chosen from
nine modes (visual, light-amplification, infrared, seismic, electromagnetic,
radar and three active probes), with a ten-second switch that cleared the target
lock, per-mode ranges and obscurant thresholds, random aim terms and a second
acquisition roll. Players had to micromanage it, and autopilots never did, so
they fought at a permanent disadvantage at night. Perception is now automatic:
one rule for line of sight, with an edge for special equipment.

## Rules

Each observer/target pair is traced once (`los::unit_terrain_geometry`).

**Clear line:** not blocked by terrain, no fire, no smoke, fewer than three woods
points on the path, not across the cloud base, an underwater target through fewer
than six water hexes, within the map ceiling (`min(maximum_visibility, 60)`).

**Cover term:** path woods + target-hex woods + (partial cover ? 3 + hull-down : 0).

| Channel | Who | Reach | Needs clear line | Aim |
|---|---|---|---|---|
| Sensors | every unit | 15 hexes (`battletech.sensor_range`); ×1.4 stationary; ½ at one Mech sensor crit; 0 at two, when jammed, with `NoSensors`, or with the map's sensor flag | yes | cover |
| Sight | every unit | map visibility; lit targets ×3 at night | yes | cover, +1 at night if unlit |
| Probe | Beagle 6 / Light 3 / Bloodhound 8 (×1.4 stationary) | not jammed, target not Angel-protected, concealed targets only for Bloodhound, map probe flag | no | partial-cover term only |
| Radar | `AntiAircraft` | airborne rules, 180 hexes | terrain only | woods + 2 partial − 3 vs high/VTOL + hull-down |

- The reaching channel with the lowest aim wins; ties go Sensors, Sight, Radar,
  Probe.
- Jammed means the observer's ECM field is `disturbed` or `angel_disturbed`. That
  includes the unit's own active stealth armor or an attached ECM beacon.
- Active stealth armor or null signature hides the target from the sensor band
  and from every probe except the Bloodhound. Sight still works.
- A probe contact behind blocking terrain is unidentified. Lock, spotting and C3
  sharing work; direct fire and `scan` are refused.
- Acquisition is instant and consumes no dice. The exception is hidden hostile
  units: never beyond five hexes unless probed, automatic under three, and between
  three and five a d10000 search against arc weight × perception ÷ 4 ×
  (100 − d/3).
- Map `sensor_flags` bits keep the reference positions: 1 sensor band, 32 radar,
  64 probes.

## Tunables and where they live

- Sensor band reach: `battletech.sensor_range` (default 15, `DEFAULT_SENSOR_RANGE`).
- Woods and water limits, partial-cover cost: `perception/sight.rs` constants.
- Hidden-unit ranges: `perception/acquisition.rs` constants.
- Probe reaches: `BattleActiveProbe::range`; radar reach: `RADAR_RANGE`.

## Choices worth revisiting

- Stealth and null signature hiding from the sensor band is new. It changes
  nothing in clear daylight but makes stealth strong at night and in fog.
- Smoke blocks both the sensor band and sight. Only probes see through it, which
  keeps smoke screens useful and probes valuable.
- The reactor-explosion "sensor flash" was removed. It only ever punished a
  sensor choice that no longer exists.
