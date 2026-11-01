// The render view's desk: the cutting mat and what lies on it, each painted by its material
// in its own millimetres. Appended to shader.wgsl. See docs/design/viewport.md.

// An instance whose overhang slot is under this is a prop: the slot holds -2 - material,
// after references::Material.
const PROP_BELOW = -1.5;

// The materials' own colours: the real things', not the window's, so not theme tokens.
const MAT_GREEN = vec3<f32>(0.07, 0.29, 0.22);
const MAT_EDGE = vec3<f32>(0.05, 0.21, 0.16);
const MAT_INK = vec3<f32>(0.78, 0.88, 0.80);
const CAN_RED = vec3<f32>(0.78, 0.10, 0.06);
const CAN_WHITE = vec3<f32>(0.95, 0.95, 0.93);
const ALUMINIUM = vec3<f32>(0.82, 0.84, 0.87);
const BANANA_YELLOW = vec3<f32>(0.95, 0.79, 0.22);
const BANANA_STEM = vec3<f32>(0.44, 0.42, 0.17);
const BANANA_TIP = vec3<f32>(0.24, 0.16, 0.08);
const PEN_BARREL = vec3<f32>(0.80, 0.86, 0.92);
const PEN_CAP = vec3<f32>(0.10, 0.28, 0.74);
const PEN_TIP = vec3<f32>(0.92, 0.92, 0.93);
const BRASS = vec3<f32>(0.80, 0.66, 0.36);
const NICKEL = vec3<f32>(0.78, 0.79, 0.80);
const STEEL = vec3<f32>(0.72, 0.74, 0.77);

// Millimetres of the unprinted band round the mat's grid.
const MAT_BORDER = 10.0;
// Where the banana's arc starts and ends, radians. Must match references/shapes.rs.
const BANANA_FROM = 0.75;
const BANANA_TO = 2.39;
// Where on a can its printed wrap starts and ends, millimetres up its wall.
const CAN_LABEL_FROM = 14.0;
const CAN_LABEL_TO = 100.0;
// Radius of the coin's nickel middle inside its brass ring, millimetres.
const COIN_MIDDLE = 8.13;

// How much of a line `width_mm` wide every `spacing_mm` covers a point at `at_mm`, smoothed
// over a pixel and faded where the lines close to a few pixels apart.
fn ruled(at_mm: f32, spacing_mm: f32, width_mm: f32, mm_per_px: f32) -> f32 {
    let off_mm = abs(fract(at_mm / spacing_mm - 0.5) - 0.5) * spacing_mm;
    let pixel = max(mm_per_px, 1e-4);
    let line = 1.0 - smoothstep(width_mm * 0.5 - pixel * 0.5, width_mm * 0.5 + pixel * 0.5, off_mm);
    return line * (1.0 - smoothstep(0.12, 0.3, pixel / spacing_mm));
}

// A green self-healing mat: a 10 mm grid inside a plain band, every fifth line heavier,
// and a 45 degree line from each near corner. Its mesh is a unit box sized by `globals.mat`.
fn mat_colour(local: vec3<f32>, px: vec3<f32>) -> vec3<f32> {
    if (local.z < 0.999) {
        return MAT_EDGE;
    }
    let size = globals.mat.xy;
    let at = local.xy * size;
    let inside = all(at > vec2<f32>(MAT_BORDER)) && all(at < size - vec2<f32>(MAT_BORDER));
    let pixel = max(px.x * size.x, px.y * size.y);
    let minor = max(ruled(at.x, 10.0, 0.3, pixel), ruled(at.y, 10.0, 0.3, pixel));
    let major = max(ruled(at.x, 50.0, 0.6, pixel), ruled(at.y, 50.0, 0.6, pixel));
    let diagonal = max(
        ruled((at.x - at.y) * 0.70710678, 1000.0, 0.4, pixel),
        ruled((size.x - at.x - at.y) * 0.70710678, 1000.0, 0.4, pixel),
    );
    let ink = max(max(minor * 0.3, major * 0.65), diagonal * 0.45) * select(0.0, 1.0, inside);
    let base = select(MAT_EDGE, MAT_GREEN, inside);
    let numbers = max(edge_numbers(at.yx, size.yx, pixel), edge_numbers(at, size, pixel));
    return mix(base, MAT_INK, max(ink, numbers * 0.8));
}

// Which segments of a seven-segment figure light for each digit: a top, b and c the right
// from the top, d the bottom, e and f the left from the bottom, g the middle.
const DIGIT_SEGMENTS = array<u32, 10>(0x3Fu, 0x06u, 0x5Bu, 0x4Fu, 0x66u, 0x6Du, 0x7Du, 0x07u, 0x7Fu, 0x6Fu);
// A figure's height and width, and the stroke, mm: as printed on a mat's edge.
const DIGIT_TALL_MM = 3.6;
const DIGIT_WIDE_MM = 2.0;
const DIGIT_STROKE_MM = 0.35;

fn segment_cover(at: vec2<f32>, start: vec2<f32>, end: vec2<f32>, pixel: f32) -> f32 {
    let along = end - start;
    let share = clamp(dot(at - start, along) / dot(along, along), 0.0, 1.0);
    let apart = distance(at, start + along * share);
    return 1.0 - smoothstep(DIGIT_STROKE_MM * 0.5 - pixel * 0.5, DIGIT_STROKE_MM * 0.5 + pixel * 0.5, apart);
}

// How much of `digit`, its bottom left corner at the origin, covers `at`, mm.
fn digit_cover(at: vec2<f32>, digit: u32, pixel: f32) -> f32 {
    let w = DIGIT_WIDE_MM;
    let h = DIGIT_TALL_MM;
    let ends = array<vec4<f32>, 7>(
        vec4<f32>(0.0, h, w, h),
        vec4<f32>(w, h, w, h * 0.5),
        vec4<f32>(w, h * 0.5, w, 0.0),
        vec4<f32>(0.0, 0.0, w, 0.0),
        vec4<f32>(0.0, 0.0, 0.0, h * 0.5),
        vec4<f32>(0.0, h * 0.5, 0.0, h),
        vec4<f32>(0.0, h * 0.5, w, h * 0.5),
    );
    var cover = 0.0;
    let lit = DIGIT_SEGMENTS[min(digit, 9u)];
    for (var segment = 0u; segment < 7u; segment = segment + 1u) {
        if ((lit >> segment & 1u) == 1u) {
            let end = ends[segment];
            cover = max(cover, segment_cover(at, end.xy, end.zw, pixel));
        }
    }
    return cover;
}

// The centimetres along the mat's near edge, printed in its band under each 10 mm line:
// `at.x` runs along the edge, `at.y` into the mat.
fn edge_numbers(at: vec2<f32>, size: vec2<f32>, pixel: f32) -> f32 {
    let line = round(at.x / 10.0);
    let centimetres = u32(line);
    if (line < 1.0 || line * 10.0 > size.x - MAT_BORDER || at.y > MAT_BORDER) {
        return 0.0;
    }
    let tens = centimetres / 10u;
    let figures = select(1.0, 2.0, tens > 0u);
    let gap = DIGIT_WIDE_MM * 0.6;
    let width = figures * DIGIT_WIDE_MM + (figures - 1.0) * gap;
    let corner = vec2<f32>(line * 10.0 - width * 0.5, (MAT_BORDER - DIGIT_TALL_MM) * 0.5);
    let ones_at = at - corner - vec2<f32>((figures - 1.0) * (DIGIT_WIDE_MM + gap), 0.0);
    var cover = digit_cover(ones_at, centimetres % 10u, pixel);
    if (tens > 0u) {
        cover = max(cover, digit_cover(at - corner, tens % 10u, pixel));
    }
    return cover * (1.0 - smoothstep(0.3, 0.6, pixel));
}

// A can: bare aluminium at its foot and lid, a red lacquered wrap between with a white wave.
fn can_finish(local: vec3<f32>, px: vec3<f32>) -> vec4<f32> {
    let pixel = max(max(px.x, px.y), px.z);
    let around = atan2(local.y, local.x);
    let wave = 57.0 + 11.0 * sin(2.0 * around + 0.6);
    let ribbon = 1.0 - smoothstep(6.0 - pixel, 6.0 + pixel, abs(local.z - wave));
    let wrap = smoothstep(CAN_LABEL_FROM - pixel, CAN_LABEL_FROM + pixel, local.z)
        * (1.0 - smoothstep(CAN_LABEL_TO - pixel, CAN_LABEL_TO + pixel, local.z));
    let painted = mix(CAN_RED, CAN_WHITE, ribbon);
    return vec4<f32>(mix(ALUMINIUM, painted, wrap), mix(1.0, 0.2, wrap));
}

fn hash(cell: vec3<f32>) -> f32 {
    return fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

// Smooth value noise, 0.0 to 1.0, a feature a unit across.
fn value_noise(at: vec3<f32>) -> f32 {
    let cell = floor(at);
    let f = at - cell;
    let w = f * f * (3.0 - 2.0 * f);
    let a = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), w.x);
    let b = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), w.x);
    let c = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), w.x);
    let d = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), w.x);
    return mix(mix(a, b, w.y), mix(c, d, w.y), w.z);
}

// A ripe banana: yellow, greening into a brown stem, a dark blossom end, streaked along its
// length and freckled with round brown spots.
fn banana_colour(local: vec3<f32>) -> vec3<f32> {
    let share = clamp((atan2(local.y, local.x) - BANANA_FROM) / (BANANA_TO - BANANA_FROM), 0.0, 1.0);
    var colour = mix(BANANA_YELLOW, BANANA_STEM, smoothstep(0.82, 0.93, share));
    colour = mix(colour, BANANA_TIP, 1.0 - smoothstep(0.015, 0.04, share));
    let streak = value_noise(vec3<f32>(share * 6.0, local.z * 0.35, length(local.xy) * 0.35));
    colour = colour * (0.9 + 0.12 * streak);
    let freckle = smoothstep(0.8, 0.88, value_noise(local * 0.7));
    return mix(colour, BANANA_TIP, freckle * 0.55);
}

// A capped ballpoint along its own z: a tip with a brass ball, a pale barrel, a blue cap.
fn pen_finish(along_mm: f32) -> vec4<f32> {
    if (along_mm < 1.2) {
        return vec4<f32>(BRASS, 1.0);
    }
    if (along_mm < 13.5) {
        return vec4<f32>(PEN_TIP, 0.0);
    }
    if (along_mm < 124.0) {
        return vec4<f32>(PEN_BARREL, 0.0);
    }
    return vec4<f32>(PEN_CAP, 0.0);
}

// What a prop of the desk paints: its colour by its material, then the studio's light.
fn prop_colour(in: ModelFragment, front_facing: bool, px: vec3<f32>) -> vec4<f32> {
    var normal = normalize(select(in.normal, in.smooth_normal, in.smoothed > 0.5));
    if (!front_facing) {
        normal = -normal;
    }
    let material = i32(round(-2.0 - in.overhang));
    // xyz the colour, w how metallic; then how rough it is.
    var surface = vec4<f32>(STEEL, 1.0);
    var roughness = 0.4;
    switch material {
        case 0: {
            surface = vec4<f32>(mat_colour(in.local, px), 0.0);
            roughness = 0.85;
        }
        case 1: {
            surface = can_finish(in.local, px);
            roughness = 0.18;
        }
        case 2: {
            surface = vec4<f32>(banana_colour(in.local), 0.0);
            roughness = 0.45;
        }
        case 3: {
            surface = pen_finish(in.local.z);
            roughness = 0.25;
        }
        case 4: {
            let out = length(in.local.xy);
            let rim = smoothstep(10.6, 10.9, out);
            let metal = select(BRASS, NICKEL, out < COIN_MIDDLE);
            surface = vec4<f32>(metal * (1.0 - 0.12 * rim), 1.0);
            roughness = 0.3;
        }
        default: {
            surface = vec4<f32>(STEEL, 1.0);
            roughness = 0.45;
        }
    }
    return vec4<f32>(finish(surface.rgb, surface.a, roughness, normal, in.world), 1.0);
}

// The render view's room: a photographer's infinity cove, the table sweeping up into a wall
// round the set with no horizon, lit by the studio. See docs/design/viewport.md.
const COVE_COLOUR = vec3<f32>(0.82, 0.82, 0.83);
// Millimetres from the mat's middle to where the floor starts curving up, and the radius
// of the curve, which ends in an upright wall.
const COVE_FLOOR_MM = 1400.0;
const COVE_CURVE_MM = 700.0;
// How the cove is lit, in linear light: what the room fills it with, the key's share, and
// how much the far wall falls off as a lit set's background does.
const COVE_AMBIENT = 0.62;
const COVE_KEY = 0.42;
const COVE_FALLOFF = 0.25;
const MAT_SHADOW = 0.35;
const MAT_SHADOW_MM = 25.0;
// How dark what stands on the table shades it from the key.
const TABLE_SHADOW = 0.45;
// The table's height, plate millimetres: under the mat, whose top lies just under the plate's
// level. Must match SINK_MM and MAT_THICK_MM in references.rs.
const TABLE_Z = -3.05;
const COVE_STEPS = 24;

fn rect_distance(at: vec2<f32>, low: vec2<f32>, high: vec2<f32>) -> f32 {
    let outside = max(max(low - at, at - high), vec2<f32>(0.0));
    return length(outside);
}

// How far a point stands from the cove's surface, in its own profile: out from the middle
// and up from the table. Unsigned, which is enough to march to it from inside.
fn cove_distance(out_mm: f32, up_mm: f32) -> f32 {
    let bend = vec2<f32>(COVE_FLOOR_MM, COVE_CURVE_MM);
    let at = vec2<f32>(out_mm, up_mm);
    if (out_mm <= COVE_FLOOR_MM) {
        return min(abs(up_mm), abs(distance(at, bend) - COVE_CURVE_MM));
    }
    if (up_mm <= COVE_CURVE_MM) {
        return abs(COVE_CURVE_MM - distance(at, bend));
    }
    return abs(COVE_FLOOR_MM + COVE_CURVE_MM - out_mm);
}

// The cove's normal, facing in, at a point of its profile.
fn cove_normal(out_mm: f32, up_mm: f32, outward: vec2<f32>) -> vec3<f32> {
    if (out_mm <= COVE_FLOOR_MM) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let towards_bend = normalize(vec2<f32>(COVE_FLOOR_MM, COVE_CURVE_MM) - vec2<f32>(out_mm, up_mm));
    if (up_mm > COVE_CURVE_MM) {
        return vec3<f32>(-outward, 0.0);
    }
    return vec3<f32>(outward * towards_bend.x, towards_bend.y);
}

@fragment
fn backdrop_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let share = (position.xy - globals.screen.xy) / max(globals.screen.zw, vec2<f32>(1.0));
    let ndc = vec2<f32>(share.x * 2.0 - 1.0, 1.0 - share.y * 2.0);
    let near = globals.inverse_view_projection * vec4<f32>(ndc, 0.0, 1.0);
    let far = globals.inverse_view_projection * vec4<f32>(ndc, 1.0, 1.0);
    let origin = near.xyz / near.w;
    let ray = normalize(far.xyz / far.w - origin);
    let middle = (globals.desk.xy + globals.desk.zw) * 0.5;
    // Straight to the floor where the ray meets it inside the cove; marched otherwise.
    var travelled = 0.0;
    if (ray.z < -1e-4) {
        travelled = (TABLE_Z - origin.z) / ray.z;
    }
    var hit = origin + ray * travelled;
    if (ray.z >= -1e-4 || length(hit.xy - middle) > COVE_FLOOR_MM) {
        travelled = 0.0;
        for (var step = 0; step < COVE_STEPS; step = step + 1) {
            let at = origin + ray * travelled;
            let gap = cove_distance(length(at.xy - middle), at.z - TABLE_Z);
            travelled = travelled + max(gap, 1.0);
        }
        hit = origin + ray * travelled;
    }
    let outward_xy = hit.xy - middle;
    let out_mm = length(outward_xy);
    let outward = select(vec2<f32>(1.0, 0.0), outward_xy / max(out_mm, 1e-3), out_mm > 1e-3);
    let normal = cove_normal(out_mm, hit.z - TABLE_Z, outward);
    let key = normalize(-globals.key_towards.xyz);
    var light = COVE_AMBIENT + COVE_KEY * max(dot(normal, key), 0.0);
    if (out_mm <= COVE_FLOOR_MM) {
        let away = rect_distance(hit.xy, globals.desk.xy, globals.desk.zw);
        let under_mat = 1.0 - MAT_SHADOW * (1.0 - smoothstep(0.0, MAT_SHADOW_MM, away));
        light = light * under_mat * mix(1.0 - TABLE_SHADOW, 1.0, key_reaches(hit, normal));
    }
    light = light * (1.0 - COVE_FALLOFF * smoothstep(0.0, COVE_FLOOR_MM + COVE_CURVE_MM, out_mm));
    return vec4<f32>(filmic(linear_of(COVE_COLOUR) * light), 1.0);
}
