// The render view's desk: the cutting mat and what lies on it, each painted by its material
// in its own millimetres. Appended to shader.wgsl. See docs/design/viewport.md.

// An instance whose overhang slot is under this is a prop: the slot holds -2 - material,
// after references::Material.
const PROP_BELOW = -1.5;

// The materials' own colours: the real things', not the window's, so not theme tokens.
const MAT_GREEN = vec3<f32>(0.07, 0.29, 0.22);
const MAT_EDGE = vec3<f32>(0.05, 0.21, 0.16);
const MAT_INK = vec3<f32>(0.78, 0.88, 0.80);
const CAN_RED = vec3<f32>(0.74, 0.07, 0.10);
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
const PRIMER = vec3<f32>(0.55, 0.56, 0.58);

// The mat, millimetres: its size and thickness, and the unprinted band round its grid.
// Must match MAT_MM in references.rs.
const MAT_SIZE = vec2<f32>(220.0, 300.0);
const MAT_TOP = 3.0;
const MAT_BORDER = 10.0;
// How dark a prop's shadow lies on the mat at its darkest.
const PROP_SHADOW = 0.5;
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

fn segment_distance(at: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let along = end - start;
    let share = clamp(dot(at - start, along) / max(dot(along, along), 1e-6), 0.0, 1.0);
    return distance(at, start + along * share);
}

// How much of the light a point of the mat keeps under the props' soft shadows.
fn under_props(at: vec2<f32>) -> f32 {
    var dark = 0.0;
    for (var index = 0u; index < MAX_PROP_SHADOWS; index = index + 1u) {
        let shadow = globals.prop_shadows[index];
        let reach = shadow.reach.x;
        let apart = segment_distance(at, shadow.ends.xy, shadow.ends.zw);
        dark = max(dark, 1.0 - smoothstep(reach * 0.7, reach + 0.4 * reach + 3.0, apart));
    }
    return 1.0 - PROP_SHADOW * dark;
}

// A green self-healing mat: a 10 mm grid inside a plain band, every fifth line heavier,
// and a 45 degree line from each near corner.
fn mat_colour(local: vec3<f32>, px: vec3<f32>) -> vec3<f32> {
    if (local.z < MAT_TOP - 0.01) {
        return MAT_EDGE;
    }
    let at = local.xy;
    let inside = all(at > vec2<f32>(MAT_BORDER)) && all(at < MAT_SIZE - vec2<f32>(MAT_BORDER));
    let pixel = max(px.x, px.y);
    let minor = max(ruled(at.x, 10.0, 0.3, pixel), ruled(at.y, 10.0, 0.3, pixel));
    let major = max(ruled(at.x, 50.0, 0.6, pixel), ruled(at.y, 50.0, 0.6, pixel));
    let diagonal = max(
        ruled((at.x - at.y) * 0.70710678, 1000.0, 0.4, pixel),
        ruled((MAT_SIZE.x - at.x - at.y) * 0.70710678, 1000.0, 0.4, pixel),
    );
    let ink = max(max(minor * 0.3, major * 0.65), diagonal * 0.45) * select(0.0, 1.0, inside);
    let base = select(MAT_EDGE, MAT_GREEN, inside);
    return mix(base, MAT_INK, ink) * under_props(at);
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

// A ripe banana: yellow, greening into a brown stem, a dark blossom end, a few freckles.
fn banana_colour(local: vec3<f32>) -> vec3<f32> {
    let share = clamp((atan2(local.y, local.x) - BANANA_FROM) / (BANANA_TO - BANANA_FROM), 0.0, 1.0);
    var colour = mix(BANANA_YELLOW, BANANA_STEM, smoothstep(0.82, 0.93, share));
    colour = mix(colour, BANANA_TIP, 1.0 - smoothstep(0.015, 0.04, share));
    let freckle = step(0.985, hash(floor(local * 0.9)));
    return mix(colour, BANANA_TIP, freckle * 0.45);
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
    // xyz the colour, w how metallic; then how strongly it reflects.
    var surface = vec4<f32>(PRIMER, 0.0);
    var shine = 0.2;
    switch material {
        case 0: {
            surface = vec4<f32>(mat_colour(in.local, px), 0.0);
            shine = 0.2;
        }
        case 1: {
            surface = can_finish(in.local, px);
            shine = 1.0;
        }
        case 2: {
            surface = vec4<f32>(banana_colour(in.local), 0.0);
            shine = 0.6;
        }
        case 3: {
            surface = pen_finish(in.local.z);
            shine = 0.9;
        }
        case 4: {
            surface = vec4<f32>(select(BRASS, NICKEL, length(in.local.xy) < COIN_MIDDLE), 1.0);
            shine = 0.85;
        }
        case 5: {
            surface = vec4<f32>(STEEL, 1.0);
            shine = 0.6;
        }
        default: {}
    }
    return vec4<f32>(finish(surface.rgb, surface.a, shine, normal, in.world), 1.0);
}
