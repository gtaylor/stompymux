// Draws the whole hex map in one pass: every pixel finds its hex, looks up that hex's terrain
// and elevation code, and is colored from the palette, with grid gaps, elevation digits and
// the brush outline computed per pixel. Panning and zooming only change the uniforms.
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
    // Nonzero to label hexes with their elevation.
    digits: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
// Fill color per hex code (terrain index * 10 + elevation). Alpha holds the label ink:
// 0 for black, 1 for white.
@group(0) @binding(1) var<uniform> palette: array<vec4<f32>, 150>;
// One hex code per texel, indexed by hex coordinate.
@group(0) @binding(2) var hexes: texture_2d<u32>;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = (in.pixel - u.offset) / u.radius;
    let hex = hex_at(point);
    if any(hex < vec2<i32>(0)) || any(hex >= vec2<i32>(u.map_size)) {
        return vec4<f32>(0.0);
    }
    let code = textureLoad(hexes, hex, 0).r;
    let entry = palette[code];
    var color = entry.rgb;
    let local = point - center(hex);
    // Distance in pixels to the visible edge of the hex, inside its share of the grid gap.
    let edge = edge_distance(local) * u.radius - u.grid_gap * 0.5;

    let elevation = code % 10u;
    if u.digits != 0.0 && elevation != 0u {
        let half_height = u.radius * 0.35;
        let stroke = (digit_distance(local * u.radius / half_height, elevation) - 0.16) * half_height;
        color = mix(color, vec3<f32>(entry.a), clamp(0.5 - stroke, 0.0, 1.0));
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
