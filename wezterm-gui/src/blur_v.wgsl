// Vertical Gaussian blur shader for glow effect
// Part of two-pass separable Gaussian blur (horizontal + vertical)
// Uses 9-tap kernel with Gaussian weights

// Vertex input for fullscreen quad
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
};

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

// Uniforms for texture dimensions and glow settings
struct BlurUniform {
    // Texture dimensions (width, height) for calculating texel offsets
    tex_size: vec2<f32>,
    // Blur radius multiplier (from config.border.glow_radius)
    blur_scale: f32,
    // Glow opacity from 0.0 to 1.0 (from config.border.glow_opacity)
    glow_opacity: f32,
};
@group(0) @binding(0) var<uniform> uniforms: BlurUniform;

// Input texture (blur_temp from horizontal pass)
@group(1) @binding(0) var input_tex: texture_2d<f32>;
@group(1) @binding(1) var input_sampler: sampler;

// Gaussian kernel weights for 9-tap filter
// Kernel: [0.05, 0.09, 0.12, 0.15, 0.18, 0.15, 0.12, 0.09, 0.05]
// Sum = 1.0 (normalized)
const WEIGHT_0: f32 = 0.05;  // offset -4
const WEIGHT_1: f32 = 0.09;  // offset -3
const WEIGHT_2: f32 = 0.12;  // offset -2
const WEIGHT_3: f32 = 0.15;  // offset -1
const WEIGHT_4: f32 = 0.18;  // offset  0 (center)
const WEIGHT_5: f32 = 0.15;  // offset +1
const WEIGHT_6: f32 = 0.12;  // offset +2
const WEIGHT_7: f32 = 0.09;  // offset +3
const WEIGHT_8: f32 = 0.05;  // offset +4

// Vertex shader - passes through position and texture coordinates
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    out.tex_coord = model.tex_coord;
    return out;
}

// Fragment shader - applies vertical Gaussian blur with glow opacity
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate vertical texel offset (1 pixel in texture space)
    let texel_offset = vec2<f32>(0.0, 1.0 / uniforms.tex_size.y) * uniforms.blur_scale;

    var color = vec4<f32>(0.0);

    // Sample at 9 vertical offsets: -4, -3, -2, -1, 0, +1, +2, +3, +4
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * -4.0) * WEIGHT_0;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * -3.0) * WEIGHT_1;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * -2.0) * WEIGHT_2;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * -1.0) * WEIGHT_3;
    color += textureSample(input_tex, input_sampler, in.tex_coord) * WEIGHT_4;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * 1.0) * WEIGHT_5;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * 2.0) * WEIGHT_6;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * 3.0) * WEIGHT_7;
    color += textureSample(input_tex, input_sampler, in.tex_coord + texel_offset * 4.0) * WEIGHT_8;

    // Apply glow opacity multiplier from config - makes glow visible but subtle
    color.a *= uniforms.glow_opacity;

    return color;
}
