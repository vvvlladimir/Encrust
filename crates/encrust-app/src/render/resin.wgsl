// A resin at rest: its thickness and exit, the clear one refracted, the opaque one glowing
// where thin. Joined to shader.wgsl; see docs/design/viewport.md.

// The pass before the frame adding up the resin each ray crosses: a back face adds its
// distance from the eye, a front face takes it away. See docs/design/viewport.md.
@fragment
fn thickness_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    if (in.surface < 0.5 || sectioned_away(in.world) || cut_away(in.world)) {
        discard;
    }
    let depth = distance(globals.eye.xyz, in.world) - globals.look.w;
    return vec4<f32>(select(depth, -depth, front_facing), 0.0, 0.0, 0.0);
}

@group(1) @binding(0) var thickness_plane: texture_2d<f32>;
@group(1) @binding(1) var behind_plane: texture_2d<f32>;
@group(1) @binding(2) var behind_sampler: sampler;
@group(1) @binding(3) var exit_plane: texture_2d<f32>;

// Before the frame, each pixel's nearest back face of a clear model, where the light first
// leaves: its outward normal, and its distance from the eye less the pivot's for a half-float.
@fragment
fn exit_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (in.surface < 0.5 || sectioned_away(in.world) || cut_away(in.world)) {
        discard;
    }
    let outward = normalize(select(in.normal, in.smooth_normal, in.smoothed > 0.5));
    return vec4<f32>(outward, distance(globals.eye.xyz, in.world) - globals.look.w);
}

// A cured acrylate's index of refraction for red, green and blue: the spread is what
// fringes an edge seen through the part. See docs/design/viewport.md.
const RESIN_IOR = vec3<f32>(1.49, 1.50, 1.52);

// How a printed surface spreads light through it, as a roughness: the layers' risers and
// treads turn apart most where they share it evenly. See docs/design/viewport.md.
const WALL_LIPS = 0.3;
// Radians of spread a roughness of 1.0 turns the light by, and the least frosting any
// unpolished print has.
const SPREAD_RAD = 0.35;
const LEAST_FROST_PX = 1.5;
const MOST_FROST_PX = 40.0;
// Even the clearest resin clouds over within this many mm as it cures, and how much of
// what a rough surface scatters comes back as a white haze.
const CURED_HAZE_MM = 400.0;
const SURFACE_HAZE = 0.28;
const FROST_TAPS = 8;

fn layer_roughness(outward: vec3<f32>) -> f32 {
    let across = length(outward.xy);
    let up = abs(outward.z);
    if (globals.look.x <= 0.0 || across < 1e-3) {
        return 0.0;
    }
    let tread = up / (across + up);
    return max(WALL_LIPS * smoothstep(0.0, 0.05, across), 2.0 * sqrt(tread * (1.0 - tread)));
}

// The pixel `world` lands on.
fn pixel_of(world: vec3<f32>) -> vec2<f32> {
    let clip = globals.view_projection * vec4<f32>(world, 1.0);
    let ndc = clip.xy / clip.w;
    return globals.screen.xy + (ndc * vec2<f32>(0.5, -0.5) + 0.5) * globals.screen.zw;
}

// The way the light through a surface facing `outward` is spread: up the surface, across
// its layers, the way the steps between them turn.
fn across_layers(outward: vec3<f32>) -> vec3<f32> {
    let up = vec3<f32>(0.0, 0.0, 1.0) - outward * outward.z;
    return select(normalize(up), vec3<f32>(0.0), dot(up, up) < 1e-6);
}

// The pixel one channel's light through the part comes from: bent in at `normal`, out at
// its first exit (or back inside past the critical angle), carried the thickness each way.
fn traced(in: ModelFragment, normal: vec3<f32>, view: vec3<f32>, crossed_mm: f32, exit: vec4<f32>, ior: f32) -> vec3<f32> {
    let inside = refract(-view, normal, 1.0 / ior);
    let wall_mm = clamp(exit.w + globals.look.w - distance(globals.eye.xyz, in.world), 0.0, crossed_mm);
    if (dot(exit.xyz, exit.xyz) < 0.5) {
        return in.world + inside * crossed_mm;
    }
    let leaves = in.world + inside * wall_mm;
    let facing = -normalize(exit.xyz);
    var out = refract(inside, facing, ior);
    if (dot(out, out) < 1e-6) {
        out = reflect(inside, facing);
    }
    return leaves + out * max(crossed_mm - wall_mm, wall_mm);
}

// What stands behind, seen through the part: traced per channel, spread across the layers
// by both faces' roughness, over the backdrop wherever the copy holds nothing.
fn through_resin(in: ModelFragment, normal: vec3<f32>, view: vec3<f32>, crossed_mm: f32, rough: f32) -> vec3<f32> {
    let exit = textureLoad(exit_plane, vec2<i32>(in.clip_position.xy), 0);
    let spread = sqrt(rough * rough + layer_roughness(normalize(exit.xyz)) * layer_roughness(normalize(exit.xyz)));
    let here = in.clip_position.xy;
    let up_px = pixel_of(in.world + across_layers(normal)) - here;
    let side_px = vec2<f32>(-up_px.y, up_px.x);
    let long_px = clamp(LEAST_FROST_PX + spread * SPREAD_RAD * crossed_mm * length(up_px), LEAST_FROST_PX, MOST_FROST_PX);
    let long = select(vec2<f32>(1.0, 0.0), normalize(up_px), dot(up_px, up_px) > 1e-8);
    let short = select(vec2<f32>(0.0, 1.0), normalize(side_px), dot(side_px, side_px) > 1e-8);
    let texel = 1.0 / vec2<f32>(textureDimensions(behind_plane));
    var seen = vec3<f32>(0.0);
    for (var channel = 0; channel < 3; channel = channel + 1) {
        let at = pixel_of(traced(in, normal, view, crossed_mm, exit, RESIN_IOR[channel]));
        var gathered = 0.0;
        for (var tap = 0; tap < FROST_TAPS; tap = tap + 1) {
            let angle = f32(tap) * 2.39996323;
            let out = sqrt((f32(tap) + 0.5) / f32(FROST_TAPS));
            let offset = (long * cos(angle) * long_px + short * sin(angle) * LEAST_FROST_PX) * out;
            let behind = textureSampleLevel(behind_plane, behind_sampler, (at + offset) * texel, 0.0);
            gathered = gathered + behind[channel] + globals.backdrop[channel] * (1.0 - behind.a);
        }
        seen[channel] = gathered / f32(FROST_TAPS);
    }
    return seen;
}

// A clear resin after KHR_materials_volume: what stands behind, refracted and spread at both
// faces, tinted and clouded over the thickness, under a reflection off the stepped surface.
@fragment
fn clear_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let along = in.world.z / max(globals.look.x, 1e-6);
    let per_px = fwidth(along);
    let colour = model_colour(in, front_facing);
    let crossed_mm = max(textureLoad(thickness_plane, vec2<i32>(in.clip_position.xy), 0).r, 0.0);
    let view = normalize(globals.eye.xyz - in.world);
    var outward = normalize(select(in.normal, in.smooth_normal, in.smoothed > 0.5));
    outward = select(outward, -outward, dot(outward, view) < 0.0);
    // Layers a few pixels apart bend the light as the stair they are; finer, they frost it.
    let normal = stepped(outward, along, per_px).xyz;
    let unresolved = smoothstep(LAYER_FADE_FROM, LAYER_FADE_TO, per_px);
    let rough = layer_roughness(outward) * unresolved;
    let seen = through_resin(in, normal, view, crossed_mm, rough);
    let depth = crossed_mm / max(globals.look.z, 1e-3);
    let tint = pow(max(in.color.rgb, vec3<f32>(0.02)), vec3<f32>(depth));
    let clouded = clamp(1.0 - exp(-depth - crossed_mm / CURED_HAZE_MM) + SURFACE_HAZE * rough, 0.0, 1.0);
    let body = seen * tint * (1.0 - clouded) + colour.rgb * clouded;
    let grazing = pow(1.0 - max(dot(normal, view), 0.0), 5.0);
    let reflected = 0.04 + 0.96 * grazing;
    let shine = studio_grooved(vec3<f32>(0.0), 1.0, normal, in.world, rough);
    return vec4<f32>(body * (1.0 - reflected) + shine, 1.0);
}

// An opaque resin's model at rest, lit through where it is thin.
@fragment
fn glow_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let crossed_mm = max(textureLoad(thickness_plane, vec2<i32>(in.clip_position.xy), 0).r, 0.0);
    // No thickness is no closed part to be thin, as an open mesh measures.
    glow_through = select(0.0, exp(-crossed_mm / GLOW_MM), crossed_mm > 0.01);
    return model_colour(in, front_facing);
}

// What a clear model's face lays into the scene before refraction, so inner walls read
// through: its studio reflection, brightest grazing, over a trace of the resin's colour.
const INNER_SHINE = 0.55;
const INNER_BODY = 0.06;

@fragment
fn inner_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let body = model_colour(in, front_facing);
    let view = normalize(globals.eye.xyz - in.world);
    var normal = normalize(select(in.normal, in.smooth_normal, in.smoothed > 0.5));
    normal = select(normal, -normal, dot(normal, view) < 0.0);
    let grazing = pow(1.0 - dot(normal, view), 3.0);
    let shine = studio(vec3<f32>(0.0), 1.0, normal, in.world) * INNER_SHINE;
    let cover = INNER_BODY + 0.25 * grazing;
    return vec4<f32>(shine * cover + body.rgb * INNER_BODY, cover);
}
