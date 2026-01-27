// Border shader for Claude Terminal status borders
// Renders colored quads for pane borders with status-based animations
//
// Status codes:
// 0 = Idle (pulse animation: sin(time * 2π) with 2 second period)
// 1 = Running (solid color)
// 2 = AwaitingPermission (blink animation - implemented in US-021)
// 3 = Error (solid color)

// Mathematical constants
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

// Status code constants
const STATUS_IDLE: f32 = 0.0;
const STATUS_RUNNING: f32 = 1.0;
const STATUS_AWAITING_PERMISSION: f32 = 2.0;
const STATUS_ERROR: f32 = 3.0;

// Animation parameters
const PULSE_PERIOD: f32 = 2.0;  // 2 seconds for full pulse cycle
const PULSE_MIN_OPACITY: f32 = 0.4;  // Minimum opacity during pulse
const PULSE_MAX_OPACITY: f32 = 1.0;  // Maximum opacity during pulse

// Vertex input with status for animation selection
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) status: f32,
};

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) status: f32,
};

// Uniforms for projection matrix and animation time
struct BorderUniform {
    projection: mat4x4<f32>,
    animation_time: f32,
};
@group(0) @binding(0) var<uniform> uniforms: BorderUniform;

// Vertex shader - transforms quad vertices and passes color/status to fragment shader
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    out.status = model.status;
    return out;
}

// Fragment shader - outputs color with status-based animation
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var final_color = in.color;

    // Apply animation based on status
    if (in.status == STATUS_IDLE) {
        // Pulse animation for Idle status
        // sin(time * 2π / period) oscillates between -1 and 1
        // Map to opacity range [PULSE_MIN_OPACITY, PULSE_MAX_OPACITY]
        let pulse_phase = sin(uniforms.animation_time * TWO_PI / PULSE_PERIOD);
        let pulse_factor = (pulse_phase + 1.0) * 0.5;  // Map -1..1 to 0..1
        let opacity = PULSE_MIN_OPACITY + pulse_factor * (PULSE_MAX_OPACITY - PULSE_MIN_OPACITY);
        final_color.a = final_color.a * opacity;
    }
    // STATUS_RUNNING and STATUS_ERROR: solid color (no animation)
    // STATUS_AWAITING_PERMISSION: will be implemented in US-021

    return final_color;
}
