// Draws the whole hex map in one pass: every pixel finds its hex, reads that hex's layers from
// the hex texture, and composes its color from the layer palette, with grid gaps, labels and
// the brush outline computed per pixel. Panning and zooming only change the uniforms.
//
// Layers are drawn bottom up: ground, foliage, elevation shading, water, a road or rail band,
// the structure, the weather condition, then fire as flames or smoke as rising puffs, and
// labels last, over every layer.
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
const PLANTED_FIELDS: u32 = 6u;
const RAIL: u32 = 3u;
const ICE: u32 = 0u;
const THIN_SNOW: u32 = 1u;
const DEEP_SNOW: u32 = 2u;
const MUD: u32 = 3u;
const BRIDGE: u32 = 2u;
const FIRE: u32 = 0u;
const SMOKE: u32 = 1u;
const RAPIDS: u32 = 1u;
const TORRENT: u32 = 2u;

// How much each construction class above light darkens a structure, matching `CLASS_SHADE` in
// map_view.rs.
const CLASS_SHADE: f32 = 0.12;

// Half widths, in map units, of a road or rail band and a bridge deck band.
const ROUTE_HALF_WIDTH: f32 = 0.16;
const BRIDGE_HALF_WIDTH: f32 = 0.3;

// Flame cell size in map units, and the color of a flame's hot core.
const FLAME_CELL: f32 = 0.46;
const FLAME_CORE: vec3<f32> = vec3<f32>(1.0, 0.88, 0.35);
// Smoke cell size in map units, and how much darker than the smoke tint its puffs draw.
const SMOKE_CELL: f32 = 0.5;
const SMOKE_SHADE: f32 = 0.62;

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

// A pseudo-random value in [0, 1) for a cell, for snow speckle.
fn hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// `color` with foliage drawn over it: woods and jungle as round canopies with the ground
// showing faintly between them, planted fields as rows. A canopy or row is centered on the
// hex center.
fn draw_foliage(color: vec3<f32>, local: vec2<f32>, foliage: u32) -> vec3<f32> {
    let leaf = palette[FOLIAGE + foliage].rgb;
    if foliage == PLANTED_FIELDS {
        let row = fract(local.y * 5.0 + 0.5) < 0.6;
        return select(mix(color, leaf, 0.4), leaf, row);
    }
    let cell = fract(local * 3.0 + 0.5) - 0.5;
    return select(mix(color, leaf, 0.6), leaf, length(cell) < 0.38);
}

// `color` under water `depth` levels deep, darker the deeper, with white streaks across rapids
// and heavier ones across torrents.
fn draw_water(local: vec2<f32>, level: u32, depth: u32, flow: u32) -> vec3<f32> {
    var color = mix(lighten(palette[WATER].rgb, 0.01 * f32(level)), vec3<f32>(0.0), 0.08 * f32(depth));
    if flow == RAPIDS || flow == TORRENT {
        let torrent = flow == TORRENT;
        let stripe = fract((local.x * 0.6 + local.y) * select(5.0, 7.0, torrent));
        if stripe < select(0.18, 0.35, torrent) {
            color = mix(color, vec3<f32>(1.0), select(0.35, 0.55, torrent));
        }
    }
    return color;
}

// `color` with a road band across the hex, or dashed dark rail track.
fn draw_route(color: vec3<f32>, local: vec2<f32>, route: u32) -> vec3<f32> {
    if abs(local.y) >= ROUTE_HALF_WIDTH {
        return color;
    }
    let surface = palette[ROUTE + route].rgb;
    if route != RAIL {
        return surface;
    }
    let dash = fract(local.x * 4.0 + 0.25) < 0.6;
    return select(mix(color, surface, 0.35), surface, dash);
}

// `color` with a structure of `kind` and construction class `grade` over it: buildings and walls
// fill the hex and a bridge deck crosses it as a band, darker for stronger classes.
fn draw_structure(color: vec3<f32>, local: vec2<f32>, kind: u32, grade: u32) -> vec3<f32> {
    if kind == BRIDGE && abs(local.y) >= BRIDGE_HALF_WIDTH {
        return color;
    }
    return palette[STRUCTURE + kind].rgb * (1.0 - CLASS_SHADE * f32(grade));
}

// `color` tinted by a weather condition: ice pale cyan, thin snow a light speckle, deep snow
// white and mud brown.
fn draw_condition(color: vec3<f32>, local: vec2<f32>, condition: u32) -> vec3<f32> {
    let tint = palette[CONDITION + condition].rgb;
    if condition == ICE {
        return mix(color, tint, 0.7);
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

// How much of a smoke puff covers this pixel, and how strongly that puff shows. Puffs rise in
// trails of three from staggered rows of cells `SMOKE_CELL` map units square, each puff larger
// and fainter than the one below it, with every trail drifting left or right and scaled by its
// own random thickness.
fn puff_coverage(local: vec2<f32>) -> f32 {
    let row = floor(local.y / SMOKE_CELL);
    let shifted = vec2<f32>(local.x / SMOKE_CELL + 0.5 * (row % 2.0), local.y / SMOKE_CELL);
    let cell = floor(shifted);
    let q = shifted - cell - vec2<f32>(0.5, 0.5);
    let drift = select(-1.0, 1.0, hash(cell) > 0.5);
    let thickness = mix(0.55, 1.5, hash(cell + vec2<f32>(3.0, 1.0)));
    var coverage = 0.0;
    for (var i = 0; i < 3; i++) {
        let k = f32(i);
        let center = vec2<f32>(drift * (0.1 * k - 0.1), 0.28 - 0.28 * k);
        let radius = thickness * (0.07 + 0.045 * k);
        let outside = (length(q - center) - radius) * SMOKE_CELL * u.radius;
        coverage = max(coverage, (1.0 - 0.15 * k) * clamp(0.5 - outside, 0.0, 1.0));
    }
    return coverage;
}

// How much of a flame covers this pixel. Flames stand in staggered rows of cells
// `FLAME_CELL` map units square, each a tongue that tapers to a flickering tip; `core` asks
// for the hotter inner tongue instead.
fn flame_coverage(local: vec2<f32>, core: bool) -> f32 {
    let row = floor(local.y / FLAME_CELL);
    let shifted = vec2<f32>(local.x / FLAME_CELL + 0.5 * (row % 2.0), local.y / FLAME_CELL);
    let cell = floor(shifted);
    let q = shifted - cell - vec2<f32>(0.5, 0.5);
    // t runs from the tip (0) at the top of the cell to the base (1) near its bottom.
    let t = (q.y + 0.45) / 0.85;
    if t <= 0.0 || t >= 1.0 {
        return 0.0;
    }
    let flicker = 0.07 * sin(t * 7.0 + hash(cell) * 6.2832) * (1.0 - t);
    var half_width = 0.34 * pow(t, 0.7) * sqrt(1.0 - t * t * t);
    if core {
        half_width = select(0.0, half_width * 0.45, t > 0.35);
    }
    let outside = (abs(q.x - flicker) - half_width) * FLAME_CELL * u.radius;
    return clamp(0.5 - outside, 0.0, 1.0);
}

// `color` with fire or smoke drawn over it, leaving the terrain beneath visible: fire as rows
// of flames with yellow cores, smoke as trails of
// rising puffs.
fn draw_overlay(color: vec3<f32>, local: vec2<f32>, overlay: u32) -> vec3<f32> {
    let ink = palette[OVERLAY + overlay].rgb;
    if overlay == FIRE {
        let flames = mix(color, ink, flame_coverage(local, false));
        return mix(flames, FLAME_CORE, flame_coverage(local, true));
    }
    return mix(color, ink * SMOKE_SHADE, puff_coverage(local));
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
        color = draw_water(vec2<f32>(0.0), level, water - 1u, 0u);
    }
    if present(structure) && structure - 1u != BRIDGE {
        color = draw_structure(color, vec2<f32>(0.0), structure - 1u, grade);
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
    if present(foliage) {
        color = draw_foliage(color, local, foliage - 1u);
    }
    color = lighten(color, 0.02 * f32(level));
    if present(water) {
        color = draw_water(local, level, water - 1u, flow);
    }
    if present(route) {
        color = draw_route(color, local, route - 1u);
    }
    if present(structure) {
        color = draw_structure(color, local, structure - 1u, grade);
    }
    if present(condition) {
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
