use milkrust_core::{
    milkrust_frame_set_from_source_with_audio, MilkRustFrameSet, MilkRustFrameSetRuntime, MilkRustInputState,
};
use milkrust_renderer_core::MilkRustBatchRenderer;
use milkrust_renderer_headless::{create_headless_batches, MilkRustHeadlessRenderer};

pub mod audio;
#[cfg(feature = "audio")]
pub mod audio_provider;
mod cli_support;
mod player_api;
mod runner_player;
#[cfg(feature = "ui")]
mod runner_player_ui;
mod runner_probe;
mod runner_studio;
pub mod session;

pub use audio::{build_audio_profile, create_spectrum, create_waveform, DesktopAudioProfile};
#[cfg(feature = "audio")]
pub use audio_provider::{
    CpalDesktopAudioProvider, CpalDesktopAudioProviderConfig, CpalDesktopAudioProviderError,
};
pub use cli_support::{
    collect_pack_plugins, collect_preset_inputs, parse_non_negative_f64, parse_positive_f64,
    parse_positive_usize, write_pack_plugin_report, PackPluginInput, PresetInput,
};
pub use player_api::{
    DesktopAudioContext, DesktopAudioFrame, DesktopAudioProvider, DesktopDataPlugin,
    DesktopPlayerEngine, DesktopPlayerEngineConfig, DesktopPlayerError, DesktopPlayerFrame,
    DesktopPlayerPreset, DesktopPlayerState, DesktopPlugin, DesktopPluginDataDescriptor,
    DesktopPluginFrameContext, DesktopPluginHeartbeatContext, DesktopPluginPresetChangeContext,
    DesktopPluginPresetContext, DesktopPluginRenderContext, SyntheticDesktopAudioProvider,
};
pub use runner_player::run_desktop_player;
#[cfg(feature = "ui")]
pub use runner_player_ui::run_desktop_player_ui;
pub use runner_probe::run_desktop_probe;
pub use runner_studio::run_desktop_studio;
pub use session::{
    collect_headless_frames, summarize_headless_frames, DesktopSessionConfig, DesktopSessionFrame,
    DesktopSessionSummary, DesktopSessionTiming,
};

#[derive(Clone, Debug)]
pub struct MilkRustFrameSetReport {
    pub source_title: String,
    pub transition_mode: String,
    pub transition_seconds: f64,
    pub source_preset_count: usize,
    pub frame_count: usize,
    pub line_vertices: usize,
    pub point_vertices: usize,
    pub textured_vertices: usize,
    pub triangle_vertices: usize,
}

/// Creates a lightweight frame-set render report for a preset source.
///
/// This is intentionally renderer-agnostic and is used by both prototype desktop hosts
/// and tooling that needs deterministic runtime coverage without opening a browser.
pub fn collect_frame_set_report(
    source: &str,
    time_seconds: f64,
    bass: f64,
    mid: f64,
    treble: f64,
    waveform: &[f64],
    spectrum: &[f64],
) -> (MilkRustFrameSet, MilkRustFrameSetReport) {
    let frame_set = milkrust_frame_set_from_source_with_audio(
        source,
        time_seconds,
        bass,
        mid,
        treble,
        waveform,
        spectrum,
    );
    let report = report_frame_set(&frame_set);
    (frame_set, report)
}

pub fn report_frame_set(frame_set: &MilkRustFrameSet) -> MilkRustFrameSetReport {
    let mut renderer = MilkRustHeadlessRenderer::new();
    let batches = create_headless_batches(frame_set);
    let stats = renderer
        .render_batches(&batches)
        .expect("headless renderer is infallible");

    MilkRustFrameSetReport {
        source_title: frame_set.title.clone(),
        transition_mode: frame_set.transition_mode.clone(),
        transition_seconds: frame_set.transition_seconds,
        source_preset_count: frame_set.preset_count,
        frame_count: frame_set.entries.len(),
        line_vertices: stats.line_vertices,
        point_vertices: stats.point_vertices,
        textured_vertices: stats.textured_vertices,
        triangle_vertices: stats.triangle_vertices,
    }
}

/// Provides a deterministic baseline runtime for future desktop host loops.
#[derive(Default)]
pub struct MilkRustFrameSetRuntimeHost {
    runtime: MilkRustFrameSetRuntime,
    source: String,
}

impl MilkRustFrameSetRuntimeHost {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_set(
        &mut self,
        source: &str,
        time_seconds: f64,
        bass: f64,
        mid: f64,
        treble: f64,
        waveform: &[f64],
        spectrum: &[f64],
    ) -> MilkRustFrameSet {
        self.source = source.to_string();
        self.runtime.render_source_with_audio(
            source,
            time_seconds,
            bass,
            mid,
            treble,
            waveform,
            spectrum,
        )
    }

    pub fn render_error(&self) -> Option<&str> { self.runtime.render_error() }

    #[allow(clippy::too_many_arguments)]
    pub fn try_render_frame_set(
        &mut self, source: &str, time_seconds: f64, bass: f64, mid: f64,
        treble: f64, waveform: &[f64], spectrum: &[f64],
    ) -> Result<MilkRustFrameSet, String> {
        let frame = self.runtime.try_render_source_with_audio_and_input(
            source, time_seconds, bass, mid, treble, waveform, spectrum, MilkRustInputState::default(),
        )?;
        self.source = source.to_string();
        Ok(frame)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_frame_set_report_returns_valid_report() {
        let source = "name=MilkRust Report Test
decay=0.9
wave_r=0.4
wave_g=0.7
wave_b=0.9
wave_a=0.8
zoom=1.1
rot=0.01";
        let (frame_set, report) =
            collect_frame_set_report(source, 0.0, 0.5, 0.5, 0.5, &[], &[]);
        assert!(!frame_set.entries.is_empty());
        assert!(!report.source_title.is_empty());
        assert!(report.frame_count > 0);
        assert!(report.line_vertices > 0 || report.point_vertices > 0);
        assert_eq!(report.transition_mode, frame_set.transition_mode);
        assert_eq!(report.transition_seconds, frame_set.transition_seconds);
    }

    #[test]
    fn report_frame_set_produces_headless_stats() {
        let source = "name=ReportStats
decay=0.85
wave_r=0.3
wave_g=0.6
wave_b=0.8
wave_a=0.7
zoom=1.0
rot=0.02";
        let frame_set = milkrust_frame_set_from_source_with_audio(
            source, 0.0, 0.5, 0.5, 0.5, &[], &[]
        );
        let report = report_frame_set(&frame_set);
        assert!(report.line_vertices > 0 || report.point_vertices > 0);
        assert!(report.frame_count > 0);
        assert_eq!(report.source_title, frame_set.title);
    }

    #[test]
    fn frame_set_report_contains_all_vertex_types() {
        let source = "name=VertexTypes
decay=0.9
wave_r=1.0
wave_g=1.0
wave_b=1.0
wave_a=1.0
zoom=2.0
rot=0.05";
        let frame_set = milkrust_frame_set_from_source_with_audio(
            source, 0.0, 0.5, 0.5, 0.5, &[], &[]
        );
        let report = report_frame_set(&frame_set);
        assert!(report.frame_count > 0);
        assert_eq!(report.source_preset_count, frame_set.preset_count);
    }

    #[test]
    fn milkrust_frame_set_runtime_host_is_default() {
        let host = MilkRustFrameSetRuntimeHost::default();
        assert_eq!(host.source, "");
    }

    #[test]
    fn milkrust_frame_set_runtime_host_renders_consistent_frames() {
        let source = "name=ConsistentRender
decay=0.9
wave_r=0.5
wave_g=0.5
wave_b=0.5
wave_a=0.5
zoom=1.0
rot=0.01";
        let mut host = MilkRustFrameSetRuntimeHost::new();
        let frame0 = host.render_frame_set(source, 0.0, 0.5, 0.5, 0.5, &[], &[]);
        let frame1 = host.render_frame_set(source, 1.0, 0.5, 0.5, 0.5, &[], &[]);
        // Same source should produce same title
        assert_eq!(frame0.title, frame1.title);
        assert!(!frame0.entries.is_empty());
        // Different time should produce different entries
        assert_ne!(frame0.entries, frame1.entries);
    }

    #[test]
    fn collect_frame_set_report_handles_empty_waveform_and_spectrum() {
        let source = "name=EmptyAudio
decay=0.9
wave_r=0.5
wave_g=0.5
wave_b=0.5
wave_a=0.5
zoom=1.0
rot=0.01";
        let (_, report) =
            collect_frame_set_report(source, 0.0, 0.5, 0.5, 0.5, &[], &[]);
        assert!(report.frame_count > 0);
    }
}
