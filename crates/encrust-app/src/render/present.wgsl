// Copies the viewport's own colour plane into egui's pass. The plane is the size of the
// window, so a fragment reads the texel at its own framebuffer position.

@group(0) @binding(0) var frame: texture_2d<f32>;

// One triangle covering the whole viewport; egui's viewport and scissor trim it.
@vertex
fn present_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn present_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(frame, vec2<i32>(position.xy), 0);
}
