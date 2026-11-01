// Copies the viewport's own colour plane, window-sized, into egui's pass texel for texel,
// shading creases from the depth plane on the way at rest. See docs/design/viewport.md.

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var depth_plane: texture_depth_2d;

struct Occlusion {
    inverse_projection: mat4x4<f32>,
    projection: mat4x4<f32>,
    // Left, top, width and height of the viewport, framebuffer pixels.
    viewport: vec4<f32>,
    // x 1.0 while creases are shaded, y the reach in mm, z the strength.
    shading: vec4<f32>,
};

@group(0) @binding(2) var<uniform> occlusion: Occlusion;

// Samples a pixel takes, on a spiral turned by a per-pixel noise so their pattern reads as
// grain rather than rings.
const SAMPLES = 16;
const GOLDEN_ANGLE = 2.39996323;
// The most pixels a crease reaches across, so a close zoom costs no more than a far one.
const MOST_REACH_PX = 40.0;
// Below this a surface counts as flat towards a sample, so a plane never shades itself.
const BIAS = 0.15;

// One triangle covering the whole viewport; egui's viewport and scissor trim it.
@vertex
fn present_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn depth_at(pixel: vec2<f32>) -> f32 {
    let low = occlusion.viewport.xy;
    let high = low + occlusion.viewport.zw - vec2<f32>(1.0);
    return textureLoad(depth_plane, vec2<i32>(clamp(pixel, low, high)), 0);
}

// The point the camera sees at a pixel's centre and depth, in its own millimetres.
fn seen_at(pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let centre = floor(pixel) + vec2<f32>(0.5);
    let share = (centre - occlusion.viewport.xy) / occlusion.viewport.zw;
    let ndc = vec4<f32>(share.x * 2.0 - 1.0, 1.0 - share.y * 2.0, depth, 1.0);
    let point = occlusion.inverse_projection * ndc;
    return point.xyz / point.w;
}

// Of the two steps to a neighbour, the shorter: at a silhouette the other runs to whatever
// stands behind, and a normal taken across it would be nonsense.
fn step_along(here: vec3<f32>, pixel: vec2<f32>, by: vec2<f32>) -> vec3<f32> {
    let ahead = seen_at(pixel + by, depth_at(pixel + by)) - here;
    let behind = here - seen_at(pixel - by, depth_at(pixel - by));
    return select(behind, ahead, abs(ahead.z) < abs(behind.z));
}

fn noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

// How much of the light around a point stays out, 0.0 open to 1.0 enclosed.
fn enclosed(pixel: vec2<f32>, depth: f32) -> f32 {
    let here = seen_at(pixel, depth);
    var normal = normalize(cross(step_along(here, pixel, vec2<f32>(0.0, 1.0)), step_along(here, pixel, vec2<f32>(1.0, 0.0))));
    normal = select(normal, -normal, dot(normal, -here) < 0.0);
    let reach_mm = occlusion.shading.y;
    let clip = occlusion.projection * vec4<f32>(here, 1.0);
    let px_per_mm = occlusion.projection[1][1] * occlusion.viewport.w * 0.5 / clip.w;
    let reach_px = min(reach_mm * px_per_mm, MOST_REACH_PX);
    if (reach_px < 1.5) {
        return 0.0;
    }
    let turn = noise(pixel) * 6.2831853;
    var shut = 0.0;
    for (var index = 0; index < SAMPLES; index = index + 1) {
        let angle = f32(index) * GOLDEN_ANGLE + turn;
        let out = sqrt((f32(index) + 0.5) / f32(SAMPLES)) * reach_px;
        let at = pixel + vec2<f32>(cos(angle), sin(angle)) * out;
        let there_depth = depth_at(at);
        if (there_depth >= 1.0) {
            continue;
        }
        let towards = seen_at(at, there_depth) - here;
        let apart = length(towards);
        if (apart < 1e-4) {
            continue;
        }
        let near = 1.0 - smoothstep(reach_mm, 2.0 * reach_mm, apart);
        shut = shut + max(dot(normal, towards / apart) - BIAS, 0.0) * near;
    }
    return shut / f32(SAMPLES);
}

// How much a lens darkens the frame's corners in the render view, and from how far out of
// its middle, as a share of the half diagonal.
const VIGNETTE = 0.22;
const VIGNETTE_FROM = 0.45;

fn vignette(pixel: vec2<f32>) -> f32 {
    if (occlusion.shading.w < 0.5) {
        return 1.0;
    }
    let share = (pixel - occlusion.viewport.xy) / max(occlusion.viewport.zw, vec2<f32>(1.0));
    let out = length((share - 0.5) * vec2<f32>(occlusion.viewport.z / max(occlusion.viewport.w, 1.0), 1.0));
    let corner = length(vec2<f32>(0.5 * occlusion.viewport.z / max(occlusion.viewport.w, 1.0), 0.5));
    return 1.0 - VIGNETTE * smoothstep(VIGNETTE_FROM, 1.0, out / corner);
}

@fragment
fn present_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let colour = textureLoad(frame, vec2<i32>(position.xy), 0);
    var kept = vignette(position.xy);
    if (occlusion.shading.x > 0.5) {
        let depth = textureLoad(depth_plane, vec2<i32>(position.xy), 0);
        if (depth < 1.0) {
            kept = kept * clamp(1.0 - occlusion.shading.z * enclosed(position.xy, depth), 0.0, 1.0);
        }
    }
    // Light taken away in linear light, not off the gamma the plane holds.
    return vec4<f32>(colour.rgb * pow(kept, 1.0 / 2.2), colour.a);
}
