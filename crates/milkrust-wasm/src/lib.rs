mod renderer;

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub use renderer::{milkrust_renderer, MilkRustRenderer};
use milkrust_core::{
    parse_milkrust_fragment, parse_milkrust_preset_set, parse_milkrust_sample_csv,
    milkrust_preset_name, milkrust_webgpu_batch_summary_json, serialize_milkrust_fragment,
    serialize_milkrust_preset_set, validate_milkrust_webgl_import as validate_milkrust_import, MilkRustFrameSetRuntime,
    MilkRustIndexedEntry, MilkRustInputState, MilkRustValue,
};
use wasm_bindgen::prelude::*;

/// Initialize the matching compiler module before importing shader presets.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = initializeHlslCompiler)]
pub fn initialize_hlsl_compiler(url: &str) -> js_sys::Promise {
    milkrust_hlsl::initialize(url)
}

#[wasm_bindgen(js_name = milkrustWebGpuBatchSummaryJson)]
pub fn wasm_milkrust_webgpu_batch_summary_json(
    source: &str,
    time_seconds: f64,
    bass: f64,
    mid: f64,
    treble: f64,
    waveform_csv: &str,
    spectrum_csv: &str,
) -> String {
    milkrust_webgpu_batch_summary_json(
        source,
        time_seconds,
        bass,
        mid,
        treble,
        &parse_milkrust_sample_csv(waveform_csv),
        &parse_milkrust_sample_csv(spectrum_csv),
    )
}

#[wasm_bindgen(js_name = MilkRustEngine)]
pub struct WasmMilkRustEngine {
    canvas: web_sys::HtmlCanvasElement,
    renderer: MilkRustRenderer,
    runtime: MilkRustFrameSetRuntime,
    source: String,
    texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
    title: String,
    render_error: Option<String>,
    waiting_for_textures: bool,
}

#[wasm_bindgen(js_class = MilkRustEngine)]
impl WasmMilkRustEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: web_sys::HtmlCanvasElement) -> Result<WasmMilkRustEngine, JsValue> {
        let source = milkrust_core::MILKRUST_PRESETS
            .first()
            .copied()
            .unwrap_or_default()
            .to_string();
        let title =
            validate_milkrust_import(&source).unwrap_or_else(|_| milkrust_preset_name(&source));
        let texture_assets = Rc::new(RefCell::new(BTreeMap::new()));
        let renderer = milkrust_renderer(&canvas, texture_assets.clone())?;
        Ok(Self {
            canvas,
            renderer,
            runtime: MilkRustFrameSetRuntime::default(),
            source,
            texture_assets,
            title,
            render_error: None,
            waiting_for_textures: false,
        })
    }

    #[wasm_bindgen(js_name = rendererLabel)]
    pub fn renderer_label(&self) -> String {
        if let Some(error) = &self.render_error { format!("Renderer stopped: {error}") }
        else if self.waiting_for_textures { "Waiting for preset textures".to_string() }
        else { self.renderer.label().to_string() }
    }

    #[wasm_bindgen(js_name = renderStatusJson)]
    pub fn render_status_json(&self) -> String {
        serde_json::json!({"state": if self.render_error.is_some() { "failed" }
            else if self.waiting_for_textures { "waiting-textures" } else { "preview" },
            "error": self.render_error, "fidelity": "not-assessed"}).to_string()
    }

    #[wasm_bindgen(js_name = loadPresetText)]
    pub fn load_preset_text(
        &mut self,
        source: &str,
        file_name: &str,
        texture_assets_json: &str,
    ) -> Result<String, JsValue> {
        let title = validate_milkrust_import(source).map_err(|error| JsValue::from_str(&error))?;
        let texture_assets = Self::parse_texture_assets(texture_assets_json)?;
        self.source = source.to_string();
        self.title = if title.trim().is_empty() {
            if file_name.trim().is_empty() {
                "Imported MilkRust preset".to_string()
            } else {
                file_name.to_string()
            }
        } else {
            title
        };
        self.runtime = MilkRustFrameSetRuntime::default();
        self.renderer.reset();
        *self.texture_assets.borrow_mut() = texture_assets;
        self.render_error = None;
        self.waiting_for_textures = false;
        Ok(self.title.clone())
    }

    #[wasm_bindgen(js_name = inspectPresetText)]
    pub fn inspect_preset_text(&self, source: &str, file_name: &str) -> Result<String, JsValue> {
        let title = validate_milkrust_import(source).map_err(|error| JsValue::from_str(&error))?;
        Ok(serde_json::json!({
            "title": if title.trim().is_empty() {
                if file_name.trim().is_empty() { "Imported MilkRust preset" } else { file_name }
            } else {
                &title
            }
        })
        .to_string())
    }

    #[wasm_bindgen(js_name = loadPresetFragmentText)]
    pub fn load_preset_fragment_text(
        &mut self,
        source: &str,
        file_name: &str,
        requested_type: &str,
        texture_assets_json: &str,
    ) -> Result<String, JsValue> {
        let mut parsed = self.parsed_source();
        let fragment = parse_milkrust_fragment(source, file_name, requested_type);
        if !fragment.parse_errors.is_empty() {
            return Err(JsValue::from_str(&fragment.parse_errors.join("; ")));
        }
        let Some(target) = parsed.presets.first_mut() else {
            return Err(JsValue::from_str("MilkRust preset is empty"));
        };
        let target_entries = if fragment.fragment_type == "wave" {
            &mut target.waves
        } else {
            &mut target.shapes
        };
        target_entries.extend(fragment.entries);
        let merged_source = serialize_milkrust_preset_set(&parsed);
        let title = format!(
            "{} + {}",
            self.title,
            if file_name.trim().is_empty() {
                fragment.fragment_type.as_str()
            } else {
                file_name
            }
        );
        self.load_preset_text(&merged_source, &title, texture_assets_json)?;
        Ok(serde_json::json!({ "source": merged_source, "title": title }).to_string())
    }

    #[wasm_bindgen(js_name = removePresetFragment)]
    pub fn remove_preset_fragment(
        &mut self,
        requested_type: &str,
        index: usize,
        texture_assets_json: &str,
    ) -> Result<String, JsValue> {
        let mut parsed = self.parsed_source();
        let Some(target) = parsed.presets.first_mut() else {
            return Ok("null".to_string());
        };
        let target_entries = if requested_type == "wave" {
            &mut target.waves
        } else {
            &mut target.shapes
        };
        if index >= target_entries.len() {
            return Ok("null".to_string());
        }
        target_entries.remove(index);
        let source = serialize_milkrust_preset_set(&parsed);
        let title = format!("{} - {} {}", self.title, requested_type, index + 1);
        self.load_preset_text(&source, &title, texture_assets_json)?;
        Ok(serde_json::json!({ "source": source, "title": title }).to_string())
    }

    #[wasm_bindgen(js_name = updatePresetBaseValue)]
    pub fn update_preset_base_value(
        &mut self,
        key: &str,
        value: f64,
        texture_assets_json: &str,
    ) -> Result<String, JsValue> {
        if !matches!(
            key,
            "decay" | "zoom" | "rot" | "wave_r" | "wave_g" | "wave_b" | "wave_a"
        ) || !value.is_finite()
        {
            return Ok("null".to_string());
        }
        let mut parsed = self.parsed_source();
        let Some(target) = parsed.presets.first_mut() else {
            return Ok("null".to_string());
        };
        target
            .base_values
            .insert(key.to_string(), MilkRustValue::Number(value));
        let source = serialize_milkrust_preset_set(&parsed);
        let title = format!("{} edited", self.title);
        self.load_preset_text(&source, &title, texture_assets_json)?;
        Ok(serde_json::json!({
            "source": source,
            "title": title,
            "values": self.preset_parameter_summary_value()
        })
        .to_string())
    }

    #[wasm_bindgen(js_name = randomizePresetParameters)]
    pub fn randomize_preset_parameters(
        &mut self,
        texture_assets_json: &str,
    ) -> Result<String, JsValue> {
        let mut parsed = self.parsed_source();
        let Some(target) = parsed.presets.first_mut() else {
            return Ok("null".to_string());
        };
        for (key, min, max, fallback) in [
            ("decay", 0.5, 1.0, 0.9),
            ("zoom", 0.5, 1.5, 1.0),
            ("rot", -0.5, 0.5, 0.0),
            ("wave_r", 0.0, 1.0, 0.7),
            ("wave_g", 0.0, 1.0, 0.7),
            ("wave_b", 0.0, 1.0, 0.7),
            ("wave_a", 0.0, 1.0, 1.0),
        ] {
            let current = target
                .base_values
                .get(key)
                .and_then(MilkRustValue::as_number)
                .unwrap_or(fallback);
            let jittered = min + js_sys::Math::random() * (max - min);
            target.base_values.insert(
                key.to_string(),
                MilkRustValue::Number(((current + jittered) / 2.0 * 100.0).round() / 100.0),
            );
        }
        let source = serialize_milkrust_preset_set(&parsed);
        let title = format!("{} randomized", self.title);
        self.load_preset_text(&source, &title, texture_assets_json)?;
        Ok(serde_json::json!({
            "source": source,
            "title": title,
            "values": self.preset_parameter_summary_value()
        })
        .to_string())
    }

    #[wasm_bindgen(js_name = exportPresetText)]
    pub fn export_preset_text(&self) -> String {
        serde_json::json!({
            "fileName": format!(
                "{}.{}",
                sanitize_milkrust_file_name(&self.title),
                self.parsed_source().format
            ),
            "source": self.source
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = exportPresetFragment)]
    pub fn export_preset_fragment(&self, requested_type: &str, index: usize) -> String {
        let parsed = self.parsed_source();
        let Some(target) = parsed.presets.first() else {
            return "null".to_string();
        };
        let entries = if requested_type == "wave" {
            &target.waves
        } else {
            &target.shapes
        };
        let Some(entry) = entries.get(index) else {
            return "null".to_string();
        };
        serde_json::json!({
            "fileName": format!("{}.{}", sanitize_milkrust_file_name(&self.title), requested_type),
            "source": serialize_milkrust_fragment(entry, requested_type)
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = getPresetFragmentSummaryJson)]
    pub fn get_preset_fragment_summary_json(&self) -> String {
        serde_json::json!(self.preset_fragment_summary_value()).to_string()
    }

    #[wasm_bindgen(js_name = getPresetParameterSummaryJson)]
    pub fn get_preset_parameter_summary_json(&self) -> String {
        serde_json::json!(self.preset_parameter_summary_value()).to_string()
    }

    #[wasm_bindgen(js_name = getPresetDebugSnapshotJson)]
    pub fn get_preset_debug_snapshot_json(&self, web_gpu_status_json: &str) -> String {
        let parsed = self.parsed_source();
        let primary = parsed.presets.first();
        let web_gpu_status = serde_json::from_str::<serde_json::Value>(web_gpu_status_json)
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "available": false,
                    "backend": "milkrust-standalone",
                    "reason": "not checked"
                })
            });
        serde_json::json!({
            "format": parsed.format,
            "parameters": self.preset_parameter_summary_value(),
            "presetCount": parsed.presets.len(),
            "renderer": self.renderer_label(),
            "renderError": self.render_error,
            "fidelity": "not-assessed",
            "shaderSections": {
                "comp": primary.is_some_and(|preset| !preset.comp_shader.trim().is_empty()),
                "warp": primary.is_some_and(|preset| !preset.warp_shader.trim().is_empty())
            },
            "shapes": primary.map(|preset| preset.shapes.len()).unwrap_or_default(),
            "sprites": primary.map(|preset| preset.sprites.len()).unwrap_or_default(),
            "title": self.title,
            "waves": primary.map(|preset| preset.waves.len()).unwrap_or_default(),
            "webGpu": web_gpu_status
        })
        .to_string()
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = render)]
    pub fn render(
        &mut self,
        time_seconds: f64,
        bass: f64,
        mid: f64,
        treble: f64,
        waveform_csv: &str,
        spectrum_csv: &str,
        mouse_down: f64,
        mouse_x: f64,
        mouse_y: f64,
        mouse_dx: f64,
        mouse_dy: f64,
    ) {
        if self.render_error.is_some() { return; }
        let input = MilkRustInputState {
            mouse_down,
            mouse_dx,
            mouse_dy,
            mouse_x,
            mouse_y,
        };
        let waveform = parse_milkrust_sample_csv(waveform_csv);
        let spectrum = parse_milkrust_sample_csv(spectrum_csv);
        let frame_set = self.runtime.try_render_source_with_audio_and_input(
            &self.source,
            time_seconds,
            bass,
            mid,
            treble,
            &waveform,
            &spectrum,
            input,
        );
        let frame_set = match frame_set {
            Ok(frame) => frame,
            Err(error) => {
                self.waiting_for_textures = false;
                self.render_error = Some(error);
                return;
            }
        };
        match self.renderer.render_frame_set(&frame_set, time_seconds) {
            Ok(rendered) => self.waiting_for_textures = !rendered,
            Err(error) => {
                self.waiting_for_textures = false;
                self.render_error = Some(error.as_string().unwrap_or_else(|| "Native WebGL operation failed".to_string()));
            }
        }
    }

    #[wasm_bindgen(js_name = resize)]
    pub fn resize(&self, width: u32, height: u32) {
        let scale = (4096.0 / width.max(1) as f64).min(4096.0 / height.max(1) as f64)
            .min((8.0 * 1024.0 * 1024.0 / (width.max(1) as f64 * height.max(1) as f64)).sqrt()).min(1.0);
        let width = (width.max(1) as f64 * scale).floor().max(1.0) as u32;
        let height = (height.max(1) as f64 * scale).floor().max(1.0) as u32;
        if self.canvas.width() != width { self.canvas.set_width(width); }
        if self.canvas.height() != height { self.canvas.set_height(height); }
    }
}

impl WasmMilkRustEngine {
    fn parsed_source(&self) -> milkrust_core::MilkRustPresetSet {
        parse_milkrust_preset_set(
            &self.source,
            self.source.to_ascii_lowercase().contains("[preset01]"),
        )
    }

    fn parse_texture_assets(texture_assets_json: &str) -> Result<BTreeMap<String, String>, JsValue> {
        if texture_assets_json.len() > 32 * 1024 * 1024 {
            return Err(JsValue::from_str("Embedded texture map exceeds 32 MiB"));
        }
        let value = serde_json::from_str::<serde_json::Value>(texture_assets_json)
            .map_err(|_| JsValue::from_str("Malformed texture map"))?;
        let object = value.as_object().ok_or_else(|| JsValue::from_str("Texture map must be an object"))?;
        if object.len() > 64 { return Err(JsValue::from_str("Texture map exceeds 64 images")); }
        let mut assets = BTreeMap::new();
        for (name, data_url) in object {
            let data_url = data_url.as_str().ok_or_else(|| JsValue::from_str("Texture value must be an embedded image"))?;
            if name.is_empty() || name.len() > 240 || data_url.len() > 24 * 1024 * 1024 ||
                !["png", "jpeg", "gif", "webp", "bmp"].iter().any(|kind|
                    data_url.starts_with(&format!("data:image/{kind};base64,"))) {
                return Err(JsValue::from_str("Unsupported texture name, size or embedded raster format"));
            }
            for alias in milkrust_core::get_milkrust_texture_name_aliases(name) {
                if assets.get(&alias).is_some_and(|existing| existing != data_url) {
                    return Err(JsValue::from_str("Texture names have conflicting aliases"));
                }
                assets.insert(alias, data_url.to_string());
            }
        }
        Ok(assets)
    }

    fn preset_parameter_summary_value(&self) -> serde_json::Value {
        let parsed = self.parsed_source();
        let values = parsed.presets.first().map(|preset| &preset.base_values);
        let mut summary = serde_json::Map::new();
        for key in [
            "decay", "zoom", "rot", "wave_r", "wave_g", "wave_b", "wave_a",
        ] {
            if let Some(value) = values
                .and_then(|values| values.get(key))
                .and_then(MilkRustValue::as_number)
            {
                summary.insert(key.to_string(), serde_json::json!(value));
            }
        }
        serde_json::Value::Object(summary)
    }

    fn preset_fragment_summary_value(&self) -> serde_json::Value {
        let parsed = self.parsed_source();
        let Some(primary) = parsed.presets.first() else {
            return serde_json::json!({ "shapes": [], "waves": [] });
        };
        serde_json::json!({
            "shapes": primary.shapes.iter().enumerate().map(|(index, entry)| {
                serde_json::json!({
                    "index": index,
                    "label": milkrust_fragment_entry_label(entry, index, "shape")
                })
            }).collect::<Vec<_>>(),
            "waves": primary.waves.iter().enumerate().map(|(index, entry)| {
                serde_json::json!({
                    "index": index,
                    "label": milkrust_fragment_entry_label(entry, index, "wave")
                })
            }).collect::<Vec<_>>()
        })
    }
}

fn sanitize_milkrust_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "MilkRust_preset".to_string()
    } else {
        trimmed.to_string()
    }
}

fn milkrust_fragment_entry_label(
    entry: &MilkRustIndexedEntry,
    index: usize,
    requested_type: &str,
) -> String {
    let prefix = if requested_type == "wave" {
        "Wave"
    } else {
        "Shape"
    };
    for key in ["name", "label", "tex_name", "texname", "texture", "image"] {
        let value = entry
            .base_values
            .get(key)
            .map(MilkRustValue::as_text)
            .unwrap_or_default();
        if !value.trim().is_empty() {
            return format!("{prefix} {}: {value}", index + 1);
        }
    }
    if requested_type == "wave" {
        for key in ["samples", "nsamples"] {
            let value = entry
                .base_values
                .get(key)
                .map(MilkRustValue::as_text)
                .unwrap_or_default();
            if !value.trim().is_empty() {
                return format!("{prefix} {}: {value} samples", index + 1);
            }
        }
    } else {
        for key in ["sides", "numsides"] {
            let value = entry
                .base_values
                .get(key)
                .map(MilkRustValue::as_text)
                .unwrap_or_default();
            if !value.trim().is_empty() {
                return format!("{prefix} {}: {value} sides", index + 1);
            }
        }
    }
    format!("{prefix} {}", index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_file_name_keeps_alphanumeric_and_dots_dashes_underscores() {
        assert_eq!(sanitize_milkrust_file_name("my_preset-1.0"), "my_preset-1.0");
    }

    #[test]
    fn sanitize_file_name_replaces_special_chars() {
        assert_eq!(sanitize_milkrust_file_name("hello@world!"), "hello_world");
    }

    #[test]
    fn sanitize_file_name_fallback_for_empty_or_pure_special_chars() {
        assert_eq!(sanitize_milkrust_file_name(""), "MilkRust_preset");
        assert_eq!(sanitize_milkrust_file_name("!!!"), "MilkRust_preset");
        assert_eq!(sanitize_milkrust_file_name("___"), "MilkRust_preset");
    }

    #[test]
    fn sanitize_file_name_trims_leading_trailing_underscores() {
        assert_eq!(sanitize_milkrust_file_name("__hello__"), "hello");
    }

    #[test]
    fn parse_sample_csv_basic() {
        let result = parse_milkrust_sample_csv("0,0.5,1.0");
        assert_eq!(result, vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn parse_sample_csv_empty() {
        let result = parse_milkrust_sample_csv("");
        assert_eq!(result, Vec::<f64>::new());
    }

    #[test]
    fn parse_sample_csv_single_value() {
        let result = parse_milkrust_sample_csv("42.0");
        assert_eq!(result, vec![1.0]);
    }

    #[test]
    fn parse_sample_csv_negative_values() {
        let result = parse_milkrust_sample_csv("-1.0,0.0,1.0");
        assert_eq!(result, vec![-1.0, 0.0, 1.0]);
    }

    #[test]
    fn parse_sample_csv_whitespace_stripping() {
        let result = parse_milkrust_sample_csv(" 1.0 , 2.0 , 3.0 ");
        assert_eq!(result, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn fragment_entry_label_uses_name_field() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::from_iter([
                ("name".to_string(), MilkRustValue::Text("MyShape".to_string())),
            ]),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 0, "shape");
        assert_eq!(label, "Shape 1: MyShape");
    }

    #[test]
    fn fragment_entry_label_uses_fallback_for_shape() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::new(),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 3, "shape");
        assert_eq!(label, "Shape 4");
    }

    #[test]
    fn fragment_entry_label_uses_fallback_for_wave() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::new(),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 1, "wave");
        assert_eq!(label, "Wave 2");
    }

    #[test]
    fn fragment_entry_label_prefers_name_over_label() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::from_iter([
                ("label".to_string(), MilkRustValue::Text("LabelText".to_string())),
                ("name".to_string(), MilkRustValue::Text("NameText".to_string())),
            ]),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 0, "shape");
        assert_eq!(label, "Shape 1: NameText");
    }

    #[test]
    fn fragment_entry_label_uses_samples_for_wave() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::from_iter([
                ("samples".to_string(), MilkRustValue::Text("64".to_string())),
            ]),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 2, "wave");
        assert_eq!(label, "Wave 3: 64 samples");
    }

    #[test]
    fn fragment_entry_label_uses_sides_for_shape() {
        let entry = MilkRustIndexedEntry {
            base_values: std::collections::BTreeMap::from_iter([
                ("sides".to_string(), MilkRustValue::Text("8".to_string())),
            ]),
            equations: milkrust_core::MilkRustEquations::default(),
            initialized: false,
        };
        let label = milkrust_fragment_entry_label(&entry, 0, "shape");
        assert_eq!(label, "Shape 1: 8 sides");
    }
}
