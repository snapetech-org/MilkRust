//! Legacy feedback coordinate transform and unique-vertex equation evaluation.
use std::collections::BTreeMap;
use crate::{MilkRustValue, MilkRustWarpMesh, evaluate_milkrust_equations};

fn number(scope: &BTreeMap<String, MilkRustValue>, key: &str, fallback: f64) -> Result<f64, String> {
    let value = scope.get(key).and_then(MilkRustValue::as_number).unwrap_or(fallback);
    if value.is_finite() { Ok(value) } else { Err(format!("Non-finite warp variable: {key}")) }
}
fn divisor(value: f64) -> f64 {
    if value < 0.0 { value.min(-0.000001) } else { value.max(0.000001) }
}

pub fn mesh(scope: &BTreeMap<String, MilkRustValue>, equations: &str) -> Result<MilkRustWarpMesh, String> {
    let columns = number(scope, "meshx", 48.0)?.floor().clamp(1.0, 128.0) as usize;
    let rows = number(scope, "meshy", 36.0)?.floor().clamp(1.0, 128.0) as usize;
    let aspect = [1.0 / number(scope, "aspectx", 1.0)?.abs().max(0.001),
        1.0 / number(scope, "aspecty", 1.0)?.abs().max(0.001)];
    let corner_radius = aspect[0].hypot(aspect[1]);
    let time = number(scope, "time", 0.0)? * number(scope, "warpanimspeed", 1.0)?;
    let scale = divisor(number(scope, "warpscale", 1.0)?);
    let frequencies = [(11.68, 4.0, 1.413, 10.0), (8.77, 3.0, 1.113, 7.0),
        (10.54, 3.0, 1.233, 3.0), (11.49, 4.0, 0.933, 5.0)]
        .map(|(base, amplitude, speed, phase)| base + amplitude * (time * speed + phase).cos());
    let mut points = Vec::with_capacity((columns + 1) * (rows + 1));
    let mut coordinates = Vec::with_capacity(points.capacity());
    for row in 0..=rows {
        for column in 0..=columns {
            milkrust_expr::limits::check_frame_budget()?;
            let position = [column as f64 * 2.0 / columns as f64 - 1.0,
                row as f64 * 2.0 / rows as f64 - 1.0];
            let math = [position[0] * aspect[0], -position[1] * aspect[1]];
            let radius = math[0].hypot(math[1]) / corner_radius;
            let evaluated;
            let values = if equations.is_empty() { scope } else {
                let mut inputs = scope.clone();
                for (key, value) in [("x", math[0] * 0.5 + 0.5), ("y", math[1] * 0.5 + 0.5),
                    ("rad", radius), ("ang", math[1].atan2(math[0]).rem_euclid(std::f64::consts::TAU))] {
                    inputs.insert(key.into(), MilkRustValue::Number(value));
                }
                evaluated = evaluate_milkrust_equations(equations, &inputs)?;
                &evaluated
            };
            let zoom = number(values, "zoom", 1.0)?.abs().max(0.000001);
            let exponent = number(values, "zoomexp", 1.0)?.abs().max(0.000001).powf(radius * 2.0 - 1.0);
            // Clamp the logarithm before exponentiation, keeping hostile but
            // finite equations from producing NaN geometry or enormous UVs.
            let zoom_inverse = (-zoom.ln() * exponent).clamp(-30.0, 30.0).exp();
            let center = [number(values, "cx", 0.5)?, number(values, "cy", 0.5)?];
            let stretch = [divisor(number(values, "sx", 1.0)?), divisor(number(values, "sy", 1.0)?)];
            let mut uv = [0.0; 2];
            for axis in 0..2 { uv[axis] = (math[axis] * 0.5 * zoom_inverse + 0.5 - center[axis]) / stretch[axis] + center[axis]; }
            let warp = number(values, "warp", 0.0)?.clamp(-1.0e6, 1.0e6) * 0.0035;
            let [x, y] = position;
            let [a, b, c, d] = frequencies;
            uv[0] += warp * ((time * 0.333 + (x * a - y * d) / scale).sin()
                + (time * 0.753 - (x * b - y * c) / scale).cos());
            uv[1] += warp * ((time * 0.375 - (x * c + y * b) / scale).cos()
                + (time * 0.825 + (x * a + y * d) / scale).sin());
            let (sin, cos) = number(values, "rot", 0.0)?.sin_cos();
            let local = [uv[0] - center[0], uv[1] - center[1]];
            uv[0] = local[0] * cos - local[1] * sin + center[0] - number(values, "dx", 0.0)?;
            uv[1] = local[0] * sin + local[1] * cos + center[1] - number(values, "dy", 0.0)?;
            uv[0] = (uv[0] - 0.5) / aspect[0] + 0.5;
            uv[1] = 0.5 - (uv[1] - 0.5) / aspect[1]; // D3D math to bottom-up GL
            if uv.iter().any(|value| !value.is_finite() || value.abs() > 1.0e12) { return Err("Warp coordinates exceed the finite geometry limit".into()); }
            points.push(position);
            coordinates.push(uv);
        }
    }
    let mut mesh = MilkRustWarpMesh { positions: Vec::with_capacity(columns * rows * 12), source_uvs: Vec::with_capacity(columns * rows * 12) };
    for row in 0..rows {
        for column in 0..columns {
            let corner = row * (columns + 1) + column;
            for index in [corner, corner + columns + 1, corner + 1, corner + 1, corner + columns + 1, corner + columns + 2] {
                mesh.positions.extend_from_slice(&points[index]);
                mesh.source_uvs.extend_from_slice(&coordinates[index]);
            }
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(values: &[(&str, f64)]) -> BTreeMap<String, MilkRustValue> {
        [("meshx", 2.0), ("meshy", 2.0)].into_iter().chain(values.iter().copied())
            .map(|(k,v)| (k.into(), MilkRustValue::Number(v))).collect()
    }
    #[test]
    fn identity_retains_orientation_on_wide_and_tall_viewports() {
        for aspect in [[1.0, 2.0], [2.0, 1.0], [1.0, 1.0]] {
            let mesh = mesh(&scope(&[("aspectx", aspect[0]), ("aspecty", aspect[1])]), "").unwrap();
            for (position, uv) in mesh.positions.iter().zip(&mesh.source_uvs) { assert!(((position + 1.0)*0.5 - uv).abs() < 1.0e-12); }
        }
    }
    #[test]
    fn translation_stretch_center_and_animated_warp_change_sampling() {
        let shifted = mesh(&scope(&[("dx", 0.1), ("dy", 0.2)]), "").unwrap();
        assert!((shifted.source_uvs[0] + 0.1).abs() < 1.0e-12);
        assert!((shifted.source_uvs[1] - 0.2).abs() < 1.0e-12);
        let stretched = mesh(&scope(&[("sx", 2.0), ("cx", 0.25)]), "").unwrap();
        assert!((stretched.source_uvs[0] - 0.125).abs() < 1.0e-12);
        let warped = mesh(&scope(&[("warp", 2.0), ("time", 1.0)]), "").unwrap();
        assert_ne!(warped.source_uvs, mesh(&scope(&[]), "").unwrap().source_uvs);
    }
    #[test]
    fn nonfinite_equation_outputs_are_rejected_before_geometry() {
        assert!(mesh(&scope(&[("dx", f64::NAN)]), "").is_err());
        assert!(mesh(&scope(&[("zoom", f64::INFINITY)]), "").is_err());
    }
    #[test]
    fn equations_receive_downward_y_and_corner_normalized_radius() {
        let transformed = mesh(&scope(&[]), "dx=x*0.1;dy=y*0.2;zoom=1+rad;").unwrap();
        assert!(transformed.source_uvs.iter().all(|value| value.is_finite()));
        assert_ne!(transformed.source_uvs, mesh(&scope(&[]), "").unwrap().source_uvs);
        assert!(mesh(&scope(&[]), "unknown_function();").is_err());
    }
}
