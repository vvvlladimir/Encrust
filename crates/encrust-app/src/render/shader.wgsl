// Viewport shading: one lit pass for models, one for the machine under the plate, and one
// unlit pass for the plate and the grid.
// The world is Z-up in plate millimetres, matching core-geometry.

// How many drain cuts the globals carry room for. Must match MAX_CUTS in gpu.rs.
const MAX_CUTS = 64u;

// How many exposure bands the globals carry room for. Must match MAX_BANDS in gpu.rs.
const MAX_BANDS = 8u;

// One drain hole or channel segment in plate millimetres: xyz is an end, w its radius.
struct DrainCut {
    mouth: vec4<f32>,
    tip: vec4<f32>,
};

// One band of print height that takes an exposure of its own: span.xy is the band in
// plate millimetres, tint.rgb what it washes the surface with.
struct ExposureBand {
    span: vec4<f32>,
    tint: vec4<f32>,
};

struct Globals {
    view_projection: mat4x4<f32>,
    // xyz is the direction light travels towards; w is unused padding.
    light_direction: vec4<f32>,
    // x is the height the models are cut at, plate millimetres; y is 1.0 while there is a
    // cut at all. See docs/decisions/0061.
    section: vec4<f32>,
    // x is how many of `cuts` carry a drain this frame, y how many of `bands` carry an
    // exposure, and z the height below which a band has no effect.
    counts: vec4<f32>,
    // rgb of what a surface needing support and a model's exposed inside are washed with;
    // w is unused padding. Both come from ui::theme, see docs/design/ui-design-system.md.
    overhang_color: vec4<f32>,
    inside_color: vec4<f32>,
    // xyz is the build volume in plate millimetres from the origin corner, w 1.0 while a
    // model past it is to be marked; outside_color.rgb is what marks it.
    volume: vec4<f32>,
    outside_color: vec4<f32>,
    cuts: array<DrainCut, MAX_CUTS>,
    bands: array<ExposureBand, MAX_BANDS>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

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
};

struct ModelFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) overhang: f32,
    // Where the fragment stands in plate millimetres: its height is what the section cut
    // is compared against, and the whole point is what a drain cut is.
    @location(3) world: vec3<f32>,
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
    let low = world < vec3<f32>(-VOLUME_SLACK);
    let high = world > globals.volume.xyz + vec3<f32>(VOLUME_SLACK);
    return any(low) || any(high);
}

@vertex
fn model_vertex(in: ModelVertex) -> ModelFragment {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    let normal_matrix = mat3x3<f32>(in.normal_0, in.normal_1, in.normal_2);

    let world = model * vec4<f32>(in.position, 1.0);

    var out: ModelFragment;
    out.clip_position = globals.view_projection * world;
    out.normal = normal_matrix * in.normal;
    out.color = in.color;
    out.overhang = in.overhang;
    out.world = world.xyz;
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

fn cut_away(world: vec3<f32>) -> bool {
    let count = i32(globals.counts.x);
    for (var index = 0; index < count; index = index + 1) {
        let cut = globals.cuts[index];
        let axis = cut.tip.xyz - cut.mouth.xyz;
        var along = dot(world - cut.mouth.xyz, axis) / max(dot(axis, axis), 1e-9);
        // A negative radius at the far end marks a cut that opens there, a channel's
        // mouth against a hole's floor: the floor is geometry and has to survive.
        if (cut.tip.w >= 0.0 && along > 1.0 - CUT_FLOOR) {
            continue;
        }
        // Rounded at the ends that open, so that a surface curving away under a mouth is
        // taken with it rather than left capping the bore.
        along = clamp(along, 0.0, 1.0);
        let radius = mix(cut.mouth.w, abs(cut.tip.w), along);
        if (distance(world, cut.mouth.xyz + axis * along) < radius * (1.0 - CUT_SKIN)) {
            return true;
        }
    }
    return false;
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

@fragment
fn model_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let cutting = globals.section.y > 0.5;
    if (cutting && in.world.z > globals.section.x) {
        discard;
    }
    if (cut_away(in.world)) {
        discard;
    }

    // Back faces are shaded as if they faced the camera. An imported mesh may still have
    // inverted faces, and a black hole reads as a rendering bug rather than as a defect.
    let outward = normalize(in.normal);
    var normal = outward;
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
        base = mix(base, globals.overhang_color.rgb, marked);
    }

    base = banded(base, in.world.z);

    if (outside_volume(in.world)) {
        base = globals.outside_color.rgb;
    }

    if (!front_facing) {
        base = mix(base, globals.inside_color.rgb, SECTION_WASH);
    }

    let light = normalize(-globals.light_direction.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let ambient = 0.30 + 0.12 * (normal.z * 0.5 + 0.5);
    return vec4<f32>(base * (ambient + 0.70 * diffuse), in.color.a);
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
    let cutting = globals.section.y > 0.5;
    if (cutting && in.world.z > globals.section.x) {
        discard;
    }

    let outward = normalize(in.normal);
    var normal = outward;
    if (!front_facing) {
        normal = -normal;
    }

    var base = in.color.rgb;
    if (in.layer >= 0.0) {
        // V runs up from the bottom of the image, and a texture samples down from the top.
        let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
        let height = textureSample(relief_texture, relief_sampler, uv, i32(in.layer)).r;
        base = base * mix(RELIEF_FLOOR, RELIEF_CEILING, height);
    }

    if (!front_facing) {
        base = mix(base, globals.inside_color.rgb, SECTION_WASH);
    }

    let light = normalize(-globals.light_direction.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let ambient = 0.30 + 0.12 * (normal.z * 0.5 + 0.5);
    return vec4<f32>(base * (ambient + 0.70 * diffuse), in.color.a);
}

struct BodyVertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct BodyFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn body_vertex(in: BodyVertexIn) -> BodyFragment {
    var out: BodyFragment;
    out.clip_position = globals.view_projection * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    return out;
}

// The machine under the plate: lit like a model, and never cut, marked or banded, because
// none of that belongs to the printer. The colour arrives premultiplied, so the light
// scales it without touching the alpha.
@fragment
fn body_fragment(in: BodyFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var normal = normalize(in.normal);
    if (!front_facing) {
        normal = -normal;
    }
    let light = normalize(-globals.light_direction.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let ambient = 0.30 + 0.12 * (normal.z * 0.5 + 0.5);
    return vec4<f32>(in.color.rgb * (ambient + 0.70 * diffuse), in.color.a);
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

// The word the platform carries, sampled out of egui's own font atlas so it is the same
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
