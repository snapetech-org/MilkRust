//! Bounded classic/custom wave geometry shared by both GPU hosts. Coordinates
//! are GL clip space; custom-wave EEL uses downward y, the classic wave_y
//! control retains its historical upward direction.
use std::collections::BTreeMap;
use crate::{audio::Stereo, MilkRustIndexedEntry, MilkRustPrimitive, MilkRustPrimitiveMode,
    MilkRustValue, evaluate_milkrust_equations, milkrust_entry_flag as flag,
    milkrust_entry_number as entry, milkrust_scope_number as number};

const AUDIO: usize = 512;
const VISIBLE: usize = 480; // Leave 32 samples for the phase-offset modes.
type Scope = BTreeMap<String, MilkRustValue>;

fn sample(source: &[f64], position: f64) -> f64 {
    if source.is_empty() { return 0.0; }
    let position = position.clamp(0.0, source.len().saturating_sub(1) as f64);
    let index = position as usize;
    let a = source[index];
    let b = source[(index + 1).min(source.len() - 1)];
    let value = a + (b - a) * position.fract();
    if value.is_finite() { value.clamp(-1.0e6, 1.0e6) } else { 0.0 }
}

fn resample(source: &[f64], count: usize) -> Vec<f64> {
    (0..count).map(|i| sample(source, i as f64 * source.len().saturating_sub(1) as f64
        / count.saturating_sub(1).max(1) as f64)).collect()
}

fn smooth_audio(values: &mut [f64], mix: f64, backwards: bool) {
    for i in 1..values.len() { values[i] = values[i] * (1.0 - mix) + values[i - 1] * mix; }
    if backwards {
        for i in (0..values.len().saturating_sub(1)).rev() {
            values[i] = values[i] * (1.0 - mix) + values[i + 1] * mix;
        }
    }
}

fn dimensions(scope: &Scope) -> [f64; 2] {
    [number(scope, "pixelsx", 512.0).clamp(1.0, 4096.0),
        number(scope, "pixelsy", 512.0).clamp(1.0, 4096.0)]
}

/// MilkDrop's four-tap interpolation adds a midpoint between adjacent vertices.
/// The channels stay separate, so stereo modes never acquire a connecting line.
fn smooth_path(vertices: &[f64], colors: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let count = vertices.len() / 2;
    if count < 2 { return (vertices.to_vec(), colors.to_vec()); }
    let mut out = Vec::with_capacity(count * 4 - 2);
    let mut rgba = Vec::with_capacity(colors.len() * 2);
    for i in 0..count - 1 {
        out.extend_from_slice(&vertices[i * 2..i * 2 + 2]);
        for axis in 0..2 {
            out.push((-0.15 * vertices[i.saturating_sub(1) * 2 + axis]
                + 1.15 * vertices[i * 2 + axis] + 1.15 * vertices[(i + 1) * 2 + axis]
                - 0.15 * vertices[(i + 2).min(count - 1) * 2 + axis]) * 0.5);
        }
        if colors.len() == count * 4 {
            rgba.extend_from_slice(&colors[i * 4..i * 4 + 4]);
            rgba.extend_from_slice(&colors[i * 4..i * 4 + 4]);
        }
    }
    out.extend_from_slice(&vertices[(count - 1) * 2..]);
    if colors.len() == count * 4 { rgba.extend_from_slice(&colors[(count - 1) * 4..]); }
    (out, rgba)
}

/// Portable pixel widths: dots become quads, thick lines use four offset draws.
/// This avoids relying on implementation-dependent GLES line/point sizes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stroke(vertices: Vec<f64>, colors: Vec<f64>, color: [f64; 4],
    dots: bool, thick: bool, additive: bool, interpolate: bool, scope: &Scope) -> Vec<MilkRustPrimitive> {
    if vertices.len() < if dots { 2 } else { 4 } { return Vec::new(); }
    let [width, height] = dimensions(scope);
    let (vertices, colors) = if !dots && interpolate { smooth_path(&vertices, &colors) } else { (vertices, colors) };
    if dots {
        let size = if width >= 1024.0 { 2.0 } else { 1.0 } + if thick { 1.0 } else { 0.0 };
        let mut quads = Vec::with_capacity(vertices.len() * 6);
        let mut rgba = Vec::with_capacity(colors.len() * 6);
        for (i, point) in vertices.chunks_exact(2).enumerate() {
            for [x, y] in [[-1.0,-1.0], [1.0,-1.0], [1.0,1.0], [-1.0,-1.0], [1.0,1.0], [-1.0,1.0]] {
                quads.extend_from_slice(&[point[0] + x * size / width, point[1] + y * size / height]);
                if colors.len() == vertices.len() * 2 { rgba.extend_from_slice(&colors[i * 4..i * 4 + 4]); }
            }
        }
        return vec![MilkRustPrimitive { additive, color, mode: MilkRustPrimitiveMode::Triangles,
            vertices: quads, vertex_colors: rgba }];
    }
    let offsets: &[[f64; 2]] = if thick { &[[0.0,0.0], [1.0,0.0], [1.0,1.0], [0.0,1.0]] } else { &[[0.0,0.0]] };
    offsets.iter().map(|offset| {
        let mut shifted = vertices.clone();
        for point in shifted.chunks_exact_mut(2) {
            point[0] += offset[0] * 2.0 / width;
            point[1] += offset[1] * 2.0 / height;
        }
        MilkRustPrimitive { additive, color, mode: MilkRustPrimitiveMode::LineStrip,
            vertices: shifted, vertex_colors: colors.clone() }
    }).collect()
}

fn classic_paths(wave: &[f64], spectrum: &[f64], stereo: Option<&Stereo>, scope: &Scope) -> Vec<Vec<f64>> {
    let left = stereo.map_or(wave, |v| v.left.as_slice());
    let right = stereo.map_or(wave, |v| v.right.as_slice());
    let mut left = resample(left, AUDIO);
    let mut right = resample(right, AUDIO);
    let scale = number(scope, "wave_scale", 1.0).clamp(-1000.0, 1000.0);
    let smoothing = number(scope, "wave_smoothing", 0.0).clamp(0.0, 1.0);
    for channel in [&mut left, &mut right] {
        for value in channel.iter_mut() { *value = value.clamp(-1.0, 1.0) * scale; }
        smooth_audio(channel, smoothing, false);
    }
    let mode = (number(scope, "wave_mode", 0.0) as i64).rem_euclid(9);
    let mut mystery = number(scope, "wave_mystery", 0.0).clamp(-1000.0, 1000.0);
    if matches!(mode, 0 | 1 | 4) && !(-1.0..=1.0).contains(&mystery) {
        mystery = (mystery * 0.5 + 0.5).rem_euclid(1.0) * 2.0 - 1.0;
    }
    let center = [number(scope, "wave_x", 0.5).clamp(-1000.0,1000.0) * 2.0 - 1.0,
        number(scope, "wave_y", 0.5).clamp(-1000.0,1000.0) * 2.0 - 1.0];
    let aspect = [1.0 / number(scope, "aspecty", 1.0).max(1.0),
        1.0 / number(scope, "aspectx", 1.0).max(1.0)];
    let time = number(scope, "time", 0.0);
    let width = dimensions(scope)[0];
    let count = match mode {
        0 | 1 => VISIBLE / 2,
        4 => VISIBLE.min((width / 3.0).max(2.0) as usize),
        6 | 7 => (VISIBLE / 2).min((width / 3.0).max(2.0) as usize),
        8 => 256,
        _ => VISIBLE,
    };
    let offset = (VISIBLE - count) / 2;
    let mut paths = vec![Vec::with_capacity((count + 1) * 2)];
    if mode == 7 { paths.push(Vec::with_capacity(count * 2)); }
    // Parametric line clipped to an expanded screen, preserving angle and
    // displacement while leaving room for the separated stereo channels.
    let angle = std::f64::consts::FRAC_PI_2 * mystery;
    let direction = [angle.cos(), angle.sin()];
    let normal = [-direction[1], direction[0]];
    let origin = [center[0] * normal[0], center[0] * normal[1]];
    let mut bounds: [f64; 2] = [-3.0, 3.0];
    for axis in 0..2 {
        if direction[axis].abs() > 1.0e-9 {
            let a = (-1.1 - origin[axis]) / direction[axis];
            let b = (1.1 - origin[axis]) / direction[axis];
            bounds[0] = bounds[0].max(a.min(b)); bounds[1] = bounds[1].min(a.max(b));
        }
    }
    for i in 0..count {
        let point = match mode {
            0 => {
                let mut radius = 0.5 + 0.4 * right[i + offset] + mystery;
                let overlap = (count / 10).max(1);
                if i < overlap {
                    let blend = 0.5 - 0.5 * (std::f64::consts::PI * i as f64 / overlap as f64).cos();
                    let tail = 0.5 + 0.4 * right[i + count + offset] + mystery;
                    radius = tail * (1.0 - blend) + radius * blend;
                }
                let angle = std::f64::consts::TAU * i as f64 / (count - 1) as f64 + time * 0.2;
                [radius * angle.cos() * aspect[0] + center[0], radius * angle.sin() * aspect[1] + center[1]]
            },
            1 => {
                let radius = 0.53 + 0.43 * right[i] + mystery;
                let angle = left[i + 32] * std::f64::consts::FRAC_PI_2 + time * 2.3;
                [radius * angle.cos() * aspect[0] + center[0], radius * angle.sin() * aspect[1] + center[1]]
            },
            2 | 3 => [right[i] * aspect[0] + center[0], left[i + 32] * aspect[1] + center[1]],
            4 => {
                let mut point = [-1.0 + 2.0 * i as f64 / count as f64 + center[0] + 0.44 * right[i + 25 + offset],
                    0.47 * left[i + offset] + center[1]];
                if i > 1 {
                    let momentum = 0.45 + 0.5 * (mystery * 0.5 + 0.5);
                    for axis in 0..2 {
                        point[axis] = point[axis] * (1.0 - momentum) + momentum
                            * (2.0 * paths[0][(i - 1) * 2 + axis] - paths[0][(i - 2) * 2 + axis]);
                    }
                }
                point
            },
            5 => {
                let x = right[i] * left[i + 32] + left[i] * right[i + 32];
                let y = right[i].powi(2) - left[i + 32].powi(2);
                let (sin, cos) = (time * 0.3).sin_cos();
                [(x * cos - y * sin) * aspect[0] + center[0], (x * sin + y * cos) * aspect[1] + center[1]]
            },
            _ => {
                let t = bounds[0] + (bounds[1] - bounds[0]) * i as f64 / count as f64;
                let base = [origin[0] + direction[0] * t, origin[1] + direction[1] * t];
                let separation = (center[1] * 0.5 + 0.5).powi(2);
                let amplitude = if mode == 8 {
                    let bins = stereo.map_or(spectrum, |v| v.spectrum_left.as_slice());
                    let energy = sample(bins, i as f64 * bins.len() as f64 / count as f64).max(1.0e-6);
                    energy.ln() * 0.1
                } else { left[i + offset] * 0.25 + if mode == 7 { separation } else { 0.0 } };
                if mode == 7 {
                    let second = right[i + offset] * 0.25 - separation;
                    paths[1].extend_from_slice(&[base[0] + normal[0] * second, base[1] + normal[1] * second]);
                }
                [base[0] + normal[0] * amplitude, base[1] + normal[1] * amplitude]
            },
        };
        paths[0].extend_from_slice(&point);
    }
    if mode == 0 { let first = [paths[0][0], paths[0][1]]; paths[0].extend_from_slice(&first); }
    paths
}

pub(crate) fn classic_vertices(samples: &[f64], scope: &Scope) -> Vec<f64> {
    if samples.len() < 2 { return Vec::new(); }
    classic_paths(samples, &[], None, scope).remove(0)
}

pub(crate) fn classic(wave: &[f64], spectrum: &[f64], stereo: Option<&Stereo>, scope: &Scope, rgb: [f64; 3]) -> Vec<MilkRustPrimitive> {
    let n = |key, fallback| number(scope, key, fallback);
    let mode = (n("wave_mode", 0.0) as i64).rem_euclid(9);
    let mut alpha = n("wave_a", 1.0);
    if n("wave_mod_alpha", n("modwavealphabyvolume", 0.0)) != 0.0 {
        let start = n("wave_mod_alpha_start", n("modwavealphastart", 0.0));
        let end = n("wave_mod_alpha_end", n("modwavealphaend", 1.0));
        let volume = (n("bass", 0.0) + n("mid", 0.0) + n("treb", 0.0)) / 3.0;
        alpha *= if (end - start).abs() < 1.0e-9 { if volume >= end { 1.0 } else { 0.0 } }
            else { ((volume - start) / (end - start)).clamp(0.0, 1.0) };
    }
    let width = dimensions(scope)[0];
    if mode == 1 { alpha *= 1.25; }
    if matches!(mode, 2 | 5) { alpha *= if width < 384.0 { 0.07 } else if width < 768.0 { 0.09 } else if width < 1536.0 { 0.11 } else { 0.13 }; }
    if mode == 3 {
        alpha *= if width < 384.0 { 0.075 } else if width < 768.0 { 0.15 } else if width < 1536.0 { 0.22 } else { 0.33 };
        alpha *= 1.3 * n("treb", 0.0).powi(2);
    }
    let mut color = [rgb[0], rgb[1], rgb[2], alpha.clamp(0.0, 1.0)];
    if color[3] == 0.0 { return Vec::new(); }
    if n("wave_brighten", n("maximizewavecolor", 0.0)) != 0.0 {
        let maximum = rgb.into_iter().fold(0.0_f64, f64::max);
        if maximum > 0.0 { for value in &mut color[..3] { *value /= maximum; } }
    }
    classic_paths(wave, spectrum, stereo, scope).into_iter().flat_map(|vertices| {
        stroke(vertices, Vec::new(), color, n("wave_usedots", n("wave_dots", 0.0)) != 0.0,
            n("wave_thick", 0.0) != 0.0, n("wave_additive", n("additivewave", 0.0)) != 0.0, true, scope)
    }).collect()
}

fn custom_geometry_with_state(wave: &MilkRustIndexedEntry, left: &[f64], right: &[f64], scope: &Scope, rgb: [f64; 3]) -> Result<(Vec<f64>, Vec<f64>, Scope), String> {
    let count = entry(wave, &["samples", "nsamples"], left.len() as f64).floor().clamp(0.0, AUDIO as f64) as usize;
    let spectrum = flag(wave, &["spectrum", "bspectrum"]);
    let separation = entry(wave, &["sep"], 0.0).clamp(-511.0, 511.0);
    let scaling = entry(wave, &["scaling"], 1.0).clamp(-1000.0, 1000.0)
        * number(scope, "wave_scale", 1.0).clamp(-1000.0, 1000.0) * if spectrum { 0.15 } else { 0.512 };
    let mut channels = [Vec::with_capacity(count), Vec::with_capacity(count)];
    for (channel, source) in [left, right].into_iter().enumerate() {
        let source = resample(source, AUDIO);
        let start = if spectrum { 0.0 } else { (AUDIO - count) as f64 * 0.5 + separation * if channel == 0 { -0.5 } else { 0.5 } };
        let step = if spectrum { (AUDIO as f64 - separation) / count.max(1) as f64 } else { 1.0 };
        for i in 0..count { channels[channel].push(sample(&source, start + i as f64 * step)); }
        smooth_audio(&mut channels[channel], (entry(wave, &["smoothing"], 0.0).clamp(0.0, 1.0) * 0.98).sqrt(), true);
        for value in &mut channels[channel] { *value *= scaling; }
    }
    let color = [entry(wave, &["r"], rgb[0]), entry(wave, &["g"], rgb[1]), entry(wave, &["b"], rgb[2]), entry(wave, &["a"], 1.0)];
    let mut point_scope = wave.point_values.clone();
    for values in [scope, &wave.runtime_values] {
        for (key,value) in values.iter().filter(|(key,_)| crate::is_milkrust_entry_input(key) ||
            key.strip_prefix('t').and_then(|v| v.parse::<usize>().ok()).is_some_and(|i| (1..=8).contains(&i))) {
            point_scope.insert(key.clone(),value.clone());
        }
    }
    let mut vertices = Vec::with_capacity(count * 2);
    let mut colors = Vec::with_capacity(count * 4);
    let aspect = [number(scope, "aspectx", 1.0).max(1.0), number(scope, "aspecty", 1.0).max(1.0)];
    for i in 0..count {
        milkrust_expr::limits::check_frame_budget()?;
        let position = i as f64 / count.saturating_sub(1).max(1) as f64;
        for (key, value) in [("sample", position), ("i", position), ("value1", channels[0][i]), ("value2", channels[1][i]),
            ("x", 0.5 + channels[0][i]), ("y", 0.5 + channels[1][i]), ("r", color[0]), ("g", color[1]), ("b", color[2]), ("a", color[3])] {
            point_scope.insert(key.into(), MilkRustValue::Number(value));
        }
        // Private temporaries/q/t survive between points; output controls reset
        // for each point. No point register writes escape to another wave.
        point_scope = evaluate_milkrust_equations(&wave.equations.point, &point_scope)?;
        let read = |key: &str| -> Result<f64, String> {
            let value = point_scope.get(key).and_then(MilkRustValue::as_number).unwrap_or(0.0);
            if value.is_finite() && value.abs() <= 1.0e9 { Ok(value) } else { Err(format!("Custom wave variable exceeds its finite limit: {key}")) }
        };
        vertices.extend_from_slice(&[(read("x")? * 2.0 - 1.0) * aspect[0], (1.0 - read("y")? * 2.0) * aspect[1]]);
        for key in ["r", "g", "b", "a"] { colors.push(read(key)?.clamp(0.0, 1.0)); }
    }
    Ok((vertices, colors, point_scope))
}

pub(crate) fn custom_geometry(wave: &MilkRustIndexedEntry, left: &[f64], right: &[f64], scope: &Scope, rgb: [f64;3]) -> Result<(Vec<f64>,Vec<f64>),String> {
    custom_geometry_with_state(wave,left,right,scope,rgb).map(|(vertices,colors,_)| (vertices,colors))
}

pub(crate) fn custom(wave: &mut MilkRustIndexedEntry, samples: &[f64], spectrum: &[f64], stereo: Option<&Stereo>, scope: &mut Scope, rgb: [f64; 3]) -> Vec<MilkRustPrimitive> {
    let spectral = flag(wave, &["spectrum", "bspectrum"]);
    let mono = if spectral { spectrum } else { samples };
    let left = stereo.map_or(mono, |s| if spectral { s.spectrum_left.as_slice() } else { s.left.as_slice() });
    let right = stereo.map_or(mono, |s| if spectral { s.spectrum_right.as_slice() } else { s.right.as_slice() });
    match custom_geometry_with_state(wave, left, right, scope, rgb) {
        Ok((vertices, colors, values)) => {
            crate::merge_milkrust_global_memory(scope, &values);
            wave.point_values = values;
            stroke(vertices, colors, [1.0; 4], flag(wave, &["usedots", "dots", "busedots"]),
                flag(wave, &["thick", "bdrawthick", "bthick"]), flag(wave, &["additive", "badditive"]), true, scope)
        },
        Err(error) => { milkrust_expr::limits::record_frame_error(&error); Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(values: &[(&str, f64)]) -> Scope { values.iter().map(|(k,v)| (k.to_string(), MilkRustValue::Number(*v))).collect() }
    #[test]
    fn classic_modes_and_stereo_break_have_distinct_bounded_geometry() {
        let wave = (0..512).map(|i| (i as f64 * 0.07).sin()).collect::<Vec<_>>();
        let paths = (0..9).map(|mode| classic_paths(&wave, &[0.4;256], None,
            &scope(&[("wave_mode", mode as f64), ("time", 2.0)]))).collect::<Vec<_>>();
        for path in &paths { assert!(path.iter().flatten().all(|v| v.is_finite())); }
        assert_eq!(paths[7].len(), 2);
        assert_ne!(paths[0], paths[1]); assert_ne!(paths[4], paths[5]);
        let circle = classic_paths(&[0.0;512], &[], None, &Scope::new());
        assert_eq!(&circle[0][..2], &circle[0][circle[0].len()-2..]);
    }
    #[test]
    fn custom_points_receive_position_stereo_colors_and_bounded_counts() {
        let mut parsed = crate::parse_milkrust_preset_set("[preset00]\nwavecode_0_enabled=1\nwavecode_0_samples=3\nwave_0_per_point1=x=sample;y=0.5+value1-value2;r=sample;\n", false);
        let wave = &mut parsed.presets[0].waves[0];
        let (vertices, colors) = custom_geometry(wave, &[0.25;512], &[-0.25;512], &Scope::new(), [1.0;3]).unwrap();
        assert_eq!(vertices[0], -1.0); assert_eq!(vertices[4], 1.0);
        assert!((vertices[1] + 0.512).abs() < 1.0e-9);
        assert_eq!(colors[0], 0.0); assert_eq!(colors[8], 1.0);
        wave.base_values.insert("samples".into(), MilkRustValue::Number(1.0e100));
        assert_eq!(custom_geometry(wave, &[], &[], &Scope::new(), [1.0;3]).unwrap().0.len(), 1024);
        wave.equations.point = "undefined_function();".into();
        assert!(custom_geometry(wave, &[], &[], &Scope::new(), [1.0;3]).is_err());
    }
    #[test]
    fn single_dots_and_thick_lines_use_portable_geometry() {
        let dot = stroke(vec![0.0,0.0], vec![0.2,0.3,0.4,0.5], [1.0;4], true, true, true, false, &Scope::new());
        assert_eq!(dot[0].vertices.len(), 12); assert_eq!(dot[0].vertex_colors.len(), 24);
        let line = stroke(vec![0.0,0.0,1.0,1.0], vec![], [1.0;4], false, true, false, false, &Scope::new());
        assert_eq!(line.len(), 4); assert_ne!(line[0].vertices, line[1].vertices);
    }
}
