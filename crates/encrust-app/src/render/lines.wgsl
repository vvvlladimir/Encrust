// What is laid flat over the scene: the plate's word, lines and the wide marks on edges.
// Joined to shader.wgsl.

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
