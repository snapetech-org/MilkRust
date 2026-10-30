//! Saved MilkDrop 3 blend-mask generators.
//!
//! The `(a,c)` pattern formulas below are a clean-room port of the published
//! behavioral implementation in M0Rf30/lactea, `md3_blend.rs` and
//! `blend_patterns.rs` (MIT; attribution and license text are in
//! `crates/milkrust-core/THIRD_PARTY_NOTICES.md`). Packed vertex upload and GPU
//! triangle interpolation preserve the documented 64x48 blend mesh.

use crate::milk2::TransitionMaskKey;

const GRID_X: usize = crate::milk2::TRANSITION_GRID_WIDTH as usize - 1;
const GRID_Y: usize = crate::milk2::TRANSITION_GRID_HEIGHT as usize - 1;
const MAX_TARGET_PIXELS: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Pattern {
    Zoom,
    Side,
    Plasma,
    Plasma2,
    Plasma3,
    Cercle,
    Square,
    Snail,
    Snail2,
    Snail3,
    Triangle,
    Donuts,
    Corner,
    Patches,
    Checkerboard,
    Bubbles,
    Stars,
    Stars2,
    Clock,
    Nuclear,
    Arrow,
    Cisor,
    Wave,
    Curtain,
    Vertical,
    Horizontal,
    LinesVertical,
    LinesHorizontal,
    Cross,
    Cross2,
}

impl Pattern {
    fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            1 => Self::Zoom,
            2 => Self::Side,
            3 => Self::Plasma,
            4 => Self::Plasma2,
            5 => Self::Plasma3,
            6 => Self::Cercle,
            7 => Self::Square,
            8 => Self::Snail,
            9 => Self::Snail2,
            10 => Self::Snail3,
            11 => Self::Triangle,
            12 => Self::Donuts,
            13 => Self::Corner,
            14 => Self::Patches,
            15 => Self::Checkerboard,
            16 => Self::Bubbles,
            17 => Self::Stars,
            18 => Self::Stars2,
            19 => Self::Clock,
            20 => Self::Nuclear,
            21 => Self::Arrow,
            22 => Self::Cisor,
            23 => Self::Wave,
            24 => Self::Curtain,
            25 => Self::Vertical,
            26 => Self::Horizontal,
            27 => Self::LinesVertical,
            28 => Self::LinesHorizontal,
            29 => Self::Cross,
            30 => Self::Cross2,
            _ => return None,
        })
    }
}

/// Compute the per-vertex `(a,c)` field from MilkDrop 3's saved pattern.
fn fill_vertices(gx: usize, gy: usize, pattern: Pattern, direction: i32, random: [f32; 5], aspect: (f32, f32), out: &mut [[f32; 2]]) {
    let width = gx + 1;
    let (ax, ay) = aspect;
    let (gxf, gyf) = (gx as f32, gy as f32);
    // Missing saved keys use the carried header values, which are zero at
    // process start. Partial blocks have already inherited their other slots.
    let [r1, r2, r3, r4, _r5] = random;
    let flip = direction == -1;
    let sqrt2 = 2.0f32.sqrt();
    let band = r1 * 0.14 + 0.02 + r2 * 0.34;

    let each = |out: &mut [[f32; 2]], f: &dyn Fn(f32, f32) -> [f32; 2]| {
        for y in 0..=gy {
            for x in 0..=gx {
                out[y * width + x] = f(x as f32 / gxf, y as f32 / gyf);
            }
        }
    };
    let banded = |out: &mut [[f32; 2]], width: f32, offset: f32, t: &dyn Fn(f32, f32) -> f32| {
        let inverse = 1.0 / width;
        each(out, &|x, y| band_entry(inverse, width + offset, t(x, y)));
    };
    let centered = |x: f32, y: f32| ((x - 0.5) * ax, (y - 0.5) * ay);
    let radius = |dx: f32, dy: f32| (dx * dx + dy * dy).sqrt();
    let flipped = |value: f32| if flip { 1.0 - value } else { value };
    let wipe = |u: f32, v: f32, vx: f32, vy: f32| {
        (((u - 0.5) * vx + (v - 0.5) * vy + 0.5) - 0.5) / sqrt2 + 0.5
    };

    match pattern {
        Pattern::Zoom => out.fill([1.0, 0.0]),
        Pattern::Side => {
            let angle = r1 * 6.28;
            let (vx, vy) = (angle.cos(), angle.sin());
            let width = 0.1 + 0.2 * r2;
            let inverse = 1.0 / width;
            each(out, &|x, y| band_entry(inverse, 1.0 + width, wipe(x * ax, y * ay, vx, vy)));
        }
        Pattern::Plasma | Pattern::Plasma2 | Pattern::Plasma3 => {
            let (width, dt) = match pattern {
                Pattern::Plasma => (0.12 + 0.13 * r1, 0.25),
                Pattern::Plasma2 => (0.25 + 0.15 * r1, 0.25),
                _ => (0.25 + 0.15 * r1, 0.75),
            };
            let inverse = 1.0 / width;
            let mut values = vec![0.0f32; (gx + 1) * (gy + 1)];
            for index in 0..4 {
                values[[0, gx, gy * width, gy * width + gx][index]] = random[index + 1];
            }
            gen_plasma(&mut values, gx, (ax, ay), random,
                Span { x0: 0, x1: gx, y0: 0, y1: gy }, dt);
            let seed = if matches!(pattern, Pattern::Plasma3) { 0.5 * values[0] } else { values[0] };
            let (minimum, maximum) = extent(seed, values.iter().copied());
            for (value, vertex) in values.iter().zip(out.iter_mut()) {
                let t = if matches!(pattern, Pattern::Plasma) {
                    (value - minimum) * (1.0 / (maximum - minimum))
                } else {
                    ((value - minimum) * 2.0 / (maximum - minimum)).sin()
                };
                let c = band_entry(inverse, 0.0, t)[1];
                *vertex = [(1.0 + width) / width, c];
            }
        }
        Pattern::Cercle => banded(out, band, 1.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            flipped(radius(dx, dy) * 1.41421)
        }),
        Pattern::Square => banded(out, band, 1.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            (dx.abs() * 1.41421).max(dy.abs() * 1.41421)
        }),
        Pattern::Snail | Pattern::Snail2 | Pattern::Snail3 => banded(out, band, 2.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            let (rho, theta) = (radius(dx, dy), dy.atan2(dx));
            match pattern {
                Pattern::Snail => (theta + 10.0 * rho - 0.2).cos() * rho,
                Pattern::Snail2 => (theta + 25.0 * rho - 0.2).cos() * rho,
                _ => (theta + 50.0 * rho + 0.5).sin() * rho,
            }
        }),
        Pattern::Triangle => banded(out, band, 1.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            flipped(dx.abs() + dy.abs())
        }),
        Pattern::Donuts => banded(out, band, 3.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            (10.0 * radius(dx, dy) + 90.0).sin()
        }),
        Pattern::Corner => banded(out, band, 1.0, &|x, y| {
            let dx = ((x - 0.5) + 0.25) * ax;
            let dy = ((y - 0.5) + 0.35) * ay;
            flipped((radius(dx, dy) * 1.41421).clamp(0.0, 1.0))
        }),
        Pattern::Patches => banded(out, band, 1.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            flipped(((10.0 * dy).cos() + (10.0 * dx).sin()) * 0.25 + 0.5)
        }),
        Pattern::Checkerboard => banded(out, band, 3.0, &|x, y| {
            let xs = (x - 1.0) * ax * 10.0 + 90.0;
            let ys = (y - 0.0) * ay * 10.0 + 90.0;
            xs.sin() * ys.cos()
        }),
        Pattern::Bubbles => each(out, &|x, y| [
            7.0,
            2.0 * ((x - 10.0 * r1) * ax * 10.0 + 0.5 * r3).sin()
                * (0.5 * r4 + (y - 0.5) * ay * 10.0).cos() - 2.0,
        ]),
        Pattern::Stars | Pattern::Stars2 | Pattern::Clock | Pattern::Nuclear => banded(out, band, 3.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            let theta = dy.atan2(dx);
            match pattern {
                Pattern::Stars => (5.0 * theta + 90.0).sin(),
                Pattern::Stars2 => (10.0 * theta + 90.0).sin(),
                Pattern::Clock => ((theta + theta) * 0.5).sin(),
                _ => ((theta + theta) * 1.5).sin(),
            }
        }),
        Pattern::Arrow => banded(out, band, 1.0, &|x, y| {
            let (dx, dy) = centered(x, y);
            let limit = |value: f32| value.clamp(-0.9999, 0.9999);
            let (ax, ay) = (limit(dx).atanh(), limit(dy).atanh());
            ((4.0 * ay).cos() + ax).sin() * 0.5 + 0.5
        }),
        Pattern::Cisor => banded(out, band, 3.0, &|x, y| {
            let theta = ((y - 0.0) * ay).atan2((x - 1.0) * ax);
            (5.0 * theta + 90.0).sin()
        }),
        Pattern::Wave => banded(out, band, 3.0, &|x, y| {
            // Wave shares the upper-right origin with cisor/checkerboard.
            let (dx, dy) = ((x - 1.0) * ax, y * ay);
            (10.0 * radius(dx, dy) + 90.0).sin()
        }),
        Pattern::Curtain => {
            let (vx, vy) = (6.28f32.cos(), 6.28f32.sin());
            let scale = 3.33333325f32;
            let a = f32::from_bits(0x408a_aaaa);
            each(out, &|x, y| {
                let s = ((x * ax - 0.5) * vx + (y * ay - 0.5) * vy + 0.5) * 3.14159274;
                let h = s.sin() * 0.5 + 0.5;
                [a, h * scale - scale]
            });
        }
        Pattern::Vertical => {
            let (vx, vy) = (6.28f32.cos(), 6.28f32.sin());
            each(out, &|x, y| [6.0, 5.0 * wipe(x * ax, y * ay, vx, vy) - 5.0]);
        }
        Pattern::Horizontal => {
            // Preserve the source's transposed loops and row-major write quirk.
            let (vx, vy) = (6.28f32.cos(), 6.28f32.sin());
            let mut k = 0;
            for i in 0..=gx {
                for j in 0..=gy {
                    let (u, v) = (i as f32 / gxf * ax, j as f32 / gyf * ay);
                    let t = ((((v - 0.5) * vy + (u - 0.5) * vx + 0.5) - 0.5) / sqrt2) + 0.5;
                    out[k] = [6.0, 5.0 * t - 5.0];
                    k += 1;
                }
            }
        }
        Pattern::LinesVertical => each(out, &|x, _| [7.0, 2.0 * ((x - 0.5) * ax * r1 * 60.0).sin() - 2.0]),
        Pattern::LinesHorizontal => each(out, &|_, y| [7.0, 2.0 * (r1 * 60.0 * (y - 0.5) * ay).sin() - 2.0]),
        Pattern::Cross | Pattern::Cross2 => {
            let cut = matches!(pattern, Pattern::Cross2);
            let inverse = 1.0 / band;
            each(out, &|x, y| {
                let (dx, dy) = centered(x, y);
                let t = (dx.abs() + dx.abs()).min(dy.abs() + dy.abs());
                if cut && t >= 0.3 { [0.0, 0.0] }
                else { [(band + 1.0) * inverse, t * inverse - inverse] }
            });
        }
    }
}


#[derive(Clone, Copy)]
struct Span { x0: usize, x1: usize, y0: usize, y1: usize }

fn gen_plasma(values: &mut [f32], gx: usize, aspect: (f32, f32), random: [f32; 5], span: Span, dt: f32) {
    let Span { x0, x1, y0, y1 } = span;
    let (ax, ay) = aspect;
    let width = gx + 1;
    let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
    let at = |values: &mut [f32], x: usize, y: usize| values[y * width + x];
    let (t00, t01, t10, t11) = (at(values, x0, y0), at(values, x1, y0), at(values, x0, y1), at(values, x1, y1));

    if y1 - y0 >= 2 {
        if x0 == 0 {
            let r = random[0];
            values[my * width + x0] = 0.5 * (t00 + t10) + (r * 2.0 - 1.0) * dt * ay;
        }
        let r = random[1];
        values[my * width + x1] = 0.5 * (t01 + t11) + (r * 2.0 - 1.0) * dt * ay;
    }
    if x1 - x0 >= 2 {
        if y0 == 0 {
            let r = random[2];
            values[y0 * width + mx] = 0.5 * (t00 + t01) + (r * 2.0 - 1.0) * dt * ax;
        }
        let r = random[3];
        values[y1 * width + mx] = 0.5 * (t10 + t11) + (r * 2.0 - 1.0) * dt * ax;
    }
    if y1 - y0 >= 2 && x1 - x0 >= 2 {
        let (left, right, top, bottom) = (at(values, x0, my), at(values, x1, my), at(values, mx, y0), at(values, mx, y1));
        let r = random[4];
        values[my * width + mx] = 0.25 * (top + bottom + left + right) + (r * 2.0 - 1.0) * dt;
        let half = dt * 0.5;
        for quadrant in [
            Span { x0, x1: mx, y0, y1: my },
            Span { x0: mx, x1, y0, y1: my },
            Span { x0, x1: mx, y0: my, y1 },
            Span { x0: mx, x1, y0: my, y1 },
        ] { gen_plasma(values, gx, aspect, random, quadrant, half); }
    }
}

fn band_entry(inverse: f32, a_multiplier: f32, t: f32) -> [f32; 2] {
    [inverse * a_multiplier, t * inverse - inverse]
}

fn extent(seed: f32, values: impl Iterator<Item = f32>) -> (f32, f32) {
    let (mut minimum, mut maximum) = (seed, seed);
    for value in values {
        if minimum > value { minimum = value; }
        if maximum < value { maximum = value; }
    }
    (minimum, maximum)
}

pub(super) fn generate(key: TransitionMaskKey) -> Result<Vec<u8>, String> {
    if Pattern::from_code(key.pattern).is_none() { return Err("Unknown MilkDrop 3 blend pattern".into()); }
    let progress = f32::from_bits(key.progress);
    let random = key.random.map(f32::from_bits);
    if !progress.is_finite() || !(0.0..=1.0).contains(&progress) || random.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
        return Err("MilkDrop 3 blend progress or random values are outside their saved domain".into());
    }
    if key.width == 0 || key.height == 0 || key.width > 4096 || key.height > 4096
        || u64::from(key.width) * u64::from(key.height) > MAX_TARGET_PIXELS {
        return Err("MilkDrop 3 transition mask exceeds the 4096-side or 8 Mi-pixel limit".into());
    }
    let aspect = ((key.width as f32 / key.height as f32).max(1.0), (key.height as f32 / key.width as f32).max(1.0));
    let mut vertices = vec![[0.0f32; 2]; (GRID_X + 1) * (GRID_Y + 1)];
    fill_vertices(GRID_X, GRID_Y, Pattern::from_code(key.pattern).expect("validated pattern"), key.direction, random, aspect, &mut vertices);
    let mut packed = Vec::with_capacity(vertices.len() * 4);
    for [a, c] in vertices {
        let alpha = (a * progress + c).clamp(0.0, 1.0);
        let alpha = if alpha.is_nan() { 0.0 } else { alpha };
        let encoded = (f64::from(alpha) * f64::from(u32::MAX)).round() as u32;
        packed.extend_from_slice(&encoded.to_be_bytes());
    }
    Ok(packed)
}


#[cfg(test)]
mod tests {
    use super::*;

    const GX: usize = 4;
    const GY: usize = 2;
    const ASPECT: (f32, f32) = (1.0, 0.5625);
    const RANDOM: [f32; 5] = [0.25, 0.5, 0.75, 0.125, 0.875];
    const PICK: [usize; 4] = [0, 3, 7, 14];

    /// Published Lactea reference table for the 30 saved patterns, sampled at
    /// the same 4x2 mesh vertices and random registers. Pattern codes are
    /// MilkRust's one-based saved-header values (the reference enum is zero-based).
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)]
    const REFERENCE: &[(i32, i32, [(f32, f32); 4])] = &[
        (1, 1, [(1.0000, 0.0000), (1.0000, 0.0000), (1.0000, 0.0000), (1.0000, 0.0000)]),
        (2, 1, [(6.0000, -4.2692), (6.0000, -4.2671), (6.0000, -3.2734), (6.0000, -2.2776)]),
        (3, 1, [(7.5574, -3.2787), (7.5574, -0.5464), (7.5574, -1.3832), (7.5574, 0.0000)]),
        (4, 1, [(4.4783, -0.5514), (4.4783, -0.1192), (4.4783, -0.0001), (4.4783, -0.3155)]),
        (5, 1, [(4.4783, -0.3137), (4.4783, -0.3155), (4.4783, -0.2387), (4.4783, -0.0357)]),
        (6, 1, [(5.4444, -0.8387), (5.4444, -2.0793), (5.4444, -4.4444), (5.4444, -0.8387)]),
        (6, -1, [(5.4444, -3.6058), (5.4444, -2.3652), (5.4444, 0.0000), (5.4444, -3.6058)]),
        (7, 1, [(5.4444, -1.3018), (5.4444, -2.6767), (5.4444, -4.4444), (5.4444, -1.3018)]),
        (8, 1, [(9.8889, -6.9246), (9.8889, -5.9697), (9.8889, -4.4444), (9.8889, -1.9643)]),
        (9, 1, [(9.8889, -3.1841), (9.8889, -5.2600), (9.8889, -4.4444), (9.8889, -5.7048)]),
        (10, 1, [(9.8889, -1.9231), (9.8889, -5.0628), (9.8889, -4.4444), (9.8889, -6.9658)]),
        (11, 1, [(5.4444, -0.9722), (5.4444, -2.0833), (5.4444, -4.4444), (5.4444, -0.9722)]),
        (11, -1, [(5.4444, -3.4722), (5.4444, -2.3611), (5.4444, 0.0000), (5.4444, -3.4722)]),
        (12, 1, [(14.3333, -0.0149), (14.3333, -6.5156), (14.3333, -0.4711), (14.3333, -0.0149)]),
        (13, 1, [(5.4444, -2.7860), (5.4444, -1.2573), (5.4444, -2.4444), (5.4444, 0.0000)]),
        (13, -1, [(5.4444, -1.6584), (5.4444, -3.1871), (5.4444, -2.0001), (5.4444, -4.4444)]),
        (14, 1, [(5.4444, -2.2082), (5.4444, -2.6087), (5.4444, -1.1111), (5.4444, -4.3392)]),
        (14, -1, [(5.4444, -2.2362), (5.4444, -1.8357), (5.4444, -3.3333), (5.4444, -0.1053)]),
        (15, 1, [(14.3333, -2.4652), (14.3333, -3.5522), (14.3333, -4.5502), (14.3333, -3.6801)]),
        (16, 1, [(7.0000, -2.8988), (7.0000, -3.8268), (7.0000, -3.3973), (7.0000, -0.2957)]),
        (17, 1, [(14.3333, -0.0294), (14.3333, -8.0757), (14.3333, -0.4711), (14.3333, -8.8595)]),
        (18, 1, [(14.3333, -1.0300), (14.3333, -4.9895), (14.3333, -0.4711), (14.3333, -1.0300)]),
        (19, 1, [(14.3333, -6.6234), (14.3333, -7.7663), (14.3333, -4.4444), (14.3333, -2.2655)]),
        (20, 1, [(14.3333, -8.8864), (14.3333, -6.9874), (14.3333, -4.4444), (14.3333, -0.0025)]),
        (21, 1, [(5.4444, -2.5465), (5.4444, -0.8628), (5.4444, -0.3523), (5.4444, -0.4118)]),
        (22, 1, [(14.3333, -8.4178), (14.3333, -8.4178), (14.3333, -2.2109), (14.3333, -6.4359)]),
        (23, 1, [(14.3333, -6.6950), (14.3333, -8.8195), (14.3333, -0.0149), (14.3333, -0.0830)]),
        (24, 1, [(4.3333, -1.6583), (4.3333, -0.4941), (4.3333, -0.0000), (4.3333, -1.6656)]),
        (25, 1, [(6.0000, -4.2621), (6.0000, -1.6105), (6.0000, -2.4975), (6.0000, -0.7329)]),
        (26, 1, [(6.0000, -4.2621), (6.0000, -3.3782), (6.0000, -2.4975), (6.0000, -0.7329)]),
        (27, 1, [(7.0000, -3.8760), (7.0000, -3.1431), (7.0000, -2.0000), (7.0000, -0.1240)]),
        (28, 1, [(7.0000, -0.2388), (7.0000, -0.2388), (7.0000, -2.0000), (7.0000, -3.7612)]),
        (29, 1, [(5.4444, -1.9444), (5.4444, -2.2222), (5.4444, -4.4444), (5.4444, -1.9444)]),
        (30, 1, [(0.0000, 0.0000), (0.0000, 0.0000), (5.4444, -4.4444), (0.0000, 0.0000)]),
    ];

    #[test]
    fn all_saved_pattern_meshes_match_reference_vertices() {
        let mut seen = [false; 30];
        for &(code, direction, expected) in REFERENCE {
            let pattern = Pattern::from_code(code).expect("reference pattern code");
            let mut vertices = vec![[0.0; 2]; (GX + 1) * (GY + 1)];
            fill_vertices(GX, GY, pattern, direction, RANDOM, ASPECT, &mut vertices);
            let index = usize::try_from(code - 1).expect("positive saved pattern code");
            seen[index] = true;
            for (&vertex, &(want_a, want_c)) in PICK.iter().zip(expected.iter()) {
                let [got_a, got_c] = vertices[vertex];
                assert!(
                    (got_a - want_a).abs() <= 1e-3 && (got_c - want_c).abs() <= 1e-3,
                    "pattern {code}, direction {direction}, vertex {vertex}: got ({got_a:.4}, {got_c:.4}), want ({want_a:.4}, {want_c:.4})"
                );
            }
        }
        assert!(seen.into_iter().all(|present| present), "every saved pattern has a reference row");
    }
}
