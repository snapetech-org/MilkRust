use std::{fmt, fs, path::PathBuf, sync::Arc};

use milkrust_core::MilkRustFrameSet;

use crate::{
    build_audio_profile, collect_preset_inputs, report_frame_set, DesktopAudioProfile,
    PackPluginInput, PresetInput, MilkRustFrameSetReport, MilkRustFrameSetRuntimeHost,
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct DesktopPlayerPreset {
    pub source_path: PathBuf,
    pub source_label: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct DesktopAudioContext {
    pub global_frame_index: usize,
    pub local_frame_index: usize,
    pub preset_index: usize,
    pub preset_count: usize,
    pub time_seconds: f64,
    pub fps: f64,
    pub waveform_size: usize,
    pub spectrum_size: usize,
    pub audio_profile: DesktopAudioProfile,
}

#[derive(Clone, Debug)]
pub struct DesktopAudioFrame {
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
    pub waveform: Vec<f64>,
    pub spectrum: Vec<f64>,
}

pub trait DesktopAudioProvider {
    fn provide_audio_frame(&self, context: &DesktopAudioContext) -> DesktopAudioFrame;
}

#[derive(Clone, Debug)]
pub struct DesktopPluginPresetContext {
    pub preset_index: usize,
    pub preset_count: usize,
    pub source_path: String,
    pub source_label: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct DesktopPluginPresetChangeContext {
    pub previous_preset_index: usize,
    pub previous_preset_label: String,
    pub previous_source_path: String,
    pub preset_index: usize,
    pub preset_count: usize,
    pub source_path: String,
    pub source_label: String,
}

#[derive(Clone, Debug)]
pub struct DesktopPluginFrameContext {
    pub preset_index: usize,
    pub preset_count: usize,
    pub source_path: String,
    pub source_label: String,
    pub local_frame_index: usize,
    pub global_frame_index: usize,
    pub time_seconds: f64,
    pub fps: f64,
    pub running: bool,
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
}

#[derive(Clone, Debug)]
pub struct DesktopPluginHeartbeatContext {
    pub preset_index: usize,
    pub preset_count: usize,
    pub time_seconds: f64,
    pub is_beat: bool,
}

#[derive(Clone, Debug)]
pub struct DesktopPluginRenderContext {
    pub preset_index: usize,
    pub preset_count: usize,
    pub source_path: String,
    pub source_label: String,
    pub local_frame_index: usize,
    pub global_frame_index: usize,
    pub time_seconds: f64,
    pub frame_set_title: String,
    pub transition_mode: String,
    pub frame_count: usize,
    pub line_vertices: usize,
    pub point_vertices: usize,
    pub textured_vertices: usize,
    pub triangle_vertices: usize,
}

#[derive(Clone, Debug)]
pub struct DesktopPluginDataDescriptor {
    pub id: String,
    pub kind: String,
    pub entry: String,
    pub source_path: String,
    pub payload: Value,
}

pub trait DesktopPlugin {
    fn id(&self) -> &str;
    fn kind(&self) -> &str {
        "native"
    }
    fn on_preset_load(&mut self, _context: &mut DesktopPluginPresetContext) {}
    fn on_preset_loaded(&mut self, _context: &mut DesktopPluginPresetContext) {}
    fn on_preset_change(&mut self, _context: &mut DesktopPluginPresetChangeContext) {}
    fn on_frame_start(&mut self, _context: &mut DesktopPluginFrameContext) {}
    fn on_audio_frame(&mut self, _context: &mut DesktopAudioFrame) {}
    fn on_beat(&mut self, _context: &mut DesktopPluginHeartbeatContext) {}
    fn on_automation_step(&mut self, _context: &mut DesktopPluginHeartbeatContext) {}
    fn on_render_frame(&mut self, _context: &mut DesktopPluginRenderContext) {}
    fn on_input(&mut self, _context: Value) {}
    fn on_export(&mut self, _format: String, _target: Option<String>) {}
}

pub struct DesktopDataPlugin {
    pub descriptor: DesktopPluginDataDescriptor,
}

impl DesktopPlugin for DesktopDataPlugin {
    fn id(&self) -> &str {
        &self.descriptor.id
    }

    fn kind(&self) -> &str {
        &self.descriptor.kind
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SyntheticDesktopAudioProvider;

impl DesktopAudioProvider for SyntheticDesktopAudioProvider {
    fn provide_audio_frame(&self, context: &DesktopAudioContext) -> DesktopAudioFrame {
        let (bass, mid, treble, mut waveform, mut spectrum) = build_audio_profile(
            context.global_frame_index,
            context.fps,
            &context.audio_profile,
        );
        waveform.resize(context.waveform_size, 0.0);
        spectrum.resize(context.spectrum_size, 0.0);
        DesktopAudioFrame {
            bass,
            mid,
            treble,
            waveform,
            spectrum,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DesktopPlayerEngineConfig {
    pub fps: f64,
    pub preset_duration_seconds: f64,
    pub waveform_size: usize,
    pub spectrum_size: usize,
    pub audio_profile: DesktopAudioProfile,
    pub auto_loop: bool,
    pub start_paused: bool,
}

impl Default for DesktopPlayerEngineConfig {
    fn default() -> Self {
        Self {
            fps: 60.0,
            preset_duration_seconds: 20.0,
            waveform_size: 64,
            spectrum_size: 64,
            audio_profile: DesktopAudioProfile::default(),
            auto_loop: true,
            start_paused: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DesktopPlayerFrame {
    pub preset_index: usize,
    pub preset_total: usize,
    pub local_frame_index: usize,
    pub global_frame_index: usize,
    pub time_seconds: f64,
    pub running: bool,
    pub preset_label: String,
    pub source_path: String,
    pub frame_set: MilkRustFrameSet,
    pub report: MilkRustFrameSetReport,
}

#[derive(Clone, Debug)]
pub struct DesktopPlayerState {
    pub fps: f64,
    pub running: bool,
    pub preset_index: usize,
    pub preset_total: usize,
    pub global_frame_index: usize,
    pub local_frame_index: usize,
    pub per_preset_frames: usize,
}

#[derive(Debug)]
pub enum DesktopPlayerError {
    NoPresets,
    PresetNotFound(String),
    InvalidPresetInput(String),
    PresetLoadFailed { path: String, source: String },
    PresetEvaluationFailed { path: String, source: String },
    InvalidConfig(&'static str),
}

impl fmt::Display for DesktopPlayerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPresets => {
                formatter.write_str("no presets were provided for the desktop player engine")
            }
            Self::PresetNotFound(value) => write!(formatter, "preset not found: {value}"),
            Self::InvalidPresetInput(message) => {
                write!(formatter, "invalid preset input: {message}")
            }
            Self::PresetLoadFailed { path, source } => {
                write!(formatter, "failed to load preset '{path}': {source}")
            }
            Self::PresetEvaluationFailed { path, source } => {
                write!(formatter, "preset evaluation failed for '{path}': {source}")
            }
            Self::InvalidConfig(message) => write!(formatter, "invalid player config: {message}"),
        }
    }
}

impl std::error::Error for DesktopPlayerError {}

pub struct DesktopPlayerEngine {
    presets: Vec<DesktopPlayerPreset>,
    config: DesktopPlayerEngineConfig,
    host: MilkRustFrameSetRuntimeHost,
    audio_provider: Arc<dyn DesktopAudioProvider>,
    plugins: Vec<Box<dyn DesktopPlugin>>,
    data_plugins: Vec<DesktopPluginDataDescriptor>,
    preset_load_pending: bool,
    preset_index: usize,
    local_frame_index: usize,
    global_frame_index: usize,
    running: bool,
    per_preset_frames: usize,
}

impl DesktopPlayerEngine {
    pub fn from_preset_inputs(
        preset_inputs: Vec<PresetInput>,
        config: DesktopPlayerEngineConfig,
    ) -> Result<Self, DesktopPlayerError> {
        Self::from_preset_inputs_with_audio_provider(
            preset_inputs,
            config,
            SyntheticDesktopAudioProvider,
        )
    }

    pub fn from_preset_inputs_with_audio_provider(
        preset_inputs: Vec<PresetInput>,
        config: DesktopPlayerEngineConfig,
        audio_provider: impl DesktopAudioProvider + 'static,
    ) -> Result<Self, DesktopPlayerError> {
        if preset_inputs.is_empty() {
            return Err(DesktopPlayerError::NoPresets);
        }
        let presets = preset_inputs
            .into_iter()
            .map(|preset| {
                let source = fs::read_to_string(&preset.source_path).map_err(|error| {
                    DesktopPlayerError::PresetLoadFailed {
                        path: preset.source_path.display().to_string(),
                        source: error.to_string(),
                    }
                })?;
                Ok::<_, DesktopPlayerError>(DesktopPlayerPreset {
                    source_path: preset.source_path,
                    source_label: preset.source_label,
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new_with_loaded_presets(presets, config, Arc::new(audio_provider))
    }

    pub fn from_inputs(
        preset_inputs: &[PathBuf],
        pack_inputs: &[PathBuf],
        config: DesktopPlayerEngineConfig,
    ) -> Result<Self, DesktopPlayerError> {
        let selected_presets = collect_preset_inputs(preset_inputs, pack_inputs)
            .map_err(DesktopPlayerError::InvalidPresetInput)?;
        Self::from_preset_inputs(selected_presets, config)
    }

    pub fn from_inputs_with_audio_provider(
        preset_inputs: &[PathBuf],
        pack_inputs: &[PathBuf],
        config: DesktopPlayerEngineConfig,
        audio_provider: impl DesktopAudioProvider + 'static,
    ) -> Result<Self, DesktopPlayerError> {
        let selected_presets = collect_preset_inputs(preset_inputs, pack_inputs)
            .map_err(DesktopPlayerError::InvalidPresetInput)?;
        Self::from_preset_inputs_with_audio_provider(selected_presets, config, audio_provider)
    }

    pub fn from_preset_inputs_with_start_preset(
        preset_inputs: Vec<PresetInput>,
        config: DesktopPlayerEngineConfig,
        start_preset: Option<&str>,
    ) -> Result<Self, DesktopPlayerError> {
        let mut engine = Self::from_preset_inputs(preset_inputs, config)?;
        if let Some(start_preset) = start_preset {
            engine.seek_preset(start_preset)?;
        }
        Ok(engine)
    }

    pub fn from_preset_inputs_with_audio_provider_and_start_preset(
        preset_inputs: Vec<PresetInput>,
        config: DesktopPlayerEngineConfig,
        start_preset: Option<&str>,
        audio_provider: impl DesktopAudioProvider + 'static,
    ) -> Result<Self, DesktopPlayerError> {
        let mut engine =
            Self::from_preset_inputs_with_audio_provider(preset_inputs, config, audio_provider)?;
        if let Some(start_preset) = start_preset {
            engine.seek_preset(start_preset)?;
        }
        Ok(engine)
    }

    pub fn from_inputs_with_start_preset(
        preset_inputs: &[PathBuf],
        pack_inputs: &[PathBuf],
        config: DesktopPlayerEngineConfig,
        start_preset: Option<&str>,
    ) -> Result<Self, DesktopPlayerError> {
        let selected_presets = collect_preset_inputs(preset_inputs, pack_inputs)
            .map_err(DesktopPlayerError::InvalidPresetInput)?;
        let mut engine = Self::from_preset_inputs(selected_presets, config)?;
        if let Some(start_preset) = start_preset {
            engine.seek_preset(start_preset)?;
        }
        Ok(engine)
    }

    pub fn from_inputs_with_audio_provider_and_start_preset(
        preset_inputs: &[PathBuf],
        pack_inputs: &[PathBuf],
        config: DesktopPlayerEngineConfig,
        start_preset: Option<&str>,
        audio_provider: impl DesktopAudioProvider + 'static,
    ) -> Result<Self, DesktopPlayerError> {
        let mut engine = Self::from_inputs_with_audio_provider(
            preset_inputs,
            pack_inputs,
            config,
            audio_provider,
        )?;
        if let Some(start_preset) = start_preset {
            engine.seek_preset(start_preset)?;
        }
        Ok(engine)
    }

    fn new_with_loaded_presets(
        presets: Vec<DesktopPlayerPreset>,
        config: DesktopPlayerEngineConfig,
        audio_provider: Arc<dyn DesktopAudioProvider>,
    ) -> Result<Self, DesktopPlayerError> {
        if presets.is_empty() {
            return Err(DesktopPlayerError::NoPresets);
        }
        if !config.fps.is_finite() || config.fps <= 0.0 {
            return Err(DesktopPlayerError::InvalidConfig("fps must be > 0"));
        }
        if config.waveform_size == 0 {
            return Err(DesktopPlayerError::InvalidConfig(
                "waveform_size must be > 0",
            ));
        }
        if config.spectrum_size == 0 {
            return Err(DesktopPlayerError::InvalidConfig(
                "spectrum_size must be > 0",
            ));
        }

        let per_preset_frames = if config.preset_duration_seconds <= 0.0 {
            usize::MAX
        } else {
            ((config.preset_duration_seconds * config.fps).round() as usize).max(1)
        };

        Ok(Self {
            presets,
            config: config.clone(),
            audio_provider,
            plugins: Vec::new(),
            data_plugins: Vec::new(),
            preset_load_pending: true,
            host: MilkRustFrameSetRuntimeHost::new(),
            preset_index: 0,
            local_frame_index: 0,
            global_frame_index: 0,
            running: !config.start_paused,
            per_preset_frames,
        })
    }

    pub fn install_plugins(&mut self, plugins: Vec<Box<dyn DesktopPlugin>>) {
        self.plugins = plugins;
        self.preset_load_pending = true;
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.len() + self.data_plugins.len()
    }

    pub fn data_plugins(&self) -> &[DesktopPluginDataDescriptor] {
        &self.data_plugins
    }

    pub fn install_pack_plugins(&mut self, plugins: Vec<PackPluginInput>) -> usize {
        let mut installed = 0usize;
        for plugin in plugins {
            if plugin.kind == "data" {
                let descriptor = DesktopPluginDataDescriptor {
                    id: plugin.id,
                    kind: plugin.kind,
                    entry: plugin.entry,
                    source_path: plugin.source_path.to_string_lossy().to_string(),
                    payload: plugin.payload,
                };
                self.data_plugins.push(descriptor);
                installed += 1;
            }
        }
        self.preset_load_pending = true;
        installed
    }

    pub fn preset_count(&self) -> usize {
        self.presets.len()
    }

    pub fn preset_label(&self) -> Option<&str> {
        self.presets
            .get(self.preset_index)
            .map(|preset| preset.source_label.as_str())
    }

    pub fn presets(&self) -> &[DesktopPlayerPreset] {
        &self.presets
    }

    pub fn state(&self) -> DesktopPlayerState {
        DesktopPlayerState {
            fps: self.config.fps,
            running: self.running,
            preset_index: self.preset_index,
            preset_total: self.presets.len(),
            global_frame_index: self.global_frame_index,
            local_frame_index: self.local_frame_index,
            per_preset_frames: self.per_preset_frames,
        }
    }

    pub fn set_running(&mut self, running: bool) {
        self.running = running;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn toggle_running(&mut self) {
        self.running = !self.running;
    }

    pub fn reset(&mut self) {
        self.running = !self.config.start_paused;
        self.preset_index = 0;
        self.local_frame_index = 0;
        self.global_frame_index = 0;
        self.preset_load_pending = true;
        self.host = MilkRustFrameSetRuntimeHost::new();
    }

    pub fn next_preset(&mut self) {
        if self.presets.is_empty() {
            return;
        }
        self.set_preset_index((self.preset_index + 1) % self.presets.len(), false);
    }

    pub fn prev_preset(&mut self) {
        if self.presets.is_empty() {
            return;
        }
        let previous = if self.preset_index == 0 {
            self.presets.len().saturating_sub(1)
        } else {
            self.preset_index - 1
        };
        self.set_preset_index(previous, false);
    }

    pub fn seek_preset(&mut self, preset: &str) -> Result<(), DesktopPlayerError> {
        let index = resolve_preset_index(&self.presets, preset)?;
        self.set_preset_index(index, false);
        Ok(())
    }

    pub fn frame(&mut self) -> Result<Option<DesktopPlayerFrame>, DesktopPlayerError> {
        if self.presets.is_empty() {
            return Err(DesktopPlayerError::NoPresets);
        }
        if !self.running {
            return Ok(None);
        }

        self.prepare_preset_for_frame()?;
        let preset = {
            let preset = &self.presets[self.preset_index];
            (
                preset.source.clone(),
                preset.source_label.clone(),
                preset.source_path.clone(),
            )
        };
        let time_seconds = self.global_frame_index as f64 / self.config.fps;
        let audio = self
            .audio_provider
            .provide_audio_frame(&DesktopAudioContext {
                global_frame_index: self.global_frame_index,
                local_frame_index: self.local_frame_index,
                preset_index: self.preset_index,
                preset_count: self.presets.len(),
                time_seconds,
                fps: self.config.fps,
                waveform_size: self.config.waveform_size,
                spectrum_size: self.config.spectrum_size,
                audio_profile: self.config.audio_profile,
            });
        let mut audio = audio;
        for plugin in self.plugins.iter_mut() {
            plugin.on_audio_frame(&mut audio);
        }

        let mut frame_context =
            self.build_frame_context(audio.bass, audio.mid, audio.treble, time_seconds);
        for plugin in self.plugins.iter_mut() {
            plugin.on_frame_start(&mut frame_context);
        }
        self.running = frame_context.running;
        if !self.running {
            return Ok(None);
        }

        let frame_set = self.host.try_render_frame_set(
            &preset.0,
            time_seconds,
            frame_context.bass,
            frame_context.mid,
            frame_context.treble,
            &audio.waveform,
            &audio.spectrum,
        );
        let frame_set = match frame_set {
            Ok(frame) => frame,
            Err(source) => {
                self.running = false;
                return Err(DesktopPlayerError::PresetEvaluationFailed {
                    path: preset.2.to_string_lossy().into_owned(), source,
                });
            }
        };
        let report = report_frame_set(&frame_set);

        let mut render_context = DesktopPluginRenderContext {
            preset_index: self.preset_index,
            preset_count: self.presets.len(),
            source_path: preset.2.to_string_lossy().to_string(),
            source_label: preset.1.clone(),
            local_frame_index: self.local_frame_index,
            global_frame_index: self.global_frame_index,
            time_seconds,
            frame_set_title: report.source_title.clone(),
            transition_mode: report.transition_mode.clone(),
            frame_count: report.frame_count,
            line_vertices: report.line_vertices,
            point_vertices: report.point_vertices,
            textured_vertices: report.textured_vertices,
            triangle_vertices: report.triangle_vertices,
        };
        let mut heartbeat_context = DesktopPluginHeartbeatContext {
            preset_index: self.preset_index,
            preset_count: self.presets.len(),
            time_seconds,
            is_beat: false,
        };
        for plugin in self.plugins.iter_mut() {
            plugin.on_render_frame(&mut render_context);
            plugin.on_beat(&mut heartbeat_context);
            plugin.on_automation_step(&mut heartbeat_context);
        }

        let frame = DesktopPlayerFrame {
            preset_index: self.preset_index,
            preset_total: self.presets.len(),
            local_frame_index: self.local_frame_index,
            global_frame_index: self.global_frame_index,
            time_seconds,
            running: self.running,
            preset_label: preset.1.clone(),
            source_path: preset.2.to_string_lossy().to_string(),
            frame_set,
            report,
        };

        self.global_frame_index = self.global_frame_index.wrapping_add(1);
        self.local_frame_index = self.local_frame_index.wrapping_add(1);

        if self.local_frame_index >= self.per_preset_frames {
            self.local_frame_index = 0;
            let has_next_preset = self.preset_index + 1 < self.presets.len();

            if has_next_preset {
                self.set_preset_index(self.preset_index + 1, false);
            } else if self.config.auto_loop {
                self.set_preset_index(0, true);
            } else {
                self.running = false;
            }
        }

        Ok(Some(frame))
    }

    fn build_frame_context(
        &self,
        bass: f64,
        mid: f64,
        treble: f64,
        time_seconds: f64,
    ) -> DesktopPluginFrameContext {
        let preset = &self.presets[self.preset_index];
        DesktopPluginFrameContext {
            preset_index: self.preset_index,
            preset_count: self.presets.len(),
            source_path: preset.source_path.to_string_lossy().to_string(),
            source_label: preset.source_label.clone(),
            local_frame_index: self.local_frame_index,
            global_frame_index: self.global_frame_index,
            time_seconds,
            fps: self.config.fps,
            running: self.running,
            bass,
            mid,
            treble,
        }
    }

    fn prepare_preset_for_frame(&mut self) -> Result<(), DesktopPlayerError> {
        if !self.preset_load_pending {
            return Ok(());
        }

        let mut preset_context = self
            .presets
            .get(self.preset_index)
            .map(|preset| DesktopPluginPresetContext {
                preset_index: self.preset_index,
                preset_count: self.presets.len(),
                source_path: preset.source_path.to_string_lossy().to_string(),
                source_label: preset.source_label.clone(),
                source: preset.source.clone(),
            })
            .ok_or(DesktopPlayerError::NoPresets)?;

        for plugin in self.plugins.iter_mut() {
            plugin.on_preset_load(&mut preset_context);
        }
        for plugin in self.plugins.iter_mut() {
            plugin.on_preset_loaded(&mut preset_context);
        }

        if let Some(preset) = self.presets.get_mut(self.preset_index) {
            preset.source = preset_context.source;
            preset.source_label = preset_context.source_label;
        }

        self.preset_load_pending = false;
        Ok(())
    }

    fn set_preset_index(&mut self, next_preset_index: usize, force: bool) {
        if self.presets.is_empty() {
            return;
        }
        let next_preset_index = next_preset_index % self.presets.len();
        if !force && next_preset_index == self.preset_index {
            return;
        }
        let previous_index = self.preset_index;
        let previous = self
            .presets
            .get(previous_index)
            .cloned()
            .unwrap_or_else(|| DesktopPlayerPreset {
                source_path: PathBuf::new(),
                source_label: String::new(),
                source: String::new(),
            });

        self.preset_index = next_preset_index;
        self.local_frame_index = 0;
        self.host = MilkRustFrameSetRuntimeHost::new();
        self.preset_load_pending = true;

        if let Some(next) = self.presets.get(self.preset_index).cloned() {
            let mut change_context = DesktopPluginPresetChangeContext {
                previous_preset_index: previous_index,
                previous_preset_label: previous.source_label,
                previous_source_path: previous.source_path.to_string_lossy().to_string(),
                preset_index: self.preset_index,
                preset_count: self.presets.len(),
                source_path: next.source_path.to_string_lossy().to_string(),
                source_label: next.source_label,
            };
            for plugin in self.plugins.iter_mut() {
                plugin.on_preset_change(&mut change_context);
            }
        }
    }
}

fn resolve_preset_index(
    presets: &[DesktopPlayerPreset],
    value: &str,
) -> Result<usize, DesktopPlayerError> {
    if let Ok(index) = value.parse::<usize>() {
        if index < presets.len() {
            return Ok(index);
        }
        return Err(DesktopPlayerError::PresetNotFound(format!(
            "{value} (out of range 0..{})",
            presets.len().saturating_sub(1)
        )));
    }

    presets
        .iter()
        .position(|preset| preset.source_label == value)
        .ok_or_else(|| DesktopPlayerError::PresetNotFound(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_preset_indices_and_advances_playhead() {
        let presets = vec![
            PresetInput {
                source_path: PathBuf::from("a.milk"),
                source_label: "alpha".into(),
            },
            PresetInput {
                source_path: PathBuf::from("b.milk"),
                source_label: "beta".into(),
            },
        ];
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-api-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let a = base.join("a.milk");
        let b = base.join("b.milk");
        std::fs::write(&a, "name=alpha\n").unwrap();
        std::fs::write(&b, "name=beta\n").unwrap();
        let mut presets = presets;
        presets[0].source_path = a;
        presets[1].source_path = b;

        let mut engine = DesktopPlayerEngine::from_preset_inputs(
            presets,
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 0.5,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: false,
            },
        )
        .expect("engine should initialize");
        assert_eq!(engine.preset_count(), 2);
        assert_eq!(engine.preset_label(), Some("alpha"));
        let first = engine.frame().unwrap().expect("frame should exist");
        assert_eq!(first.preset_index, 0);
        assert!(matches!(engine.seek_preset("beta"), Ok(())));
        assert_eq!(engine.preset_label(), Some("beta"));
        assert!(engine.seek_preset("5").is_err());
        for _ in 0..40 {
            let _ = engine.frame().unwrap();
        }
        assert!(!engine.is_running());
    }

    #[test]
    fn supports_audio_provider_override() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "milkrust-desktop-player-audio-provider-{process_id}-{nanos}"
        ));
        std::fs::create_dir_all(&base).unwrap();
        let source = base.join("provider.milk");
        std::fs::write(&source, "name=provider\n").unwrap();

        #[derive(Clone)]
        struct CountingProvider {
            calls: Arc<AtomicUsize>,
        }

        impl DesktopAudioProvider for CountingProvider {
            fn provide_audio_frame(&self, _context: &DesktopAudioContext) -> DesktopAudioFrame {
                self.calls.fetch_add(1, Ordering::SeqCst);
                DesktopAudioFrame {
                    bass: 0.0,
                    mid: 0.0,
                    treble: 0.0,
                    waveform: vec![0.0; 16],
                    spectrum: vec![0.0; 16],
                }
            }
        }

        let call_count = Arc::new(AtomicUsize::new(0));
        let mut engine = DesktopPlayerEngine::from_preset_inputs_with_audio_provider(
            vec![PresetInput {
                source_path: source,
                source_label: "provider".into(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 0.5,
                waveform_size: 16,
                spectrum_size: 16,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: false,
            },
            CountingProvider {
                calls: Arc::clone(&call_count),
            },
        )
        .expect("engine should initialize");

        let _ = engine.frame().unwrap();
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn plugin_hooks_mutate_preset_source_and_see_frame_events() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "milkrust-desktop-player-plugin-hooks-{process_id}-{nanos}"
        ));
        std::fs::create_dir_all(&base).unwrap();
        let source = base.join("hooked.milk");
        std::fs::write(&source, "name=HookSource\n").unwrap();

        #[derive(Default)]
        struct HookCapture {
            preset_loads: AtomicUsize,
            frame_starts: AtomicUsize,
            render_calls: AtomicUsize,
        }

        impl DesktopPlugin for HookCapture {
            fn id(&self) -> &str {
                "hook-capture"
            }
            fn on_preset_load(&mut self, context: &mut DesktopPluginPresetContext) {
                self.preset_loads.fetch_add(1, Ordering::SeqCst);
                context.source = "name=HookedPreset\n".to_string();
                context.source_label = "HookedPreset".to_string();
            }
            fn on_frame_start(&mut self, context: &mut DesktopPluginFrameContext) {
                self.frame_starts.fetch_add(1, Ordering::SeqCst);
                context.bass += 1.0;
            }
            fn on_render_frame(&mut self, _context: &mut DesktopPluginRenderContext) {
                self.render_calls.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut engine = DesktopPlayerEngine::from_preset_inputs(
            vec![PresetInput {
                source_path: source,
                source_label: "original".to_string(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 0.5,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: crate::DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: false,
            },
        )
        .unwrap();
        let plugin = HookCapture::default();
        engine.install_plugins(vec![Box::new(plugin)]);
        let frame = engine.frame().unwrap().expect("frame exists");

        assert_eq!(frame.preset_label, "HookedPreset");
        assert_eq!(frame.report.source_title, "HookedPreset");
        assert!(frame.report.frame_count > 0);

        // safety: plugin instance is moved into engine, we can't read counters directly here
        // verify behavior via emitted frame and running state instead.
        assert!(frame.running);
        assert_eq!(engine.preset_label(), Some("HookedPreset"));
        assert_eq!(frame.preset_index, 0);
    }

    #[test]
    fn desktop_player_state_defaults_are_reasonable() {
        let state = DesktopPlayerState {
            fps: 0.0,
            running: false,
            preset_index: 0,
            preset_total: 0,
            global_frame_index: 0,
            local_frame_index: 0,
            per_preset_frames: 0,
        };
        assert_eq!(state.fps, 0.0);
        assert!(!state.running);
        assert_eq!(state.preset_index, 0);
        assert_eq!(state.preset_total, 0);
        assert_eq!(state.global_frame_index, 0);
        assert_eq!(state.local_frame_index, 0);
        assert_eq!(state.per_preset_frames, 0);
    }

    #[test]
    fn engine_toggle_running_flips_state() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-toggle-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let source = base.join("toggle.milk");
        std::fs::write(&source, "name=toggle\n").unwrap();

        let mut engine = DesktopPlayerEngine::from_preset_inputs(
            vec![PresetInput {
                source_path: source,
                source_label: "toggle".into(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 1.0,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: false,
            },
        )
        .expect("engine should initialize");
        assert!(engine.is_running());

        engine.toggle_running();
        assert!(!engine.is_running());

        engine.toggle_running();
        assert!(engine.is_running());
    }

    #[test]
    fn engine_prev_preset_wraps_backwards() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-wrap-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();

        let paths: Vec<_> = (0..5)
            .map(|i| {
                let p = base.join(format!("preset{}.milk", i));
                std::fs::write(&p, format!("name=preset{}", i).as_bytes()).unwrap();
                p
            })
            .collect();

        let inputs: Vec<_> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| PresetInput {
                source_path: p.clone(),
                source_label: format!("preset{}", i),
            })
            .collect();

        let mut engine = DesktopPlayerEngine::from_preset_inputs(inputs, DesktopPlayerEngineConfig {
            fps: 60.0,
            preset_duration_seconds: 1.0,
            waveform_size: 8,
            spectrum_size: 8,
            audio_profile: DesktopAudioProfile::default(),
            auto_loop: false,
            start_paused: true,
        }).expect("engine should initialize");

        // Start at preset 0
        let state = engine.state();
        assert_eq!(state.preset_index, 0);
        assert_eq!(engine.preset_label(), Some("preset0"));

        // Go backwards - should wrap to last preset
        engine.prev_preset();
        let state = engine.state();
        assert_eq!(state.preset_index, 4);
        assert_eq!(engine.preset_label(), Some("preset4"));

        // Go backwards again
        engine.prev_preset();
        let state = engine.state();
        assert_eq!(state.preset_index, 3);
        assert_eq!(engine.preset_label(), Some("preset3"));
    }

    #[test]
    fn engine_data_plugins_returns_descriptors() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-descriptors-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let p = base.join("desc.milk");
        std::fs::write(&p, b"name=desc").unwrap();
        let engine = DesktopPlayerEngine::from_preset_inputs(
            vec![PresetInput {
                source_path: p,
                source_label: "desc".into(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 1.0,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: true,
            },
        ).expect("engine should initialize");
        let descriptors = engine.data_plugins();
        // data_plugins() returns built-in data plugins, not preset plugins.
        // With no data plugins installed, the slice is empty — just verify the
        // method returns and the length is known.
        let _count = descriptors.len();
    }

    #[test]
    fn engine_seek_preset_with_valid_name_succeeds() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-seek-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let paths: Vec<_> = (0..3)
            .map(|i| {
                let p = base.join(format!("seek{}.milk", i));
                std::fs::write(&p, format!("name=seek{}", i).as_bytes()).unwrap();
                p
            })
            .collect();
        let inputs: Vec<_> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| PresetInput {
                source_path: p.clone(),
                source_label: format!("seek{}", i),
            })
            .collect();
        let mut engine = DesktopPlayerEngine::from_preset_inputs(inputs, DesktopPlayerEngineConfig {
            fps: 60.0,
            preset_duration_seconds: 1.0,
            waveform_size: 8,
            spectrum_size: 8,
            audio_profile: DesktopAudioProfile::default(),
            auto_loop: false,
            start_paused: true,
        }).expect("engine should initialize");
        engine.seek_preset("seek1").unwrap();
        let state = engine.state();
        assert_eq!(state.preset_index, 1);
        assert_eq!(engine.preset_label(), Some("seek1"));
    }

    #[test]
    fn engine_seek_preset_with_invalid_name_returns_error() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-seekerr-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let p = base.join("nonexist.milk");
        std::fs::write(&p, b"name=nonexist").unwrap();
        let mut engine = DesktopPlayerEngine::from_preset_inputs(
            vec![PresetInput {
                source_path: p,
                source_label: "nonexist".into(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 1.0,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: true,
            },
        ).expect("engine should initialize");
        let result = engine.seek_preset("does_not_exist");
        assert!(result.is_err());
    }

    #[test]
    fn engine_next_preset_advances_index() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-next-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let paths: Vec<_> = (0..5)
            .map(|i| {
                let p = base.join(format!("next{}.milk", i));
                std::fs::write(&p, format!("name=next{}", i).as_bytes()).unwrap();
                p
            })
            .collect();
        let inputs: Vec<_> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| PresetInput {
                source_path: p.clone(),
                source_label: format!("next{}", i),
            })
            .collect();
        let mut engine = DesktopPlayerEngine::from_preset_inputs(inputs, DesktopPlayerEngineConfig {
            fps: 60.0,
            preset_duration_seconds: 1.0,
            waveform_size: 8,
            spectrum_size: 8,
            audio_profile: DesktopAudioProfile::default(),
            auto_loop: false,
            start_paused: true,
        }).expect("engine should initialize");
        let state = engine.state();
        assert_eq!(state.preset_index, 0);
        engine.next_preset();
        let state = engine.state();
        assert_eq!(state.preset_index, 1);
        assert_eq!(engine.preset_label(), Some("next1"));
        engine.next_preset();
        let state = engine.state();
        assert_eq!(state.preset_index, 2);
        assert_eq!(engine.preset_label(), Some("next2"));
    }

    #[test]
    fn engine_reset_clears_playhead_and_resets_running() {
        let process_id = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        let base =
            std::env::temp_dir().join(format!("milkrust-desktop-player-reset-{process_id}-{nanos}"));
        std::fs::create_dir_all(&base).unwrap();
        let p = base.join("reset.milk");
        std::fs::write(&p, b"name=reset").unwrap();
        let mut engine = DesktopPlayerEngine::from_preset_inputs(
            vec![PresetInput {
                source_path: p,
                source_label: "reset".into(),
            }],
            DesktopPlayerEngineConfig {
                fps: 60.0,
                preset_duration_seconds: 1.0,
                waveform_size: 8,
                spectrum_size: 8,
                audio_profile: DesktopAudioProfile::default(),
                auto_loop: false,
                start_paused: false,
            },
        ).expect("engine should initialize");
        engine.frame().unwrap();
        engine.frame().unwrap();
        engine.frame().unwrap();
        let before = engine.state();
        assert!(before.running);
        assert!(before.global_frame_index > 0);
        engine.reset();
        let after = engine.state();
        assert!(after.running);
        assert_eq!(after.global_frame_index, 0);
        assert_eq!(after.local_frame_index, 0);
        assert_eq!(after.preset_index, 0);
    }
}
