//! Shared built-in shader resources. Generated noise is original deterministic
//! data; no textures are taken from community preset collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureKind { Main, Blur(usize), Noise2d(u32), Noise3d(u32), Asset }
impl TextureKind {
    pub fn is_volume(self) -> bool { matches!(self, Self::Noise3d(_)) }
    pub fn is_builtin(self) -> bool { !matches!(self, Self::Asset) }
}

pub fn base_name(name: &str) -> &str {
    let name = name.strip_prefix("sampler_").unwrap_or(name);
    let name = name.strip_prefix("sampler_").unwrap_or(name);
    ["fc_", "fw_", "pc_", "pw_"].iter().find_map(|p| name.strip_prefix(p)).unwrap_or(name)
}
pub fn texture_kind(name: &str) -> TextureKind {
    match base_name(name) {
        "main" => TextureKind::Main,
        "blur1" => TextureKind::Blur(0), "blur2" => TextureKind::Blur(1), "blur3" => TextureKind::Blur(2),
        "noise_lq" => TextureKind::Noise2d(1), "noise_mq" => TextureKind::Noise2d(2),
        "noise_hq" => TextureKind::Noise2d(3), "noise_lq_lite" => TextureKind::Noise2d(4),
        "noisevol_lq" => TextureKind::Noise3d(5), "noisevol_hq" => TextureKind::Noise3d(6),
        _ => TextureKind::Asset,
    }
}
/// (nearest filtering, repeat addressing). Unqualified samplers use linear
/// repeat, the MilkDrop convention; explicit fc/pc request clamp addressing.
pub fn sampler_state(name: &str) -> (bool, bool) {
    let name = name.strip_prefix("sampler_").unwrap_or(name);
    let nearest = name.starts_with("pc_") || name.starts_with("pw_");
    let repeat = !(name.starts_with("fc_") || name.starts_with("pc_"));
    (nearest, repeat)
}

pub fn noise_texture(seed: u32, volume: bool) -> (u32, u32, u32, Vec<u8>) {
    let side = if volume { 32 } else if seed == 4 { 32 } else { 256 };
    let depth = if volume { side } else { 1 };
    let mut bytes = vec![0u8; (side * side * depth * 4) as usize];
    let mut state = seed.wrapping_mul(0x9e37_79b9).max(1);
    for pixel in bytes.chunks_exact_mut(4) {
        for channel in &mut pixel[..3] {
            state ^= state << 13; state ^= state >> 17; state ^= state << 5;
            *channel = (state >> 24) as u8;
        }
        pixel[3] = 255;
    }
    (side, side, depth, bytes)
}

pub const BLUR_FRAGMENT: &str = "#version 300 es\nprecision highp float; uniform sampler2D previousFrame; uniform vec2 blurStep; in vec2 uv; out vec4 outColor; void main(){ vec3 c=texture(previousFrame,uv).rgb*0.2270270270; c+=(texture(previousFrame,uv+blurStep*1.3846153846).rgb+texture(previousFrame,uv-blurStep*1.3846153846).rgb)*0.3162162162; c+=(texture(previousFrame,uv+blurStep*3.2307692308).rgb+texture(previousFrame,uv-blurStep*3.2307692308).rgb)*0.0702702703; outColor=vec4(c,1.0); }";

pub const SCALAR_UNIFORMS: &[&str] = &["frame", "fps", "progress", "decay", "vol", "vol_att",
    "blur1_min", "blur1_max", "blur2_min", "blur2_max", "blur3_min", "blur3_max"];
pub const VECTOR_UNIFORMS: &[&str] = &["rand_frame", "rand_preset", "roam_sin", "roam_cos", "slow_roam_sin", "slow_roam_cos",
    "texsize_noise_lq", "texsize_noise_mq", "texsize_noise_hq", "texsize_noise_lq_lite", "texsize_noisevol_lq", "texsize_noisevol_hq",
    "_qa", "_qb", "_qc", "_qd", "_qe", "_qf", "_qg", "_qh", "_qi", "_qj", "_qk", "_ql", "_qm", "_qn", "_qo", "_qp"];

pub fn frame_values(scope: &std::collections::BTreeMap<String, crate::MilkRustValue>, title: &str) -> std::collections::BTreeMap<String, f64> {
    let mut values = std::collections::BTreeMap::new();
    for name in SCALAR_UNIFORMS.iter().copied().chain(["bass_att", "mid_att", "treb_att"]) {
        let fallback = if name == "fps" { 60.0 } else if name.ends_with("_max") || name == "decay" { 1.0 } else { 0.0 };
        let value = match scope.get(name) { Some(crate::MilkRustValue::Number(v)) if v.is_finite() => *v, _ => fallback };
        values.insert(name.to_string(), value);
    }
    let seed = title.bytes().fold(2166136261u32, |s, c| (s ^ u32::from(c)).wrapping_mul(16777619));
    values.insert("preset_seed".into(), f64::from(seed));
    for (target, suffix) in [("vol", ""), ("vol_att", "_att")] {
        let sum: f64 = ["bass", "mid", "treb"].iter().map(|band| crate::milkrust_scope_number(scope, &format!("{band}{suffix}"), 0.0)).sum();
        values.insert(target.into(), sum / 3.0);
    }
    values
}

pub fn uniform_vectors(frame: &crate::MilkRustFrame, time: f64, width: u32, height: u32) -> Vec<(&'static str, [f32; 4])> {
    let mut result = Vec::new();
    let x = (width as f32 / height.max(1) as f32).min(1.0);
    let y = (height as f32 / width.max(1) as f32).min(1.0);
    result.push(("hlslAspect", [x, y, 1.0/x, 1.0/y]));
    let mut seed = frame.shader_values.get("preset_seed").copied().unwrap_or(1.0) as u32;
    for name in ["rand_preset", "rand_frame"] {
        if name == "rand_frame" { seed ^= frame.shader_values.get("frame").copied().unwrap_or(time * 60.0) as u32; }
        let values = std::array::from_fn(|_| { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); (seed >> 8) as f32 / 16777216.0 });
        result.push((name, values));
    }
    for name in ["roam_sin", "roam_cos", "slow_roam_sin", "slow_roam_cos"] {
        let scale = if name.starts_with("slow") { 0.005 } else { 0.1 };
        let values = std::array::from_fn(|i| {
            let phase = time * scale * [1.0, 1.37, 1.73, 2.11][i];
            (0.5 + 0.5 * if name.ends_with("sin") { phase.sin() } else { phase.cos() }) as f32
        });
        result.push((name, values));
    }
    for &name in VECTOR_UNIFORMS {
        if let Some(texture) = name.strip_prefix("texsize_") {
            let side = if texture.starts_with("noisevol") || texture.ends_with("lite") { 32.0 } else { 256.0 };
            result.push((name, [side, side, 1.0 / side, 1.0 / side]));
        } else if name.starts_with("_q") {
            let base = usize::from(name.as_bytes()[2] - b'a') * 4;
            result.push((name, std::array::from_fn(|i| frame.q_registers[base+i] as f32)));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_and_noise_are_deterministic() {
        assert_eq!(texture_kind("sampler_pw_noisevol_hq"), TextureKind::Noise3d(6));
        assert_eq!(sampler_state("sampler_pw_main"), (true, true));
        assert_eq!(sampler_state("sampler_fc_blur1"), (false, false));
        assert_eq!(noise_texture(1, false), noise_texture(1, false));
        assert_ne!(noise_texture(1, false).3, noise_texture(2, false).3);
    }
}
