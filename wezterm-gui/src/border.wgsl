// Border shader for Claude Terminal status borders
// Renders colored quads for pane borders with status-based animations
//
// Status codes:
// 0 = Idle (pulse animation: sin(time * 2π / pulse_period) - configurable via uniform)
// 1 = Running (solid color)
// 2 = AwaitingPermission (blink animation: toggle at blink_rate_hz - configurable via uniform)
// 3 = Error (solid color)

// Mathematical constants
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

// Status code constants
const STATUS_IDLE: f32 = 0.0;
const STATUS_RUNNING: f32 = 1.0;
const STATUS_AWAITING_PERMISSION: f32 = 2.0;
const STATUS_ERROR: f32 = 3.0;

// Animation opacity parameters (fixed, not configurable)
const PULSE_MIN_OPACITY: f32 = 0.4;  // Minimum opacity during pulse
const PULSE_MAX_OPACITY: f32 = 1.0;  // Maximum opacity during pulse
const BLINK_ON_OPACITY: f32 = 1.0;  // Full brightness during "on" phase
const BLINK_DIM_OPACITY: f32 = 0.3;  // Dim during "off" phase

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

// Uniforms for projection matrix, animation time, and animation settings
struct BorderUniform {
    projection: mat4x4<f32>,
    animation_time: f32,
    pulse_idle_enabled: f32,      // 1.0 = enabled, 0.0 = disabled
    blink_permission_enabled: f32, // 1.0 = enabled, 0.0 = disabled
    blink_rate_hz: f32,           // Blink frequency in Hz
    pulse_period_secs: f32,       // Pulse period in seconds
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
    if (in.status == STATUS_IDLE && uniforms.pulse_idle_enabled > 0.5) {
        // Pulse animation for Idle status (only if enabled)
        // sin(time * 2π / period) oscillates between -1 and 1
        // Map to opacity range [PULSE_MIN_OPACITY, PULSE_MAX_OPACITY]
        let pulse_phase = sin(uniforms.animation_time * TWO_PI / uniforms.pulse_period_secs);
        let pulse_factor = (pulse_phase + 1.0) * 0.5;  // Map -1..1 to 0..1
        let opacity = PULSE_MIN_OPACITY + pulse_factor * (PULSE_MAX_OPACITY - PULSE_MIN_OPACITY);
        final_color.a = final_color.a * opacity;
    } else if (in.status == STATUS_AWAITING_PERMISSION && uniforms.blink_permission_enabled > 0.5) {
        // Blink animation for AwaitingPermission status (only if enabled)
        // Toggle between full and dim opacity at blink_rate_hz
        // Use step function: if (time % period) < half_period then full brightness, else dim
        let blink_period = 1.0 / uniforms.blink_rate_hz;
        let blink_phase = uniforms.animation_time % blink_period;
        let opacity = select(BLINK_DIM_OPACITY, BLINK_ON_OPACITY, blink_phase < (blink_period * 0.5));
        final_color.a = final_color.a * opacity;
    }
    // STATUS_RUNNING and STATUS_ERROR: solid color (no animation)
    // Animations disabled: solid color

    return final_color;
}
