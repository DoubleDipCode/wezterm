// Border shader for Claude Terminal status borders
// Renders colored quads for pane borders with optional animation support

// Vertex input - simplified for solid color borders
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Uniforms for projection matrix
struct BorderUniform {
    projection: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: BorderUniform;

// Vertex shader - transforms quad vertices and passes color to fragment shader
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    return out;
}

// Fragment shader - outputs the solid color from vertex shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
