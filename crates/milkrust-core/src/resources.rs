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
    ["fc_", "fw_", "pc_", "pw_", "FC_", "FW_", "PC_", "PW_"].iter().find_map(|p| name.strip_prefix(*p)).unwrap_or(name)
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
    let nearest = ["pc_", "pw_", "PC_", "PW_"].iter().any(|p| name.starts_with(p));
    let repeat = !["fc_", "pc_", "FC_", "PC_"].iter().any(|p| name.starts_with(p));
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

pub const BLUR_FRAGMENT: &str = "#version 300 es\nprecision highp float; uniform sampler2D previousFrame; uniform vec2 blurStep; uniform vec2 sourceTexel; uniform vec2 blurTransform; uniform vec4 blurWeights; uniform vec4 blurOffsets; uniform float blurWeightScale; uniform vec4 blurEdge; uniform float blurHorizontal; in vec2 uv; out vec4 outColor; void main(){bool horizontal=blurHorizontal>0.5;vec2 base=uv+sourceTexel*(horizontal?vec2(1.0,1.0):vec2(1.0,0.0));vec3 c;if(horizontal){c=((texture(previousFrame,base+blurStep*blurOffsets.x).rgb+texture(previousFrame,base-blurStep*blurOffsets.x).rgb)*blurWeights.x)+((texture(previousFrame,base+blurStep*blurOffsets.y).rgb+texture(previousFrame,base-blurStep*blurOffsets.y).rgb)*blurWeights.y)+((texture(previousFrame,base+blurStep*blurOffsets.z).rgb+texture(previousFrame,base-blurStep*blurOffsets.z).rgb)*blurWeights.z)+((texture(previousFrame,base+blurStep*blurOffsets.w).rgb+texture(previousFrame,base-blurStep*blurOffsets.w).rgb)*blurWeights.w);c*=blurWeightScale;c=c*blurTransform.x+blurTransform.y;}else{c=((texture(previousFrame,base+blurStep*blurOffsets.x).rgb+texture(previousFrame,base-blurStep*blurOffsets.x).rgb)*blurWeights.x)+((texture(previousFrame,base+blurStep*blurOffsets.y).rgb+texture(previousFrame,base-blurStep*blurOffsets.y).rgb)*blurWeights.y);c*=blurWeightScale;float edge=min(min(uv.x,uv.y),1.0-max(uv.x,uv.y));edge=sqrt(max(edge,0.0));edge=blurEdge.x+blurEdge.y*clamp(edge*blurEdge.z,0.0,1.0);c*=edge;}outColor=vec4(c,1.0); }";

pub const SCALAR_UNIFORMS: &[&str] = &["frame", "fps", "progress", "decay", "vol", "vol_att", "blur1_edge_darken",
    "blur1_min", "blur1_max", "blur2_min", "blur2_max", "blur3_min", "blur3_max"];
pub const VECTOR_UNIFORMS: &[&str] = &["rand_frame", "rand_preset", "roam_sin", "roam_cos", "slow_roam_sin", "slow_roam_cos",
    "hue_corner1", "hue_corner2", "hue_corner3", "hue_corner4",
    "_c0", "_c1", "_c2", "_c3", "_c4", "_c5", "_c6", "_c7", "_c8", "_c9", "_c10", "_c11", "_c12", "_c13",
    "texsize_noise_lq", "texsize_noise_mq", "texsize_noise_hq", "texsize_noise_lq_lite", "texsize_noisevol_lq", "texsize_noisevol_hq",
    "_qa", "_qb", "_qc", "_qd", "_qe", "_qf", "_qg", "_qh", "_qi", "_qj", "_qk", "_ql", "_qm", "_qn", "_qo", "_qp"];

pub fn frame_values(scope: &std::collections::BTreeMap<String, crate::MilkRustValue>, title: &str) -> std::collections::BTreeMap<String, f64> {
    let mut values = std::collections::BTreeMap::new();
    for name in SCALAR_UNIFORMS.iter().copied().chain(["bass_att", "mid_att", "treb_att"]) {
        let fallback = if name == "fps" { 60.0 } else if name.ends_with("_max") || name == "decay" { 1.0 } else { 0.0 };
        let value = match scope.get(name) { Some(crate::MilkRustValue::Number(v)) if v.is_finite() => *v, _ => fallback };
        values.insert(name.to_string(), value);
    }
    for (name, fallback) in crate::classic::VALUES {
        let value = crate::milkrust_scope_number(scope, name, *fallback);
        values.insert((*name).into(), if value.is_finite() { value } else { *fallback });
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
        let (rates, phases) = if name.starts_with("slow") {
            ([0.005, 0.0085, 0.0133, 0.0217], [2.7, 5.3, 4.5, 3.8])
        } else { ([0.329, 1.293, 5.070, 20.051], [1.2, 3.9, 2.5, 5.4]) };
        let values = std::array::from_fn(|i| {
            let phase = time * rates[i] + phases[i];
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
    let value = |name: &str, fallback: f32| frame.shader_values.get(name).map_or(fallback, |v| *v as f32);
    let ranges = blur_ranges(frame);
    let texsize = [width as f32, height as f32, 1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32];
    result.extend([
        ("_c0", [x, y, 1.0/x, 1.0/y]), ("_c1", [0.0; 4]),
        ("_c2", [time.rem_euclid(10000.0) as f32, value("fps", 60.0), value("frame", 0.0), value("progress", 0.0)]),
        ("_c3", [frame.bass as f32, frame.mid as f32, frame.treble as f32, value("vol", 0.0)]),
        ("_c4", [value("bass_att", 0.0), value("mid_att", 0.0), value("treb_att", 0.0), value("vol_att", 0.0)]),
        ("_c5", [ranges[0][1]-ranges[0][0], ranges[0][0], ranges[1][1]-ranges[1][0], ranges[1][0]]),
        ("_c6", [ranges[2][1]-ranges[2][0], ranges[2][0], ranges[0][0], ranges[0][1]]),
        ("_c7", texsize),
        ("_c12", [(width.max(1) as f32).log2(), (height.max(1) as f32).log2(),
            ((width.max(1) as f32).log2() + (height.max(1) as f32).log2()) * 0.5, 0.0]),
        ("_c13", [ranges[1][0], ranges[1][1], ranges[2][0], ranges[2][1]]),
    ]);
    for (target, source) in [("_c8", "roam_cos"), ("_c9", "roam_sin"), ("_c10", "slow_roam_cos"), ("_c11", "slow_roam_sin")] {
        if let Some((_, v)) = result.iter().find(|(name, _)| *name == source) { let v = *v; result.push((target, v)); }
    }
    let random = result.iter().find(|(name, _)| *name == "rand_preset").map_or([0.0;4], |(_, v)| *v);
    for (corner, name) in ["hue_corner1", "hue_corner2", "hue_corner3", "hue_corner4"].iter().enumerate() {
        let channels: [f32; 3] = std::array::from_fn(|i| {
            let phase=time*[0.429,0.321,0.387][i]+[3.0,1.0,6.0][i]+corner as f64*[21.0,13.0,9.0][i]+f64::from(random[[3,1,2][i]])*std::f64::consts::TAU;
            (0.6+0.3*phase.sin()) as f32
        });
        let peak=channels.iter().copied().fold(0.0f32,f32::max);
        result.push((*name,[0.5+0.5*channels[0]/peak,0.5+0.5*channels[1]/peak,0.5+0.5*channels[2]/peak,1.0]));
    }
    result
}

pub const MATRIX_UNIFORMS: &[&str] = &[
    "rot_s1", "rot_s2", "rot_s3", "rot_s4", "rot_d1", "rot_d2", "rot_d3", "rot_d4",
    "rot_f1", "rot_f2", "rot_f3", "rot_f4", "rot_vf1", "rot_vf2", "rot_vf3", "rot_vf4",
    "rot_uf1", "rot_uf2", "rot_uf3", "rot_uf4", "rot_rand1", "rot_rand2", "rot_rand3", "rot_rand4",
];

/// Column-major GLSL mat3x4 payloads represent HLSL float4x3. In particular,
/// translation is in the fourth ROW, not the last GLSL column.
pub fn uniform_matrices(frame: &crate::MilkRustFrame, time: f64) -> Vec<(&'static str, [f32; 12])> {
    let preset_seed = frame.shader_values.get("preset_seed").copied().unwrap_or(1.0) as u32;
    MATRIX_UNIFORMS.iter().enumerate().map(|(index, &name)| {
        let mut seed = preset_seed ^ (index as u32 + 1).wrapping_mul(0x9e37_79b9);
        if index >= 20 { seed ^= (frame.shader_values.get("frame").copied().unwrap_or(time * 60.0) as u32).wrapping_mul(0x85eb_ca6b); }
        let mut random = || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); f64::from(seed >> 8) / 16777216.0 };
        let translation: [f64; 3] = std::array::from_fn(|_| if index >= 20 { random() } else { random() * 2.0 - 1.0 });
        let rates = if index >= 20 { 0.0 } else { 0.9 * (index as f64 / 8.0).powf(3.2) };
        let angles: [f64; 3] = std::array::from_fn(|_| random() * std::f64::consts::TAU + (random()*2.0-1.0) * rates * time);
        let [(sx, cx), (sy, cy), (sz, cz)] = angles.map(f64::sin_cos);
        // Row-vector affine composition Rx * T * Rz * Ry, as in MilkDrop.
        let rx = [[1.0,0.0,0.0,0.0], [0.0,cx,sx,0.0], [0.0,-sx,cx,0.0], [0.0,0.0,0.0,1.0]];
        let t = [[1.0,0.0,0.0,0.0], [0.0,1.0,0.0,0.0], [0.0,0.0,1.0,0.0], [translation[0],translation[1],translation[2],1.0]];
        let rz = [[cz,sz,0.0,0.0], [-sz,cz,0.0,0.0], [0.0,0.0,1.0,0.0], [0.0,0.0,0.0,1.0]];
        let ry = [[cy,0.0,-sy,0.0], [0.0,1.0,0.0,0.0], [sy,0.0,cy,0.0], [0.0,0.0,0.0,1.0]];
        let multiply = |a: [[f64;4];4], b: [[f64;4];4]| -> [[f64;4];4] {
            std::array::from_fn(|r| std::array::from_fn(|c| (0..4).map(|k| a[r][k]*b[k][c]).sum()))
        };
        let matrix = multiply(multiply(multiply(rx, t), rz), ry);
        (name, std::array::from_fn(|i| matrix[i%4][i/4] as f32))
    }).collect()
}

/// Keep each range nested and nondegenerate; the same values must drive the
/// blur writer and GetBlur reader, including presets with reversed bounds.
pub fn blur_ranges(frame: &crate::MilkRustFrame) -> [[f32; 2]; 3] {
    let mut ranges = [[0.0, 1.0]; 3];
    for index in 0..3 {
        let read = |suffix: &str, fallback: f64| frame.shader_values.get(&format!("blur{}_{suffix}", index+1))
            .filter(|v| v.is_finite()).copied().unwrap_or(fallback).clamp(-1.0e6, 1.0e6) as f32;
        let (mut low, mut high) = (read("min", 0.0), read("max", 1.0));
        if index > 0 { low = low.max(ranges[index-1][0]); high = high.min(ranges[index-1][1]); }
        if high-low < 0.1 {
            let center = (low+high)*0.5;
            low = center-0.05; high = center+0.05;
            if index > 0 {
                let span = (ranges[index-1][1]-ranges[index-1][0]).min(0.1);
                low = low.clamp(ranges[index-1][0], (ranges[index-1][1]-span).max(ranges[index-1][0]));
                high = low+span;
            }
        }
        ranges[index] = [low, high];
    }
    ranges
}

/// Normalize at the horizontal pass only. Subsequent levels first undo the
/// previous level's encoding. Vertical passes preserve the encoded range.
pub fn blur_transform(frame: &crate::MilkRustFrame, level: usize, pass: usize) -> [f32; 2] {
    if pass != 0 { return [1.0, 0.0]; }
    let ranges = blur_ranges(frame);
    let [low, high] = ranges[level];
    let [previous_low, previous_high] = if level == 0 { [0.0, 1.0] } else { ranges[level-1] };
    [(previous_high-previous_low)/(high-low), (previous_low-low)/(high-low)]
}


/// Return the optimized separable blur kernel used by MilkDrop's two passes.
/// The values preserve its eight source weights and paired-tap offsets.
pub fn blur_kernel(pass: usize) -> ([f32; 4], [f32; 4], f32) {
    let source = [4.0f32, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3];
    if pass == 0 {
        let weights = [source[0] + source[1], source[2] + source[3],
            source[4] + source[5], source[6] + source[7]];
        let offsets = [
            2.0 * source[1] / weights[0],
            2.0 + 2.0 * source[3] / weights[1],
            4.0 + 2.0 * source[5] / weights[2],
            6.0 + 2.0 * source[7] / weights[3],
        ];
        let normalization = 0.5 / weights.iter().sum::<f32>();
        (weights, offsets, normalization)
    } else {
        let first = source[..4].iter().sum::<f32>();
        let second = source[4..].iter().sum::<f32>();
        let offsets = [
            2.0 * (source[2] + source[3]) / first,
            2.0 + 2.0 * (source[6] + source[7]) / second,
            0.0,
            0.0,
        ];
        let normalization = 1.0 / ((first + second) * 2.0);
        ([first, second, 0.0, 0.0], offsets, normalization)
    }
}

/// MilkDrop darkens the first vertical blur pass only.
pub fn blur_edge(frame: &crate::MilkRustFrame, level: usize, pass: usize) -> [f32; 4] {
    if level != 0 || pass != 1 { return [1.0, 0.0, 5.0, 0.0]; }
    let edge = frame.shader_values.get("blur1_edge_darken").copied()
        .filter(|value| value.is_finite()).unwrap_or(0.0).clamp(-1.0e4, 1.0e4) as f32;
    [1.0 - edge, edge, 5.0, 0.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrices_preserve_basis_length_and_affine_layout() {
        let frame = crate::MilkRustFrame::default();
        let matrices = uniform_matrices(&frame, 2.0);
        assert_eq!(matrices.len(), 24);
        assert_eq!(matrices[0].1, uniform_matrices(&frame, 20.0)[0].1);
        assert_ne!(matrices[8].1, uniform_matrices(&frame, 20.0)[8].1);
        for (_, m) in matrices {
            assert!(m.iter().all(|v| v.is_finite()));
            for col in 0..3 {
                let norm: f32 = (0..3).map(|row| m[col*4+row].powi(2)).sum();
                assert!((norm-1.0).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn blur_encoding_round_trips_and_handles_reversed_bounds() {
        let mut frame = crate::MilkRustFrame::default();
        frame.shader_values.insert("blur1_min".into(), 0.2);
        frame.shader_values.insert("blur1_max".into(), 0.8);
        let [scale, bias] = blur_transform(&frame, 0, 0);
        let [low, high] = blur_ranges(&frame)[0];
        assert!(((0.4*scale+bias)*(high-low)+low-0.4).abs() < 1e-6);
        frame.shader_values.insert("blur2_min".into(), 500.0);
        frame.shader_values.insert("blur2_max".into(), -500.0);
        for [low, high] in blur_ranges(&frame) { assert!(low.is_finite() && high > low); }
        assert_eq!(blur_transform(&frame, 2, 1), [1.0, 0.0]);
    }
    #[test]
    fn aliases_and_noise_are_deterministic() {
        assert_eq!(texture_kind("sampler_pw_noisevol_hq"), TextureKind::Noise3d(6));
        assert_eq!(sampler_state("sampler_pw_main"), (true, true));
        assert_eq!(sampler_state("sampler_fc_blur1"), (false, false));
        assert_eq!(noise_texture(1, false), noise_texture(1, false));
        assert_ne!(noise_texture(1, false).3, noise_texture(2, false).3);
    }
}
