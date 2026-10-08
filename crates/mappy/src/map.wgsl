// Draws the whole hex map in one pass: every pixel finds its hex, reads that hex's layers from
// the hex texture, and composes its color from the layer palette, with grid gaps, labels and
// the brush outline computed per pixel. Panning and zooming only change the uniforms.
//
// Layers are drawn bottom up: ground with its texture or icon, foliage, elevation shading,
// water, a road or rail line, the structure with a skyline icon on buildings, the weather
// condition, then fire as flames or smoke as rising puffs, and labels last, over every layer.
//
// Hexes are flat-topped in staggered columns, even columns offset half a hex south. Map
// units are hex vertex radii, with the top-left of the map's bounding box at the origin.

struct Uniforms {
    // Widget size in logical pixels.
    size: vec2<f32>,
    // Logical-pixel position of the map origin within the widget.
    offset: vec2<f32>,
    // Map width and height in hexes.
    map_size: vec2<f32>,
    // Hex under the cursor; only used when `brush` is not negative.
    hover: vec2<f32>,
    // Hex vertex radius in logical pixels.
    radius: f32,
    // Width of the background gap drawn between hexes, or zero for no grid.
    grid_gap: f32,
    // Brush radius in hexes around `hover`, or negative for no brush outline.
    brush: f32,
    // Nonzero to write each hex's layer heights on it.
    labels: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
// Layer colors in the order of `palette_colors` in render.rs, then the ink threshold in the
// last entry's red.
@group(0) @binding(1) var<uniform> palette: array<vec4<f32>, 34>;
// One texel of packed layers per hex, indexed by hex coordinate; see `hex_texel` in render.rs.
@group(0) @binding(2) var hexes: texture_2d<u32>;

// Where each layer's colors start in the palette, matching the `PALETTE_` constants in
// render.rs.
const GROUND: u32 = 0u;
const FOLIAGE: u32 = 12u;
const ROUTE: u32 = 19u;
const WATER: u32 = 23u;
const CONDITION: u32 = 24u;
const STRUCTURE: u32 = 28u;
const OVERLAY: u32 = 31u;
const INK_THRESHOLD: u32 = 33u;

// Positions within their layers, matching each layer's `ALL` order.
const ROUGH: u32 = 2u;
const ULTRA_ROUGH: u32 = 3u;
const RUBBLE: u32 = 4u;
const ULTRA_RUBBLE: u32 = 5u;
const SAND: u32 = 6u;
const TUNDRA: u32 = 7u;
const SWAMP: u32 = 8u;
const MAGMA_CRUST: u32 = 9u;
const MAGMA: u32 = 10u;
const HEAVY_INDUSTRIAL: u32 = 11u;
const CLEAR: u32 = 0u;
const PAVEMENT: u32 = 1u;
const LIGHT_JUNGLE: u32 = 3u;
const PLANTED_FIELDS: u32 = 6u;
const PAVED_ROAD: u32 = 0u;
const GRAVEL_ROAD: u32 = 1u;
const DIRT_ROAD: u32 = 2u;
const RAIL: u32 = 3u;
const ICE: u32 = 0u;
const THIN_SNOW: u32 = 1u;
const DEEP_SNOW: u32 = 2u;
const MUD: u32 = 3u;
const BUILDING: u32 = 0u;
const WALL: u32 = 1u;
const BRIDGE: u32 = 2u;
const FIRE: u32 = 0u;
const SMOKE: u32 = 1u;
const RAPIDS: u32 = 1u;
const TORRENT: u32 = 2u;

// How much each construction class above light darkens a structure, matching `CLASS_SHADE` in
// map_view.rs.
const CLASS_SHADE: f32 = 0.12;

// Half width, in map units, of a road, rail line or bridge deck, all one width so they run
// into each other evenly.
const WAY_HALF_WIDTH: f32 = 0.16;

// Hex radii in logical pixels over which the details of roads, rail and bridges fade in.
const WAY_DETAIL_FADE: vec2<f32> = vec2<f32>(10.0, 18.0);

// Paved roads' dashed center line: its half width, the distance from one dash to the next
// (two thirds of the way from a hex edge to its center, so a straight road has a dash centered
// on each hex and gaps across its edges, matching on both sides), and its color.
const LANE_HALF_WIDTH: f32 = 0.03;
const LANE_PERIOD: f32 = 0.57735;
const LANE_COLOR: vec3<f32> = vec3<f32>(0.95, 0.95, 0.9);

// Dirt roads: their edges wander in and out to `DIRT_RAGGEDNESS` of their width, and two
// wheel ruts `RUT_HALF_WIDTH` either side run `RUT_OFFSET` either side of the middle, in the
// road's color darkened to `RUT_SHADE`. All distances are in map units.
const DIRT_RAGGEDNESS: f32 = 0.1;
const RUT_OFFSET: f32 = 0.07;
const RUT_HALF_WIDTH: f32 = 0.018;
const RUT_SHADE: f32 = 0.72;

// Gravel roads: a bed speckled with stones in cells `GRAVEL_GRAIN` map units across, of which
// `GRAVEL_SPECKS` are darkened to `GRAVEL_SPECK_SHADE`, between shoulders `SHOULDER_WIDTH` map
// units wide lightened by `SHOULDER_LIGHT`.
const GRAVEL_GRAIN: f32 = 0.03125;
const GRAVEL_SPECKS: f32 = 0.3;
const GRAVEL_SPECK_SHADE: f32 = 0.75;
const SHOULDER_WIDTH: f32 = 0.035;
const SHOULDER_LIGHT: f32 = 0.35;

// Rail track: a ballast bed `BALLAST_WIDTH` of the way's width in `BALLAST_COLOR`, crossed by
// ties in `TIE_COLOR` every `TIE_PERIOD`, each `TIE_HALF_WIDTH` either side of its middle and
// reaching `TIE_HALF_LENGTH` either side of the track, under two steel rails `RAIL_GAUGE`
// either side of the middle with strokes `RAIL_HALF_WIDTH` either side. Distances are in map
// units.
const BALLAST_WIDTH: f32 = 0.85;
const BALLAST_COLOR: vec3<f32> = vec3<f32>(0.52, 0.5, 0.47);
const TIE_COLOR: vec3<f32> = vec3<f32>(0.38, 0.26, 0.16);
const TIE_PERIOD: f32 = 0.09;
const TIE_HALF_WIDTH: f32 = 0.018;
const TIE_HALF_LENGTH: f32 = 0.11;
const RAIL_GAUGE: f32 = 0.055;
const RAIL_HALF_WIDTH: f32 = 0.011;

// Bridge decks: concrete crossed by expansion joints every `JOINT_PERIOD` (a third of the way
// across a hex from edge to edge, so a joint lies on every hex edge) with strokes
// `JOINT_HALF_WIDTH` either side, darkened to `JOINT_SHADE`; dark railings along both edges,
// `RAILING_HALF_WIDTH` either side of a line `RAILING_INSET` in from the edge, with posts
// every `POST_PERIOD` reaching `POST_HALF_SIZE` either side, all darkened to `RAILING_SHADE`.
// Distances are in map units.
const JOINT_PERIOD: f32 = 0.57735;
const JOINT_HALF_WIDTH: f32 = 0.008;
const JOINT_SHADE: f32 = 0.7;
const RAILING_INSET: f32 = 0.012;
const RAILING_HALF_WIDTH: f32 = 0.01;
const POST_PERIOD: f32 = 0.12;
const POST_HALF_SIZE: f32 = 0.02;
const RAILING_SHADE: f32 = 0.4;

// What a hex carries that joins its neighbors: nothing, a road of any surface, rail, a bridge
// deck, or a wall. Roads join roads and rail joins rail; a bridge joins either, and other
// bridges; walls join only walls.
const WAY_NONE: u32 = 0u;
const WAY_ROAD: u32 = 1u;
const WAY_RAIL: u32 = 2u;
const WAY_BRIDGE: u32 = 3u;
const WAY_WALL: u32 = 4u;

// Walls (see `draw_wall`): a line of stone `WALL_HALF_WIDTH` map units either side of its
// middle, its top in the wall color crossed by joints every `WALL_BLOCK` darkened to
// `WALL_JOINT_SHADE`, its sides within `WALL_SIDE` of its edges darkened to `WALL_SIDE_SHADE`,
// over a soft shadow reaching `WALL_SHADOW` past it that darkens the ground to
// `WALL_SHADOW_SHADE`. Square pillars stand every `PILLAR_SPACING` along it (the apothem, so one
// stands on every hex edge and at every hex center), reaching `PILLAR_HALF_WIDTH` either side
// of the wall's middle and `PILLAR_HALF_LENGTH` either way along it, with tops lightened by
// `PILLAR_TOP` inside a rim `PILLAR_RIM` wide darkened to `PILLAR_RIM_SHADE`.
const WALL_HALF_WIDTH: f32 = 0.19;
const PILLAR_SPACING: f32 = APOTHEM;
const PILLAR_HALF_WIDTH: f32 = 0.28;
const PILLAR_HALF_LENGTH: f32 = 0.1;
const PILLAR_TOP: f32 = 0.14;
const PILLAR_RIM: f32 = 0.018;
const PILLAR_RIM_SHADE: f32 = 0.55;
const WALL_BLOCK: f32 = 0.12;
const WALL_JOINT_SHADE: f32 = 0.7;
const WALL_SIDE: f32 = 0.025;
const WALL_SIDE_SHADE: f32 = 0.65;
const WALL_SHADOW: f32 = 0.05;
const WALL_SHADOW_SHADE: f32 = 0.62;

// The building icon (see `draw_building`): a skyline of three blocks in the building's color
// darkened to `SKYLINE_SHADE` (every other block a shade lighter), outlined in
// `SKYLINE_OUTLINE` reaching `SKYLINE_OUTLINE_WIDTH` past them, casting a shadow
// `SKYLINE_SHADOW` map units toward the lower right that darkens the ground to
// `SKYLINE_SHADOW_SHADE`. A block's windows are inset from its sides by `WINDOW_INSET`, one per
// `WINDOW_PITCH` cell, `WINDOW_HALF` across.
const SKYLINE_SHADE: f32 = 0.72;
const SKYLINE_OUTLINE: vec3<f32> = vec3<f32>(0.18, 0.18, 0.22);
const SKYLINE_OUTLINE_WIDTH: f32 = 0.02;
const SKYLINE_SHADOW: vec2<f32> = vec2<f32>(0.04, 0.055);
const SKYLINE_SHADOW_SHADE: f32 = 0.62;
const WINDOW_INSET: f32 = 0.06;
const WINDOW_PITCH: vec2<f32> = vec2<f32>(0.1, 0.12);
const WINDOW_HALF: vec2<f32> = vec2<f32>(0.028, 0.034);
const WINDOW_LIGHT: vec3<f32> = vec3<f32>(0.95, 0.9, 0.62);
// Hex radii in logical pixels over which windows fade in, below which they would only blur
// the silhouette.
const WINDOW_FADE: vec2<f32> = vec2<f32>(14.0, 24.0);
// How strongly the icon shows while labels are on, faint enough to keep the numbers legible.
const LABELLED_SKYLINE: f32 = 0.3;

// Planted fields (see `draw_fields`): plots about `PLOT_SIZE` map units across, edged by
// hedgerows `HEDGE_HALF_WIDTH` either side in the crop's color darkened to `HEDGE_SHADE`, each
// planted with crops in rows over soil mixed `SOIL_MIX` of the way to `SOIL_COLOR`. Plants
// stand `PLANT_SPACING` apart along and across their rows, each `PLANT_RADIUS` across. All
// distances are in map units.
const PLOT_SIZE: f32 = 1.05;
const HEDGE_HALF_WIDTH: f32 = 0.012;
const HEDGE_SHADE: f32 = 0.5;
const SOIL_COLOR: vec3<f32> = vec3<f32>(0.55, 0.42, 0.26);
const SOIL_MIX: f32 = 0.6;
const PLANT_SPACING: vec2<f32> = vec2<f32>(0.1, 0.13);
const PLANT_RADIUS: f32 = 0.042;

// The fire icon (see `draw_fire`): three flames over ground scorched `SCORCH` of the way to
// `CHAR`, each outlined in `FLAME_OUTLINE` and filled from the outside in with `FLAME_EDGE`,
// the fire color, and `FLAME_HEART`. The outline reaches `OUTLINE_WIDTH` map units past each
// flame.
const SCORCH: f32 = 0.3;
const CHAR: vec3<f32> = vec3<f32>(0.16, 0.1, 0.08);
const FLAME_OUTLINE: vec3<f32> = vec3<f32>(0.6, 0.11, 0.04);
const FLAME_EDGE: vec3<f32> = vec3<f32>(0.88, 0.3, 0.06);
const FLAME_HEART: vec3<f32> = vec3<f32>(1.0, 0.66, 0.24);
const OUTLINE_WIDTH: f32 = 0.035;
// The smoke icon (see `draw_smoke`): `SMOKE_BANDS` streaming bands `SMOKE_BAND_SPACING` map
// units apart down the hex, shaded from `SMOKE_SHADE` of the smoke color to lightened by
// `SMOKE_LIGHT`, each outlined in `SMOKE_OUTLINE` reaching `SMOKE_OUTLINE_WIDTH` past it.
const SMOKE_BANDS: i32 = 3;
const SMOKE_BAND_SPACING: f32 = 0.3;
const SMOKE_SHADE: f32 = 0.7;
const SMOKE_LIGHT: f32 = 0.4;
const SMOKE_OUTLINE: vec3<f32> = vec3<f32>(0.3, 0.3, 0.32);
const SMOKE_OUTLINE_WIDTH: f32 = 0.02;

// The fire icon is drawn `ICON_SCALE` times its base size and shifted `ICON_SHIFT` map units,
// large enough to stand out while keeping mostly clear of the top label row.
const ICON_SCALE: f32 = 1.18;
const ICON_SHIFT: vec2<f32> = vec2<f32>(0.0, 0.05);

const PI: f32 = 3.1415927;

// Water channels (see `water_channel`): the water in a hex runs from a pool
// `CHANNEL_POOL_RADIUS` across around its center toward each neighboring water hex as a
// channel `CHANNEL_HALF_WIDTH` either side of the line between their centers, the two blended
// over `CHANNEL_BLEND`. A hex with no water beside it holds a round pond
// `POND_RADIUS` across. All distances are in map units.
const CHANNEL_POOL_RADIUS: f32 = 0.45;
const CHANNEL_HALF_WIDTH: f32 = 0.42;
const CHANNEL_BLEND: f32 = 0.15;
const POND_RADIUS: f32 = 0.6;

// Banks beside water (see `draw_water_hex`): the ground darkens toward `BANK_MUD` by up to
// `BANK_MUD_MIX` within `BANK_WIDTH` map units of the water, which a foam line
// `FOAM_HALF_WIDTH` either side lightens toward white by `FOAM_LIGHT`.
const BANK_MUD: vec3<f32> = vec3<f32>(0.45, 0.37, 0.26);
const BANK_MUD_MIX: f32 = 0.45;
const BANK_WIDTH: f32 = 0.14;
const FOAM_HALF_WIDTH: f32 = 0.012;
const FOAM_LIGHT: f32 = 0.45;

// Ripples and streaks on the current (see `draw_current`): at most one to each cell
// `CURRENT_CELL` map units along and across the flow, in rows offset by half a cell. Still
// water leaves `RIPPLE_SKIP` of cells calm and bows each ripple `RIPPLE_BOW` across the flow;
// rapids and torrents lay down straight streaks in more cells, longer and brighter, and a
// torrent whitens all its water by `TORRENT_FROTH`. They fade in across `CURRENT_FADE` hex radii
// in logical pixels.
const CURRENT_CELL: vec2<f32> = vec2<f32>(0.24, 0.11);
const RIPPLE_SKIP: f32 = 0.55;
const RIPPLE_BOW: f32 = 0.02;
const TORRENT_FROTH: f32 = 0.12;
const CURRENT_FADE: vec2<f32> = vec2<f32>(10.0, 18.0);

// Ice (see `draw_ice`): its tint covers `ICE_TINT` of what lies beneath, glints lighten it by
// `ICE_GLINT`, and white cracks `ICE_CRACK_HALF_WIDTH` map units either side, lightening it by
// `ICE_CRACK_LIGHT`, split it into floes about `ICE_FLOE` map units across.
const ICE_TINT: f32 = 0.7;
const ICE_GLINT: f32 = 0.18;
const ICE_FLOE: f32 = 0.55;
const ICE_CRACK_HALF_WIDTH: f32 = 0.007;
const ICE_CRACK_LIGHT: f32 = 0.75;

// Reliefs lit from the upper left (see `draw_relief`): a slope facing `RELIEF_LIGHT_FROM`
// lightens the ground by up to `RELIEF_MAX`, and one facing away darkens it as far toward
// `RELIEF_SHADOW` of its color. Slopes are measured `RELIEF_STEP` apart in the relief's own
// units.
const RELIEF_LIGHT_FROM: vec2<f32> = vec2<f32>(0.6, 0.8);
const RELIEF_MAX: f32 = 0.35;
const RELIEF_SHADOW: f32 = 0.55;
const RELIEF_STEP: f32 = 0.05;

// Hummocks on rough ground (see `draw_hummocks`): lumps `HUMMOCK_SCALE` to a map unit whose
// slopes light the ground by `HUMMOCK_RELIEF` per unit of steepness; ultra rough ground packs
// them closer, rougher and steeper.
const HUMMOCK_SCALE: f32 = 4.5;
const HUMMOCK_RELIEF: f32 = 0.19;
const ULTRA_HUMMOCK_SCALE: f32 = 6.0;
const ULTRA_HUMMOCK_RELIEF: f32 = 0.31;

// Dunes on sand (see `draw_dunes`): crests `DUNE_SPACING` map units apart, running across
// `DUNE_AXIS` and wandering up to `DUNE_WANDER` map units either way, each a long windward rise
// and a steep lee face `DUNE_LEE` of the way from crest to crest. Their slopes light the sand by
// `DUNE_RELIEF` per unit of steepness.
const DUNE_SPACING: f32 = 0.6;
const DUNE_AXIS: vec2<f32> = vec2<f32>(0.287, 0.958);
const DUNE_WANDER: f32 = 0.15;
const DUNE_LEE: f32 = 0.25;
const DUNE_RELIEF: f32 = 0.05;

// Debris on rubble (see `draw_debris`): broken blocks whose edges are the ground's color
// darkened to `BLOCK_EDGE` and whose top faces it lightened by `BLOCK_FACE`, each face inset
// `BLOCK_INSET` map units from the block's outline and shifted `BLOCK_FACE_SHIFT` up and to the
// left, casting shadows `DEBRIS_SHADOW` map units toward the lower right that darken the ground
// to `DEBRIS_SHADOW_SHADE`.
const BLOCK_EDGE: f32 = 0.5;
const BLOCK_FACE: f32 = 0.22;
const BLOCK_INSET: f32 = 0.012;
const BLOCK_FACE_SHIFT: vec2<f32> = vec2<f32>(-0.006, -0.009);
const DEBRIS_SHADOW: vec2<f32> = vec2<f32>(0.018, 0.026);
const DEBRIS_SHADOW_SHADE: f32 = 0.68;

// Ruins on rubble (see `draw_ruins`): fragments of wall `RUIN_WALL_HALF_WIDTH` map units either
// side of their line, in the ground's color darkened to `RUIN_WALL_SHADE` with tops lightened
// by `RUIN_WALL_TOP`, casting shadows like debris.
const RUIN_WALL_HALF_WIDTH: f32 = 0.034;
const RUIN_WALL_SHADE: f32 = 0.42;
const RUIN_WALL_TOP: f32 = 0.18;

// Wind ripples on sand (see `draw_ripples`): lines `RIPPLE_SPACING` apart that wave
// `RIPPLE_AMPLITUDE` north and south over every `RIPPLE_WAVELENGTH`, `RIPPLE_HALF_WIDTH`
// either side, all in map units, in the sand's color darkened to `RIPPLE_SHADE`. They fade in
// across `RIPPLE_FADE` hex radii in logical pixels.
const RIPPLE_SPACING: f32 = 0.3;
const RIPPLE_AMPLITUDE: f32 = 0.06;
const RIPPLE_WAVELENGTH: f32 = 1.3;
const RIPPLE_HALF_WIDTH: f32 = 0.018;
const RIPPLE_SHADE: f32 = 0.8;
const RIPPLE_FADE: vec2<f32> = vec2<f32>(10.0, 18.0);

// Reed tufts (see `tuft_distance`): three blades `TUFT_HEIGHT` map units tall with strokes
// `TUFT_HALF_WIDTH` either side.
const TUFT_HEIGHT: f32 = 0.1;
const TUFT_HALF_WIDTH: f32 = 0.014;

// Paving slabs (see `draw_slabs`): `SLAB_SIZE` map units square, laid in a grid, each shaded
// between `SLAB_TONES`, with joints `SEAM_HALF_WIDTH` map units either side in the pavement's
// color darkened to `SEAM_SHADE`. `STAINED` of slabs carry an oil stain darkening them to
// `STAIN_SHADE`, and `CRACKED` a hairline crack `CRACK_HALF_WIDTH` either side darkened to
// `CRACK_SHADE`.
const SLAB_SIZE: f32 = 0.48;
const SLAB_TONES: vec2<f32> = vec2<f32>(0.93, 1.06);
const SEAM_HALF_WIDTH: f32 = 0.012;
const SEAM_SHADE: f32 = 0.82;
const STAINED: f32 = 0.25;
const STAIN_SHADE: f32 = 0.8;
const CRACKED: f32 = 0.3;
const CRACK_HALF_WIDTH: f32 = 0.004;
const CRACK_SHADE: f32 = 0.7;

// Rolling plains on clear ground (see `draw_plains`): broad swells `PLAIN_SWELL` map units
// across, with smaller ones half that size, whose slopes light the ground by `PLAIN_RELIEF`
// per unit of steepness.
const PLAIN_SWELL: f32 = 1.8;
const PLAIN_RELIEF: f32 = 0.12;

// Bog on swamp (see `draw_bog`): low hummocks `BOG_SCALE` map units across whose slopes light
// the ground by `BOG_RELIEF` per unit of steepness. Hollows below `POOL_LEVEL` of the bog's
// height hold murky pools, the water color mixed `POOL_MURK` of the way to the swamp's
// darkened to `POOL_MUD`, and darken toward their middles; the ground within `MUD_RISE` above
// them darkens to `MUD_SHADE`. Reed tufts, at most one per cell `REED_CELL` map units square
// with `REED_SKIP` of cells left bare, stand on the dry ground in the swamp's color darkened to
// `REED_SHADE`, casting shadows `REED_SHADOW` map units toward the lower right.
const BOG_SCALE: f32 = 0.55;
const BOG_RELIEF: f32 = 0.1;
const POOL_LEVEL: f32 = 0.5;
const POOL_MURK: f32 = 0.5;
const POOL_MUD: f32 = 0.6;
const MUD_RISE: f32 = 0.14;
const MUD_SHADE: f32 = 0.75;
const REED_CELL: f32 = 0.3;
const REED_SKIP: f32 = 0.35;
const REED_SHADE: f32 = 0.55;
const REED_SHADOW: vec2<f32> = vec2<f32>(0.015, 0.02);

// Tundra (see `draw_tundra`): tussocks `TUSSOCK_SCALE` map units across whose slopes light the
// ground by `TUSSOCK_RELIEF` per unit of steepness, under patches of pale lichen about
// `LICHEN_PATCH` map units across, lightened by `LICHEN_LIGHT`, and dark moss about
// `MOSS_PATCH` across, tinted by `MOSS_TINT`, both covering `PATCH_COVER` of what lies beneath.
// Pale specks, one in `SPECK_SHARE` of cells `SPECK_CELL` map units across, lighten it by
// `SPECK_LIGHT`.
const TUSSOCK_SCALE: f32 = 0.22;
const TUSSOCK_RELIEF: f32 = 0.05;
const LICHEN_PATCH: f32 = 0.32;
const LICHEN_LIGHT: f32 = 0.28;
const MOSS_PATCH: f32 = 0.4;
const MOSS_TINT: vec3<f32> = vec3<f32>(0.7, 0.82, 0.62);
const PATCH_COVER: f32 = 0.85;
const SPECK_CELL: f32 = 0.035;
const SPECK_SHARE: f32 = 0.06;
const SPECK_LIGHT: f32 = 0.4;

// Lava (see `draw_magma` and `draw_magma_crust`): cells around feature points `LAVA_CELL` map
// units apart. Molten magma glows `LAVA_HOT` toward the middle of each cell and cools to
// `LAVA_COOL` of its color along the seams between cells, `LAVA_SEAM` either side; a crust
// splits along glowing `LAVA_CRACK` cracks, `LAVA_CRACK_HALF_WIDTH` either side, that light
// the crust around them out to `LAVA_GLOW`.
const LAVA_CELL: f32 = 0.5;
const LAVA_HOT: vec3<f32> = vec3<f32>(1.0, 0.78, 0.2);
const LAVA_COOL: f32 = 0.62;
const LAVA_SEAM: f32 = 0.03;
const LAVA_CRACK: vec3<f32> = vec3<f32>(1.0, 0.45, 0.08);
const LAVA_CRACK_HALF_WIDTH: f32 = 0.016;
const LAVA_GLOW: f32 = 0.07;

// Heavy industry (see `draw_industry`): a concrete yard of slabs `YARD_SLAB` map units square
// with joints darkened to `YARD_JOINT_SHADE`, divided into big lots `BIG_LOT` map units square.
// `BIG_LOT_SHARE` of them hold one large structure; the rest split into four small lots. Each
// structure casts a shadow `STRUCTURE_SHADOW` map units toward the lower right that darkens what
// lies beneath to `STRUCTURE_SHADOW_SHADE`.
const YARD_SLAB: f32 = 0.4;
const YARD_JOINT_SHADE: f32 = 0.86;
const BIG_LOT: f32 = 0.8;
const BIG_LOT_SHARE: f32 = 0.45;
const STRUCTURE_SHADOW: vec2<f32> = vec2<f32>(0.03, 0.04);
const STRUCTURE_SHADOW_SHADE: f32 = 0.64;
// The shares of big and small lots given to warehouses, cooling towers, tank clusters and
// chimneyed sheds; whatever is left over stays open yard.
const BIG_LOT_ODDS: vec4<f32> = vec4<f32>(0.6, 0.4, 0.0, 0.0);
const SMALL_LOT_ODDS: vec4<f32> = vec4<f32>(0.35, 0.0, 0.35, 0.22);

// Woods and jungle (see `draw_stand`): trees with round crowns and palms with star-shaped
// fronds, each casting a shadow `PLANT_SHADOW` map units toward the lower right that darkens
// what lies beneath to `PLANT_SHADOW_SHADE`. Palms spread `FRONDS` fronds. Where a stand
// fades out at a small zoom it takes a flat color, its leaf color mixed over the ground by
// `stand_shape`'s cover.
const PLANT_SHADOW: vec2<f32> = vec2<f32>(0.035, 0.05);
const PLANT_SHADOW_SHADE: f32 = 0.6;
const FRONDS: f32 = 6.0;

// Hex radii in logical pixels over which ground textures fade in, below which they would only
// smudge the ground's color.
const GROUND_FADE: vec2<f32> = vec2<f32>(8.0, 16.0);

// Digit half height in map units for the label rows, and how far the top and bottom rows sit
// from the hex center.
const ROW_SIZE: f32 = 0.17;
const ROW_OFFSET: f32 = 0.5;

const SQRT_3: f32 = 1.7320508;
const APOTHEM: f32 = 0.8660254;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // Logical-pixel position within the widget.
    @location(0) pixel: vec2<f32>,
};

// One triangle that covers the whole viewport, which is set to the widget's bounds.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VertexOutput;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.pixel = uv * u.size;
    return out;
}

// Center of a hex in map units.
fn center(hex: vec2<i32>) -> vec2<f32> {
    let stagger = select(0.5, 1.0, (hex.x & 1) == 0);
    return vec2<f32>(1.0 + 1.5 * f32(hex.x), SQRT_3 * (f32(hex.y) + stagger));
}

// The hex containing a map point: the one with the nearest center, which is always within
// one column and one row of a rough estimate.
fn hex_at(point: vec2<f32>) -> vec2<i32> {
    let column = i32(round((point.x - 1.0) / 1.5));
    var best = vec2<i32>(0, 0);
    var best_distance = 1e9;
    for (var dx = -1; dx <= 1; dx++) {
        let x = column + dx;
        let stagger = select(0.5, 1.0, (x & 1) == 0);
        let row = i32(round(point.y / SQRT_3 - stagger));
        for (var dy = -1; dy <= 1; dy++) {
            let hex = vec2<i32>(x, row + dy);
            let d = distance(point, center(hex));
            if d < best_distance {
                best_distance = d;
                best = hex;
            }
        }
    }
    return best;
}

// Cube coordinates for hex distance, matching `HexCoordinate::distance`.
fn cube(hex: vec2<i32>) -> vec3<i32> {
    let q = hex.x;
    let r = hex.y - (q + (q & 1)) / 2;
    return vec3<i32>(q, r, -q - r);
}

fn hex_distance(a: vec2<i32>, b: vec2<i32>) -> i32 {
    let d = abs(cube(a) - cube(b));
    return max(d.x, max(d.y, d.z));
}

// Distance from a point inside a unit hex, relative to its center, to the nearest edge.
fn edge_distance(local: vec2<f32>) -> f32 {
    let q = abs(local);
    return APOTHEM - max(q.y, q.x * APOTHEM + q.y * 0.5);
}

// Distance from `p` to the segment from `a` to `b`.
fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Distance to a seven-segment digit spanning x in [-0.5, 0.5] and y in [-1, 1].
fn digit_distance(p: vec2<f32>, digit: u32) -> f32 {
    // Segments a to g as bits 0 to 6.
    var masks = array<u32, 10>(0x3Fu, 0x06u, 0x5Bu, 0x4Fu, 0x66u, 0x6Du, 0x7Du, 0x07u, 0x7Fu, 0x6Fu);
    var segments = array<vec4<f32>, 7>(
        vec4<f32>(-0.5, -1.0, 0.5, -1.0),
        vec4<f32>(0.5, -1.0, 0.5, 0.0),
        vec4<f32>(0.5, 0.0, 0.5, 1.0),
        vec4<f32>(-0.5, 1.0, 0.5, 1.0),
        vec4<f32>(-0.5, 0.0, -0.5, 1.0),
        vec4<f32>(-0.5, -1.0, -0.5, 0.0),
        vec4<f32>(-0.5, 0.0, 0.5, 0.0),
    );
    let mask = masks[digit];
    var d = 1e9;
    for (var i = 0u; i < 7u; i++) {
        if ((mask >> i) & 1u) == 1u {
            let s = segments[i];
            d = min(d, segment(p, s.xy, s.zw));
        }
    }
    return d;
}

// Leading signs a number can carry.
const SIGN_NONE: u32 = 0u;
const SIGN_MINUS: u32 = 1u;
const SIGN_PLUS: u32 = 2u;

// The sign for a value measured `direction` (SIGN_MINUS or SIGN_PLUS) from a surface: none
// for zero, which lies on the surface itself.
fn sign_for(value: u32, direction: u32) -> u32 {
    return select(direction, SIGN_NONE, value == 0u);
}

// Glyphs in a number of up to two digits, counting a leading sign.
fn glyph_count(value: u32, sign: u32) -> u32 {
    return select(1u, 2u, value >= 10u) + select(1u, 0u, sign == SIGN_NONE);
}

// Distance to a number of up to two digits with an optional leading sign, centered on the
// origin, in digit units.
fn number_distance(p: vec2<f32>, value: u32, sign: u32) -> f32 {
    let count = glyph_count(value, sign);
    // Glyph centers are 1.7 digit units apart, the first at the left.
    let first = -0.85 * f32(count - 1u);
    var d = digit_distance(p - vec2<f32>(first + 1.7 * f32(count - 1u), 0.0), value % 10u);
    if value >= 10u {
        d = min(d, digit_distance(p - vec2<f32>(first + 1.7 * f32(count - 2u), 0.0), (value / 10u) % 10u));
    }
    let at = p - vec2<f32>(first, 0.0);
    if sign != SIGN_NONE {
        d = min(d, segment(at, vec2<f32>(-0.4, 0.0), vec2<f32>(0.4, 0.0)));
    }
    if sign == SIGN_PLUS {
        d = min(d, segment(at, vec2<f32>(0.0, -0.4), vec2<f32>(0.0, 0.4)));
    }
    return d;
}

// `color` with `value` written over it, centered `y` map units below the hex center and led
// by `sign` (a SIGN_ value). `size` is the half height of a lone digit in map units; longer
// numbers are drawn smaller to fit. The ink is black or white, whichever contrasts with the
// hex's `backdrop`, so a whole number keeps one ink over patterned foliage, fire or smoke, and
// a halo of the other shade keeps it readable over the pattern.
fn ink_number(
    color: vec3<f32>,
    backdrop: vec3<f32>,
    local: vec2<f32>,
    y: f32,
    size: f32,
    value: u32,
    sign: u32,
) -> vec3<f32> {
    var scales = array<f32, 3>(1.0, 0.8, 0.65);
    let half_height = u.radius * size * scales[glyph_count(value, sign) - 1u];
    let p = (local - vec2<f32>(0.0, y)) * u.radius / half_height;
    let stroke = (number_distance(p, value, sign) - 0.16) * half_height;
    let luminance = dot(backdrop, vec3<f32>(0.299, 0.587, 0.114));
    let dark = luminance > palette[INK_THRESHOLD].r;
    let ink = select(vec3<f32>(1.0), vec3<f32>(0.0), dark);
    let halo = select(vec3<f32>(0.0), vec3<f32>(1.0), dark);
    let halo_width = max(1.0, half_height * 0.18);
    let haloed = mix(color, halo, 0.8 * clamp(0.5 + halo_width - stroke, 0.0, 1.0));
    return mix(haloed, ink, clamp(0.5 - stroke, 0.0, 1.0));
}

// `color` mixed toward white by `amount`.
fn lighten(color: vec3<f32>, amount: f32) -> vec3<f32> {
    return mix(color, vec3<f32>(1.0), clamp(amount, 0.0, 0.7));
}

// A layer stored as one plus its position, or zero for none, as its position.
fn present(stored: u32) -> bool {
    return stored != 0u;
}

// A pseudo-random value in [0, 1) for a cell given by whole-number coordinates, for speckle
// and for scattering marks. An integer hash (PCG2D), so neighboring cells stay unrelated
// however far the cell lies from the origin.
fn hash(cell: vec2<f32>) -> f32 {
    var v = bitcast<vec2<u32>>(vec2<i32>(cell)) * 1664525u + 1013904223u;
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    return f32(v.x >> 8u) / 16777216.0;
}

// How much of a pixel `size` map units outside a shape (negative inside) the shape covers.
fn coverage(outside: f32) -> f32 {
    return clamp(0.5 - outside * u.radius, 0.0, 1.0);
}

// Smooth noise in [0, 1) at `p`: hashes at whole-number corners, blended between them.
fn value_noise(p: vec2<f32>) -> f32 {
    let corner = floor(p);
    let f = fract(p);
    let blend = f * f * (3.0 - 2.0 * f);
    let top = mix(hash(corner), hash(corner + vec2<f32>(1.0, 0.0)), blend.x);
    let below = corner + vec2<f32>(0.0, 1.0);
    let bottom = mix(hash(below), hash(below + vec2<f32>(1.0, 0.0)), blend.x);
    return mix(top, bottom, blend.y);
}

// `color` shaded by `light`, a relief's lighting from -1 (facing away) to 1 (facing the light)
// before `RELIEF_MAX` limits it, fading to plain `color` when the hexes are too small to show
// it.
fn draw_relief(color: vec3<f32>, light: f32) -> vec3<f32> {
    let amount = clamp(light, -RELIEF_MAX, RELIEF_MAX);
    let shadowed = mix(color, color * RELIEF_SHADOW, -amount);
    let lit = select(shadowed, lighten(color, amount), amount > 0.0);
    return mix(color, lit, ground_fade());
}

// The height of rough ground's hummocks at `p`, in hummock units: two layers of noise, and a
// third, finer one on ultra rough ground.
fn hummock_height(p: vec2<f32>, ultra: bool) -> f32 {
    var height = value_noise(p + vec2<f32>(60.0, 0.0));
    height += 0.5 * value_noise(p * 2.1 + vec2<f32>(65.0, 9.0));
    if ultra {
        height += 0.3 * value_noise(p * 4.3 + vec2<f32>(62.0, 3.0));
    }
    return height;
}

// `color`, bare rough ground at map position `point`, shaded into hummocks lit from the upper
// left: gentle lumps on rough ground, and closer, broken ones on ultra rough ground. They are
// set by map position rather than by hex, so they run on across neighboring rough ground.
fn draw_hummocks(color: vec3<f32>, point: vec2<f32>, ultra: bool) -> vec3<f32> {
    let p = point * select(HUMMOCK_SCALE, ULTRA_HUMMOCK_SCALE, ultra);
    let height = hummock_height(p, ultra);
    let east = hummock_height(p + vec2<f32>(RELIEF_STEP, 0.0), ultra) - height;
    let south = hummock_height(p + vec2<f32>(0.0, RELIEF_STEP), ultra) - height;
    let slope = vec2<f32>(east, south) / RELIEF_STEP;
    let relief = select(HUMMOCK_RELIEF, ULTRA_HUMMOCK_RELIEF, ultra);
    return draw_relief(color, dot(slope, RELIEF_LIGHT_FROM) * relief);
}

// The height of sand's dunes at map position `point`, from 0 in a trough to 1 on a crest: a
// long rise into the wind, then a short drop down the lee face.
fn dune_height(point: vec2<f32>) -> f32 {
    let wander = (value_noise(point * 0.9 + vec2<f32>(90.0, 0.0)) - 0.5) * 2.0 * DUNE_WANDER;
    let phase = fract((dot(point, DUNE_AXIS) + wander) / DUNE_SPACING);
    let windward = phase / (1.0 - DUNE_LEE);
    let lee = (1.0 - phase) / DUNE_LEE;
    return smoothstep(0.0, 1.0, select(lee, windward, phase < 1.0 - DUNE_LEE));
}

// `color`, bare sand at map position `point`, shaded into dunes lit from the upper left, with
// wind ripples over them. They are set by map position rather than by hex, so they run on
// across neighboring sand.
fn draw_dunes(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let step = 0.01;
    let height = dune_height(point);
    let east = dune_height(point + vec2<f32>(step, 0.0)) - height;
    let south = dune_height(point + vec2<f32>(0.0, step)) - height;
    let slope = vec2<f32>(east, south) / step;
    let dunes = draw_relief(color, dot(slope, RELIEF_LIGHT_FROM) * DUNE_RELIEF);
    return draw_ripples(dunes, point);
}

// Distance from `q`, in a block's own frame, to a block with half extents `half` that has
// lost the corner `corner` points toward.
fn broken_block_distance(q: vec2<f32>, half: vec2<f32>, corner: vec2<f32>) -> f32 {
    let block = box_distance(q, vec4<f32>(-half, half));
    let cut = dot(q, normalize(corner)) - 0.6 * dot(half, abs(normalize(corner)));
    return max(block, cut);
}

// `p` turned `angle` radians counterclockwise about the origin, so a shape drawn turned
// clockwise by `angle` can be measured in its own frame.
fn turned(p: vec2<f32>, angle: f32) -> vec2<f32> {
    return vec2<f32>(p.x * cos(angle) + p.y * sin(angle), p.y * cos(angle) - p.x * sin(angle));
}

// `color` with debris strewn over it at map position `point`: broken blocks of `ground`'s
// color, one to each cell `size` map units across that is not among the `skip` share left
// bare, the largest reaching `largest` either side of its center. Each casts a shadow, is
// turned and sized at random, and has lost an upper corner. `seed`, a whole number, sets
// different scatters apart.
fn draw_debris(
    color: vec3<f32>,
    point: vec2<f32>,
    ground: vec3<f32>,
    size: f32,
    skip: f32,
    largest: f32,
    seed: f32,
) -> vec3<f32> {
    var result = color;
    let cell = floor(point / size);
    let fade = ground_fade();
    for (var dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (var dx = -1.0; dx <= 1.0; dx += 1.0) {
            let c = cell + vec2<f32>(dx, dy);
            let keyed = c + vec2<f32>(seed, seed * 3.0);
            if hash(keyed) < skip {
                continue;
            }
            let jitter = vec2<f32>(
                hash(keyed + vec2<f32>(1.0, 7.0)),
                hash(keyed + vec2<f32>(5.0, 3.0)),
            );
            let center = (c + 0.5 + (jitter - 0.5) * 0.6) * size;
            let scale = largest * mix(0.45, 1.0, hash(keyed + vec2<f32>(2.0, 9.0)));
            let aspect = mix(0.55, 0.85, hash(keyed + vec2<f32>(6.0, 1.0)));
            let half = vec2<f32>(scale, scale * aspect);
            let turn = hash(keyed + vec2<f32>(4.0, 4.0)) * PI;
            let east_corner = hash(keyed + vec2<f32>(8.0, 8.0)) > 0.5;
            let corner = vec2<f32>(select(-1.0, 1.0, east_corner), -1.0);
            let q = turned(point - center, turn);
            let shadow_q = turned(point - center - DEBRIS_SHADOW, turn);
            let shadow = coverage(broken_block_distance(shadow_q, half, corner));
            result = mix(result, result * DEBRIS_SHADOW_SHADE, shadow * fade);
            let body = coverage(broken_block_distance(q, half, corner));
            let face_q = turned(point - center - BLOCK_FACE_SHIFT, turn);
            let face = coverage(broken_block_distance(face_q, half, corner) + BLOCK_INSET);
            let block = mix(ground * BLOCK_EDGE, lighten(ground, BLOCK_FACE), face);
            result = mix(result, block, body * fade);
        }
    }
    return result;
}

// `color`, bare rubble at map position `point`, with the ruins of buildings over it: broken
// runs and corners of wall, one to each cell that is not left bare, among scattered debris.
// Ultra rubble packs ruins and debris closer. They are set by map position rather than by
// hex, so they run on across neighboring rubble.
fn draw_ruins(color: vec3<f32>, point: vec2<f32>, ultra: bool) -> vec3<f32> {
    let size = select(0.6, 0.45, ultra);
    let skip = select(0.3, 0.05, ultra);
    let cell = floor(point / size);
    let fade = ground_fade();
    var result = color;
    for (var dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (var dx = -1.0; dx <= 1.0; dx += 1.0) {
            let c = cell + vec2<f32>(dx, dy);
            let keyed = c + vec2<f32>(33.0, 0.0);
            if hash(keyed) < skip {
                continue;
            }
            let jitter = vec2<f32>(
                hash(keyed + vec2<f32>(1.0, 7.0)),
                hash(keyed + vec2<f32>(5.0, 3.0)),
            );
            let center = (c + 0.5 + (jitter - 0.5) * 0.4) * size;
            let turn = hash(keyed + vec2<f32>(4.0, 4.0)) * PI * 0.5;
            let q = turned(point - center, turn);
            // A corner of wall: a run east and, most often, a run south, each broken off
            // partway.
            let reach = size * mix(0.32, 0.46, hash(keyed + vec2<f32>(2.0, 2.0)));
            let east = vec2<f32>(reach * mix(0.5, 1.0, hash(keyed + vec2<f32>(6.0, 6.0))), 0.0);
            let south = vec2<f32>(0.0, reach * mix(0.3, 1.0, hash(keyed + vec2<f32>(7.0, 3.0))));
            let cornered = hash(keyed + vec2<f32>(3.0, 8.0)) > 0.3;
            var wall = segment(q, vec2<f32>(0.0), east);
            var shadow_line = segment(q - DEBRIS_SHADOW, vec2<f32>(0.0), east);
            if cornered {
                wall = min(wall, segment(q, vec2<f32>(0.0), south));
                shadow_line = min(shadow_line, segment(q - DEBRIS_SHADOW, vec2<f32>(0.0), south));
            }
            let shadow = coverage(shadow_line - RUIN_WALL_HALF_WIDTH);
            result = mix(result, result * DEBRIS_SHADOW_SHADE, shadow * fade);
            let standing = coverage(wall - RUIN_WALL_HALF_WIDTH);
            result = mix(result, color * RUIN_WALL_SHADE, standing * fade);
            let top = coverage(wall - RUIN_WALL_HALF_WIDTH * 0.45);
            result = mix(result, lighten(color, RUIN_WALL_TOP), top * fade * 0.6);
        }
    }
    let debris_size = select(0.26, 0.2, ultra);
    let debris_skip = select(0.6, 0.35, ultra);
    return draw_debris(result, point, color, debris_size, debris_skip, 0.05, 14.0);
}

// `color`, bare sand, with wind ripples over it at map position `point`: wavy lines broken
// into stretches, set by map position rather than by hex so they run on across neighboring
// sand.
fn draw_ripples(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let k = 2.0 * PI / RIPPLE_WAVELENGTH;
    let v = point.y + RIPPLE_AMPLITUDE * sin(point.x * k);
    let slope = RIPPLE_AMPLITUDE * k * cos(point.x * k);
    let row = round(v / RIPPLE_SPACING);
    let d = abs(v - row * RIPPLE_SPACING) / sqrt(1.0 + slope * slope);
    // Each ripple breaks off and resumes at its own place along the row.
    let stretch = smoothstep(0.0, 0.3, sin(point.x * 2.3 + row * 2.1));
    let fade = smoothstep(RIPPLE_FADE.x, RIPPLE_FADE.y, u.radius);
    return mix(color, color * RIPPLE_SHADE, coverage(d - RIPPLE_HALF_WIDTH) * stretch * fade);
}

// A mark scattered over map position `point`: in staggered rows of cells `size` map units
// square, all but `skip` of them hold one mark, nudged off the cell's middle by a hash of the
// cell and the whole number `seed`, which sets different textures' marks apart. Returns where
// `point` lies relative to the mark's base, in map units, and one if this cell holds a mark or
// zero if not.
fn scatter(point: vec2<f32>, size: f32, skip: f32, seed: f32) -> vec3<f32> {
    let row = floor(point.y / size);
    let shifted = vec2<f32>(point.x / size + 0.5 * (row % 2.0), point.y / size);
    let cell = floor(shifted);
    let keyed = cell + vec2<f32>(seed, seed * 3.0);
    let jitter = vec2<f32>(hash(keyed + vec2<f32>(1.0, 7.0)), hash(keyed + vec2<f32>(5.0, 3.0)));
    // The mark's base, in cell units.
    let base = vec2<f32>(0.5, 0.65) + (jitter - 0.5) * 0.3;
    return vec3<f32>((shifted - cell - base) * size, select(1.0, 0.0, hash(keyed) < skip));
}

// Distance from `p` to a tuft of three blades fanning up from the origin.
fn tuft_distance(p: vec2<f32>) -> f32 {
    var d = segment(p, vec2<f32>(0.0), vec2<f32>(0.0, -TUFT_HEIGHT));
    d = min(d, segment(p, vec2<f32>(0.0), vec2<f32>(-0.06, -0.75 * TUFT_HEIGHT)));
    return min(d, segment(p, vec2<f32>(0.0), vec2<f32>(0.06, -0.75 * TUFT_HEIGHT)));
}

// How strongly ground textures show at the current zoom.
fn ground_fade() -> f32 {
    return smoothstep(GROUND_FADE.x, GROUND_FADE.y, u.radius);
}

// `color`, bare pavement at map position `point`, laid as a concrete lot: a grid of square
// slabs of slightly different shades between darker joints, some carrying an oil stain or a
// hairline crack. It runs on across neighboring pavement.
fn draw_slabs(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let fade = ground_fade();
    let slab = floor(point / SLAB_SIZE);
    let within = point / SLAB_SIZE - slab;
    let tone = mix(SLAB_TONES.x, SLAB_TONES.y, hash(slab + vec2<f32>(600.0, 0.0)));
    var result = color * mix(1.0, tone, fade);
    if hash(slab + vec2<f32>(601.0, 2.0)) < STAINED {
        let spot_x = hash(slab + vec2<f32>(602.0, 0.0));
        let spot = vec2<f32>(spot_x, hash(slab + vec2<f32>(603.0, 0.0)));
        let from_spot = (within - (spot * 0.6 + 0.2)) * vec2<f32>(1.0, 1.4);
        let stain = smoothstep(0.22, 0.05, length(from_spot));
        result = mix(result, result * STAIN_SHADE, stain * fade);
    }
    if hash(slab + vec2<f32>(604.0, 5.0)) < CRACKED {
        let west = vec2<f32>(0.0, hash(slab + vec2<f32>(605.0, 0.0)));
        let east = vec2<f32>(1.0, hash(slab + vec2<f32>(606.0, 0.0)));
        let crack = coverage(segment(within, west, east) * SLAB_SIZE - CRACK_HALF_WIDTH);
        result = mix(result, color * CRACK_SHADE, crack * fade);
    }
    let seams = abs(fract(point / SLAB_SIZE + 0.5) - 0.5);
    let d = min(seams.x, seams.y) * SLAB_SIZE;
    return mix(result, color * SEAM_SHADE, coverage(d - SEAM_HALF_WIDTH) * fade);
}

// The height of clear ground's swells at map position `point`.
fn plain_height(point: vec2<f32>) -> f32 {
    let broad = value_noise(point / PLAIN_SWELL + vec2<f32>(120.0, 40.0));
    return broad + 0.35 * value_noise(point * 2.0 / PLAIN_SWELL + vec2<f32>(133.0, 41.0));
}

// `color`, bare clear ground at map position `point`, shaded into broad, gentle swells lit
// from the upper left, so open ground reads as rolling plains rather than a flat fill. They are
// set by map position rather than by hex, so they run on across neighboring clear ground.
fn draw_plains(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let step = 0.02;
    let height = plain_height(point);
    let east = plain_height(point + vec2<f32>(step, 0.0)) - height;
    let south = plain_height(point + vec2<f32>(0.0, step)) - height;
    let slope = vec2<f32>(east, south) / step;
    return draw_relief(color, dot(slope, RELIEF_LIGHT_FROM) * PLAIN_RELIEF);
}

// The height of a swamp's bog at map position `point`, in bog units: low hummocks with
// smaller ones over them.
fn bog_height(point: vec2<f32>) -> f32 {
    let p = point / BOG_SCALE;
    let small = value_noise(p * 2.2 + vec2<f32>(160.0, 22.0));
    return value_noise(p + vec2<f32>(150.0, 20.0)) + 0.5 * small;
}

// `color`, bare swamp at map position `point`: a bog of low hummocks lit from the upper left,
// with murky pools standing in its hollows, darker toward their middles and ringed by wet mud,
// and reed tufts casting shadows on the drier ground. It is set by map position rather than by
// hex, so it runs on across neighboring swamp.
fn draw_bog(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let fade = ground_fade();
    let step = 0.01;
    let height = bog_height(point);
    let east = bog_height(point + vec2<f32>(step, 0.0)) - height;
    let south = bog_height(point + vec2<f32>(0.0, step)) - height;
    let slope = vec2<f32>(east, south) / step;
    var result = draw_relief(color, dot(slope, RELIEF_LIGHT_FROM) * BOG_RELIEF * BOG_SCALE);
    let mud = smoothstep(POOL_LEVEL + MUD_RISE, POOL_LEVEL, height);
    result = mix(result, result * MUD_SHADE, mud * fade);
    // The pool's edge, antialiased across the height the bog changes by in half a pixel.
    let edge_blur = max(0.5 * length(slope) / u.radius, 0.001);
    let pool = smoothstep(POOL_LEVEL + edge_blur, POOL_LEVEL - edge_blur, height);
    let depth = clamp((POOL_LEVEL - height) / POOL_LEVEL, 0.0, 1.0);
    let murky = mix(palette[WATER].rgb, color * POOL_MUD, POOL_MURK) * (1.0 - 0.35 * depth);
    result = mix(result, murky, pool * fade);
    let mark = scatter(point, REED_CELL, REED_SKIP, 17.0);
    // Reeds grow only out of the water.
    let dry = 1.0 - pool;
    let shadow = coverage(tuft_distance(mark.xy - REED_SHADOW) - TUFT_HALF_WIDTH) * mark.z;
    result = mix(result, result * DEBRIS_SHADOW_SHADE, shadow * dry * fade);
    let reeds = coverage(tuft_distance(mark.xy) - TUFT_HALF_WIDTH) * mark.z;
    return mix(result, color * REED_SHADE, reeds * dry * fade);
}

// The height of tundra's tussocks at map position `point`.
fn tussock_height(point: vec2<f32>) -> f32 {
    return value_noise(point / TUSSOCK_SCALE + vec2<f32>(180.0, 60.0));
}

// `color`, bare tundra at map position `point`: small tussocks lit from the upper left, under
// patches of pale lichen and dark moss and a scatter of pale specks. It is set by map position
// rather than by hex, so it runs on across neighboring tundra.
fn draw_tundra(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let fade = ground_fade();
    let step = 0.01;
    let height = tussock_height(point);
    let east = tussock_height(point + vec2<f32>(step, 0.0)) - height;
    let south = tussock_height(point + vec2<f32>(0.0, step)) - height;
    let slope = vec2<f32>(east, south) / step;
    let light = dot(slope, RELIEF_LIGHT_FROM) * TUSSOCK_RELIEF * TUSSOCK_SCALE;
    var result = draw_relief(color, light);
    let lichen_noise = value_noise(point / LICHEN_PATCH + vec2<f32>(200.0, 10.0));
    let moss_noise = value_noise(point / MOSS_PATCH + vec2<f32>(220.0, 70.0));
    let lichen = smoothstep(0.58, 0.68, lichen_noise);
    let moss = smoothstep(0.6, 0.7, moss_noise);
    let pale = lighten(color * vec3<f32>(0.98, 1.0, 0.95), LICHEN_LIGHT);
    result = mix(result, pale, lichen * fade * PATCH_COVER);
    result = mix(result, color * MOSS_TINT, moss * fade * PATCH_COVER);
    let speck = hash(floor(point / SPECK_CELL) + vec2<f32>(5.0, 9.0)) < SPECK_SHARE;
    return mix(result, lighten(color, SPECK_LIGHT), select(0.0, 0.6 * fade, speck));
}

// Distances from `point` to the nearest and second nearest of feature points scattered one
// to each cell `size` map units square.
fn cell_distances(point: vec2<f32>, size: f32) -> vec2<f32> {
    let p = point / size;
    let cell = floor(p);
    var nearest = vec2<f32>(9.0, 9.0);
    for (var dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (var dx = -1.0; dx <= 1.0; dx += 1.0) {
            let c = cell + vec2<f32>(dx, dy);
            let feature = c + 0.15 + 0.7 * vec2<f32>(hash(c), hash(c + vec2<f32>(9.0, 4.0)));
            let d = distance(p, feature);
            if d < nearest.x {
                nearest = vec2<f32>(d, nearest.x);
            } else if d < nearest.y {
                nearest.y = d;
            }
        }
    }
    return nearest * size;
}

// `color`, molten magma, at map position `point`: glowing hotter toward the middle of each
// lava cell and cooling darker along the seams between them.
fn draw_magma(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let cells = cell_distances(point, LAVA_CELL);
    let fade = ground_fade();
    let hot = smoothstep(0.5 * LAVA_CELL, 0.0, cells.x);
    var result = mix(color, LAVA_HOT, hot * hot * fade);
    // Halfway between two feature points, the seam between their cells.
    let seam = (cells.y - cells.x) * 0.5;
    return mix(result, color * LAVA_COOL, coverage(seam - LAVA_SEAM) * fade);
}

// `color`, magma crust, at map position `point`: dark plates split by glowing cracks that
// light the crust beside them.
fn draw_magma_crust(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let cells = cell_distances(point, LAVA_CELL);
    let crack = (cells.y - cells.x) * 0.5;
    let fade = ground_fade();
    let glow = smoothstep(LAVA_GLOW, 0.0, crack) * 0.45;
    let lit = mix(color, LAVA_CRACK, glow * fade);
    return mix(lit, LAVA_CRACK, coverage(crack - LAVA_CRACK_HALF_WIDTH) * fade);
}

// `color`, the ground, with a concrete yard's slab joints over it at map position `point`.
fn draw_yard(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let joints = abs(fract(point / YARD_SLAB + 0.5) - 0.5) * YARD_SLAB;
    let joint = coverage(min(joints.x, joints.y) - 0.009);
    return mix(color, color * YARD_JOINT_SHADE, joint * ground_fade());
}

// `result` with a warehouse over it, centered `q` away with half extents `half` and built of
// `color`: a gabled roof whose ridge runs the long way, lit on its slope toward the upper left,
// with dark eaves, casting a shadow.
fn draw_warehouse(result: vec3<f32>, color: vec3<f32>, q: vec2<f32>, half: vec2<f32>) -> vec3<f32> {
    let bounds = vec4<f32>(-half, half);
    var out = result;
    let shadow = coverage(box_distance(q - STRUCTURE_SHADOW, bounds));
    out = mix(out, out * STRUCTURE_SHADOW_SHADE, shadow);
    let along_x = half.x >= half.y;
    let lit_side = select(q.x < 0.0, q.y < 0.0, along_x);
    var roof = select(color * 0.68, lighten(color, 0.22), lit_side);
    roof = mix(roof, color * 0.5, coverage(select(abs(q.x), abs(q.y), along_x) - 0.006));
    let eaves = coverage(-box_distance(q, bounds) - 0.012);
    roof = mix(roof, color * 0.5, eaves);
    return mix(out, roof, coverage(box_distance(q, bounds)));
}

// `result` with a cooling tower `radius` across over it, centered `q` away and built of
// `color`, seen from above: a shell lit toward the upper left around a dark mouth whose far
// wall catches the light, casting a shadow.
fn draw_cooling_tower(result: vec3<f32>, color: vec3<f32>, q: vec2<f32>, radius: f32) -> vec3<f32> {
    var out = result;
    let shadow = coverage(length(q - STRUCTURE_SHADOW * 1.6) - radius);
    out = mix(out, out * STRUCTURE_SHADOW_SHADE, shadow);
    let lit = clamp(0.5 + 0.5 * dot(q / radius, vec2<f32>(-0.6, -0.8)), 0.0, 1.0);
    var shell = mix(color * 0.78, lighten(color, 0.5), lit);
    let mouth_radius = radius * 0.66;
    let far_wall = clamp(0.5 + 0.6 * dot(q / mouth_radius, vec2<f32>(0.6, 0.8)), 0.0, 1.0);
    let mouth = mix(color * 0.22, color * 0.6, far_wall);
    shell = mix(shell, mouth, coverage(length(q) - mouth_radius));
    shell = mix(shell, color * 0.5, coverage(abs(length(q) - mouth_radius) - 0.006));
    return mix(out, shell, coverage(length(q) - radius));
}

// `result` with a storage tank `radius` across over it, centered `q` away and built of
// `color`: a drum top lit toward the upper left with a darker band inside its rim, casting a
// shadow.
fn draw_tank(result: vec3<f32>, color: vec3<f32>, q: vec2<f32>, radius: f32) -> vec3<f32> {
    var out = result;
    let shadow = coverage(length(q - STRUCTURE_SHADOW * 0.85) - radius);
    out = mix(out, out * STRUCTURE_SHADOW_SHADE, shadow);
    let lit = clamp(0.55 + 0.45 * dot(q / radius, vec2<f32>(-0.6, -0.8)), 0.0, 1.0);
    var tank = mix(color * 0.82, lighten(color, 0.42), lit);
    tank = mix(tank, color * 0.58, coverage(abs(length(q) - radius * 0.78) - 0.007));
    return mix(out, tank, coverage(length(q) - radius));
}

// `result` with a small shed centered `q` away, built of `color`, with a chimney at its upper
// right corner casting a short shadow.
fn draw_chimney_shed(result: vec3<f32>, color: vec3<f32>, q: vec2<f32>) -> vec3<f32> {
    var out = draw_warehouse(result, color, q, vec2<f32>(0.09, 0.06));
    let stack = q - vec2<f32>(0.1, -0.07);
    let shadow = coverage(segment(stack, vec2<f32>(0.0), vec2<f32>(0.07, 0.09)) - 0.025);
    out = mix(out, out * STRUCTURE_SHADOW_SHADE, shadow);
    out = mix(out, color * 0.55, coverage(length(stack) - 0.038));
    return mix(out, color * 0.2, coverage(length(stack) - 0.022));
}

// `result` with whatever stands on one lot over it: the lot is `size` map units square,
// `q` from its middle, and `odds` give the shares of warehouses, cooling towers, tank clusters
// and chimneyed sheds that `key`, a whole-number pair naming the lot, chooses among. Sizes
// scale with the lot.
fn draw_lot(
    result: vec3<f32>,
    color: vec3<f32>,
    q: vec2<f32>,
    size: f32,
    key: vec2<f32>,
    odds: vec4<f32>,
) -> vec3<f32> {
    let pick = hash(key);
    let scale = size / 0.5;
    if pick < odds.x {
        let long = mix(0.15, 0.2, hash(key + vec2<f32>(1.0, 0.0))) * scale;
        let short = mix(0.08, 0.11, hash(key + vec2<f32>(2.0, 0.0))) * scale;
        let lengthwise = hash(key + vec2<f32>(3.0, 0.0)) < 0.6;
        let half = select(vec2<f32>(short, long), vec2<f32>(long, short), lengthwise);
        return draw_warehouse(result, color, q, half);
    }
    if pick < odds.x + odds.y {
        let radius = mix(0.15, 0.18, hash(key + vec2<f32>(4.0, 0.0))) * scale;
        return draw_cooling_tower(result, color, q, radius);
    }
    if pick < odds.x + odds.y + odds.z {
        var out = result;
        for (var i = 0; i < 3; i++) {
            if hash(key + vec2<f32>(5.0, f32(i))) < 0.25 {
                continue;
            }
            let at = (vec2<f32>(f32(i % 2), f32(i / 2)) - vec2<f32>(0.5, 0.3)) * 0.19 * scale;
            let radius = mix(0.06, 0.08, hash(key + vec2<f32>(6.0, f32(i)))) * scale;
            out = draw_tank(out, color, q - at, radius);
        }
        return out;
    }
    if pick < odds.x + odds.y + odds.z + odds.w {
        return draw_chimney_shed(result, color, q / scale);
    }
    return result;
}

// `color`, bare heavy industrial ground at map position `point`: a concrete yard built up with
// a mix of warehouses, cooling towers, storage tanks and chimneyed sheds, all lit from the
// upper left and casting shadows. Big lots hold one large warehouse or cooling tower; the rest
// split into four small lots of smaller structures. Zoomed out, it fades to the yard's plain
// color. It is set by map position rather than by hex, so it runs on across neighboring
// industrial ground.
fn draw_industry(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    let yard = draw_yard(color, point);
    let lot = floor(point / BIG_LOT);
    let key = lot + vec2<f32>(820.0, 0.0);
    var built = yard;
    if hash(key + vec2<f32>(0.0, 50.0)) < BIG_LOT_SHARE {
        let q = point - (lot + 0.5) * BIG_LOT;
        built = draw_lot(yard, color, q, BIG_LOT, key, BIG_LOT_ODDS);
    } else {
        let small = BIG_LOT * 0.5;
        let sub = floor(point / small);
        let q = point - (sub + 0.5) * small;
        built = draw_lot(yard, color, q, small, sub + vec2<f32>(830.0, 0.0), SMALL_LOT_ODDS);
    }
    return mix(yard, built, ground_fade());
}

// `color`, bare `ground`, with that ground's texture over it at map position `point`.
fn draw_ground(color: vec3<f32>, point: vec2<f32>, ground: u32) -> vec3<f32> {
    if ground == CLEAR {
        return draw_plains(color, point);
    }
    if ground == PAVEMENT {
        return draw_slabs(color, point);
    }
    if ground == ROUGH || ground == ULTRA_ROUGH {
        return draw_hummocks(color, point, ground == ULTRA_ROUGH);
    }
    if ground == RUBBLE || ground == ULTRA_RUBBLE {
        return draw_ruins(color, point, ground == ULTRA_RUBBLE);
    }
    if ground == SAND {
        return draw_dunes(color, point);
    }
    if ground == TUNDRA {
        return draw_tundra(color, point);
    }
    if ground == SWAMP {
        return draw_bog(color, point);
    }
    if ground == MAGMA {
        return draw_magma(color, point);
    }
    if ground == MAGMA_CRUST {
        return draw_magma_crust(color, point);
    }
    if ground == HEAVY_INDUSTRIAL {
        return draw_industry(color, point);
    }
    return color;
}

// How a stand of `foliage`, woods or jungle, grows: x is the size in map units of the cells
// that each hold at most one plant, y the share of cells left empty, z a plant's radius in map
// units, and w how far the ground between plants darkens toward shaded leaf. Denser stands
// pack larger plants closer over a darker understory.
fn stand_shape(foliage: u32) -> vec4<f32> {
    var shapes = array<vec4<f32>, 6>(
        vec4<f32>(0.42, 0.3, 0.15, 0.0),
        vec4<f32>(0.34, 0.05, 0.19, 0.45),
        vec4<f32>(0.3, 0.0, 0.22, 0.8),
        vec4<f32>(0.44, 0.3, 0.2, 0.0),
        vec4<f32>(0.36, 0.05, 0.24, 0.45),
        vec4<f32>(0.32, 0.0, 0.27, 0.8),
    );
    return shapes[foliage];
}

// How much of the plant reaching `radius` from its center covers a point `q` from that
// center: a round tree crown, or for a palm, fronds turned `turn` radians that taper to points.
fn plant_coverage(q: vec2<f32>, radius: f32, palm: bool, turn: f32) -> f32 {
    if !palm {
        return coverage(length(q) - radius);
    }
    let frond = pow(abs(cos(0.5 * FRONDS * (atan2(q.y, q.x) - turn))), 1.4);
    return coverage(length(q) - radius * (0.35 + 0.65 * frond));
}

// The color of a plant in `leaf` at `q` from its center: a tree crown lit from the upper left
// and darkening toward its rim, or a palm's fronds turned `turn` radians, lighter toward their
// tips, each with a darker rib down its middle, around a dark crown.
fn plant_color(q: vec2<f32>, radius: f32, palm: bool, turn: f32, leaf: vec3<f32>) -> vec3<f32> {
    let n = q / radius;
    if palm {
        let reach = length(n);
        if reach < 0.15 {
            return leaf * 0.45;
        }
        // Roughly how far across from the nearest frond's middle line.
        let half_turns = 0.5 * FRONDS * (atan2(q.y, q.x) - turn);
        let from_rib = length(q) * abs(sin(half_turns)) / (0.5 * FRONDS);
        let rib = coverage(from_rib - 0.008);
        return mix(leaf * (0.7 + 0.45 * reach), leaf * 0.55, rib);
    }
    let lit = 0.85 + 0.3 * dot(n, vec2<f32>(-0.6, -0.8));
    return leaf * lit * (1.0 - 0.25 * smoothstep(0.7, 1.0, length(n)));
}

// `color`, the ground, under a stand of woods or jungle `foliage` in `leaf` at map position
// `point`: one plant to each cell that is not left empty, nudged off the cell's middle and
// varied in size and shade, drawn from back to front with shadows over whatever lies behind.
// The stand is set by map position rather than by hex, so it runs on across neighboring hexes.
fn draw_stand(color: vec3<f32>, point: vec2<f32>, foliage: u32, leaf: vec3<f32>) -> vec3<f32> {
    let shape = stand_shape(foliage);
    let palm = foliage >= LIGHT_JUNGLE;
    let seed = select(0.0, 31.0, palm);
    var result = mix(color, leaf * 0.55, shape.w);
    let cell = floor(point / shape.x);
    // Rows run north to south, and every plant stays within its own row of cells, so drawing
    // them row by row puts nearer plants over farther ones.
    for (var dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (var dx = -1.0; dx <= 1.0; dx += 1.0) {
            let c = cell + vec2<f32>(dx, dy);
            let keyed = c + vec2<f32>(seed, seed * 3.0);
            if hash(keyed) < shape.y {
                continue;
            }
            let jitter = vec2<f32>(
                hash(keyed + vec2<f32>(1.0, 7.0)),
                hash(keyed + vec2<f32>(5.0, 3.0)),
            );
            let center = (c + 0.5 + (jitter - 0.5) * 0.6) * shape.x;
            let radius = shape.z * mix(0.8, 1.15, hash(keyed + vec2<f32>(2.0, 9.0)));
            let turn = hash(keyed + vec2<f32>(4.0, 4.0)) * 2.0 * PI;
            let tint = leaf * mix(0.9, 1.08, hash(keyed + vec2<f32>(8.0, 1.0)));
            let q = point - center;
            let shadow = plant_coverage(q - PLANT_SHADOW, radius, palm, turn);
            result = mix(result, result * PLANT_SHADOW_SHADE, shadow);
            let body = plant_coverage(q, radius, palm, turn);
            result = mix(result, plant_color(q, radius, palm, turn, tint), body);
        }
    }
    let flat = mix(color, leaf, 0.55 + 0.5 * shape.w);
    return mix(flat, result, ground_fade());
}

// The plot of farmland containing map position `point`: xy names it, and z is the distance in
// map units to its edge. Plots are the cells around feature points scattered `PLOT_SIZE` apart.
fn field_plot(point: vec2<f32>) -> vec3<f32> {
    let p = point / PLOT_SIZE + vec2<f32>(51.0, 13.0);
    let cell = floor(p);
    var nearest = vec2<f32>(9.0, 9.0);
    var plot = vec2<f32>(0.0);
    for (var dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (var dx = -1.0; dx <= 1.0; dx += 1.0) {
            let c = cell + vec2<f32>(dx, dy);
            let feature = c + 0.15 + 0.7 * vec2<f32>(hash(c), hash(c + vec2<f32>(9.0, 4.0)));
            let d = distance(p, feature);
            if d < nearest.x {
                nearest = vec2<f32>(d, nearest.x);
                plot = c;
            } else if d < nearest.y {
                nearest.y = d;
            }
        }
    }
    return vec3<f32>(plot, (nearest.y - nearest.x) * 0.5 * PLOT_SIZE);
}

// The crop growing in `plot`: the field color `leaf`, a paler gold, or a greener one.
fn plot_crop(leaf: vec3<f32>, plot: vec2<f32>) -> vec3<f32> {
    let pick = hash(plot + vec2<f32>(3.0, 3.0));
    if pick < 0.34 {
        return leaf;
    }
    if pick < 0.67 {
        return lighten(leaf, 0.22);
    }
    return mix(leaf, vec3<f32>(0.5, 0.62, 0.25), 0.3);
}

// `color`, the ground, under planted fields of `leaf` at map position `point`: plots edged by
// hedgerows, each sown in staggered rows of round plants, lit from the upper left, over soil.
// Each plot's rows run east to west or north to south, a little askew. Zoomed out, a plot
// takes one flat mix of crop and soil. The fields are set by map position rather than by hex,
// so they run on across neighboring hexes.
fn draw_fields(color: vec3<f32>, point: vec2<f32>, leaf: vec3<f32>) -> vec3<f32> {
    let plot = field_plot(point);
    let crop = plot_crop(leaf, plot.xy);
    let soil = mix(color, SOIL_COLOR, SOIL_MIX);
    let quarter = select(0.0, PI * 0.5, hash(plot.xy + vec2<f32>(7.0, 2.0)) > 0.5);
    let turn = quarter + 0.2 * hash(plot.xy + vec2<f32>(1.0, 1.0));
    let p = turned(point, turn) / PLANT_SPACING;
    let row = floor(p.y);
    let q = (vec2<f32>(fract(p.x + 0.5 * (row % 2.0)), fract(p.y)) - 0.5) * PLANT_SPACING;
    let plant = coverage(length(q) - PLANT_RADIUS);
    let lit = mix(crop * 0.8, lighten(crop, 0.15), clamp(0.5 - q.y * 12.0 - q.x * 8.0, 0.0, 1.0));
    let fade = ground_fade();
    let rows = mix(mix(soil, crop, 0.35), lit, plant);
    let sown = mix(mix(soil, crop, 0.6), rows, fade);
    return mix(sown, crop * HEDGE_SHADE, coverage(plot.z - HEDGE_HALF_WIDTH) * fade);
}

// `color` with foliage drawn over it at map position `point`: woods and jungle as stands of
// trees and palms, and planted fields as plots of crops.
fn draw_foliage(color: vec3<f32>, point: vec2<f32>, foliage: u32) -> vec3<f32> {
    let leaf = palette[FOLIAGE + foliage].rgb;
    if foliage == PLANTED_FIELDS {
        return draw_fields(color, point, leaf);
    }
    return draw_stand(color, point, foliage, leaf);
}

// The color of water `depth` levels deep over ground at `level`, darker the deeper.
fn water_color(level: u32, depth: u32) -> vec3<f32> {
    let surface = lighten(palette[WATER].rgb, 0.01 * f32(level));
    return mix(surface, vec3<f32>(0.0), 0.08 * f32(depth));
}

// What way a hex's packed layers carry, a WAY_ value.
fn way_of(texel: vec4<u32>) -> u32 {
    if (texel.b & 3u) == BRIDGE + 1u {
        return WAY_BRIDGE;
    }
    if (texel.b & 3u) == WALL + 1u {
        return WAY_WALL;
    }
    let route = (texel.r >> 7u) & 7u;
    if !present(route) {
        return WAY_NONE;
    }
    return select(WAY_ROAD, WAY_RAIL, route - 1u == RAIL);
}

// Whether ways `a` and `b` in adjacent hexes join.
fn joins(a: u32, b: u32) -> bool {
    if a == WAY_NONE || b == WAY_NONE {
        return false;
    }
    if a == WAY_WALL || b == WAY_WALL {
        return a == b;
    }
    return a == b || a == WAY_BRIDGE || b == WAY_BRIDGE;
}

// The neighbor of `hex` across `side`, numbered clockwise from north as in
// `HexCoordinate::neighbors`. Even columns sit half a hex lower, so their east and west
// neighbors are one row further south than an odd column's.
fn neighbor(hex: vec2<i32>, side: u32) -> vec2<i32> {
    if side == 0u || side == 3u {
        return hex + vec2<i32>(0, select(1, -1, side == 0u));
    }
    let east = side < 3u;
    let upper = side == 1u || side == 5u;
    let even = (hex.x & 1) == 0;
    let dy = select(select(0, 1, even), select(-1, 0, even), upper);
    return hex + vec2<i32>(select(-1, 1, east), dy);
}

// Unit vector from a hex center toward the middle of its edge on `side`.
fn side_direction(side: u32) -> vec2<f32> {
    let angle = f32(side) * 1.0471976;
    return vec2<f32>(sin(angle), -cos(angle));
}

fn on_map(hex: vec2<i32>) -> bool {
    return all(hex >= vec2<i32>(0)) && all(hex < vec2<i32>(u.map_size));
}

// The sides, as bits clockwise from north, across which `way` in `hex` joins its neighbors.
fn way_links(hex: vec2<i32>, way: u32) -> u32 {
    var links = 0u;
    for (var side = 0u; side < 6u; side++) {
        let next = neighbor(hex, side);
        if on_map(next) && joins(way, way_of(textureLoad(hexes, next, 0))) {
            links |= 1u << side;
        }
    }
    return links;
}

// The arc a way follows through a hex it enters across `first` and leaves across `second`,
// two sides that are not opposite, measured from `local` as in `way_line`. The arc meets the
// middle of each edge square on, so it runs on smoothly into the neighbors: a tight turn
// around the corner between two neighboring sides, or a wide sweep past the side between two
// sides one apart.
fn way_arc(local: vec2<f32>, first: u32, second: u32) -> vec2<f32> {
    let a = side_direction(first);
    let b = side_direction(second);
    let toward_bend = normalize(a + b);
    // Neighboring sides meet at the corner between them, a hex radius out, and the arc around
    // it reaches the middle of each edge half a radius away; sides one apart sweep around a
    // point twice the apothem out, past the side between them.
    let tight = dot(a, b) > 0.0;
    let center = toward_bend * select(2.0 * APOTHEM, 1.0, tight);
    let radius = select(SQRT_3 * APOTHEM, 0.5, tight);
    let from_center = local - center;
    let to_a = acos(clamp(dot(normalize(from_center), normalize(a * APOTHEM - center)), -1.0, 1.0));
    let to_b = acos(clamp(dot(normalize(from_center), normalize(b * APOTHEM - center)), -1.0, 1.0));
    return vec2<f32>(abs(length(from_center) - radius), radius * min(to_a, to_b));
}

// The centerline of `way` in `hex`, measured from `local`: x is the distance to it, and y how
// far along it from the nearest hex edge it crosses, which runs the same way from both sides of
// an edge so rail dashes meet. The line runs from the hex center to the middle of each edge it
// joins a neighbor across, except that a road, rail line or bridge joining exactly two
// neighbors that are not opposite curves between them (see `way_arc`); walls keep their
// corners. A way that joins nothing crosses the hex east to west. A way joining one neighbor
// ends in the middle of the hex, except a bridge, or a way whose opposite edge is the map's,
// which carry on straight across.
fn way_line(hex: vec2<i32>, local: vec2<f32>, way: u32) -> vec2<f32> {
    let links = way_links(hex, way);
    if links == 0u {
        return vec2<f32>(abs(local.y), local.x + APOTHEM);
    }
    if way != WAY_WALL && countOneBits(links) == 2u {
        let first = firstTrailingBit(links);
        let second = firstLeadingBit(links);
        if second - first != 3u {
            return way_arc(local, first, second);
        }
    }
    var best = vec2<f32>(1e9, 0.0);
    for (var side = 0u; side < 6u; side++) {
        if ((links >> side) & 1u) == 0u {
            continue;
        }
        let toward = side_direction(side);
        var far_end = vec2<f32>(0.0);
        if countOneBits(links) == 1u {
            let through = way == WAY_BRIDGE || !on_map(neighbor(hex, (side + 3u) % 6u));
            far_end = select(far_end, -toward * APOTHEM, through);
        }
        let d = segment(local, far_end, toward * APOTHEM);
        if d < best.x {
            best = vec2<f32>(d, APOTHEM - dot(local, toward));
        }
    }
    return best;
}

// Water depth plus one, or zero for none, in a hex's packed layers.
fn water_of(texel: vec4<u32>) -> u32 {
    return (texel.g >> 8u) & 15u;
}

// The sides of `hex`, as bits clockwise from north, across which its water runs on: those
// whose neighbor holds water or lies off the map, so water carries on past the map's edge.
fn water_links(hex: vec2<i32>) -> u32 {
    var links = 0u;
    for (var side = 0u; side < 6u; side++) {
        let next = neighbor(hex, side);
        if !on_map(next) || present(water_of(textureLoad(hexes, next, 0))) {
            links |= 1u << side;
        }
    }
    return links;
}

// The smaller of `a` and `b`, rounded off where they come within `k` of each other, so shapes
// joined with it meet in a smooth curve.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// The water in a hex whose water runs on across the sides in `links` (see `water_links`),
// measured from `local`. x is the signed distance to the water's edge, negative in the water;
// yz is the direction of the nearest channel, or east in open water; w is how far along that
// channel from the hex edge it crosses. Water runs from a pool around the center out along a
// channel to each linked side, and fills the corner between any two linked neighboring sides,
// so lakes are open water with rounded shores and rivers wind between banks. A hex with water
// on every side is open water, and one with none holds a round pond.
fn water_channel(local: vec2<f32>, links: u32) -> vec4<f32> {
    if links == 63u {
        return vec4<f32>(-1.0, 1.0, 0.0, 0.0);
    }
    if links == 0u {
        return vec4<f32>(length(local) - POND_RADIUS, 1.0, 0.0, 0.0);
    }
    var d = length(local) - CHANNEL_POOL_RADIUS;
    var nearest = 1e9;
    var flow = vec3<f32>(1.0, 0.0, 0.0);
    // Clockwise from north, matching `side_direction`.
    let bearing = atan2(local.x, -local.y);
    for (var side = 0u; side < 6u; side++) {
        if ((links >> side) & 1u) == 0u {
            continue;
        }
        let toward = side_direction(side);
        // Reaching past the edge, so the channel meets its neighbor's at full width.
        let axis = segment(local, vec2<f32>(0.0), toward * APOTHEM * 1.5);
        d = smooth_min(d, axis - CHANNEL_HALF_WIDTH, CHANNEL_BLEND);
        if axis < nearest {
            nearest = axis;
            flow = vec3<f32>(toward, APOTHEM - dot(local, toward));
        }
        // The corner between this side and the next clockwise, when both run on, is open water.
        // Its boundaries lie along the two channels' middles, deep in the water.
        let next = (side + 1u) % 6u;
        let from_side = fract((bearing - f32(side) * PI / 3.0) / (2.0 * PI)) * 2.0 * PI;
        if ((links >> next) & 1u) == 1u && from_side <= PI / 3.0 {
            d = min(d, -CHANNEL_HALF_WIDTH);
        }
    }
    return vec4<f32>(d, flow);
}

// `color`, water of `flow`, with ripples or streaks on its current, laid along `channel`'s
// direction (see `water_channel`), or across the map at `point` in open water and ponds,
// where they run on across neighboring hexes. Still water ripples gently; rapids streak and
// torrents froth.
fn draw_current(
    color: vec3<f32>,
    local: vec2<f32>,
    point: vec2<f32>,
    channel: vec4<f32>,
    links: u32,
    flow: u32,
) -> vec3<f32> {
    var frame = point;
    if links != 63u && links != 0u {
        frame = vec2<f32>(channel.w, dot(local, vec2<f32>(-channel.z, channel.y)));
    }
    // Half length, half width and lightness of each mark, the share of cells left calm, and
    // how far ripples bow.
    var mark = vec3<f32>(0.055, 0.011, 0.35);
    var skip = RIPPLE_SKIP;
    var bow = RIPPLE_BOW;
    if flow == RAPIDS {
        mark = vec3<f32>(0.085, 0.011, 0.5);
        skip = 0.2;
        bow = 0.0;
    }
    if flow == TORRENT {
        mark = vec3<f32>(0.11, 0.015, 0.65);
        skip = 0.0;
        bow = 0.0;
    }
    let base = mix(color, vec3<f32>(1.0), select(0.0, TORRENT_FROTH, flow == TORRENT));
    let v = frame / CURRENT_CELL;
    let row = floor(v.y);
    let shifted = vec2<f32>(v.x + 0.5 * (row % 2.0), v.y);
    let cell = floor(shifted);
    if hash(cell + vec2<f32>(40.0, 0.0)) < skip {
        return base;
    }
    let jitter = vec2<f32>(hash(cell + vec2<f32>(41.0, 3.0)), hash(cell + vec2<f32>(43.0, 7.0)));
    let q = (shifted - cell - 0.5 - (jitter - 0.5) * 0.3) * CURRENT_CELL;
    let x = clamp(q.x / mark.x, -1.0, 1.0);
    let d = length(vec2<f32>(q.x - x * mark.x, q.y + bow * (1.0 - x * x))) - mark.y;
    let fade = smoothstep(CURRENT_FADE.x, CURRENT_FADE.y, u.radius);
    return mix(base, vec3<f32>(1.0), mark.z * coverage(d) * fade);
}

// A water hex at `hex`, `depth` levels deep with `flow`, over `ground` (its ground, already
// drawn, and lightened by its `level`): water running toward its neighbors' water, with
// muddy banks of `ground` beside it edged in foam. Returns the color and how much of this
// pixel is water.
fn draw_water_hex(
    ground: vec3<f32>,
    hex: vec2<i32>,
    local: vec2<f32>,
    point: vec2<f32>,
    level: u32,
    depth: u32,
    flow: u32,
) -> vec4<f32> {
    let links = water_links(hex);
    let channel = water_channel(local, links);
    let wet = draw_current(water_color(level, depth), local, point, channel, links, flow);
    let mud = BANK_MUD_MIX * smoothstep(BANK_WIDTH, 0.0, channel.x);
    let bank = mix(ground, BANK_MUD, mud);
    let water = coverage(channel.x);
    let foam = coverage(abs(channel.x) - FOAM_HALF_WIDTH) * ground_fade();
    let shore = mix(mix(bank, wet, water), vec3<f32>(1.0), FOAM_LIGHT * foam);
    return vec4<f32>(shore, water);
}

// `color` under a sheet of ice at map position `point`: tinted pale, with faint glints, split
// into floes by white cracks that run on across neighboring hexes.
fn draw_ice(color: vec3<f32>, point: vec2<f32>) -> vec3<f32> {
    var result = draw_condition(color, vec2<f32>(0.0), ICE);
    let fade = ground_fade();
    let glint = smoothstep(0.1, 0.0, abs(fract((point.x * 0.7 + point.y) * 1.3) - 0.5) - 0.38);
    result = mix(result, vec3<f32>(1.0), ICE_GLINT * glint * fade);
    let floes = cell_distances(point + vec2<f32>(13.0, 7.0), ICE_FLOE);
    let crack = (floes.y - floes.x) * 0.5;
    let cracks = coverage(crack - ICE_CRACK_HALF_WIDTH) * fade;
    return mix(result, vec3<f32>(1.0), ICE_CRACK_LIGHT * cracks);
}

// How strongly the details of roads, rail and bridges show at the current zoom.
fn way_detail() -> f32 {
    return smoothstep(WAY_DETAIL_FADE.x, WAY_DETAIL_FADE.y, u.radius);
}

// Distance along a way from `along` to the middle of the nearest of a row of marks `period`
// apart, the first on the edge where the way enters its hex.
fn from_mark(along: f32, period: f32) -> f32 {
    return abs((fract(along / period + 0.5) - 0.5) * period);
}

// `color` with a road or rail of `route` along `line` (see `way_line`), at map position
// `point`: a paved road with a dashed white center line, a dirt road with ragged edges and
// two wheel ruts, a gravel road speckled with stones between pale shoulders, or rail track of
// two steel rails on wooden ties over a ballast bed.
fn draw_route(color: vec3<f32>, line: vec2<f32>, point: vec2<f32>, route: u32) -> vec3<f32> {
    let surface = palette[ROUTE + route].rgb;
    let detail = way_detail();
    if route == PAVED_ROAD {
        let cover = coverage(line.x - WAY_HALF_WIDTH);
        // How far along the line from the middle of the nearest dash.
        let from_dash = (fract(line.y / LANE_PERIOD) - 0.5) * LANE_PERIOD;
        let outside = max(line.x - LANE_HALF_WIDTH, abs(from_dash) - LANE_PERIOD * 0.25);
        return mix(color, mix(surface, LANE_COLOR, coverage(outside) * detail), cover);
    }
    if route == DIRT_ROAD {
        let wander = sin(line.y * 23.0) * sin(line.y * 7.3 + 1.0);
        let edge = WAY_HALF_WIDTH * (1.0 - DIRT_RAGGEDNESS * (0.8 - wander));
        let ruts = coverage(abs(line.x - RUT_OFFSET) - RUT_HALF_WIDTH) * detail;
        return mix(color, mix(surface, surface * RUT_SHADE, ruts), coverage(line.x - edge));
    }
    if route == GRAVEL_ROAD {
        let speck = hash(floor(point / GRAVEL_GRAIN)) < GRAVEL_SPECKS;
        var bed = mix(surface, surface * GRAVEL_SPECK_SHADE, select(0.0, detail, speck));
        let shoulder = coverage(WAY_HALF_WIDTH - SHOULDER_WIDTH - line.x);
        bed = mix(bed, lighten(surface, SHOULDER_LIGHT), shoulder);
        return mix(color, bed, coverage(line.x - WAY_HALF_WIDTH));
    }
    var track = mix(color, BALLAST_COLOR, coverage(line.x - WAY_HALF_WIDTH * BALLAST_WIDTH));
    let tie = max(line.x - TIE_HALF_LENGTH, from_mark(line.y, TIE_PERIOD) - TIE_HALF_WIDTH);
    track = mix(track, TIE_COLOR, coverage(tie) * detail);
    // Zoomed out, the rails merge into one dark line down the middle.
    let rail_line = mix(line.x, abs(line.x - RAIL_GAUGE), detail);
    return mix(track, surface, coverage(rail_line - RAIL_HALF_WIDTH));
}

// `color` with a concrete bridge deck of construction class `grade` along `line` (see
// `way_line`), darker for stronger classes: crossed by expansion joints, between dark railings
// with posts along them.
fn draw_bridge(color: vec3<f32>, line: vec2<f32>, grade: u32) -> vec3<f32> {
    let deck = palette[STRUCTURE + BRIDGE].rgb * (1.0 - CLASS_SHADE * f32(grade));
    let detail = way_detail();
    let joint = coverage(from_mark(line.y, JOINT_PERIOD) - JOINT_HALF_WIDTH) * detail;
    var surface = mix(deck, deck * JOINT_SHADE, joint);
    let railing_line = WAY_HALF_WIDTH - RAILING_INSET;
    let railing = coverage(abs(line.x - railing_line) - RAILING_HALF_WIDTH);
    let post_across = abs(line.x - railing_line) - POST_HALF_SIZE;
    let post = coverage(max(post_across, from_mark(line.y, POST_PERIOD) - POST_HALF_SIZE));
    surface = mix(surface, deck * RAILING_SHADE, max(railing, post * detail));
    return mix(color, surface, coverage(line.x - WAY_HALF_WIDTH));
}

// `color` with a stone wall of construction class `grade` along `line` (see `way_line`): a top
// of the wall color, darker for stronger classes, crossed by block joints, with darker sides,
// buttressed at intervals by square pillars that stand out past it with raised, rimmed tops,
// all over a soft shadow on the ground at their feet.
fn draw_wall(color: vec3<f32>, line: vec2<f32>, grade: u32) -> vec3<f32> {
    let stone = draw_structure(WALL, grade);
    let detail = way_detail();
    let wall_edge = line.x - WALL_HALF_WIDTH;
    let pillar_edge = max(
        line.x - PILLAR_HALF_WIDTH,
        from_mark(line.y, PILLAR_SPACING) - PILLAR_HALF_LENGTH,
    );
    let outline = min(wall_edge, pillar_edge);
    let shadow = 1.0 - smoothstep(0.0, WALL_SHADOW, outline);
    var result = mix(color, color * WALL_SHADOW_SHADE, shadow);
    let joint = coverage(from_mark(line.y, WALL_BLOCK) - 0.006) * detail;
    var wall = mix(stone, stone * WALL_JOINT_SHADE, joint);
    let side = coverage(WALL_HALF_WIDTH - WALL_SIDE - line.x);
    wall = mix(wall, stone * WALL_SIDE_SHADE, side);
    result = mix(result, wall, coverage(wall_edge));
    let rim = coverage(-pillar_edge - PILLAR_RIM);
    let pillar = mix(lighten(stone, PILLAR_TOP), stone * PILLAR_RIM_SHADE, rim);
    return mix(result, pillar, coverage(pillar_edge));
}

// The color of a structure of `kind` and construction class `grade`, darker for stronger
// classes: what `draw_building` builds its skyline from and `draw_wall` lays along a line.
fn draw_structure(kind: u32, grade: u32) -> vec3<f32> {
    return palette[STRUCTURE + kind].rgb * (1.0 - CLASS_SHADE * f32(grade));
}

// Signed distance from `p` to the box `bounds` (left, top, right, bottom), negative inside.
fn box_distance(p: vec2<f32>, bounds: vec4<f32>) -> f32 {
    let q = abs(p - (bounds.xy + bounds.zw) * 0.5) - (bounds.zw - bounds.xy) * 0.5;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

// How much of a lit window covers `p` inside the skyline block `bounds`: windows sit in a grid
// centered within the block's inset, one per whole `WINDOW_PITCH` cell that fits.
fn window_coverage(p: vec2<f32>, bounds: vec4<f32>) -> f32 {
    let inner = bounds + vec4<f32>(WINDOW_INSET, WINDOW_INSET, -WINDOW_INSET, -WINDOW_INSET);
    let span = inner.zw - inner.xy;
    // A little slack so a span that is an exact multiple of the pitch keeps its last cell.
    let cells = floor(span / WINDOW_PITCH + 0.001);
    let start = inner.xy + (span - cells * WINDOW_PITCH) * 0.5;
    let cell = floor((p - start) / WINDOW_PITCH);
    if any(cell < vec2<f32>(0.0)) || any(cell >= cells) {
        return 0.0;
    }
    let middle = start + (cell + 0.5) * WINDOW_PITCH;
    let window = vec4<f32>(middle - WINDOW_HALF, middle + WINDOW_HALF);
    return clamp(0.5 - box_distance(p, window) * u.radius, 0.0, 1.0);
}

// `color`, the ground a building stands on, with the building's skyline icon over it in
// `stone`, the building's color: three blocks of different heights with rows of lit windows
// that fade out when the hexes are too small to show them, outlined in dark gray and casting
// a shadow, so the icon stands out on any ground. Labels fade the whole icon.
fn draw_building(color: vec3<f32>, local: vec2<f32>, stone: vec3<f32>) -> vec3<f32> {
    // Each block's left, top, right and bottom.
    var blocks = array<vec4<f32>, 3>(
        vec4<f32>(-0.5, -0.1, -0.18, 0.4),
        vec4<f32>(-0.18, -0.5, 0.16, 0.4),
        vec4<f32>(0.16, -0.24, 0.48, 0.4),
    );
    var outline = 9.0;
    var shadow = 9.0;
    for (var i = 0; i < 3; i++) {
        outline = min(outline, box_distance(local, blocks[i]));
        shadow = min(shadow, box_distance(local - SKYLINE_SHADOW, blocks[i]));
    }
    var result = mix(color, color * SKYLINE_SHADOW_SHADE, coverage(shadow));
    result = mix(result, SKYLINE_OUTLINE, coverage(outline - SKYLINE_OUTLINE_WIDTH));
    let windows = smoothstep(WINDOW_FADE.x, WINDOW_FADE.y, u.radius);
    for (var i = 0; i < 3; i++) {
        let bounds = blocks[i];
        // Alternate blocks a shade apart, so neighbors read as separate towers.
        let shade = SKYLINE_SHADE + 0.1 * f32(i % 2);
        let block = mix(stone * shade, WINDOW_LIGHT, windows * window_coverage(local, bounds));
        result = mix(result, block, coverage(box_distance(local, bounds)));
    }
    return mix(color, result, select(1.0, LABELLED_SKYLINE, u.labels != 0.0));
}

// `color` tinted by a weather condition: ice pale cyan, thin snow a light speckle, deep snow
// white and mud brown.
fn draw_condition(color: vec3<f32>, local: vec2<f32>, condition: u32) -> vec3<f32> {
    let tint = palette[CONDITION + condition].rgb;
    if condition == ICE {
        return mix(color, tint, ICE_TINT);
    }
    if condition == THIN_SNOW {
        let speck = hash(floor(local * 9.0)) > 0.5;
        return mix(color, tint, select(0.3, 0.9, speck));
    }
    if condition == DEEP_SNOW {
        return mix(color, tint, 0.85);
    }
    return mix(color, tint, 0.75);
}

// How much of one flame standing on the origin covers `q`: `height` map units tall and `width`
// across at its widest, a tongue tapering from a rounded base to a tip that flickers by
// `phase`. `shrink` narrows it to an inner layer, and `start`, from 0 at the tip to 1 at the
// base, cuts that layer off short of the tip.
fn flame_coverage(
    q: vec2<f32>,
    height: f32,
    width: f32,
    phase: f32,
    shrink: f32,
    start: f32,
) -> f32 {
    let t = (q.y + height) / height;
    if t <= start || t >= 1.0 {
        return 0.0;
    }
    let flicker = 0.25 * width * sin(t * 7.0 + phase) * (1.0 - t);
    let half_width = width * shrink * pow(t, 0.7) * sqrt(1.0 - t * t * t);
    return coverage(abs(q.x - flicker) - half_width);
}

// `color` with the fire icon over it: a tall flame between two shorter ones, outlined in deep
// red and burning from red-orange at their edges through the fire color `ink` to light orange
// at their hearts, over lightly scorched ground. The icon keeps clear of the top label row.
fn draw_fire(color: vec3<f32>, local: vec2<f32>, ink: vec3<f32>) -> vec3<f32> {
    let scorched = mix(color * 0.35, CHAR, 0.5);
    var result = mix(color, scorched, SCORCH);
    // Base, height and widest half width of each flame, at the icon's base size.
    var flames = array<vec4<f32>, 3>(
        vec4<f32>(-0.17, 0.4, 0.5, 0.17),
        vec4<f32>(0.19, 0.4, 0.56, 0.17),
        vec4<f32>(0.0, 0.42, 0.8, 0.26),
    );
    for (var i = 0; i < 3; i++) {
        flames[i] = vec4<f32>(flames[i].xy * ICON_SCALE + ICON_SHIFT, flames[i].zw * ICON_SCALE);
    }
    var outline = 0.0;
    for (var i = 0; i < 3; i++) {
        let f = flames[i];
        let q = local - f.xy + vec2<f32>(0.0, OUTLINE_WIDTH);
        let height = f.z + 2.0 * OUTLINE_WIDTH;
        let width = f.w + OUTLINE_WIDTH;
        outline = max(outline, flame_coverage(q, height, width, f32(i), 1.0, 0.0));
    }
    result = mix(result, FLAME_OUTLINE, outline);
    for (var i = 0; i < 3; i++) {
        let f = flames[i];
        let q = local - f.xy;
        let phase = f32(i);
        result = mix(result, FLAME_EDGE, flame_coverage(q, f.z, f.w, phase, 1.0, 0.0));
        result = mix(result, ink, flame_coverage(q, f.z, f.w, phase, 0.6, 0.25));
        result = mix(result, FLAME_HEART, flame_coverage(q, f.z, f.w, phase, 0.28, 0.5));
    }
    return result;
}

// Approximate distance from `p` to the ellipse centered on `e.xy` with radii `e.zw`, close
// enough near its edge for antialiasing.
fn ellipse_distance(p: vec2<f32>, e: vec4<f32>) -> f32 {
    return (length((p - e.xy) / e.zw) - 1.0) * min(e.z, e.w);
}

// `color` with the smoke icon over it: bands of smoke streaming east, stacked down the hex and
// staggered, each a row of flattened billows trailing off into a thin wisp. Every band is
// outlined in dark gray and shaded in the smoke color `ink`, lighter toward the top of each
// billow and in the upper bands; lower bands overlap the ones above them.
fn draw_smoke(color: vec3<f32>, local: vec2<f32>, ink: vec3<f32>) -> vec3<f32> {
    var result = color;
    for (var band = 0; band < SMOKE_BANDS; band++) {
        let y = SMOKE_BAND_SPACING * (f32(band) - 1.0);
        let x = select(-0.04, 0.04, band % 2 == 1);
        // Center and radii of each billow, the last a trailing wisp.
        var billows = array<vec4<f32>, 5>(
            vec4<f32>(x - 0.36, y + 0.02, 0.2, 0.12),
            vec4<f32>(x - 0.1, y - 0.01, 0.24, 0.14),
            vec4<f32>(x + 0.18, y + 0.01, 0.22, 0.12),
            vec4<f32>(x + 0.42, y + 0.03, 0.2, 0.08),
            vec4<f32>(x + 0.62, y - 0.02, 0.2, 0.035),
        );
        var d = 9.0;
        var crown = 0.0;
        for (var i = 0; i < 5; i++) {
            let billow = billows[i];
            let to_billow = ellipse_distance(local, billow);
            if to_billow < d {
                d = to_billow;
                crown = clamp(0.5 - (local.y - billow.y) / billow.w * 0.6, 0.0, 1.0);
            }
        }
        let height = 1.0 - f32(band) / f32(SMOKE_BANDS - 1);
        let shade = mix(ink * SMOKE_SHADE, lighten(ink, SMOKE_LIGHT), 0.6 * crown + 0.4 * height);
        result = mix(result, SMOKE_OUTLINE, coverage(d - SMOKE_OUTLINE_WIDTH));
        result = mix(result, shade, coverage(d));
    }
    return result;
}

// `color` with fire or smoke drawn over it: a flame icon over scorched ground, or a cloud of
// smoke.
fn draw_overlay(color: vec3<f32>, local: vec2<f32>, overlay: u32) -> vec3<f32> {
    let ink = palette[OVERLAY + overlay].rgb;
    if overlay == FIRE {
        return draw_fire(color, local, ink);
    }
    return draw_smoke(color, local, ink);
}

// The layers' color with every pattern evened out: what a label's ink has to contrast with.
fn backdrop_color(
    ground: u32,
    foliage: u32,
    level: u32,
    water: u32,
    structure: u32,
    grade: u32,
    condition: u32,
    overlay: u32,
) -> vec3<f32> {
    var color = palette[GROUND + ground].rgb;
    if present(foliage) {
        color = mix(color, palette[FOLIAGE + foliage - 1u].rgb, 0.8);
    }
    color = lighten(color, 0.02 * f32(level));
    if present(water) {
        color = water_color(level, water - 1u);
    }
    if present(condition) && condition - 1u != THIN_SNOW {
        color = draw_condition(color, vec2<f32>(0.0), condition - 1u);
    }
    if present(overlay) {
        color = mix(color, palette[OVERLAY + overlay - 1u].rgb, 0.3);
    }
    return color;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = (in.pixel - u.offset) / u.radius;
    let hex = hex_at(point);
    if any(hex < vec2<i32>(0)) || any(hex >= vec2<i32>(u.map_size)) {
        return vec4<f32>(0.0);
    }
    let texel = textureLoad(hexes, hex, 0);
    let ground = texel.r & 15u;
    let foliage = (texel.r >> 4u) & 7u;
    let route = (texel.r >> 7u) & 7u;
    let condition = (texel.r >> 10u) & 7u;
    let overlay = (texel.r >> 13u) & 3u;
    let level = texel.g & 255u;
    let water = (texel.g >> 8u) & 15u;
    let flow = (texel.g >> 12u) & 3u;
    let structure = texel.b & 3u;
    let grade = (texel.b >> 2u) & 3u;
    let structure_height = (texel.b >> 8u) & 63u;
    let local = point - center(hex);

    // Ground and foliage over it; higher ground is lighter.
    var color = palette[GROUND + ground].rgb;
    color = draw_ground(color, point, ground);
    if present(foliage) {
        color = draw_foliage(color, point, foliage - 1u);
    }
    color = lighten(color, 0.02 * f32(level));
    // How much of this pixel is water, which ice covers.
    var wet = 1.0;
    if present(water) {
        let shore = draw_water_hex(color, hex, local, point, level, water - 1u, flow);
        color = shore.rgb;
        wet = shore.a;
    }
    let way = way_of(texel);
    if way != WAY_NONE {
        let line = way_line(hex, local, way);
        if way == WAY_BRIDGE {
            color = draw_bridge(color, line, grade);
        } else if way == WAY_WALL {
            color = draw_wall(color, line, grade);
        } else {
            color = draw_route(color, line, point, route - 1u);
        }
    }
    if present(structure) && structure - 1u == BUILDING {
        color = draw_building(color, local, draw_structure(BUILDING, grade));
    }
    if present(condition) && condition - 1u == ICE {
        color = mix(color, draw_ice(color, point), wet);
    } else if present(condition) {
        color = draw_condition(color, local, condition - 1u);
    }
    if present(overlay) {
        color = draw_overlay(color, local, overlay - 1u);
    }

    // Distance in pixels to the visible edge of the hex, inside its share of the grid gap.
    let edge = edge_distance(local) * u.radius - u.grid_gap * 0.5;

    // Labels. Ground level is the one elevation; every other layer is an offset from it,
    // signed to say which way: water depth below the surface (which sits at ground level),
    // and a bridge deck, building or wall above.
    let has_water = present(water);
    let has_structure = present(structure);
    let depth = water - 1u;
    let top = structure_height;
    let top_sign = sign_for(top, SIGN_PLUS);
    let depth_sign = sign_for(depth, SIGN_MINUS);
    if u.labels != 0.0 {
        let backdrop = backdrop_color(
            ground,
            foliage,
            level,
            water,
            structure,
            grade,
            condition,
            overlay,
        );
        // Fixed rows, so a lone number still says which layer it belongs to: ground level on
        // top, the structure above it in the middle, the water below it at the bottom.
        if level != 0u || has_water || has_structure {
            color = ink_number(color, backdrop, local, -ROW_OFFSET, ROW_SIZE, level, SIGN_NONE);
        }
        if has_structure {
            color = ink_number(color, backdrop, local, 0.0, ROW_SIZE, top, top_sign);
        }
        if has_water {
            color = ink_number(color, backdrop, local, ROW_OFFSET, ROW_SIZE, depth, depth_sign);
        }
    }

    if u.brush >= 0.0 && hex_distance(hex, vec2<i32>(u.hover)) <= i32(u.brush) {
        color = mix(color, vec3<f32>(1.0), clamp(2.5 - edge, 0.0, 1.0));
    }

    // Without a grid, neighbors must meet exactly or seams would show between them.
    var coverage = 1.0;
    if u.grid_gap > 0.0 {
        coverage = clamp(edge + 0.5, 0.0, 1.0);
    }
    return vec4<f32>(color, coverage);
}
