// Viewport shading: one lit pass for models, one unlit pass for the plate and the grid,
// and the contact shadow under the models.
// The world is Z-up in plate millimetres, matching core-geometry.

// How many drain cuts the globals carry room for. Must match MAX_CUTS in gpu.rs.
const MAX_CUTS = 64u;

// How many exposure bands the globals carry room for. Must match MAX_BANDS in gpu.rs.
const MAX_BANDS = 8u;

// How many pockets of trapped resin the globals carry room for. Must match MAX_POCKETS
// in gpu.rs.
const MAX_POCKETS = 32u;


// One drain hole or channel segment in plate millimetres: xyz is an end, w its radius.
// corner.xyz points at a polygon's first corner and corner.w counts its sides, 0 if round.
// start and end are the planes a channel's leg is mitred off at, w 1.0 where it is.
struct DrainCut {
    mouth: vec4<f32>,
    tip: vec4<f32>,
    corner: vec4<f32>,
    start: vec4<f32>,
    end: vec4<f32>,
};

// One band of print height that takes an exposure of its own: span.xy is the band in
// plate millimetres, tint.rgb what it washes the surface with.
struct ExposureBand {
    span: vec4<f32>,
    tint: vec4<f32>,
};

// The box one pocket of trapped resin stands in, plate millimetres. See
// docs/decisions/0200.
struct TrapBox {
    low: vec4<f32>,
    high: vec4<f32>,
};

struct Globals {
    view_projection: mat4x4<f32>,
    // Its inverse, which the render view's backdrop turns a pixel into a line of sight by.
    inverse_view_projection: mat4x4<f32>,
    // The key light's view over the render view's set, which its shadow map is drawn in.
    key_view_projection: mat4x4<f32>,
    // The two lights: xyz the direction each travels towards, and their colours. The sky
    // and the ground are what the ambient is mixed from. All of it comes from
    // ui::theme, see docs/design/viewport.md; every w is unused padding.
    key_towards: vec4<f32>,
    fill_towards: vec4<f32>,
    key_color: vec4<f32>,
    fill_color: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // The plate the contact shadow is laid on, plate millimetres: xy its near corner and
    // zw its size, or a zero size while there is none. shadow_color.a is how dark it goes.
    shadow_area: vec4<f32>,
    shadow_color: vec4<f32>,
    // The colour the plate's surface is filled with, over shadow_area, and the two colours
    // of the grid drawn into it. grid.x is the minor spacing and grid.y the major one,
    // millimetres; grid.z is 1.0 while the grid is shown.
    plate_color: vec4<f32>,
    grid_minor: vec4<f32>,
    grid_major: vec4<f32>,
    grid: vec4<f32>,
    // xyz is where the camera stands, plate millimetres; w is how much of itself a surface
    // keeps at most while the models are drawn seen through, and 0.0 while they are not.
    // See docs/design/viewport.md.
    eye: vec4<f32>,
    // x is the height the models are cut at, plate millimetres; y is 1.0 while there is a
    // cut at all. See docs/decisions/0061.
    section: vec4<f32>,
    // x is how many of `cuts` carry a drain this frame, y how many of `bands` carry an
    // exposure, z the height below which a band has no effect, and w how many of
    // `pockets` hold trapped resin.
    counts: vec4<f32>,
    // rgb of what a surface needing support and a model's exposed inside are washed with;
    // w is unused padding. Both come from ui::theme, see docs/design/ui-design-system.md.
    overhang_color: vec4<f32>,
    inside_color: vec4<f32>,
    // xyz is the high corner of where a model stands unmarked, in plate millimetres, w 1.0
    // while a model past it is to be marked; outside_color.rgb is what marks it.
    volume: vec4<f32>,
    // xyz is that box's low corner: the origin less the plate's inset; w is unused padding.
    volume_low: vec4<f32>,
    outside_color: vec4<f32>,
    // The Cut tool's plane: xyz its normal, w its offset along it in millimetres. It is
    // traced only inside cut_low..cut_high, and only while cut_low.w is 1.0.
    cut_plane: vec4<f32>,
    cut_low: vec4<f32>,
    cut_high: vec4<f32>,
    cut_color: vec4<f32>,
    // The render view, all 0.0 while it is off: x the layer height stepped into the models,
    // y 1.0 under studio light, z the resin's attenuation distance, w the eye's pivot distance.
    look: vec4<f32>,
    // The viewport in framebuffer pixels: xy its corner, zw its size.
    screen: vec4<f32>,
    // What shows wherever the scene drew nothing: the viewport's own backdrop.
    backdrop: vec4<f32>,
    // The render view's mat: desk its corners in plate millimetres, xy low and zw high,
    // and mat.xy its own size across and along.
    desk: vec4<f32>,
    mat: vec4<f32>,
    cuts: array<DrainCut, MAX_CUTS>,
    bands: array<ExposureBand, MAX_BANDS>,
    pockets: array<TrapBox, MAX_POCKETS>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var key_shadow_map: texture_depth_2d;
@group(0) @binding(2) var key_shadow_sampler: sampler_comparison;

struct ModelVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) normal_0: vec3<f32>,
    @location(7) normal_1: vec3<f32>,
    @location(8) normal_2: vec3<f32>,
    @location(9) color: vec4<f32>,
    // Sine of the lean a surface may have before it needs holding up, or a negative
    // number on a mesh that is not the model: supports do not mark themselves.
    @location(10) overhang: f32,
    // 1.0 for a surface of a model, 0.0 for a volume painted in its own colour. 11 and 12
    // belong to the relief pass.
    @location(13) surface: f32,
    // Share of the way to the eye the depth is taken from, so that of two coplanar faces
    // of different meshes the same one wins every frame.
    @location(14) pull: f32,
    // 1.0 where every corner carries a normal smoothed over its faces, which it is shaded by.
    @location(15) smoothed: f32,
};

struct ModelFragment {
    @builtin(position) clip_position: vec4<f32>,
    // The face's own, carried by its first corner alone: corners are shared between faces
    // and only the first is read on a flat input (docs/design/viewport.md).
    @location(0) @interpolate(flat) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) overhang: f32,
    // Where the fragment stands in plate millimetres: its height is what the section cut
    // is compared against, and the whole point is what a drain cut is.
    @location(3) world: vec3<f32>,
    @location(4) surface: f32,
    // The corners' normals blended across the face, read where `smoothed` is set.
    @location(5) smooth_normal: vec3<f32>,
    @location(6) @interpolate(flat) smoothed: f32,
    // Where the fragment stands in its own mesh's millimetres, which a prop is painted by.
    @location(7) local: vec3<f32>,
};

// How much lean it takes to go from unmarked to fully marked. A band rather than a step,
// so a curve shades into it instead of breaking along a ring of triangles.
const OVERHANG_BAND = 0.06;

// How much of the inside colour shows through the openings the section cap leaves
// (docs/decisions/0062) and through a drain hole, which would otherwise read as a flat
// disc of the same colour as the surface it is drilled in.
const SECTION_WASH = 0.35;

// How much of its tint a band of exposure washes over the surface: enough to read the
// height it covers, not so much that the shape under it stops reading.
const BAND_WASH = 0.45;

// Millimetres a surface may stand past the build volume before it is marked, so a foot
// lying on the plate is not painted for the rounding of its underside.
const VOLUME_SLACK = 0.01;

fn outside_volume(world: vec3<f32>) -> bool {
    if (globals.volume.w < 0.5) {
        return false;
    }
    let low = world < globals.volume_low.xyz - vec3<f32>(VOLUME_SLACK);
    let high = world > globals.volume.xyz + vec3<f32>(VOLUME_SLACK);
    return any(low) || any(high);
}

// How the light is shared out, in the gamma space the target is written in. The ambient
// has a floor so a face turned from both lights still reads as resin rather than as a
// hole, and the highlight is small and broad, as on a matt resin with no metal in it.
const AMBIENT_FLOOR = 0.26;
const AMBIENT_SKY = 0.20;
const KEY_SHARE = 0.62;
const FILL_SHARE = 0.18;
const SPECULAR_SHARE = 0.08;
const SHININESS = 24.0;

// A surface of premultiplied colour `base` and coverage `alpha` facing `normal` at
// `world`, under the sky, the key and the fill, with a Blinn-Phong highlight of the key.
fn lit(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let up = normal.z * 0.5 + 0.5;
    let ambient = AMBIENT_FLOOR + AMBIENT_SKY * mix(globals.ground.rgb, globals.sky.rgb, up);
    let key = normalize(-globals.key_towards.xyz);
    let fill = normalize(-globals.fill_towards.xyz);
    let facing_key = max(dot(normal, key), 0.0);
    let diffuse = globals.key_color.rgb * KEY_SHARE * facing_key
        + globals.fill_color.rgb * FILL_SHARE * max(dot(normal, fill), 0.0);
    let half_way = normalize(key + normalize(globals.eye.xyz - world));
    let shine = pow(max(dot(normal, half_way), 0.0), SHININESS) * step(1e-4, facing_key);
    return base * (ambient + diffuse) + globals.key_color.rgb * SPECULAR_SHARE * shine * alpha;
}

// The render view's studio: a sky-to-floor gradient, a large key softbox and one overhead,
// both with soft edges, and the room seen at a grazing angle. See docs/design/viewport.md.
const STUDIO_AMBIENT = 0.30;
const STUDIO_KEY = 0.55;
const STUDIO_TOP = 0.22;
const STUDIO_WRAP = 0.35;
const SOFTBOX_SHARE = 0.22;
const FRESNEL_SHARE = 0.28;
// Cosines of the angle off a softbox's centre where its reflection starts and is full:
// wide enough that a flat-shaded face shades into its neighbour rather than stepping.
const SOFTBOX_EDGE = 0.35;
const SOFTBOX_CORE = 0.98;

fn wrapped(facing: f32) -> f32 {
    return max((facing + STUDIO_WRAP) / (1.0 + STUDIO_WRAP), 0.0);
}

fn softbox(mirror: vec3<f32>, towards: vec3<f32>) -> f32 {
    let lobe = smoothstep(SOFTBOX_EDGE, SOFTBOX_CORE, dot(mirror, towards));
    return lobe * lobe;
}

// The render view's key light shadow. A point is lifted off its surface this far before it
// is looked up, and the map is read over a disc this many texels across, which softens the
// edge as a softbox does.
const KEY_LIFT_MM = 0.4;
const KEY_TURNED_LIFT_MM = 2.0;
const KEY_SOFT_TEXELS = 2.5;
const KEY_TAPS = 12;

// How much of the key light reaches `world` on a surface facing `normal`: all of it outside
// the render view or off the map.
fn key_reaches(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if (globals.look.y < 0.5) {
        return 1.0;
    }
    // Lifted further the more it turns from the key, where a curve's own map is coarsest.
    let turned = 1.0 - clamp(dot(normal, normalize(-globals.key_towards.xyz)), 0.0, 1.0);
    let lift = KEY_LIFT_MM + KEY_TURNED_LIFT_MM * turned;
    let clip = globals.key_view_projection * vec4<f32>(world + normal * lift, 1.0);
    let at = clip.xyz / clip.w;
    let uv = at.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || at.z > 1.0) {
        return 1.0;
    }
    let texel = KEY_SOFT_TEXELS / vec2<f32>(textureDimensions(key_shadow_map));
    var lit = 0.0;
    for (var tap = 0; tap < KEY_TAPS; tap = tap + 1) {
        let angle = f32(tap) * 2.39996323;
        let out = sqrt((f32(tap) + 0.5) / f32(KEY_TAPS));
        let shifted = uv + vec2<f32>(cos(angle), sin(angle)) * out * texel;
        lit = lit + textureSampleCompareLevel(key_shadow_map, key_shadow_sampler, shifted, at.z);
    }
    return lit / f32(KEY_TAPS);
}

// A mesh as the key light sees it, for its shadow map.
@vertex
fn key_cast_vertex(in: ModelVertex) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    return globals.key_view_projection * model * vec4<f32>(in.position, 1.0);
}

// The render view lights in linear light and hands the sum to a film's curve, AgX as
// Blender and three.js draw it, before it goes out in the gamma the target is written in.
// See docs/design/viewport.md.
const EXPOSURE = 1.0;
// A milder Punchy look than Blender's over AgX, its power alone: a resin's colour holds its
// strength instead of AgX's own muted base, its hue untouched.
const LOOK_POWER = 1.15;

fn linear_of(gamma: vec3<f32>) -> vec3<f32> {
    return pow(max(gamma, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// AgX's sigmoid as the polynomial three.js and Godot fit to it, over log2 exposure.
fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

// Linear sRGB light to the gamma the target takes, through AgX.
fn filmic(linear: vec3<f32>) -> vec3<f32> {
    let srgb_to_rec2020 = mat3x3<f32>(
        vec3<f32>(0.6274, 0.0691, 0.0164),
        vec3<f32>(0.3293, 0.9195, 0.0880),
        vec3<f32>(0.0433, 0.0113, 0.8956),
    );
    let rec2020_to_srgb = mat3x3<f32>(
        vec3<f32>(1.6605, -0.1246, -0.0182),
        vec3<f32>(-0.5876, 1.1329, -0.1006),
        vec3<f32>(-0.0728, -0.0083, 1.1187),
    );
    let inset = mat3x3<f32>(
        vec3<f32>(0.856627153315983, 0.137318972929847, 0.11189821299995),
        vec3<f32>(0.0951212405381588, 0.761241990602591, 0.0767994186031903),
        vec3<f32>(0.0482516061458583, 0.101439036467562, 0.811302368396859),
    );
    let outset = mat3x3<f32>(
        vec3<f32>(1.1271005818144368, -0.1413297634984383, -0.14132976349843826),
        vec3<f32>(-0.11060664309660323, 1.157823702216272, -0.11060664309660294),
        vec3<f32>(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405),
    );
    let low_ev = -12.47393;
    let high_ev = 4.026069;
    var colour = inset * (srgb_to_rec2020 * (linear * EXPOSURE));
    colour = clamp((log2(max(colour, vec3<f32>(1e-10))) - low_ev) / (high_ev - low_ev), vec3<f32>(0.0), vec3<f32>(1.0));
    colour = outset * pow(max(agx_contrast(colour), vec3<f32>(0.0)), vec3<f32>(LOOK_POWER));
    colour = rec2020_to_srgb * pow(max(colour, vec3<f32>(0.0)), vec3<f32>(2.2));
    return pow(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

// `lit` under the studio, for the same premultiplied `base` and `alpha`.
fn studio(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    return finish(base, 0.0, alpha, normal, world);
}

// The same over a printed surface whose layers, too fine to see, groove it by `grooves`:
// its reflections stretch across them, a satin sheen, as KHR_materials_anisotropy bends the
// normal it reflects about towards the direction of the stretch.
fn studio_grooved(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>, grooves: f32) -> vec3<f32> {
    let view = normalize(globals.eye.xyz - world);
    let bent = bent_by_layers(normal, view, grooves);
    let sheen = layer_sheen(normal, view, world, grooves) * alpha;
    return filmic(radiance(linear_of(base), 0.0, alpha, normal, bent, world) + sheen);
}

// The key's highlight off the grooves, Ward's anisotropic lobe: narrow along the layers,
// stretched across them by how deep they groove. Its share of the key, and its widths.
const SHEEN_SHARE = 0.35;
const SHEEN_ALONG = 0.12;
const SHEEN_ACROSS = 0.55;

fn layer_sheen(normal: vec3<f32>, view: vec3<f32>, world: vec3<f32>, grooves: f32) -> vec3<f32> {
    let across = across_layers(normal);
    let key = normalize(-globals.key_towards.xyz);
    let lit = dot(normal, key);
    if (grooves <= 0.0 || dot(across, across) < 0.5 || lit <= 0.0) {
        return vec3<f32>(0.0);
    }
    let along = cross(across, normal);
    let half_way = normalize(key + view);
    let width_across = SHEEN_ALONG + SHEEN_ACROSS * grooves;
    let up = max(dot(half_way, normal), 1e-3);
    let spread = pow(dot(half_way, along) / SHEEN_ALONG, 2.0) + pow(dot(half_way, across) / width_across, 2.0);
    let lobe = exp(-spread / (up * up)) / (4.0 * 3.14159265 * SHEEN_ALONG * width_across * sqrt(lit * max(dot(normal, view), 1e-3)));
    let reaches = key_reaches(world, normal);
    return linear_of(globals.key_color.rgb) * SHEEN_SHARE * grooves * lobe * lit * reaches;
}

// How far the layers' grooves bend the normal a reflection is taken about, at their most.
const SHEEN_BEND = 0.65;

// KHR_materials_anisotropy's bent normal: towards the one facing the eye in the plane the
// grooves stretch the light in, up the surface across the layers.
fn bent_by_layers(normal: vec3<f32>, view: vec3<f32>, grooves: f32) -> vec3<f32> {
    let stretch = across_layers(normal);
    if (grooves <= 0.0 || dot(stretch, stretch) < 0.5) {
        return normal;
    }
    let along = cross(stretch, view);
    let facing = cross(along, stretch);
    return normalize(mix(normal, facing, grooves * SHEEN_BEND));
}

// A surface under the studio: `metal` 0.0 for a dielectric, whose reflection is white, to
// 1.0 for a metal, which has no diffuse and reflects in its own colour; `shine` scales both.
fn finish(base: vec3<f32>, metal: f32, shine: f32, normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    return filmic(radiance(linear_of(base), metal, shine, normal, normal, world));
}

// The light leaving a surface of linear colour `base` under the studio, before the curve:
// lit about `normal`, reflecting about `reflecting`, which grooves may bend away from it.
fn radiance(base: vec3<f32>, metal: f32, shine: f32, normal: vec3<f32>, reflecting: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let view = normalize(globals.eye.xyz - world);
    let key = normalize(-globals.key_towards.xyz);
    let fill = normalize(-globals.fill_towards.xyz);
    let top = vec3<f32>(0.0, 0.0, 1.0);
    let sky = linear_of(globals.sky.rgb);
    let ground = linear_of(globals.ground.rgb);
    let key_colour = linear_of(globals.key_color.rgb);
    let room = mix(ground, sky, smoothstep(-0.6, 1.0, normal.z));
    let reaches = key_reaches(world, normal);
    let diffuse = room * STUDIO_AMBIENT
        + key_colour * STUDIO_KEY * wrapped(dot(normal, key)) * reaches
        + sky * STUDIO_TOP * wrapped(normal.z)
        + linear_of(globals.fill_color.rgb) * FILL_SHARE * max(dot(normal, fill), 0.0);
    let mirror = reflect(-view, reflecting);
    let boxes = key_colour * softbox(mirror, key) * reaches + sky * 0.6 * softbox(mirror, top);
    let grazing = pow(1.0 - max(dot(reflecting, view), 0.0), 5.0);
    let tint = mix(vec3<f32>(1.0), base, metal);
    let fresnel = mix(vec3<f32>(0.04), base, metal) + (1.0 - mix(vec3<f32>(0.04), base, metal)) * grazing;
    let seen = mix(ground, sky, mirror.z * 0.5 + 0.5);
    let reflected = boxes * SOFTBOX_SHARE * tint + seen * fresnel * mix(FRESNEL_SHARE, 1.0, metal);
    return base * (1.0 - metal) * diffuse + reflected * shine;
}

// Whichever light the frame is under: the working view's or the render view's studio.
fn shade(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>, grooves: f32) -> vec3<f32> {
    if (globals.look.y > 0.5) {
        return studio_grooved(base, alpha, normal, world, grooves);
    }
    return lit(base, alpha, normal, world);
}

// Millimetres between one stripe of a hatch and the next, along x + y - z, so the stripes
// run diagonally whichever way the surface faces.
const HATCH_PITCH_MM = 3.4;

// How much of a stripe covers a fragment, smoothed over a pixel so the hatch does not
// shimmer, and evened out to half where the stripes are closer together than pixels. The
// derivative is taken before anything can discard, since WGSL asks for it in uniform
// control flow.
fn hatch(world: vec3<f32>) -> f32 {
    let along = (world.x + world.y - world.z) / HATCH_PITCH_MM;
    let pixel = max(fwidth(along), 1e-4);
    var off_centre = fract(along) - 0.75;
    off_centre = abs(off_centre - round(off_centre));
    let cover = clamp((0.25 - off_centre) / pixel + 0.5, 0.0, 1.0);
    return mix(cover, 0.5, smoothstep(0.25, 0.5, pixel));
}

// How much of the overhang colour a marked surface takes: filled, not hatched, so the edge
// of what needs holding up is a line and not a row of stripe ends. See docs/decisions/0245.
const OVERHANG_FILL = 0.85;

// How much of its colour whatever stands past the build volume takes off a stripe and on
// one. Not nothing between the stripes: a part narrower than the pitch would vanish.
const OUTSIDE_GAP = 0.3;
const OUTSIDE_STRIPE = 0.9;

@vertex
fn model_vertex(in: ModelVertex) -> ModelFragment {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    let normal_matrix = mat3x3<f32>(in.normal_0, in.normal_1, in.normal_2);

    let world = model * vec4<f32>(in.position, 1.0);

    var out: ModelFragment;
    let seen = mix(world.xyz, globals.eye.xyz, in.pull);
    out.clip_position = globals.view_projection * vec4<f32>(seen, 1.0);
    out.normal = normal_matrix * in.normal;
    out.smooth_normal = out.normal;
    out.smoothed = in.smoothed;
    out.color = in.color;
    out.overhang = in.overhang;
    out.world = world.xyz;
    out.surface = in.surface;
    out.local = in.position;
    return out;
}

// A drain hole is a body appended wound inward, so the mesh it cuts still carries every
// triangle it had: the subtraction happens in the fill rule, and on screen it has to
// happen here. A fragment inside a hole's own tube is not there. See docs/decisions/0073.
//
// The skin narrows the test, so that the wall of the bore — a prism inscribed in the
// circle, standing just inside the radius — survives it and is what shows inside a hole.
const CUT_SKIN = 0.03;

// How far short of a closed far end the test stops, as a share of the cut's length: the
// floor of a blind hole sits exactly there, and is drawn rather than cut away.
const CUT_FLOOR = 0.002;

// How far `offset` stands from a cut's axis, measured so that the cut's outline reads as its
// corner radius: plain distance for a circle, scaled up off a polygon's corners to its sides.
fn reach(offset: vec3<f32>, axis: vec3<f32>, corner: vec4<f32>) -> f32 {
    if (corner.w < 3.0) {
        return length(offset);
    }
    let unit = normalize(axis);
    let lengthwise = dot(offset, unit);
    let across = offset - unit * lengthwise;
    let sector = 6.2831853 / corner.w;
    let angle = atan2(dot(across, cross(unit, corner.xyz)), dot(across, corner.xyz));
    let from_side = angle - sector * floor(angle / sector) - 0.5 * sector;
    let outline = length(across) * cos(from_side) / cos(0.5 * sector);
    return sqrt(outline * outline + lengthwise * lengthwise);
}

fn cut_away(world: vec3<f32>) -> bool {
    let count = i32(globals.counts.x);
    for (var index = 0; index < count; index = index + 1) {
        let cut = globals.cuts[index];
        let axis = cut.tip.xyz - cut.mouth.xyz;
        let along = dot(world - cut.mouth.xyz, axis) / max(dot(axis, axis), 1e-9);
        // A negative radius at the far end marks a cut that opens there, a channel's
        // mouth against a hole's floor: the floor is geometry and has to survive.
        if (cut.tip.w >= 0.0 && along > 1.0 - CUT_FLOOR) {
            continue;
        }
        // A leg of a channel ends at the plane it shares with the next, where that one
        // takes over, so the outline runs on unbroken through the bend.
        let past_start = dot(world - cut.mouth.xyz, cut.start.xyz) < 0.0;
        let past_end = dot(world - cut.tip.xyz, cut.end.xyz) > 0.0;
        if ((cut.start.w > 0.5 && past_start) || (cut.end.w > 0.5 && past_end)) {
            continue;
        }
        // Rounded at the ends that open, so that a surface curving away under a mouth is
        // taken with it rather than left capping the bore; straight on to a mitre.
        let held = clamp(along, 0.0, 1.0);
        let radius = mix(cut.mouth.w, abs(cut.tip.w), held);
        var offset = world - (cut.mouth.xyz + axis * held);
        if ((along < 0.0 && cut.start.w > 0.5) || (along > 1.0 && cut.end.w > 0.5)) {
            let unit = normalize(axis);
            offset = offset - unit * dot(offset, unit);
        }
        if (reach(offset, axis, cut.corner) < radius * (1.0 - CUT_SKIN)) {
            return true;
        }
    }
    return false;
}

// Whether a point stands in a pocket the last drainage check found. The cavity is one
// mesh however many pockets it holds, so this is what keeps the red to the ones that are
// still trapped: drill into one and it goes out, the others stay. See
// docs/decisions/0200.
fn in_a_pocket(world: vec3<f32>) -> bool {
    let count = i32(globals.counts.w);
    for (var index = 0; index < count; index = index + 1) {
        let pocket = globals.pockets[index];
        if (all(world >= pocket.low.xyz) && all(world <= pocket.high.xyz)) {
            return true;
        }
    }
    return false;
}

// Half the width of the line the Cut tool's plane is traced with, in pixels, and how far
// past the model's box a fragment may stand and still be traced, in millimetres.
const CUT_LINE_HALF_PX = 1.5;
const CUT_LINE_SLACK = 0.5;

// How much of the line covers a fragment. The derivative is taken before anything can
// discard, since WGSL asks for it in uniform control flow.
fn cut_line(world: vec3<f32>) -> f32 {
    let along = dot(globals.cut_plane.xyz, world) - globals.cut_plane.w;
    let pixels = abs(along) / max(fwidth(along), 1e-6);
    let low = world < globals.cut_low.xyz - vec3<f32>(CUT_LINE_SLACK);
    let high = world > globals.cut_high.xyz + vec3<f32>(CUT_LINE_SLACK);
    if (globals.cut_low.w < 0.5 || any(low) || any(high)) {
        return 0.0;
    }
    return 1.0 - smoothstep(CUT_LINE_HALF_PX - 0.5, CUT_LINE_HALF_PX + 0.5, pixels);
}

// The band a fragment stands in, washed over its colour. Bands are walked backwards so
// that the last one drawn wins, as it does when the file is written. See ADR 0090.
fn banded(base: vec3<f32>, height: f32) -> vec3<f32> {
    if (height <= globals.counts.z) {
        return base;
    }
    for (var index = i32(globals.counts.y) - 1; index >= 0; index = index - 1) {
        let band = globals.bands[index];
        if (height >= band.span.x && height < band.span.y) {
            return mix(base, band.tint.rgb, BAND_WASH);
        }
    }
    return base;
}

// The pass that counts how often a view ray crosses a surface before it reaches the
// cutting plane. A drain hole is wound inward and so counts the other way round on its
// own, which is why nothing is subtracted here. See docs/decisions/0074.
@fragment
fn section_crossing_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (in.world.z <= globals.section.x) {
        discard;
    }
    return vec4<f32>(0.0);
}

// The same count for the bottom cut, over what stands under it: seen from below, those are
// the crossings in front of the plane, and so the cap is filled by the same rule.
@fragment
fn bottom_crossing_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (in.world.z >= globals.section.z) {
        discard;
    }
    return vec4<f32>(0.0);
}

// Whether the section cut took a point away: above the top cut, or under the bottom one.
fn sectioned_away(world: vec3<f32>) -> bool {
    return (globals.section.y > 0.5 && world.z > globals.section.x)
        || (globals.section.w > 0.5 && world.z < globals.section.z);
}

// Whether a fragment of a cut's own body is there to be drawn at all: what the section cut
// took away is gone, and so is what another cut took out of this one, or two holes drilled
// side by side would each lay down a depth the other's test then refuses to paint — a black
// band between them. See docs/decisions/0201.
fn cut_surface_gone(world: vec3<f32>) -> bool {
    return sectioned_away(world) || cut_away(world);
}

// The pass that lays the far wall of a cut body into the depth plane, painting nothing.
@fragment
fn cut_candidate_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (cut_surface_gone(in.world)) {
        discard;
    }
    return vec4<f32>(0.0);
}

// The pass that counts the material standing in front of that wall. It subtracts no cut,
// nor the section: whether the wall stands in material is a question about the whole solid.
@fragment
fn cut_crossing_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

struct WipedFragment {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
};

// The pass that takes the cut's own depth back out where no material stood in front of it:
// the far wall of a tube hanging in the air would otherwise hide the plate behind it.
@fragment
fn cut_wipe_fragment(in: ModelFragment) -> WipedFragment {
    if (cut_surface_gone(in.world)) {
        discard;
    }
    var out: WipedFragment;
    out.depth = 1.0;
    out.color = vec4<f32>(0.0);
    return out;
}

// How dark the lip of a printed layer reads, and over how many pixels its line is drawn.
const LAYER_LIP = 0.22;
const LAYER_LIP_PX = 1.0;
// Layers a pixel where the stair starts fading to the smooth surface, and where it is gone:
// a stair under about four pixels a layer only beats against the pixel grid.
const LAYER_FADE_FROM = 0.12;
const LAYER_FADE_TO = 0.3;

// The surface as printed, each layer a riser and a tread shared by its steepness: xyz the normal
// to shade by, w the light its lip keeps. See docs/design/viewport.md.
fn stepped(outward: vec3<f32>, along: f32, per_px: f32) -> vec4<f32> {
    let across = length(outward.xy);
    if (globals.look.x <= 0.0 || across < 1e-4) {
        return vec4<f32>(outward, 1.0);
    }
    let riser = vec3<f32>(outward.xy / across, 0.0);
    let tread = vec3<f32>(0.0, 0.0, sign(outward.z));
    let tread_share = abs(outward.z) / (across + abs(outward.z));
    let soft = max(per_px, 1e-4) * 0.5;
    let in_layer = fract(along);
    var on_tread = smoothstep(1.0 - tread_share - soft, 1.0 - tread_share + soft, in_layer);
    if (outward.z < 0.0) {
        on_tread = 1.0 - smoothstep(tread_share - soft, tread_share + soft, in_layer);
    }
    let fine = smoothstep(LAYER_FADE_FROM, LAYER_FADE_TO, per_px);
    let normal = normalize(mix(normalize(mix(riser, tread, on_tread)), outward, fine));
    let lip_px = min(in_layer, 1.0 - in_layer) / max(per_px, 1e-4);
    let lip = 1.0 - smoothstep(LAYER_LIP_PX - 0.5, LAYER_LIP_PX + 0.5, lip_px);
    return vec4<f32>(normal, 1.0 - LAYER_LIP * lip * (1.0 - fine));
}

@fragment
fn model_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    return model_colour(in, front_facing);
}

// What a model's fragment paints, premultiplied, in the flat pass and the clear one alike.
fn model_colour(in: ModelFragment, front_facing: bool) -> vec4<f32> {
    let traced = cut_line(in.world);
    let stripe = hatch(in.world);
    let along = in.world.z / max(globals.look.x, 1e-6);
    let per_px = fwidth(along);
    if (sectioned_away(in.world)) {
        discard;
    }
    if (cut_away(in.world)) {
        discard;
    }
    // A volume is the space a pocket of resin fills, drawn out of the whole cavity's
    // faces: only the part of them standing in a pocket is that space.
    if (in.surface < 0.5 && !in_a_pocket(in.world)) {
        discard;
    }

    // Back faces are shaded as if they faced the camera. An imported mesh may still have
    // inverted faces, and a black hole reads as a rendering bug rather than as a defect.
    let outward = normalize(select(in.normal, in.smooth_normal, in.smoothed > 0.5));
    let layered = stepped(outward, along, per_px);
    var normal = layered.xyz;
    if (!front_facing) {
        normal = -normal;
    }

    // How far the surface leans away from vertical, as the sine of that angle: a wall
    // reads zero and a ceiling one. The outward normal is asked, not the one turned to
    // face the camera, because which way the surface hangs does not depend on where the
    // camera is. See docs/decisions/0034.
    var base = in.color.rgb;
    if (in.overhang >= 0.0) {
        let lean = -outward.z;
        let marked = smoothstep(in.overhang, in.overhang + OVERHANG_BAND, lean);
        base = mix(base, globals.overhang_color.rgb, marked * OVERHANG_FILL);
    }

    base = banded(base, in.world.z);

    if (outside_volume(in.world)) {
        base = mix(base, globals.outside_color.rgb, mix(OUTSIDE_GAP, OUTSIDE_STRIPE, stripe));
    }

    // A volume is not a surface: it is the space between the two walls that bound it, so
    // neither the light nor the inside wash has anything to say about it and both walls
    // lay down the same colour. See docs/design/viewport.md.
    var shaded = base;
    if (in.surface > 0.5) {
        if (!front_facing) {
            shaded = mix(base, globals.inside_color.rgb, SECTION_WASH);
        }
        let grooves = layer_roughness(outward) * smoothstep(LAYER_FADE_FROM, LAYER_FADE_TO, per_px);
        shaded = shade(shaded, in.color.a, normal, in.world, grooves) * layered.w;
    }
    // The token arrives premultiplied, so the wash scales colour and alpha alike and the
    // result stays premultiplied for the blend.
    let wash = seen_through(normal, in.world, in.surface);
    return vec4<f32>(mix(shaded, globals.cut_color.rgb, traced) * wash, in.color.a * wash);
}

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

// The pass ahead of the frame that keeps, for each pixel, the nearest back face of a clear
// model: where the light inside first leaves it. Its outward normal, and its distance from
// the eye less the pivot's, so a half-float holds it.
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

// How a printed surface spreads the light through it, as a roughness. The layers cut every
// surface into a stair of risers and treads, which past a pixel no picture resolves: what
// is left is how far apart their normals turn, most where the two share the surface evenly.
// A wall's lips frost it a little, a flat top is glossy. See docs/design/viewport.md.
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

// One channel's light through the part: bent into it at `normal`, carried to where it
// first leaves, bent out at that face (or turned back inside past the critical angle), and
// carried as far again; the pixel it comes from.
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

// What stands behind, seen through the part: traced per channel, then spread over an
// ellipse long across the layers by how rough both faces are, over the backdrop wherever
// the copy holds nothing.
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

// A clear resin after KHR_materials_volume, its surface the layers left: what stands behind,
// refracted at both faces, spread by their layers, tinted over the thickness crossed and
// clouding into the resin's lit colour, under a reflection off the stepped surface.
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

// What a face of a clear model lays into the scene before the models refract it, so walls
// inside, a cavity's or a lattice's, read through the part: the studio's reflection off it,
// brightest at a grazing angle, over a trace of the resin's own colour.
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

// A marker a tool put down, cut like the model. Its far half is lit as the flat face the cut
// leaves, so a cut ball reads as a disc, not a bowl: see docs/design/viewport.md.
@fragment
fn marker_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    // A prop of the render view's desk names its material below every marker's threshold.
    let local_px = fwidth(in.local);
    if (in.overhang < PROP_BELOW) {
        return prop_colour(in, front_facing, local_px);
    }
    if (sectioned_away(in.world)) {
        discard;
    }
    var normal = normalize(in.normal);
    var base = in.color.rgb;
    if (!front_facing) {
        normal = -normal;
        if (globals.section.y > 0.5 && globals.eye.z > globals.section.x) {
            normal = vec3<f32>(0.0, 0.0, 1.0);
        } else if (globals.section.w > 0.5 && globals.eye.z < globals.section.z) {
            normal = vec3<f32>(0.0, 0.0, -1.0);
        }
        base = mix(base, globals.inside_color.rgb * in.color.a, SECTION_WASH);
    }
    return vec4<f32>(shade(base, in.color.a, normal, in.world, 0.0), in.color.a);
}

// How much of itself a surface keeps when the models are drawn seen through: least where
// it faces the camera, so a wall is a pane of glass, and most where it turns away, so
// every edge — the rim of a cavity, a cell of the lattice — draws itself.
const XRAY_FLOOR: f32 = 0.35;

fn seen_through(normal: vec3<f32>, world: vec3<f32>, surface: f32) -> f32 {
    if (globals.eye.w <= 0.0 || surface < 0.5) {
        return 1.0;
    }
    let view = normalize(globals.eye.xyz - world);
    let rim = 1.0 - abs(dot(normal, view));
    return globals.eye.w * (XRAY_FLOOR + (1.0 - XRAY_FLOOR) * rim * rim);
}

// The textured pass: the model washed with the heights a relief would press into it, so
// the image can be seen where it lands before it is pressed. White stands proud, black
// stays where it is; see docs/decisions/0116.
@group(1) @binding(0) var relief_texture: texture_2d_array<f32>;
@group(1) @binding(1) var relief_sampler: sampler;

struct ReliefVertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Locations 2 to 10 are the instance's, the same ones the flat pass reads.
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) normal_0: vec3<f32>,
    @location(7) normal_1: vec3<f32>,
    @location(8) normal_2: vec3<f32>,
    @location(9) color: vec4<f32>,
    @location(11) uv: vec2<f32>,
    // Which of the model's images this face is textured from, or negative for a face the
    // map does not cover.
    @location(12) layer: f32,
};

struct ReliefFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) layer: f32,
    @location(4) world: vec3<f32>,
};

@vertex
fn relief_vertex(in: ReliefVertexIn) -> ReliefFragment {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    let normal_matrix = mat3x3<f32>(in.normal_0, in.normal_1, in.normal_2);
    let world = model * vec4<f32>(in.position, 1.0);

    var out: ReliefFragment;
    out.clip_position = globals.view_projection * world;
    out.normal = normal_matrix * in.normal;
    out.color = in.color;
    out.uv = in.uv;
    out.layer = in.layer;
    out.world = world.xyz;
    return out;
}

// How dark a black pixel leaves the surface and how bright a white one. The floor is not
// zero: an unlit surface reads as a hole rather than as a texture.
const RELIEF_FLOOR = 0.45;
const RELIEF_CEILING = 1.35;

@fragment
fn relief_fragment(in: ReliefFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    if (sectioned_away(in.world)) {
        discard;
    }

    let outward = normalize(in.normal);
    var normal = outward;
    if (!front_facing) {
        normal = -normal;
    }

    // Sampled before the branch: a sample may only sit in uniform control flow, which a
    // browser's shader compiler enforces. V runs up the image, a texture samples down it.
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let height = textureSample(relief_texture, relief_sampler, uv, max(i32(in.layer), 0)).r;
    var base = in.color.rgb;
    if (in.layer >= 0.0) {
        base = base * mix(RELIEF_FLOOR, RELIEF_CEILING, height);
    }

    if (!front_facing) {
        base = mix(base, globals.inside_color.rgb, SECTION_WASH);
    }
    return vec4<f32>(lit(base, in.color.a, normal, in.world), in.color.a);
}

@group(1) @binding(0) var label_atlas: texture_2d<f32>;
@group(1) @binding(1) var label_sampler: sampler;

struct LabelVertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct LabelFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn label_vertex(in: LabelVertexIn) -> LabelFragment {
    var out: LabelFragment;
    out.clip_position = globals.view_projection * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// The word lying in front of the plate, sampled out of egui's own font atlas so it is the same
// face the rest of the window is set in. Both the atlas and the colour are premultiplied,
// so coverage scales the whole thing.
@fragment
fn label_fragment(in: LabelFragment) -> @location(0) vec4<f32> {
    return in.color * textureSample(label_atlas, label_sampler, in.uv).a;
}

struct LineVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn line_vertex(in: LineVertex) -> LineFragment {
    var out: LineFragment;
    out.clip_position = globals.view_projection * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn line_fragment(in: LineFragment) -> @location(0) vec4<f32> {
    return in.color;
}

struct WideLine {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) half_width: vec2<f32>,
};

// Share of the way to the eye a wide line's depth is taken from, so the faces beside the
// edge it marks never cover its width; the pull runs along the view ray, so it stays put.
const WIDE_LINE_PULL = 0.003;

// Six corners, two triangles, laid across the screen round the line and past each end by
// half its width. A line with an end behind the eye is dropped.
@vertex
fn wide_line_vertex(@builtin(vertex_index) corner: u32, in: WideLine) -> LineFragment {
    let along = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
    let side = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
    let start = globals.view_projection * vec4<f32>(mix(in.start, globals.eye.xyz, WIDE_LINE_PULL), 1.0);
    let finish = globals.view_projection * vec4<f32>(mix(in.end, globals.eye.xyz, WIDE_LINE_PULL), 1.0);
    var screen = (finish.xy / finish.w - start.xy / start.w) / in.half_width;
    if dot(screen, screen) < 1e-8 {
        screen = vec2<f32>(1.0, 0.0);
    }
    let forward = normalize(screen);
    let across = vec2<f32>(-forward.y, forward.x);
    let at = along[corner];
    let corner_at = mix(start, finish, at);
    let offset = (across * side[corner] + forward * (at * 2.0 - 1.0)) * in.half_width;

    var out: LineFragment;
    out.clip_position = vec4<f32>(corner_at.xy + offset * corner_at.w, corner_at.zw);
    if start.w <= 0.0 || finish.w <= 0.0 {
        out.clip_position = vec4<f32>(0.0);
    }
    out.color = in.color;
    return out;
}

// Three vertices covering the screen: what the stencil is put back to zero with between
// one object's cuts and the next. It paints nothing and takes no depth.
@vertex
fn screen_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn screen_fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// The contact shadow, baked only when the plate changes: every model is laid flat onto a
// texture over the plate, darkest where it stands closest to it, and the texture is
// blurred once; the plate then reads it every frame. See docs/design/viewport.md.

// Millimetres above the plate past which a surface no longer darkens it.
const SHADOW_REACH_MM = 30.0;

struct SilhouetteVertex {
    @location(0) position: vec3<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(9) color: vec4<f32>,
    @location(13) surface: f32,
};

struct SilhouetteFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) height: f32,
    @location(1) weight: f32,
};

// Plate millimetres onto the shadow texture, whose top row is the back of the plate.
@vertex
fn silhouette_vertex(in: SilhouetteVertex) -> SilhouetteFragment {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    let world = model * vec4<f32>(in.position, 1.0);
    let across = (world.xy - globals.shadow_area.xy) / max(globals.shadow_area.zw, vec2<f32>(1e-3));

    var out: SilhouetteFragment;
    out.clip_position = vec4<f32>(across * 2.0 - 1.0, 0.5, 1.0);
    out.height = world.z;
    // A volume is space rather than a body and casts nothing; a translucent marker casts
    // as much as it covers.
    out.weight = in.color.a * step(0.5, in.surface);
    return out;
}

// Nothing under the plate shades its top.
@fragment
fn silhouette_fragment(in: SilhouetteFragment) -> @location(0) vec4<f32> {
    let above = step(-VOLUME_SLACK, in.height);
    let near = 1.0 - smoothstep(0.0, SHADOW_REACH_MM, max(in.height, 0.0));
    return vec4<f32>(near * above * in.weight, 0.0, 0.0, 0.0);
}

@group(1) @binding(0) var shadow_texture: texture_2d<f32>;
@group(1) @binding(1) var shadow_sampler: sampler;

// Texels between one tap of the blur and the next, and how many taps out from the middle
// it reaches each way: a soft edge of a few millimetres on a plate of ordinary size.
const BLUR_STEP = 1.5;
const BLUR_REACH = 3;

struct ScreenFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blur_vertex(@builtin(vertex_index) index: u32) -> ScreenFragment {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: ScreenFragment;
    out.clip_position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

@fragment
fn blur_fragment(in: ScreenFragment) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_texture));
    let sigma = f32(BLUR_REACH) * 0.6;
    var sum = 0.0;
    var weights = 0.0;
    for (var y = -BLUR_REACH; y <= BLUR_REACH; y = y + 1) {
        for (var x = -BLUR_REACH; x <= BLUR_REACH; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y));
            let weight = exp(-dot(offset, offset) / (2.0 * sigma * sigma));
            let at = in.uv + offset * BLUR_STEP * texel;
            sum = sum + weight * textureSampleLevel(shadow_texture, shadow_sampler, at, 0.0).r;
            weights = weights + weight;
        }
    }
    return vec4<f32>(sum / weights, 0.0, 0.0, 0.0);
}

struct FloorFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Where the fragment stands on the plate, millimetres.
    @location(1) plate: vec2<f32>,
};

// Two triangles lying on the plate over the shadow's area.
@vertex
fn floor_vertex(@builtin(vertex_index) index: u32) -> FloorFragment {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[index];
    let world = globals.shadow_area.xy + corner * globals.shadow_area.zw;
    var out: FloorFragment;
    out.clip_position = globals.view_projection * vec4<f32>(world, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    out.plate = world;
    return out;
}

// Pixels wide the minor and the major grid lines are drawn.
const MINOR_LINE_PX = 1.0;
const MAJOR_LINE_PX = 1.5;

// How much of a line of a grid `spacing_mm` apart covers the fragment, smoothed over a
// pixel, and faded out as the lines close to within a few pixels of each other, where they
// would only be noise.
fn grid_cover(at_mm: vec2<f32>, spacing_mm: f32, width_px: f32) -> f32 {
    let cells = at_mm / spacing_mm;
    let per_px = max(fwidth(cells), vec2<f32>(1e-6));
    let off_px = abs(fract(cells - 0.5) - 0.5) / per_px;
    let edge = vec2<f32>(width_px * 0.5);
    let line = 1.0 - smoothstep(edge - 0.5, edge + 0.5, off_px);
    let dense = smoothstep(0.12, 0.25, max(per_px.x, per_px.y));
    return max(line.x, line.y) * (1.0 - dense);
}

// The plate's own surface with its grid drawn into it, before anything stands on it: one
// surface, so the grid can never fight the floor for depth.
@fragment
fn plate_fragment(in: FloorFragment) -> @location(0) vec4<f32> {
    let minor = grid_cover(in.plate, globals.grid.x, MINOR_LINE_PX);
    let major = grid_cover(in.plate, globals.grid.y, MAJOR_LINE_PX);
    var colour = globals.plate_color;
    if (globals.grid.z > 0.5) {
        colour = mix(colour, globals.grid_minor, minor);
        colour = mix(colour, globals.grid_major, major);
    }
    if (in_inset(in.plate)) {
        colour = vec4<f32>(colour.rgb * (1.0 - INSET_SHADE), colour.a);
    }
    return colour;
}

// How much darker the plate is in its inset (ADR 0371): about a third of the way from the
// plate's grey to the backdrop's, which is 0.6 of it.
const INSET_SHADE = 0.12;

fn in_inset(at_mm: vec2<f32>) -> bool {
    if (globals.volume.w < 0.5) {
        return false;
    }
    return any(at_mm < globals.volume_low.xy) || any(at_mm > globals.volume.xy);
}

// Black at the shadow's own darkness, premultiplied. Nothing from under the plate: the
// shadow is on its top.
@fragment
fn floor_fragment(in: FloorFragment) -> @location(0) vec4<f32> {
    let shade = textureSample(shadow_texture, shadow_sampler, in.uv).r;
    if (globals.eye.z < 0.0) {
        discard;
    }
    return vec4<f32>(0.0, 0.0, 0.0, shade * globals.shadow_color.a);
}
