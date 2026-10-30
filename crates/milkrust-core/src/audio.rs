//! Shared bounded PCM analysis for native and browser hosts. Call on the render
//! worker, never the playback callback. Hosts supply interleaved normalized stereo.
const N: usize = 512;

pub struct Analyzer {
    left: [f64; N],
    right: [f64; N],
    cursor: usize,
    available: usize,
    generation: u32,
    averages: [f64; 3],
    last_time: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Stereo {
    pub left: [f64; N],
    pub right: [f64; N],
    pub spectrum_left: [f64; N / 2],
    pub spectrum_right: [f64; N / 2],
}

#[derive(Clone, Debug)]
pub struct Features {
    pub stereo: Stereo,
    pub waveform: [f64; N],
    pub spectrum: [f64; N / 2],
    pub bands: [f64; 3],
}

impl Default for Analyzer {
    fn default() -> Self {
        Self { left: [0.0; N], right: [0.0; N], cursor: 0, available: 0, generation: 0,
            averages: [0.001; 3], last_time: None }
    }
}

impl Analyzer {
    pub fn push(&mut self, stereo: &[f32], generation: u32) -> Result<(), String> {
        if stereo.len() % 2 != 0 || stereo.len() > 8192 {
            return Err("PCM requires at most 4096 interleaved stereo frames".into());
        }
        if stereo.iter().any(|sample| !sample.is_finite()) { return Err("Non-finite PCM sample".into()); }
        if generation != self.generation { *self = Self { generation, ..Self::default() }; }
        for pair in stereo.chunks_exact(2) {
            self.left[self.cursor] = f64::from(pair[0]).clamp(-1.0, 1.0);
            self.right[self.cursor] = f64::from(pair[1]).clamp(-1.0, 1.0);
            self.cursor = (self.cursor + 1) % N;
            self.available = (self.available + 1).min(N);
        }
        Ok(())
    }

    pub fn analyze(&mut self, time: f64) -> Features {
        let left: [f64; N] = std::array::from_fn(|index| self.left[(self.cursor + index) % N]);
        let right: [f64; N] = std::array::from_fn(|index| self.right[(self.cursor + index) % N]);
        let waveform: [f64; N] = std::array::from_fn(|index| (left[index] + right[index]) * 0.5);
        let spectrum_left = spectrum(&left);
        let spectrum_right = spectrum(&right);
        // Average channel energy, not phase. Anti-phase music must not turn
        // the spectrum/band detectors into silence.
        let spectrum: [f64; N / 2] = std::array::from_fn(|index| ((spectrum_left[index].powi(2)
            + spectrum_right[index].powi(2)) * 0.5).sqrt());
        let mut bands = [0.0; 3];
        let ranges = [(1, 3), (3, 24), (24, 128)];
        let dt = self.last_time.map_or(1.0 / 60.0, |previous| (time - previous).clamp(0.0, 0.25));
        self.last_time = Some(time);
        let smoothing = 1.0 - (-dt / 1.5).exp();
        for (index, (low, high)) in ranges.into_iter().enumerate() {
            let energy = spectrum[low..high].iter().map(|v| v * v).sum::<f64>().sqrt() / ((high - low) as f64).sqrt();
            self.averages[index] += (energy - self.averages[index]) * smoothing;
            bands[index] = if self.available == 0 { 0.0 } else { (energy / self.averages[index].max(0.001)).min(20.0) };
        }
        Features { waveform, spectrum, bands, stereo: Stereo { left, right, spectrum_left, spectrum_right } }
    }
}

fn spectrum(waveform: &[f64; N]) -> [f64; N / 2] {
        // Iterative radix-2 FFT, fixed storage and work. A Hann window prevents
        // discontinuities at block boundaries from dominating the spectrum.
        let mut real = [0.0; N];
        let mut imaginary = [0.0; N];
        for index in 0..N {
            let reversed = index.reverse_bits() >> (usize::BITS - 9);
            real[reversed] = waveform[index] * (0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / (N - 1) as f64).cos());
        }
        let mut size = 2;
        while size <= N {
            for start in (0..N).step_by(size) {
                for index in 0..size / 2 {
                    let phase = -std::f64::consts::TAU * index as f64 / size as f64;
                    let (sine, cosine) = phase.sin_cos();
                    let a = start + index;
                    let b = a + size / 2;
                    let r = real[b] * cosine - imaginary[b] * sine;
                    let i = real[b] * sine + imaginary[b] * cosine;
                    real[b] = real[a] - r; imaginary[b] = imaginary[a] - i;
                    real[a] += r; imaginary[a] += i;
                }
            }
            size *= 2;
        }
        let mut spectrum = [0.0; N / 2];
        for (index, value) in spectrum.iter_mut().enumerate() {
            *value = (real[index].hypot(imaginary[index]) * 4.0 / N as f64).min(1.0);
        }
        spectrum
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anti_phase_channels_retain_energy_and_distinct_waveforms() {
        let mut audio = Analyzer::default();
        let pcm = (0..N).flat_map(|i| {
            let value = (std::f32::consts::TAU * 16.0 * i as f32 / N as f32).sin();
            [value, -value]
        }).collect::<Vec<_>>();
        audio.push(&pcm, 1).unwrap();
        let frame = audio.analyze(0.0);
        assert!(frame.waveform.iter().all(|v| *v == 0.0));
        assert!(frame.spectrum[16] > 0.5);
        assert_eq!(frame.stereo.left[8], -frame.stereo.right[8]);
        assert!(frame.bands[1] > 0.0);
    }
    #[test]
    fn actual_pcm_produces_spectrum_and_generation_clears_history() {
        let mut audio = Analyzer::default();
        let samples = (0..N).flat_map(|i| {
            let value = (std::f32::consts::TAU * 16.0 * i as f32 / N as f32).sin();
            [value, value]
        }).collect::<Vec<_>>();
        audio.push(&samples, 1).unwrap();
        let features = audio.analyze(0.0);
        assert!(features.spectrum[16] > features.spectrum[100]);
        audio.push(&[0.0; N * 2], 2).unwrap();
        assert!(audio.analyze(1.0).spectrum.iter().all(|value| *value == 0.0));
    }
}
