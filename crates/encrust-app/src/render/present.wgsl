// Copies the viewport's own colour plane, window-sized, into egui's pass texel for texel,
// shading creases from the depth plane on the way at rest. See docs/design/viewport.md.

@group(0) @binding(0) var frame: texture_2d<f32>;
// Plain floats, not a depth texture: GLSL cannot load a texel of one.
@group(0) @binding(1) var depth_plane: texture_2d<f32>;

struct Occlusion {
    inverse_projection: mat4x4<f32>,
    projection: mat4x4<f32>,
    // Left, top, width and height of the viewport, framebuffer pixels.
    viewport: vec4<f32>,
    // x 1.0 while creases are shaded, y the reach in mm, z the strength.
    shading: vec4<f32>,
    // x the share of a turn the slices are rotated by in this frame.
    turn: vec4<f32>,
};

@group(0) @binding(2) var<uniform> occlusion: Occlusion;

// Ground-truth ambient occlusion (Jimenez et al. 2016): horizons both ways along a few slices
// round each pixel, integrated over the arc the normal sees. See docs/design/viewport.md.
const SLICES = 3;
const STEPS = 4;
const PI_HALF = 1.5707963;
// The most pixels a crease reaches across, so a close zoom costs no more than a far one.
const MOST_REACH_PX = 48.0;
// The share of the reach over which a sample's say fades out, as the paper's falloff.
const FALLOFF_SHARE = 0.615;

// One triangle covering the whole viewport; egui's viewport and scissor trim it.
@vertex
fn present_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn depth_at(pixel: vec2<f32>) -> f32 {
    let low = occlusion.viewport.xy;
    let high = low + occlusion.viewport.zw - vec2<f32>(1.0);
    return textureLoad(depth_plane, vec2<i32>(clamp(pixel, low, high)), 0).r;
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
    let view = normalize(-here);
    normal = select(normal, -normal, dot(normal, view) < 0.0);
    let reach_mm = occlusion.shading.y;
    let clip = occlusion.projection * vec4<f32>(here, 1.0);
    let px_per_mm = occlusion.projection[1][1] * occlusion.viewport.w * 0.5 / clip.w;
    let reach_px = min(reach_mm * px_per_mm, MOST_REACH_PX);
    if (reach_px < 1.5) {
        return 0.0;
    }
    let falloff_mm = FALLOFF_SHARE * reach_mm;
    let falloff_mul = -1.0 / falloff_mm;
    let falloff_add = (reach_mm - falloff_mm) / falloff_mm + 1.0;
    let turn = fract(noise(pixel) + occlusion.turn.x);
    let jitter = fract(turn * 7.31);
    var visible = 0.0;
    for (var slice = 0; slice < SLICES; slice = slice + 1) {
        let angle = (f32(slice) + turn) * 3.14159265 / f32(SLICES);
        // Up the screen is up the view; the screen's rows run down.
        let direction = vec3<f32>(cos(angle), sin(angle), 0.0);
        let on_screen = vec2<f32>(direction.x, -direction.y);
        let ortho = direction - dot(direction, view) * view;
        let axis = normalize(cross(ortho, view));
        let projected = normal - axis * dot(normal, axis);
        let projected_length = length(projected);
        let cos_n = clamp(dot(projected, view) / max(projected_length, 1e-4), -1.0, 1.0);
        let n = sign(dot(ortho, projected)) * acos(cos_n);
        let low0 = cos(n + PI_HALF);
        let low1 = cos(n - PI_HALF);
        var horizon0 = low0;
        var horizon1 = low1;
        for (var step = 0; step < STEPS; step = step + 1) {
            var share = (f32(step) + jitter) / f32(STEPS);
            share = share * share;
            let offset = on_screen * max(share * reach_px, 1.0);
            let ahead = seen_at(pixel + offset, depth_at(pixel + offset)) - here;
            let behind = seen_at(pixel - offset, depth_at(pixel - offset)) - here;
            let far0 = length(ahead);
            let far1 = length(behind);
            let weight0 = clamp(far0 * falloff_mul + falloff_add, 0.0, 1.0);
            let weight1 = clamp(far1 * falloff_mul + falloff_add, 0.0, 1.0);
            horizon0 = max(horizon0, mix(low0, dot(ahead / max(far0, 1e-4), view), weight0));
            horizon1 = max(horizon1, mix(low1, dot(behind / max(far1, 1e-4), view), weight1));
        }
        var h0 = -acos(clamp(horizon1, -1.0, 1.0));
        var h1 = acos(clamp(horizon0, -1.0, 1.0));
        h0 = n + clamp(h0 - n, -PI_HALF, PI_HALF);
        h1 = n + clamp(h1 - n, -PI_HALF, PI_HALF);
        let arc0 = (cos_n + 2.0 * h0 * sin(n) - cos(2.0 * h0 - n)) / 4.0;
        let arc1 = (cos_n + 2.0 * h1 * sin(n) - cos(2.0 * h1 - n)) / 4.0;
        visible = visible + projected_length * (arc0 + arc1);
    }
    return 1.0 - clamp(visible / f32(SLICES), 0.0, 1.0);
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
        let depth = textureLoad(depth_plane, vec2<i32>(position.xy), 0).r;
        if (depth < 1.0) {
            kept = kept * clamp(1.0 - occlusion.shading.z * enclosed(position.xy, depth), 0.0, 1.0);
        }
    }
    // Light taken away in linear light, not off the gamma the plane holds.
    return vec4<f32>(colour.rgb * pow(kept, 1.0 / 2.2), colour.a);
}

// The render view at rest: the frames averaged into the accumulated plane, shown as they are.
@group(1) @binding(0) var accumulated: texture_2d<f32>;

@fragment
fn accumulated_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(accumulated, vec2<i32>(position.xy), 0);
}
