use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use milkrust_core::{
    build_milkrust_compatibility_entry, parse_milkrust_preset_set,
    MilkRustFrameSetRuntime, MilkRustInputState, milkrust_preset_name,
    summarize_milkrust_compatibility_matrix, validate_milkrust_import,
    MilkRustCompatibilityEntry,
};
use milkrust_pack::{load_milkrust_pack_manifest, validate_milkrust_pack_dir};
use milkrust_renderer_core::MilkRustRenderer;
use milkrust_renderer_headless::{create_headless_batches, MilkRustHeadlessRenderer};

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustCliResult {
    pub code: i32,
    pub stderr: String,
    pub stdout: String,
}

impl MilkRustCliResult {
    fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stderr: String::new(),
            stdout: stdout.into(),
        }
    }

    fn err(code: i32, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stderr: stderr.into(),
            stdout: String::new(),
        }
    }
}

pub fn run_milkrust_cli(args: &[String]) -> MilkRustCliResult {
    let Some(command) = args.first().map(String::as_str) else {
        return MilkRustCliResult::err(2, usage());
    };

    match command {
        "validate" => match read_one_path(args.get(1)) {
            Ok((path, source)) => validate_milkrust_import(&source)
                .map(|title| {
                    MilkRustCliResult::ok(format!(
                        "{}\n",
                        serde_json::json!({
                            "path": path.display().to_string(),
                            "status": "valid",
                            "title": title,
                        })
                    ))
                })
                .unwrap_or_else(|error| {
                    MilkRustCliResult::err(
                        1,
                        format!(
                            "{}\n",
                            serde_json::json!({
                                "path": path.display().to_string(),
                                "status": "invalid",
                                "error": error,
                            })
                        ),
                    )
                }),
            Err(result) => result,
        },
        "inspect" => match read_one_path(args.get(1)) {
            Ok((path, source)) => {
                let parsed = parse_milkrust_preset_set(
                    &source,
                    source.to_ascii_lowercase().contains("[preset01]"),
                );
                let presets = parsed
                    .presets
                    .iter()
                    .map(|preset| {
                        serde_json::json!({
                            "index": preset.index,
                            "title": preset.title,
                            "baseValueKeys": preset.base_values.keys().cloned().collect::<Vec<_>>(),
                            "shapeCount": preset.shapes.len(),
                            "spriteCount": preset.sprites.len(),
                            "waveCount": preset.waves.len(),
                            "hasWarpShader": !preset.warp_shader.trim().is_empty(),
                            "hasCompShader": !preset.comp_shader.trim().is_empty(),
                        })
                    })
                    .collect::<Vec<_>>();
                MilkRustCliResult::ok(format!(
                    "{}\n",
                    serde_json::json!({
                        "format": parsed.format,
                        "path": path.display().to_string(),
                        "presetCount": parsed.presets.len(),
                        "presets": presets,
                        "title": milkrust_preset_name(&source),
                    })
                ))
            }
            Err(result) => result,
        },
        "compat" => compat_command(args.get(1)),
        "pack-inspect" => pack_inspect_command(args.get(1)),
        "pack-validate" => pack_validate_command(args.get(1)),
        "render-stats" => match read_one_path(args.get(1)) {
            Ok((path, source)) => {
                let frame_set = MilkRustFrameSetRuntime::default().try_render_source_with_audio_and_input(
                    &source,
                    1.0,
                    0.55,
                    0.35,
                    0.25,
                    &[-1.0, -0.25, 0.0, 0.25, 1.0],
                    &[0.0, 0.2, 0.7, 1.0, 0.4],
                    MilkRustInputState::default(),
                );
                let frame_set = match frame_set {
                    Ok(frame) => frame,
                    Err(error) => return MilkRustCliResult::err(1, format!("preset equation evaluation failed: {error}\n")),
                };
                let batches = create_headless_batches(&frame_set);
                let mut renderer = MilkRustHeadlessRenderer::new();
                let stats = match renderer.render_frame_set(&frame_set) {
                    Ok(stats) => stats,
                    Err(infallible) => match infallible {},
                };
                MilkRustCliResult::ok(format!(
                    "{}\n",
                    serde_json::json!({
                        "path": path.display().to_string(),
                        "frameEntries": stats.frame_entries,
                        "lineVertices": stats.line_vertices,
                        "pointVertices": stats.point_vertices,
                        "texturedVertices": stats.textured_vertices,
                        "triangleVertices": stats.triangle_vertices,
                        "renderStatus": "not-run",
                        "resultKind": "geometry-statistics-only",
                        "batches": {
                            "filledVertices": batches.filled_vertices.len() / 6,
                            "lineVertices": batches.line_vertices.len() / 6,
                            "pointVertices": batches.point_vertices.len() / 6,
                            "texturedVertices": batches.textured_vertices.len() / 8,
                            "texturedBatches": batches.textured_batches.len(),
                        }
                    })
                ))
            }
            Err(result) => result,
        },
        "--help" | "-h" | "help" => MilkRustCliResult::ok(usage()),
        _ => MilkRustCliResult::err(2, usage()),
    }
}

fn compat_command(path: Option<&String>) -> MilkRustCliResult {
    let Some(path) = path else {
        return MilkRustCliResult::err(2, usage());
    };
    let path = PathBuf::from(path);
    let Ok(files) = collect_preset_files(&path) else {
        return MilkRustCliResult::err(1, format!("failed to read {}\n", path.display()));
    };
    if files.is_empty() {
        return MilkRustCliResult::err(
            1,
            format!("no .milk or .milk2 files in {}\n", path.display()),
        );
    }

    let mut file_read_errors = Vec::new();
    let mut encoding_warnings = Vec::new();
    let mut source_read_statuses = Vec::with_capacity(files.len());
    let entries = files
        .iter()
        .map(|file| {
            let file_name = file.display().to_string();
            let id = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("preset")
                .to_string();
            let is_milk2 = file
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("milk2"));
            match read_bounded_preset(file) {
                Ok(bytes) => match milkrust_core::text::decode_preset(&bytes) {
                    Ok(decoded) => {
                        source_read_statuses.push(if decoded.encoding == "utf-8" { "readable-utf8" } else { "readable-decoded" });
                        if decoded.encoding != "utf-8" {
                            encoding_warnings.push(serde_json::json!({
                                "fileName": file_name, "encoding": decoded.encoding,
                                "status": "decoded-losslessly",
                            }));
                        }
                        let source = decoded.source;
                        build_milkrust_compatibility_entry(
                            &id,
                            &file_name,
                            &source,
                            is_milk2,
                        )
                    }
                    Err(error) => {
                        source_read_statuses.push("decode-failed");
                        encoding_warnings.push(serde_json::json!({
                            "fileName": file_name,
                            "error": error,
                        }));
                        let source = String::from_utf8_lossy(&bytes);
                        let mut entry = build_milkrust_compatibility_entry(
                            &id,
                            &file_name,
                            &source,
                            is_milk2,
                        );
                        entry.supported = false;
                        entry.webgpu_supported = false;
                        for report in &mut entry.preset_reports {
                            let encoding_diagnostic =
                                "source encoding could not be decoded without loss";
                            if !report.error.contains(encoding_diagnostic) {
                                if !report.error.is_empty() {
                                    report.error.push_str("; ");
                                }
                                report.error.push_str(encoding_diagnostic);
                            }
                            report.webgpu_supported = false;
                        }
                        entry
                    }
                },
                Err(error) => {
                    source_read_statuses.push("read-failed");
                    file_read_errors.push(serde_json::json!({
                        "fileName": file_name,
                        "error": error.to_string(),
                    }));
                    MilkRustCompatibilityEntry {
                        file_name,
                        format: if is_milk2 { "milk2" } else { "milk" }.to_string(),
                        id,
                        supported: false,
                        webgpu_supported: false,
                        ..MilkRustCompatibilityEntry::default()
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let summary = summarize_milkrust_compatibility_matrix(&entries);
    MilkRustCliResult::ok(format!(
        "{}\n",
        serde_json::json!({
            "reportSchemaVersion": 3,
            "source": path.display().to_string(),
            "analysisScope": "static parser/function/shader-subset/render-feature analysis only",
            "renderStatus": "not-run",
            "fidelityStatus": "not-assessed",
            "renderedCount": 0,
            "visuallyReviewedCount": 0,
            "totalCount": summary.total_count,
            "fileReadErrorCount": file_read_errors.len(),
            "fileReadErrors": file_read_errors,
            "encodingWarningCount": encoding_warnings.len(),
            "encodingWarnings": encoding_warnings,
            "presetCount": summary.preset_count,
            "supportedCount": summary.supported_count,
            "unsupportedCount": summary.unsupported_count,
            "webGpuSupportedCount": summary.webgpu_supported_count,
            "webGpuUnsupportedCount": summary.webgpu_unsupported_count,
            "maxShapeCount": summary.max_shape_count,
            "maxSpriteCount": summary.max_sprite_count,
            "maxWaveCount": summary.max_wave_count,
            "maxQRegisterIndex": summary.max_q_register_index,
            "qRegisters": summary.q_registers,
            "unsupportedFunctions": summary.unsupported_functions,
            "shaderDiagnostics": summary.shader_diagnostics,
            "unsupportedRendererFeatures": summary.unsupported_renderer_features,
            "unsupportedShaderSections": summary.unsupported_shader_sections,
            "webGpuUnsupportedRendererFeatures": summary.webgpu_unsupported_renderer_features,
            "webGpuUnsupportedShaderSections": summary.webgpu_unsupported_shader_sections,
            "entries": entries.iter().enumerate().map(|(index, entry)| serde_json::json!({
                "id": entry.id,
                "fileName": entry.file_name,
                "format": entry.format,
                "sourceReadStatus": source_read_statuses[index],
                "parseStatus": match source_read_statuses[index] {
                    "read-failed" => "not-run",
                    "decode-failed" => "lossy-diagnostic-only",
                    _ if entry.preset_count == 0 => "no-presets-parsed",
                    _ if entry.preset_reports.iter().any(|report| !report.parse_errors.is_empty()) => "rejected-input-limits",
                    _ => "parsed",
                },
                "featureSubsetStatus": match source_read_statuses[index] {
                    "read-failed" => "not-evaluated",
                    "decode-failed" => "unsupported-encoding",
                    _ if entry.preset_count == 0 => "no-presets-parsed",
                    _ if entry.supported => "supported-by-current-static-subset",
                    _ => "unsupported-by-current-static-subset",
                },
                "renderStatus": "not-run",
                "fidelityStatus": "not-assessed",
                "webGpuSupportScope": "shader-translation-subset-only",
                "presetCount": entry.preset_count,
                "supported": entry.supported,
                "webGpuSupported": entry.webgpu_supported,
                "unsupportedFunctions": entry.unsupported_functions,
                "shaderDiagnostics": entry.shader_diagnostics,
                "rendererFeatures": entry.renderer_features,
                "shaderSections": entry.shader_sections,
                "webGpuRendererFeatures": entry.webgpu_renderer_features,
                "webGpuShaderSections": entry.webgpu_shader_sections,
                "presetReports": entry.preset_reports.iter().map(|report| serde_json::json!({
                    "index": report.index,
                    "title": report.title,
                    "error": report.error,
                    "parseErrors": report.parse_errors,
                    "supportedByCurrentSubset": report.error.is_empty(),
                    "unsupportedFunctions": report.unsupported_functions,
                    "shaderDiagnostics": report.shader_diagnostics,
                    "rendererFeatures": report.renderer_features,
                    "shaderSections": report.shader_sections,
                    "webGpuRendererFeatures": report.webgpu_renderer_features,
                    "webGpuShaderSections": report.webgpu_shader_sections,
                    "webGpuShaderSubsetSupported": report.webgpu_supported,
                    "shapeCount": report.metrics.shape_count,
                    "spriteCount": report.metrics.sprite_count,
                    "waveCount": report.metrics.wave_count,
                    "qRegisters": report.metrics.q_registers,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    ))
}

fn pack_inspect_command(path: Option<&String>) -> MilkRustCliResult {
    let Some(path) = path else {
        return MilkRustCliResult::err(2, usage());
    };
    match load_milkrust_pack_manifest(path) {
        Ok((manifest_path, manifest)) => MilkRustCliResult::ok(format!(
            "{}\n",
            serde_json::json!({
                "manifestPath": manifest_path.display().to_string(),
                "schemaVersion": manifest.schema_version,
                "kind": manifest.kind,
                "dependencies": manifest.dependencies.iter().map(|dependency| serde_json::json!({
                    "id": dependency.id, "version": dependency.version,
                    "integritySha256": dependency.integrity_sha256,
                })).collect::<Vec<_>>(),
                "id": manifest.id,
                "name": manifest.name,
                "version": manifest.version,
                "author": manifest.author,
                "description": manifest.description,
                "license": manifest.license,
                "requiredMilkRustVersion": manifest.required_milkrust_version,
                "sourceUrls": manifest.source_urls,
                "presetCount": manifest.presets.len(),
                "textureCount": manifest.textures.len(),
                "fragmentCount": manifest.fragments.len(),
                "pluginCount": manifest.plugins.len(),
                "playlist": manifest.playlist,
                "presets": manifest.presets.iter().map(|preset| serde_json::json!({
                    "id": preset.id,
                    "title": preset.title,
                    "file": preset.file,
                    "sourceFormat": preset.source_format,
                    "tags": preset.tags,
                    "thumbnail": preset.thumbnail,
                })).collect::<Vec<_>>(),
                "textures": manifest.textures.iter().map(|texture| serde_json::json!({
                    "id": texture.id,
                    "file": texture.file,
                    "aliases": texture.aliases,
                })).collect::<Vec<_>>(),
                "fragments": manifest.fragments.iter().map(|fragment| serde_json::json!({
                    "id": fragment.id,
                    "kind": fragment.kind,
                    "file": fragment.file,
                    "tags": fragment.tags,
                })).collect::<Vec<_>>(),
                "plugins": manifest.plugins.iter().map(|plugin| serde_json::json!({
                    "id": plugin.id,
                    "kind": plugin.kind,
                    "entry": plugin.entry,
                })).collect::<Vec<_>>(),
            })
        )),
        Err(error) => MilkRustCliResult::err(1, format!("{error}\n")),
    }
}

fn pack_validate_command(path: Option<&String>) -> MilkRustCliResult {
    let Some(path) = path else {
        return MilkRustCliResult::err(2, usage());
    };
    let report = validate_milkrust_pack_dir(path);
    let output = format!("{}\n", report.to_json());
    if report.valid {
        MilkRustCliResult::ok(output)
    } else {
        MilkRustCliResult {
            code: 1,
            stderr: output,
            stdout: String::new(),
        }
    }
}

fn collect_preset_files(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    if path.is_file() {
        return Ok(if is_preset_file(path) {
            vec![path.to_path_buf()]
        } else {
            Vec::new()
        });
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            files.extend(collect_preset_files(&entry_path)?);
        } else if is_preset_file(&entry_path) {
            files.push(entry_path);
        }
    }
    files.sort();
    Ok(files)
}

fn is_preset_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension.eq_ignore_ascii_case("milk") || extension.eq_ignore_ascii_case("milk2")
        })
        .unwrap_or(false)
}

fn read_bounded_preset(path: &Path) -> io::Result<Vec<u8>> {
    const LIMIT: usize = milkrust_core::input_limits::MAX_PRESET_BYTES;
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Preset is not a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Preset exceeds 2 MiB"));
    }
    Ok(bytes)
}

fn read_one_path(path: Option<&String>) -> Result<(PathBuf, String), MilkRustCliResult> {
    let Some(path) = path else {
        return Err(MilkRustCliResult::err(2, usage()));
    };
    let path = PathBuf::from(path);
    match read_bounded_preset(&path).and_then(|bytes|
        milkrust_core::text::decode_preset(&bytes).map(|decoded| decoded.source).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))) {
        Ok(source) => Ok((path, source)),
        Err(error) => Err(MilkRustCliResult::err(
            1,
            format!("failed to read {}: {error}\n", path.display()),
        )),
    }
}

fn usage() -> String {
    "usage: milkrust <validate|inspect|compat|render-stats|pack-inspect|pack-validate> <file-or-directory>\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "milkrust-cli-test-{}-{name}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_preset(name: &str, source: &str) -> PathBuf {
        let path = temp_path(name);
        fs::write(&path, source).unwrap();
        path
    }

    fn smoke_source() -> &'static str {
        "name=CLI Smoke\n\
         decay=0.9\n\
         wave_r=0.8\n\
         wave_g=0.4\n\
         wave_b=0.2\n\
         wave_a=0.9\n\
         shape00_enabled=1\n\
         shape00_sides=5\n\
         shape00_rad=0.2\n\
         shape00_a=0.4\n\
         wavecode_0_enabled=1\n\
         wavecode_0_samples=16\n\
         wavecode_0_per_point1=x=i;\n\
         wavecode_0_per_point2=y=0.5+sample*0.25;\n"
    }

    #[test]
    fn validate_reports_valid_preset_as_json() {
        let path = write_preset("valid.milk", smoke_source());
        let result = run_milkrust_cli(&["validate".to_string(), path.display().to_string()]);
        let _ = fs::remove_file(&path);

        assert_eq!(result.code, 0);
        let value: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(value["status"], "valid");
        assert_eq!(value["title"], "CLI Smoke");
    }

    #[test]
    fn inspect_reports_preset_structure() {
        let path = write_preset("inspect.milk", smoke_source());
        let result = run_milkrust_cli(&["inspect".to_string(), path.display().to_string()]);
        let _ = fs::remove_file(&path);

        assert_eq!(result.code, 0);
        let value: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(value["presetCount"], 1);
        assert_eq!(value["presets"][0]["shapeCount"], 1);
        assert_eq!(value["presets"][0]["waveCount"], 1);
    }

    #[test]
    fn render_stats_reports_headless_geometry() {
        let path = write_preset("render.milk", smoke_source());
        let result = run_milkrust_cli(&["render-stats".to_string(), path.display().to_string()]);
        let _ = fs::remove_file(&path);

        assert_eq!(result.code, 0);
        let value: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert!(value["triangleVertices"].as_u64().unwrap() > 0);
        assert!(value["batches"]["filledVertices"].as_u64().unwrap() > 0);
    }

    #[test]
    fn compat_walks_directory_trees() {
        let dir = temp_path("compat-dir");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nested.milk");
        fs::write(&path, smoke_source()).unwrap();

        let result = run_milkrust_cli(&["compat".to_string(), dir.display().to_string()]);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result.code, 0);
        let value: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(value["totalCount"], 1);
        assert_eq!(value["entries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn pack_commands_inspect_and_validate_manifest_directories() {
        let dir = temp_path("pack-dir");
        fs::create_dir_all(dir.join("presets")).unwrap();
        fs::write(
            dir.join("manifest.json"),
            r#"{
              "schemaVersion": 1,
              "id": "cli-pack",
              "name": "CLI Pack",
              "version": "0.1.0",
              "presets": [
                { "id": "smoke", "title": "Smoke", "file": "presets/smoke.milk" }
              ]
            }"#,
        )
        .unwrap();
        fs::write(dir.join("presets/smoke.milk"), smoke_source()).unwrap();

        let inspect = run_milkrust_cli(&["pack-inspect".to_string(), dir.display().to_string()]);
        let validate = run_milkrust_cli(&["pack-validate".to_string(), dir.display().to_string()]);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(inspect.code, 0);
        let inspect_value: serde_json::Value = serde_json::from_str(&inspect.stdout).unwrap();
        assert_eq!(inspect_value["id"], "cli-pack");
        assert_eq!(inspect_value["presetCount"], 1);

        assert_eq!(validate.code, 0);
        let validate_value: serde_json::Value = serde_json::from_str(&validate.stdout).unwrap();
        assert_eq!(validate_value["valid"], true);
        assert_eq!(validate_value["presets"][0]["title"], "Smoke");
    }

    #[test]
    fn missing_command_returns_usage_error() {
        let result = run_milkrust_cli(&[]);
        assert_eq!(result.code, 2);
        assert!(result.stderr.contains("usage: milkrust"));
    }

    // --- Helper function tests ---

    #[test]
    fn cli_result_ok_has_zero_code_and_stdout() {
        let result = MilkRustCliResult::ok("hello");
        assert_eq!(result.code, 0);
        assert_eq!(result.stdout, "hello");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn cli_result_err_has_error_code_and_stderr() {
        let result = MilkRustCliResult::err(1, "boom");
        assert_eq!(result.code, 1);
        assert_eq!(result.stderr, "boom");
        assert!(result.stdout.is_empty());
    }

    #[test]
    fn usage_returns_usage_string() {
        let output = usage();
        assert!(output.contains("validate"));
        assert!(output.contains("inspect"));
    }

    #[test]
    fn is_preset_file_accepts_milk_extension() {
        assert!(is_preset_file(Path::new("test.milk")));
    }

    #[test]
    fn is_preset_file_accepts_milk2_extension() {
        assert!(is_preset_file(Path::new("test.milk2")));
    }

    #[test]
    fn is_preset_file_rejects_other_extensions() {
        assert!(!is_preset_file(Path::new("test.json")));
        assert!(!is_preset_file(Path::new("test.txt")));
    }

    #[test]
    fn is_preset_file_no_extension_returns_false() {
        assert!(!is_preset_file(Path::new("test")));
    }

    #[test]
    fn read_one_path_returns_error_for_missing_file() {
        let result = read_one_path(Some(&temp_path("nonexistent.txt").display().to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn collect_preset_files_collects_only_preset_files() {
        let dir = temp_path("collect-dir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.milk"), "name=test\n").unwrap();
        fs::write(dir.join("test.milk2"), "name=test\n").unwrap();
        fs::write(dir.join("other.json"), "{}").unwrap();

        let files = collect_preset_files(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn collect_preset_files_empty_directory_returns_empty() {
        let dir = temp_path("empty-dir");
        fs::create_dir_all(&dir).unwrap();

        let files = collect_preset_files(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert!(files.is_empty());
    }

    #[test]
    fn run_milkrust_cli_help_command_shows_usage() {
        let result = run_milkrust_cli(&["help".to_string()]);
        assert_eq!(result.code, 0);
        assert!(result.stdout.contains("usage"));
    }

    #[test]
    fn run_milkrust_cli_invalid_command_shows_usage() {
        let result = run_milkrust_cli(&["badcommand".to_string()]);
        assert_eq!(result.code, 2);
        assert!(result.stderr.contains("usage"));
    }
    #[test]
    fn compatibility_reports_resource_rejections_without_dropping_files() {
        let root = temp_path("resource-limits");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("sparse.milk"), "shapecode_999999999_enabled=1").unwrap();
        fs::write(root.join("oversized.milk"), vec![b'x'; milkrust_core::input_limits::MAX_PRESET_BYTES + 1]).unwrap();
        let result = run_milkrust_cli(&["compat".to_string(), root.to_string_lossy().into_owned()]);
        let report: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(report["totalCount"], 2);
        assert_eq!(report["unsupportedCount"], 2);
        assert_eq!(report["fileReadErrorCount"], 1);
        let entries = report["entries"].as_array().unwrap();
        let sparse = entries.iter().find(|entry| entry["fileName"].as_str().unwrap().ends_with("sparse.milk")).unwrap();
        assert_eq!(sparse["parseStatus"], "rejected-input-limits");
        assert!(!sparse["presetReports"][0]["parseErrors"].as_array().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

}
