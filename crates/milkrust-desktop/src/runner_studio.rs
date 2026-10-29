use std::{fs, path::PathBuf};

use milkrust_core::{
    build_milkrust_compatibility_entry, summarize_milkrust_compatibility_matrix,
    validate_milkrust_import, MilkRustCompatibilityEntry, MilkRustCompatibilitySummary,
};

use crate::{collect_pack_plugins, collect_preset_inputs};

#[derive(Clone, Debug)]
struct StudioPresetRecord {
    path: String,
    label: String,
    status: String,
    validation_title: Option<String>,
    validation_error: Option<String>,
    compatibility_entry: Option<MilkRustCompatibilityEntry>,
}

#[derive(Clone, Debug, Default)]
struct StudioRunConfig {
    preset_inputs: Vec<PathBuf>,
    pack_inputs: Vec<PathBuf>,
    emit_json: bool,
    plugin_report: Option<PathBuf>,
    fail_on_unsupported: bool,
}

pub fn run_desktop_studio(args: &[String]) -> i32 {
    let mut args = args.iter();
    let mut config = StudioRunConfig::default();

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" | "-h" => {
                print_studio_help();
                return 0;
            }
            "--preset" => {
                let Some(value) = args.next() else {
                    eprintln!("missing --preset value");
                    print_studio_help();
                    return 2;
                };
                config.preset_inputs.push(PathBuf::from(value));
            }
            "--pack" => {
                let Some(value) = args.next() else {
                    eprintln!("missing --pack value");
                    print_studio_help();
                    return 2;
                };
                config.pack_inputs.push(PathBuf::from(value));
            }
            "--json" => {
                config.emit_json = true;
            }
            "--plugin-report" => {
                let Some(value) = args.next() else {
                    eprintln!("missing --plugin-report value");
                    print_studio_help();
                    return 2;
                };
                config.plugin_report = Some(PathBuf::from(value));
            }
            "--fail-on-unsupported" => {
                config.fail_on_unsupported = true;
            }
            unknown => {
                eprintln!("Unknown option: {unknown}");
                print_studio_help();
                return 2;
            }
        }
    }

    let selected_presets = match collect_preset_inputs(&config.preset_inputs, &config.pack_inputs) {
        Ok(inputs) => inputs,
        Err(error) => {
            eprintln!("Either --preset or --pack is required.");
            eprintln!("Failed to collect source list: {error}");
            print_studio_help();
            return 2;
        }
    };

    let (pack_plugins, pack_plugin_error) = match collect_pack_plugins(&config.pack_inputs) {
        Ok(plugins) => (plugins, None),
        Err(error) => {
            eprintln!("Failed to load pack plugins: {error}");
            (Vec::new(), Some(error))
        }
    };

    if let Some(report_path) = config.plugin_report.as_deref() {
        if let Err(error) = crate::write_pack_plugin_report(
            report_path,
            "studio",
            &config.pack_inputs,
            &pack_plugins,
            pack_plugin_error.as_deref(),
        ) {
            eprintln!("Failed to write plugin report '{report_path:?}': {error}");
            return 2;
        }
    }

    let mut reports = Vec::with_capacity(selected_presets.len());
    for preset in &selected_presets {
        let source = match fs::read_to_string(&preset.source_path) {
            Ok(source) => source,
            Err(error) => {
                reports.push(StudioPresetRecord {
                    path: preset.source_path.to_string_lossy().to_string(),
                    label: preset.source_label.clone(),
                    status: "invalid".to_string(),
                    validation_title: None,
                    validation_error: Some(format!("unable to read file: {error}")),
                    compatibility_entry: None,
                });
                continue;
            }
        };

        let validation_title = match validate_milkrust_import(&source) {
            Ok(title) => Some(title),
            Err(error) => {
                reports.push(StudioPresetRecord {
                    path: preset.source_path.to_string_lossy().to_string(),
                    label: preset.source_label.clone(),
                    status: "invalid".to_string(),
                    validation_title: None,
                    validation_error: Some(error),
                    compatibility_entry: None,
                });
                continue;
            }
        };

        let file_name = preset.source_path.to_string_lossy().to_string();
        let force_milk2 = preset
            .source_path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("milk2"));
        let entry = build_milkrust_compatibility_entry(
            &preset.source_label,
            &file_name,
            &source,
            force_milk2,
        );
        let status = if entry.supported {
            "supported"
        } else {
            "unsupported"
        };

        reports.push(StudioPresetRecord {
            path: file_name,
            label: preset.source_label.clone(),
            status: status.to_string(),
            validation_title,
            validation_error: None,
            compatibility_entry: Some(entry),
        });
    }

    let compatibility_entries = reports
        .iter()
        .filter_map(|record| record.compatibility_entry.clone())
        .collect::<Vec<_>>();
    let compatibility_summary = summarize_milkrust_compatibility_matrix(&compatibility_entries);

    if config.emit_json {
        print_studio_json(&reports, &compatibility_summary, &pack_plugins);
    } else {
        print_studio_text(&reports, &compatibility_summary, &pack_plugins);
    }

    if config.fail_on_unsupported && compatibility_summary.unsupported_count > 0 {
        return 1;
    }
    if reports
        .iter()
        .any(|record| record.status == "invalid" || record.validation_error.is_some())
    {
        return 1;
    }
    0
}

fn print_studio_help() {
    println!("milkrust-desktop studio - compatibility inspection and preset report mode");
    println!();
    println!("Usage:");
    println!("  cargo run -p milkrust-desktop --bin studio -- [options]");
    println!();
    println!("Options:");
    println!("  --preset <path>           Path to a .milk/.milk2 source file (repeatable)");
    println!("  --pack <path>             Path to a MilkRust pack folder or manifest");
    println!("  --json                    Emit a JSON report");
    println!("  --plugin-report <path>     Write discovered pack plugin report JSON to path");
    println!("  --fail-on-unsupported     Return non-zero for unsupported presets");
    println!("  --help                    Show this help");
}

fn print_studio_text(
    reports: &[StudioPresetRecord],
    summary: &MilkRustCompatibilitySummary,
    pack_plugins: &[crate::PackPluginInput],
) {
    println!(
        "MilkRust desktop studio: {} entries ({} supported, {} unsupported)",
        reports.len(),
        summary.supported_count,
        summary.unsupported_count
    );
    if !summary.unsupported_functions.is_empty() {
        println!("Unsupported functions: {:?}", summary.unsupported_functions);
    }
    if !summary.unsupported_shader_sections.is_empty() {
        println!(
            "Unsupported shader sections: {:?}",
            summary.unsupported_shader_sections
        );
    }
    if !summary.shader_diagnostics.is_empty() {
        println!("HLSL shader diagnostics: {:?}", summary.shader_diagnostics);
    }
    println!("Pack plugins discovered: {}", pack_plugins.len());
    for plugin in pack_plugins {
        println!("  - {} ({})", plugin.id, plugin.kind);
        println!("    entry: {}", plugin.entry);
        if let Some(payload) = plugin.payload.as_object() {
            if !payload.is_empty() {
                println!(
                    "    payload keys: {}",
                    payload.keys().cloned().collect::<Vec<_>>().join(", ")
                );
            }
        }
    }

    for report in reports {
        println!("[{}] {}", report.label, report.path);
        println!("  status: {}", report.status);
        if let Some(title) = &report.validation_title {
            println!("  title: {title}");
        }
        if let Some(error) = &report.validation_error {
            println!("  error: {error}");
        }
        if let Some(entry) = &report.compatibility_entry {
            println!("  format: {}", entry.format);
            println!("  presetCount: {}", entry.preset_count);
            println!("  supported: {}", entry.supported);
            println!("  webGpuSupported: {}", entry.webgpu_supported);
            if !entry.unsupported_functions.is_empty() {
                println!(
                    "  unsupportedFunctions: {}",
                    entry.unsupported_functions.join(", ")
                );
            }
            if !entry.shader_sections.is_empty() {
                println!("  unsupportedShaders: {}", entry.shader_sections.join(", "));
            }
            if !entry.shader_diagnostics.is_empty() {
                println!("  HLSL shader diagnostics: {}", entry.shader_diagnostics.join(", "));
            }
            if !entry.webgpu_shader_sections.is_empty() {
                println!(
                    "  webGpuUnsupportedShaders: {}",
                    entry.webgpu_shader_sections.join(", ")
                );
            }
        }
    }
}

fn print_studio_json(
    reports: &[StudioPresetRecord],
    summary: &MilkRustCompatibilitySummary,
    pack_plugins: &[crate::PackPluginInput],
) {
    let entries = reports
        .iter()
        .map(|report| {
            serde_json::json!({
                "label": report.label,
                "path": report.path,
                "status": report.status,
                "title": report.validation_title,
                "validationError": report.validation_error,
                "format": report.compatibility_entry.as_ref().map(|entry| &entry.format),
                "presetCount": report.compatibility_entry.as_ref().map(|entry| entry.preset_count),
                "supported": report.compatibility_entry.as_ref().map(|entry| entry.supported),
                "webGpuSupported": report.compatibility_entry
                    .as_ref()
                    .map(|entry| entry.webgpu_supported),
                "unsupportedFunctions": report.compatibility_entry
                    .as_ref()
                    .map(|entry| &entry.unsupported_functions),
                "shaderDiagnostics": report.compatibility_entry
                    .as_ref()
                    .map(|entry| &entry.shader_diagnostics),
                "rendererFeatures": report.compatibility_entry
                    .as_ref()
                    .map(|entry| &entry.renderer_features),
                "unsupportedShaderSections": report.compatibility_entry
                    .as_ref()
                    .map(|entry| &entry.shader_sections),
                "webGpuRendererFeatures": report.compatibility_entry
                    .as_ref()
                    .map(|entry| &entry.webgpu_renderer_features),
            })
        })
        .collect::<Vec<_>>();

    let json = serde_json::json!({
        "mode": "studio",
        "totalCount": reports.len(),
        "supportedCount": summary.supported_count,
        "unsupportedCount": summary.unsupported_count,
        "presetCount": summary.preset_count,
        "webGpuSupportedCount": summary.webgpu_supported_count,
        "webGpuUnsupportedCount": summary.webgpu_unsupported_count,
        "unsupportedFunctions": summary.unsupported_functions,
        "shaderDiagnostics": summary.shader_diagnostics,
        "unsupportedRendererFeatures": summary.unsupported_renderer_features,
        "unsupportedShaderSections": summary.unsupported_shader_sections,
        "webGpuUnsupportedRendererFeatures": summary.webgpu_unsupported_renderer_features,
        "webGpuUnsupportedShaderSections": summary.webgpu_unsupported_shader_sections,
        "packPlugins": {
            "count": pack_plugins.len(),
            "entries": pack_plugins
                .iter()
                .map(|plugin| {
                    serde_json::json!({
                        "id": plugin.id,
                        "kind": plugin.kind,
                        "entry": plugin.entry,
                        "sourcePath": plugin.source_path.to_string_lossy(),
                        "payloadKeyCount": plugin.payload.as_object().map(|payload| payload.len()).unwrap_or(0),
                    })
                })
                .collect::<Vec<_>>(),
        },
        "entries": entries,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
    );
}
