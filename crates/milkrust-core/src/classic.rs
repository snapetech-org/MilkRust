//! Fixed-function MilkDrop display controls, shared by the GL hosts.
use crate::MilkRustFrame;

pub const VALUES: &[(&str, f64)] = &[
    ("gamma", 2.0), ("echo_zoom", 2.0), ("echo_alpha", 0.0), ("echo_orient", 0.0),
    ("brighten", 0.0), ("darken", 0.0), ("solarize", 0.0), ("invert", 0.0), ("wrap", 1.0),
];

pub fn uniforms(frame: &MilkRustFrame) -> ([f32; 4], [f32; 4], f32) {
    let value = |name: &str, default: f64| frame.shader_values.get(name).copied().filter(|v| v.is_finite()).unwrap_or(default);
    ([value("gamma", 2.0).clamp(0.0, 8.0) as f32,
        value("echo_zoom", 2.0).abs().clamp(0.001, 1000.0) as f32,
        value("echo_alpha", 0.0).clamp(0.0, 1.0) as f32,
        value("echo_orient", 0.0).trunc().rem_euclid(4.0) as f32],
     ["brighten", "darken", "solarize", "invert"].map(|key| if value(key, 0.0) != 0.0 { 1.0 } else { 0.0 }),
     if value("wrap", 1.0) != 0.0 { 1.0 } else { 0.0 })
}

pub fn darken_center(scope: &std::collections::BTreeMap<String, crate::MilkRustValue>) -> Option<crate::MilkRustPrimitive> {
    if crate::milkrust_scope_number(scope, "darken_center", 0.0) == 0.0 { return None; }
    let half_width = 0.05 / crate::milkrust_scope_number(scope, "aspecty", 1.0).abs().max(0.001);
    let mut colors = vec![0.0; 24];
    colors[3] = 3.0 / 32.0;
    Some(crate::MilkRustPrimitive {
        additive: false, color: [0.0; 4], mode: crate::MilkRustPrimitiveMode::TriangleFan,
        vertex_colors: colors,
        vertices: vec![0.0, 0.0, -half_width, 0.0, 0.0, -0.05, half_width, 0.0, 0.0, 0.05, -half_width, 0.0],
    })
}

/// Gamma is MilkDrop's additive brightness gain, not a transfer-function power.
/// Display effects are deliberately absent from the feedback and HLSL passes.
pub const GLSL: &str = r#"
uniform vec4 classicEcho; // gamma, echo zoom, alpha, orientation
uniform vec4 classicFilters; // brighten, darken, solarize, invert
uniform float classicWrap;
vec3 milkrust_classic_display(sampler2D history, vec2 uv) {
    vec2 echoUv = (uv - vec2(0.5)) / classicEcho.y + vec2(0.5);
    int orientation = int(classicEcho.w);
    if ((orientation & 1) != 0) echoUv.x = 1.0 - echoUv.x;
    if ((orientation & 2) != 0) echoUv.y = 1.0 - echoUv.y;
    vec3 color = mix(texture(history, uv).rgb, texture(history, clamp(echoUv, 0.0, 1.0)).rgb, classicEcho.z);
    color = clamp(color * classicEcho.x, 0.0, 1.0);
    if (classicFilters.x != 0.0) color = vec3(1.0) - (vec3(1.0) - color) * (vec3(1.0) - color);
    if (classicFilters.y != 0.0) color *= color;
    if (classicFilters.z != 0.0) color = 2.0 * color * (vec3(1.0) - color);
    if (classicFilters.w != 0.0) color = vec3(1.0) - color;
    return color;
}
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn serialized_and_equation_controls_reach_the_display_pass() {
        let source = "fGammaAdj=1.8\nfVideoEchoZoom=2\nfVideoEchoAlpha=0.4\nnVideoEchoOrientation=3\nbInvert=1\nbTexWrap=0\nper_frame_1=gamma=1.5;echo_alpha=0.25;\n";
        let frame = crate::milkrust_frame_set_from_source(source, 0.0, 0.0, 0.0, 0.0);
        let (echo, flags, wrap) = super::uniforms(&frame.entries[0].frame);
        assert_eq!(echo, [1.5, 2.0, 0.25, 3.0]);
        assert_eq!(flags, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(wrap, 0.0);
    }

    #[test]
    fn additive_dots_and_center_fade_survive_geometry_generation() {
        let mut runtime = crate::MilkRustFrameSetRuntime::default();
        runtime.set_viewport(200, 100).unwrap();
        let set = runtime.render_source("wave_a=1\nbAdditiveWaves=1\nbWaveDots=1\nbDarkenCenter=1\n", 0.0, 0.1, 0.2, 0.3);
        let primitives = &set.entries[0].frame.primitives;
        assert!(primitives.iter().any(|p| p.additive && p.mode == crate::MilkRustPrimitiveMode::Triangles));
        let center = primitives.iter().find(|p| p.mode == crate::MilkRustPrimitiveMode::TriangleFan && p.vertex_colors.len() == 24).unwrap();
        assert_eq!(center.vertex_colors[3], 3.0 / 32.0);
        assert_eq!(center.vertices[2], -0.025);
        assert!(!center.additive);
    }
}
