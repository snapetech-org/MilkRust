use crate::audio::{
    build_audio_profile, DesktopAudioProfile, DEFAULT_SPECTRUM_SIZE, DEFAULT_WAVEFORM_SIZE,
};
use crate::{report_frame_set, MilkRustFrameSetRuntimeHost};

#[derive(Clone, Debug, Default)]
pub struct DesktopSessionConfig {
    pub frames: usize,
    pub fps: f64,
    pub waveform_size: usize,
    pub spectrum_size: usize,
    pub audio_profile: DesktopAudioProfile,
}

#[derive(Clone, Copy, Debug)]
pub struct DesktopSessionTiming {
    pub frame_index: usize,
    pub time_seconds: f64,
}

#[derive(Clone, Debug)]
pub struct DesktopSessionFrame {
    pub timing: DesktopSessionTiming,
    pub source_title: String,
    pub transition_mode: String,
    pub transition_seconds: f64,
    pub preset_count: usize,
    pub report: crate::MilkRustFrameSetReport,
}

#[derive(Clone, Debug)]
pub struct DesktopSessionSummary {
    pub source: String,
    pub source_label: String,
    pub frames: usize,
    pub timing: DesktopSessionTiming,
    pub source_line: String,
    pub frame_total: usize,
    pub average_line_vertices: f64,
    pub average_point_vertices: f64,
    pub average_textured_vertices: f64,
    pub average_triangle_vertices: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct DesktopSessionError {
    pub message: &'static str,
}

impl std::fmt::Display for DesktopSessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for DesktopSessionError {}

pub fn collect_headless_frames(
    source: &str,
    config: DesktopSessionConfig,
) -> Result<Vec<DesktopSessionFrame>, DesktopSessionError> {
    if config.frames == 0 {
        return Err(DesktopSessionError {
            message: "frames must be greater than zero",
        });
    }
    let fps = if config.fps > 0.0 { config.fps } else { 60.0 };
    let waveform_size = if config.waveform_size == 0 {
        DEFAULT_WAVEFORM_SIZE
    } else {
        config.waveform_size
    };
    let spectrum_size = if config.spectrum_size == 0 {
        DEFAULT_SPECTRUM_SIZE
    } else {
        config.spectrum_size
    };

    let mut host = MilkRustFrameSetRuntimeHost::new();
    let mut output = Vec::with_capacity(config.frames);

    for frame in 0..config.frames {
        let time_seconds = frame as f64 / fps;
        let (bass, mid, treble, mut waveform, mut spectrum) =
            build_audio_profile(frame, fps, &config.audio_profile);

        waveform.resize(waveform_size, 0.0);
        spectrum.resize(spectrum_size, 0.0);
        let frame_set = host.try_render_frame_set(
            source,
            time_seconds,
            bass,
            mid,
            treble,
            &waveform,
            &spectrum,
        );
        let frame_set = frame_set.map_err(|_| DesktopSessionError {
            message: "preset equation evaluation failed; no geometry report was emitted",
        })?;
        let report = report_frame_set(&frame_set);
        output.push(DesktopSessionFrame {
            timing: DesktopSessionTiming {
                frame_index: frame,
                time_seconds,
            },
            source_title: report.source_title.clone(),
            transition_mode: report.transition_mode.clone(),
            transition_seconds: report.transition_seconds,
            preset_count: report.source_preset_count,
            report,
        });
    }

    Ok(output)
}

pub fn summarize_headless_frames(
    source: &str,
    source_label: &str,
    frames: Vec<DesktopSessionFrame>,
) -> DesktopSessionSummary {
    let average = if frames.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        let len = frames.len() as f64;
        let (line, point, textured, tri) = frames.iter().fold(
            (0.0, 0.0, 0.0, 0.0),
            |(line, point, textured, tri), frame| {
                (
                    line + frame.report.line_vertices as f64,
                    point + frame.report.point_vertices as f64,
                    textured + frame.report.textured_vertices as f64,
                    tri + frame.report.triangle_vertices as f64,
                )
            },
        );
        (line / len, point / len, textured / len, tri / len)
    };
    let last = frames.last().cloned();
    DesktopSessionSummary {
        source: source.to_string(),
        source_label: source_label.to_string(),
        frames: frames.len(),
        timing: last
            .as_ref()
            .map(|frame| frame.timing)
            .unwrap_or(DesktopSessionTiming {
                frame_index: 0,
                time_seconds: 0.0,
            }),
        source_line: last
            .as_ref()
            .map(|frame| frame.source_title.clone())
            .unwrap_or_else(|| "Unknown preset".to_string()),
        frame_total: frames.len(),
        average_line_vertices: average.0,
        average_point_vertices: average.1,
        average_textured_vertices: average.2,
        average_triangle_vertices: average.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_headless_frames_and_calculates_summary() {
        let source = "name=MilkRust Desktop Test
decay=0.9
wave_r=0.4
wave_g=0.7
wave_b=0.9
wave_a=0.8
zoom=1.1
rot=0.01";

        let frames = collect_headless_frames(
            source,
            DesktopSessionConfig {
                frames: 4,
                fps: 60.0,
                waveform_size: DEFAULT_WAVEFORM_SIZE,
                spectrum_size: DEFAULT_SPECTRUM_SIZE,
                audio_profile: DesktopAudioProfile::default(),
            },
        )
        .expect("session should collect frames");
        let summary = summarize_headless_frames(source, "MilkRust Desktop Test", frames.clone());

        assert_eq!(summary.frames, 4);
        assert_eq!(summary.timing.frame_index, 3);
        assert!(summary.average_line_vertices > 0.0);
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].preset_count, 1);
        assert!(!frames[0].source_title.is_empty());
    }

    #[test]
    fn summarize_headless_frames_returns_defaults_for_empty_input() {
        let frames: Vec<DesktopSessionFrame> = Vec::new();
        let summary = summarize_headless_frames("name=Empty", "Empty", frames);

        assert_eq!(summary.frames, 0);
        assert_eq!(summary.frame_total, 0);
        assert_eq!(summary.timing.frame_index, 0);
        assert_eq!(summary.timing.time_seconds, 0.0);
        assert_eq!(summary.average_line_vertices, 0.0);
        assert_eq!(summary.average_point_vertices, 0.0);
        assert_eq!(summary.average_textured_vertices, 0.0);
        assert_eq!(summary.average_triangle_vertices, 0.0);
        assert_eq!(summary.source_line, "Unknown preset");
    }

    #[test]
    fn summarize_headless_frames_calculates_correct_averages() {
        let source = "name=AveragesTest
decay=0.9
wave_r=0.5
wave_g=0.5
wave_b=0.5
wave_a=0.5
zoom=1.0
rot=0.01";

        let frames = collect_headless_frames(
            source,
            DesktopSessionConfig {
                frames: 10,
                fps: 60.0,
                waveform_size: DEFAULT_WAVEFORM_SIZE,
                spectrum_size: DEFAULT_SPECTRUM_SIZE,
                audio_profile: DesktopAudioProfile::default(),
            },
        )
        .expect("session should collect frames");

        let summary = summarize_headless_frames(source, "AveragesTest", frames.clone());

        assert_eq!(summary.frames, frames.len());
        assert_eq!(summary.frame_total, frames.len());
        assert_eq!(summary.timing.frame_index, frames.last().unwrap().timing.frame_index);
        assert!(summary.average_line_vertices > 0.0);

        let len = frames.len() as f64;
        let expected_avg = frames.iter().map(|f| f.report.line_vertices as f64).sum::<f64>() / len;
        assert!((summary.average_line_vertices - expected_avg).abs() < f64::EPSILON);
    }

    #[test]
    fn collect_headless_frames_rejects_zero_frames() {
        let source = "name=ZeroFrames
decay=0.9
wave_r=0.5
wave_g=0.5
wave_b=0.5
wave_a=0.5
zoom=1.0
rot=0.01";

        let result = collect_headless_frames(
            source,
            DesktopSessionConfig {
                frames: 0,
                ..Default::default()
            },
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().message, "frames must be greater than zero");
    }

    #[test]
    fn collect_headless_frames_applies_default_fps_and_sizes() {
        let source = "name=DefaultsTest
decay=0.9
wave_r=0.5
wave_g=0.5
wave_b=0.5
wave_a=0.5
zoom=1.0
rot=0.01";

        let frames = collect_headless_frames(
            source,
            DesktopSessionConfig {
                frames: 5,
                fps: 0.0,
                waveform_size: 0,
                spectrum_size: 0,
                audio_profile: DesktopAudioProfile::default(),
            },
        )
        .expect("session should collect frames with defaults");

        assert_eq!(frames.len(), 5);
        assert!(frames.iter().all(|f| f.report.line_vertices > 0 || f.report.point_vertices > 0));
    }

    #[test]
    fn desktop_session_error_display_shows_message() {
        let err = DesktopSessionError {
            message: "test error",
        };
        assert_eq!(format!("{err}"), "test error");
    }

    #[test]
    fn desktop_session_config_defaults_are_reasonable() {
        let config = DesktopSessionConfig::default();
        assert_eq!(config.frames, 0);
        assert_eq!(config.fps, 0.0);
    }
}
