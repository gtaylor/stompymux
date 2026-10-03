// Draws the whole hex map in one pass: every pixel finds its hex, reads that hex's layers from
// the hex texture, and composes its color from the terrain palette, with grid gaps, labels,
// the brush outline and unsavable-hex hatching computed per pixel. Panning and zooming only
// change the uniforms.
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
    // Value to label hexes with (see LABEL_*), or negative for no labels.
    label: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
// One color per terrain in `Terrain::ALL` order, then the ink threshold in entry 15's red.
@group(0) @binding(1) var<uniform> palette: array<vec4<f32>, 16>;
// One texel of packed layers per hex, indexed by hex coordinate; see `hex_texel` in render.rs.
@group(0) @binding(2) var hexes: texture_2d<u32>;

// Palette positions, matching `Terrain::ALL`.
const LIGHT_FOREST: u32 = 2u;
const WATER: u32 = 4u;
const ICE: u32 = 5u;
const BRIDGE: u32 = 6u;
const FIRE: u32 = 9u;
const SMOKE: u32 = 10u;
const BUILDING: u32 = 12u;
const WALL: u32 = 13u;
const INK_THRESHOLD: u32 = 15u;

// Structure kinds in the texel's alpha.
const STRUCTURE_BUILDING: u32 = 1u;
const STRUCTURE_WALL: u32 = 2u;
const STRUCTURE_BRIDGE: u32 = 3u;

// Label modes, matching `Label` in render.rs.
const LABEL_ALL: i32 = 0;
const LABEL_LEVEL: i32 = 1;
const LABEL_DEPTH: i32 = 2;
const LABEL_HEIGHT: i32 = 3;

// Digit half heights in map units for one centered number and for the stacked rows of
// LABEL_ALL, and how far the top and bottom rows sit from the hex center.
const SINGLE_SIZE: f32 = 0.35;
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

// Cube coordinates for hex distance, matching `BattleHexCoordinate::distance`.
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

// `color` with `value` written over it in black or white, whichever contrasts, centered
// `y` map units below the hex center, led by `sign` (a SIGN_ value). `size` is the half
// height of a lone digit in map units; longer numbers are drawn smaller to fit.
fn ink_number(
    color: vec3<f32>,
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
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let ink = select(vec3<f32>(1.0), vec3<f32>(0.0), luminance > palette[INK_THRESHOLD].r);
    return mix(color, ink, clamp(0.5 - stroke, 0.0, 1.0));
}

// `color` mixed toward white by `amount`.
fn lighten(color: vec3<f32>, amount: f32) -> vec3<f32> {
    return mix(color, vec3<f32>(1.0), clamp(amount, 0.0, 0.7));
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
    let woods = (texel.r >> 4u) & 3u;
    let overlay = (texel.r >> 6u) & 3u;
    let level = texel.g;
    let water = texel.b & 15u;
    let frozen = ((texel.b >> 4u) & 1u) == 1u;
    let unsavable = ((texel.b >> 7u) & 1u) == 1u;
    let structure = texel.a >> 6u;
    let structure_height = texel.a & 63u;
    let local = point - center(hex);

    // Ground, then woods over it; higher ground is lighter.
    var color = palette[ground].rgb;
    if woods != 0u {
        color = palette[LIGHT_FOREST + woods - 1u].rgb;
    }
    color = lighten(color, 0.02 * f32(level));
    // Water and ice over that, darker the deeper they are.
    if water != 0u {
        let depth = f32(water - 1u);
        let surface = palette[select(WATER, ICE, frozen)].rgb;
        color = mix(lighten(surface, 0.01 * f32(level)), vec3<f32>(0.0), 0.08 * depth);
    }
    // Buildings and walls fill the hex; a bridge deck crosses it as a band.
    if structure == STRUCTURE_BUILDING {
        color = palette[BUILDING].rgb;
    } else if structure == STRUCTURE_WALL {
        color = palette[WALL].rgb;
    } else if structure == STRUCTURE_BRIDGE && abs(local.y) < 0.3 {
        color = palette[BRIDGE].rgb;
    }
    // Fire and smoke tint whatever they cover.
    if overlay == 1u {
        color = mix(color, palette[FIRE].rgb, 0.7);
    } else if overlay == 2u {
        color = mix(color, palette[SMOKE].rgb, 0.6);
    }

    // Distance in pixels to the visible edge of the hex, inside its share of the grid gap.
    let edge = edge_distance(local) * u.radius - u.grid_gap * 0.5;

    // Hexes a map file cannot store are hatched red.
    if unsavable {
        let stripe = fract((in.pixel.x + in.pixel.y) / 8.0);
        color = mix(color, vec3<f32>(0.85, 0.05, 0.05), select(0.0, 0.75, stripe < 0.4));
    }

    // Labels. Ground level is the one elevation; every other layer is an offset from it,
    // signed to say which way: water depth below the surface (which sits at ground level),
    // and a bridge deck, building or wall above.
    let mode = i32(u.label);
    let has_water = water != 0u;
    let has_structure = structure != 0u;
    let depth = water - 1u;
    let top = structure_height;
    let top_sign = sign_for(top, SIGN_PLUS);
    let depth_sign = sign_for(depth, SIGN_MINUS);
    if mode == LABEL_ALL {
        // Fixed rows, so a lone number still says which layer it belongs to.
        if has_structure {
            color = ink_number(color, local, -ROW_OFFSET, ROW_SIZE, top, top_sign);
        }
        if level != 0u || has_water || has_structure {
            color = ink_number(color, local, 0.0, ROW_SIZE, level, SIGN_NONE);
        }
        if has_water {
            color = ink_number(color, local, ROW_OFFSET, ROW_SIZE, depth, depth_sign);
        }
    } else if mode == LABEL_LEVEL && level != 0u {
        color = ink_number(color, local, 0.0, SINGLE_SIZE, level, SIGN_NONE);
    } else if mode == LABEL_DEPTH && has_water {
        color = ink_number(color, local, 0.0, SINGLE_SIZE, depth, depth_sign);
    } else if mode == LABEL_HEIGHT && has_structure {
        color = ink_number(color, local, 0.0, SINGLE_SIZE, top, top_sign);
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
