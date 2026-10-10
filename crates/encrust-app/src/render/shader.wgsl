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

// The box one pocket of trapped resin stands in, plate millimetres. See
// docs/decisions/0200.
struct TrapBox {
    low: vec4<f32>,
    high: vec4<f32>,
};

struct Globals {
    view_projection: mat4x4<f32>,
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
    // xyz is the build volume in plate millimetres from the origin corner, w 1.0 while a
    // model past it is to be marked; outside_color.rgb is what marks it.
    volume: vec4<f32>,
    outside_color: vec4<f32>,
    // The Cut tool's plane: xyz its normal, w its offset along it in millimetres. It is
    // traced only inside cut_low..cut_high, and only while cut_low.w is 1.0.
    cut_plane: vec4<f32>,
    cut_low: vec4<f32>,
    cut_high: vec4<f32>,
    cut_color: vec4<f32>,
    cuts: array<DrainCut, MAX_CUTS>,
    bands: array<ExposureBand, MAX_BANDS>,
    pockets: array<TrapBox, MAX_POCKETS>,
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
    // 1.0 for a surface of a model, 0.0 for a volume painted in its own colour. 11 and 12
    // belong to the relief pass.
    @location(13) surface: f32,
};

struct ModelFragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) overhang: f32,
    // Where the fragment stands in plate millimetres: its height is what the section cut
    // is compared against, and the whole point is what a drain cut is.
    @location(3) world: vec3<f32>,
    @location(4) surface: f32,
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

// How much of the overhang colour a marked surface takes off a stripe and on one. Not
// nothing between the stripes: a patch narrower than the pitch would otherwise vanish.
const HATCH_GAP = 0.2;
const HATCH_STRIPE = 0.85;

// The same for whatever stands past the build volume.
const OUTSIDE_GAP = 0.3;
const OUTSIDE_STRIPE = 0.9;

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
    out.surface = in.surface;
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

// Whether a fragment of a cut's own body is there to be drawn at all: what the section cut
// took away is gone, and so is what another cut took out of this one, or two holes drilled
// side by side would each lay down a depth the other's test then refuses to paint — a black
// band between them. See docs/decisions/0201.
fn cut_surface_gone(world: vec3<f32>) -> bool {
    return (globals.section.y > 0.5 && world.z > globals.section.x) || cut_away(world);
}

// The pass that lays the far wall of a cut body into the depth plane, painting nothing.
@fragment
fn cut_candidate_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (cut_surface_gone(in.world)) {
        discard;
    }
    return vec4<f32>(0.0);
}

// The pass that counts the material standing in front of that wall. It subtracts no cut: a
// crossing inside a hole is still a crossing of the solid the count is about. Only the
// section cut applies, because what it took away is not there to be counted.
@fragment
fn cut_crossing_fragment(in: ModelFragment) -> @location(0) vec4<f32> {
    if (globals.section.y > 0.5 && in.world.z > globals.section.x) {
        discard;
    }
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

@fragment
fn model_fragment(in: ModelFragment, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let traced = cut_line(in.world);
    let stripe = hatch(in.world);
    let cutting = globals.section.y > 0.5;
    if (cutting && in.world.z > globals.section.x) {
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
        base = mix(base, globals.overhang_color.rgb, marked * mix(HATCH_GAP, HATCH_STRIPE, stripe));
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
        shaded = lit(shaded, in.color.a, normal, in.world);
    }
    // The token arrives premultiplied, so the wash scales colour and alpha alike and the
    // result stays premultiplied for the blend.
    let wash = seen_through(normal, in.world, in.surface);
    return vec4<f32>(mix(shaded, globals.cut_color.rgb, traced) * wash, in.color.a * wash);
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
    let cutting = globals.section.y > 0.5;
    if (cutting && in.world.z > globals.section.x) {
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
    return colour;
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
