use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use milkrust_core::{build_milkrust_compatibility_entry, milkrust_preset_name};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackManifest {
    pub kind: String,
    pub dependencies: Vec<MilkRustPackDependency>,
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub required_milkrust_version: String,
    pub source_urls: Vec<String>,
    pub presets: Vec<MilkRustPackPreset>,
    pub textures: Vec<MilkRustPackTexture>,
    pub fragments: Vec<MilkRustPackFragment>,
    pub plugins: Vec<MilkRustPackPlugin>,
    pub playlist: Vec<String>,
    pub automation_defaults: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackPreset {
    pub id: String,
    pub title: String,
    pub file: String,
    pub source_format: String,
    pub tags: Vec<String>,
    pub thumbnail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackDependency {
    pub id: String,
    pub version: String,
    pub integrity_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackTexture {
    pub id: String,
    pub file: String,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackFragment {
    pub id: String,
    pub kind: String,
    pub file: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackPlugin {
    pub id: String,
    pub kind: String,
    pub entry: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackPresetReport {
    pub id: String,
    pub file: String,
    pub title: String,
    pub source_format: String,
    pub format: String,
    pub preset_count: usize,
    pub supported: bool,
    pub webgpu_supported: bool,
    pub missing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MilkRustPackValidationReport {
    pub manifest_path: String,
    pub pack_id: String,
    pub pack_name: String,
    pub version: String,
    pub valid: bool,
    pub preset_count: usize,
    pub texture_count: usize,
    pub fragment_count: usize,
    pub plugin_count: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub presets: Vec<MilkRustPackPresetReport>,
}

impl MilkRustPackValidationReport {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "manifestPath": self.manifest_path,
            "packId": self.pack_id,
            "packName": self.pack_name,
            "version": self.version,
            "valid": self.valid,
            "presetCount": self.preset_count,
            "textureCount": self.texture_count,
            "fragmentCount": self.fragment_count,
            "pluginCount": self.plugin_count,
            "errors": self.errors,
            "warnings": self.warnings,
            "presets": self.presets.iter().map(|preset| serde_json::json!({
                "id": preset.id,
                "file": preset.file,
                "title": preset.title,
                "sourceFormat": preset.source_format,
                "format": preset.format,
                "presetCount": preset.preset_count,
                "supported": preset.supported,
                "webGpuSupported": preset.webgpu_supported,
                "missing": preset.missing,
            })).collect::<Vec<_>>(),
        })
    }
}

pub fn parse_milkrust_pack_manifest(source: &str) -> Result<MilkRustPackManifest, Vec<String>> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(vec!["manifest exceeds 16 MiB".to_string()]);
    }
    let value = serde_json::from_str::<Value>(source)
        .map_err(|error| vec![format!("manifest is not valid JSON: {error}")])?;
    let Some(object) = value.as_object() else {
        return Err(vec!["manifest root must be a JSON object".to_string()]);
    };

    let mut errors = Vec::new();
    let schema_version = optional_u64(object.get("schemaVersion"))
        .or_else(|| optional_u64(object.get("schema_version")))
        .unwrap_or(1) as u32;
    let id = required_string(object.get("id"), "id", &mut errors);
    let name = required_string(object.get("name"), "name", &mut errors);
    let version = required_string(object.get("version"), "version", &mut errors);
    let author = optional_string(object.get("author")).unwrap_or_default();
    let description = optional_string(object.get("description")).unwrap_or_default();
    let license = optional_string(object.get("license")).unwrap_or_default();
    let required_milkrust_version = optional_string(object.get("requiredMilkRustVersion"))
        .or_else(|| optional_string(object.get("required_milkrust_version")))
        .unwrap_or_default();
    let source_urls = string_array(object.get("sourceUrls"))
        .or_else(|| string_array(object.get("source_urls")))
        .unwrap_or_default();

    let kind = if object.contains_key("kind") {
        required_string(object.get("kind"), "kind", &mut errors)
    } else { "presets".to_string() };
    if kind != "presets" && kind != "textures" {
        errors.push("unknown pack kind".to_string());
    }
    if object.get("dependencies").and_then(Value::as_array).is_some_and(|items| items.len() > 16) {
        return Err(vec!["at most 16 texture dependencies are allowed".to_string()]);
    }
    let dependencies = pack_array(object.get("dependencies"), "dependencies",
        &mut errors, parse_pack_dependency);
    if dependencies.len() > 16 { errors.push("at most 16 texture dependencies are allowed".to_string()); }
    let mut dependency_ids = std::collections::BTreeSet::new();
    for dependency in &dependencies {
        if !dependency_ids.insert((dependency.id.clone(), dependency.version.clone())) {
            errors.push("duplicate texture dependency".to_string());
        }
        if dependency.id == id && dependency.version == version {
            errors.push("a pack cannot depend on itself".to_string());
        }
    }

    let presets = pack_array(
        object.get("presets"),
        "presets",
        &mut errors,
        parse_pack_preset,
    );
    let textures = pack_array(
        object.get("textures"),
        "textures",
        &mut errors,
        parse_pack_texture,
    );
    let fragments = pack_array(
        object.get("fragments"),
        "fragments",
        &mut errors,
        parse_pack_fragment,
    );
    let plugins = pack_array(
        object.get("plugins"),
        "plugins",
        &mut errors,
        parse_pack_plugin,
    );
    let playlist = string_array(object.get("playlist")).unwrap_or_default();
    let automation_defaults = object
        .get("automationDefaults")
        .or_else(|| object.get("automation_defaults"))
        .cloned()
        .unwrap_or(Value::Null);

    if kind == "textures" && (!presets.is_empty() || textures.is_empty() || !dependencies.is_empty()) {
        errors.push("texture packs need textures, no presets and no dependencies".to_string());
    }
    if errors.is_empty() {
        Ok(MilkRustPackManifest {
            kind,
            dependencies,
            schema_version,
            id,
            name,
            version,
            author,
            description,
            license,
            required_milkrust_version,
            source_urls,
            presets,
            textures,
            fragments,
            plugins,
            playlist,
            automation_defaults,
        })
    } else {
        Err(errors)
    }
}

pub fn load_milkrust_pack_manifest(
    pack_path: impl AsRef<Path>,
) -> Result<(PathBuf, MilkRustPackManifest), String> {
    let manifest_path = milkrust_pack_manifest_path(pack_path.as_ref());
    let mut source = String::new();
    fs::File::open(&manifest_path)
        .and_then(|file| file.take(16 * 1024 * 1024 + 1).read_to_string(&mut source))
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    parse_milkrust_pack_manifest(&source)
        .map(|manifest| (manifest_path, manifest))
        .map_err(|errors| errors.join("; "))
}

pub fn validate_milkrust_pack_dir(pack_path: impl AsRef<Path>) -> MilkRustPackValidationReport {
    let pack_path = pack_path.as_ref();
    let manifest_path = milkrust_pack_manifest_path(pack_path);
    let manifest_path_label = manifest_path.display().to_string();
    let source = match fs::read_to_string(&manifest_path) {
        Ok(source) => source,
        Err(error) => {
            return MilkRustPackValidationReport {
                manifest_path: manifest_path_label,
                pack_id: String::new(),
                pack_name: String::new(),
                version: String::new(),
                valid: false,
                preset_count: 0,
                texture_count: 0,
                fragment_count: 0,
                plugin_count: 0,
                errors: vec![format!("failed to read manifest: {error}")],
                warnings: Vec::new(),
                presets: Vec::new(),
            };
        }
    };
    validate_milkrust_pack_source(pack_path, &manifest_path_label, &source)
}

pub fn validate_milkrust_pack_source(
    pack_path: impl AsRef<Path>,
    manifest_path_label: &str,
    source: &str,
) -> MilkRustPackValidationReport {
    let pack_path = pack_path.as_ref();
    let manifest = match parse_milkrust_pack_manifest(source) {
        Ok(manifest) => manifest,
        Err(errors) => {
            return MilkRustPackValidationReport {
                manifest_path: manifest_path_label.to_string(),
                pack_id: String::new(),
                pack_name: String::new(),
                version: String::new(),
                valid: false,
                preset_count: 0,
                texture_count: 0,
                fragment_count: 0,
                plugin_count: 0,
                errors,
                warnings: Vec::new(),
                presets: Vec::new(),
            };
        }
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_id("id", &manifest.id, &mut errors);
    if manifest.schema_version != 1 {
        errors.push(format!(
            "unsupported schemaVersion {}; expected 1",
            manifest.schema_version
        ));
    }
    if manifest.presets.is_empty() && manifest.kind != "textures" {
        warnings.push("pack contains no presets".to_string());
    }

    for dependency in &manifest.dependencies {
        warnings.push(format!("host must resolve texture dependency {}@{} with integrity SHA-256 {}",
            dependency.id, dependency.version, dependency.integrity_sha256));
    }
    let mut preset_reports = Vec::new();
    for preset in &manifest.presets {
        validate_id(
            &format!("presets.{}.id", preset.id),
            &preset.id,
            &mut errors,
        );
        validate_relative_pack_path(&preset.file, &format!("preset {}", preset.id), &mut errors);
        let source_format = normalize_source_format(&preset.source_format, &preset.file);
        if preset.title.trim().is_empty() {
            warnings.push(format!("preset {} has no title", preset.id));
        }
        let preset_path = pack_path.join(&preset.file);
        match fs::read_to_string(&preset_path) {
            Ok(preset_source) => {
                if source_format == "milk" || source_format == "milk2" {
                    let entry = build_milkrust_compatibility_entry(
                        &preset.id,
                        &preset.file,
                        &preset_source,
                        source_format == "milk2",
                    );
                    preset_reports.push(MilkRustPackPresetReport {
                        id: preset.id.clone(),
                        file: preset.file.clone(),
                        title: if preset.title.trim().is_empty() {
                            milkrust_preset_name(&preset_source)
                        } else {
                            preset.title.clone()
                        },
                        source_format: source_format.clone(),
                        format: entry.format,
                        preset_count: entry.preset_count,
                        supported: entry.supported,
                        webgpu_supported: entry.webgpu_supported,
                        missing: false,
                    });
                } else if source_format == "butterchurn-json" {
                    if serde_json::from_str::<Value>(&preset_source).is_err() {
                        errors.push(format!("preset {} is not valid JSON", preset.file));
                    }
                    preset_reports.push(MilkRustPackPresetReport {
                        id: preset.id.clone(),
                        file: preset.file.clone(),
                        title: preset.title.clone(),
                        source_format: source_format.clone(),
                        format: source_format,
                        preset_count: 1,
                        supported: false,
                        webgpu_supported: false,
                        missing: false,
                    });
                } else {
                    errors.push(format!(
                        "preset {} uses unsupported sourceFormat {}",
                        preset.file, source_format
                    ));
                    preset_reports.push(MilkRustPackPresetReport {
                        id: preset.id.clone(),
                        file: preset.file.clone(),
                        title: preset.title.clone(),
                        source_format: source_format.clone(),
                        format: source_format,
                        preset_count: 0,
                        supported: false,
                        webgpu_supported: false,
                        missing: false,
                    });
                }
            }
            Err(error) => {
                errors.push(format!("preset {} failed to read: {error}", preset.file));
                preset_reports.push(MilkRustPackPresetReport {
                    id: preset.id.clone(),
                    file: preset.file.clone(),
                    title: preset.title.clone(),
                    source_format,
                    format: String::new(),
                    preset_count: 0,
                    supported: false,
                    webgpu_supported: false,
                    missing: true,
                });
            }
        }
        if !preset.thumbnail.trim().is_empty() {
            validate_relative_pack_path(
                &preset.thumbnail,
                &format!("thumbnail {}", preset.thumbnail),
                &mut errors,
            );
            if !pack_path.join(&preset.thumbnail).is_file() {
                warnings.push(format!("thumbnail {} is missing", preset.thumbnail));
            }
        }
    }

    for texture in &manifest.textures {
        validate_id(
            &format!("textures.{}.id", texture.id),
            &texture.id,
            &mut errors,
        );
        validate_relative_pack_path(
            &texture.file,
            &format!("texture {}", texture.id),
            &mut errors,
        );
        if !pack_path.join(&texture.file).is_file() {
            warnings.push(format!("texture {} is missing", texture.file));
        }
    }
    for fragment in &manifest.fragments {
        validate_id(
            &format!("fragments.{}.id", fragment.id),
            &fragment.id,
            &mut errors,
        );
        validate_relative_pack_path(
            &fragment.file,
            &format!("fragment {}", fragment.id),
            &mut errors,
        );
        if !pack_path.join(&fragment.file).is_file() {
            warnings.push(format!("fragment {} is missing", fragment.file));
        }
    }
    for plugin in &manifest.plugins {
        validate_id(
            &format!("plugins.{}.id", plugin.id),
            &plugin.id,
            &mut errors,
        );
        validate_relative_pack_path(&plugin.entry, &format!("plugin {}", plugin.id), &mut errors);
        if !pack_path.join(&plugin.entry).is_file() {
            warnings.push(format!("plugin {} is missing", plugin.entry));
        }
    }

    MilkRustPackValidationReport {
        manifest_path: manifest_path_label.to_string(),
        pack_id: manifest.id,
        pack_name: manifest.name,
        version: manifest.version,
        valid: errors.is_empty(),
        preset_count: preset_reports.len(),
        texture_count: manifest.textures.len(),
        fragment_count: manifest.fragments.len(),
        plugin_count: manifest.plugins.len(),
        errors,
        warnings,
        presets: preset_reports,
    }
}

pub fn milkrust_pack_manifest_path(pack_path: &Path) -> PathBuf {
    if pack_path.is_dir() {
        pack_path.join("manifest.json")
    } else {
        pack_path.to_path_buf()
    }
}

fn required_string(value: Option<&Value>, field: &str, errors: &mut Vec<String>) -> String {
    match optional_string(value) {
        Some(value) if !value.trim().is_empty() => value,
        _ => {
            errors.push(format!("manifest field {field} must be a non-empty string"));
            String::new()
        }
    }
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

fn optional_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect()
    })
}

fn pack_array<T>(
    value: Option<&Value>,
    field: &str,
    errors: &mut Vec<String>,
    parse: fn(&Value, usize, &str, &mut Vec<String>) -> T,
) -> Vec<T> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(values) = value.as_array() else {
        errors.push(format!("manifest field {field} must be an array"));
        return Vec::new();
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse(value, index, field, errors))
        .collect()
}

fn parse_pack_preset(
    value: &Value,
    index: usize,
    field: &str,
    errors: &mut Vec<String>,
) -> MilkRustPackPreset {
    if !value.is_object() {
        errors.push(format!("{field}[{index}] must be an object"));
    }
    MilkRustPackPreset {
        id: required_string(value.get("id"), &format!("{field}[{index}].id"), errors),
        title: optional_string(value.get("title")).unwrap_or_default(),
        file: required_string(value.get("file"), &format!("{field}[{index}].file"), errors),
        source_format: optional_string(value.get("sourceFormat"))
            .or_else(|| optional_string(value.get("source_format")))
            .or_else(|| optional_string(value.get("format")))
            .unwrap_or_default(),
        tags: string_array(value.get("tags")).unwrap_or_default(),
        thumbnail: optional_string(value.get("thumbnail")).unwrap_or_default(),
    }
}

fn parse_pack_dependency(value: &Value, index: usize, field: &str, errors: &mut Vec<String>) -> MilkRustPackDependency {
    let id = required_string(value.get("id"), &format!("{field}[{index}].id"), errors);
    let version = required_string(value.get("version"), &format!("{field}[{index}].version"), errors);
    let integrity_sha256 = required_string(value.get("integritySha256"), &format!("{field}[{index}].integritySha256"), errors);
    for component in [&id, &version] {
        let lower = component.to_ascii_lowercase();
        let stem = lower.split('.').next().unwrap_or("");
        let reserved = matches!(stem, "con" | "prn" | "aux" | "nul") ||
            ((stem.starts_with("com") || stem.starts_with("lpt")) && stem.len() == 4 &&
                stem.as_bytes()[3].is_ascii_digit());
        if component.len() > 80 || !component.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric) ||
            !component.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) ||
            component.ends_with('.') || component.ends_with(".previous") || component.ends_with(".partial") || reserved {
            errors.push(format!("{field}[{index}] has an unsafe dependency identity"));
        }
    }
    if integrity_sha256.len() != 64 || !integrity_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        errors.push(format!("{field}[{index}] needs a lowercase integrity SHA-256"));
    }
    MilkRustPackDependency { id, version, integrity_sha256 }
}

fn parse_pack_texture(
    value: &Value,
    index: usize,
    field: &str,
    errors: &mut Vec<String>,
) -> MilkRustPackTexture {
    if !value.is_object() {
        errors.push(format!("{field}[{index}] must be an object"));
    }
    MilkRustPackTexture {
        id: required_string(value.get("id"), &format!("{field}[{index}].id"), errors),
        file: required_string(value.get("file"), &format!("{field}[{index}].file"), errors),
        aliases: string_array(value.get("aliases")).unwrap_or_default(),
    }
}

fn parse_pack_fragment(
    value: &Value,
    index: usize,
    field: &str,
    errors: &mut Vec<String>,
) -> MilkRustPackFragment {
    if !value.is_object() {
        errors.push(format!("{field}[{index}] must be an object"));
    }
    MilkRustPackFragment {
        id: required_string(value.get("id"), &format!("{field}[{index}].id"), errors),
        kind: optional_string(value.get("kind")).unwrap_or_else(|| "preset".to_string()),
        file: required_string(value.get("file"), &format!("{field}[{index}].file"), errors),
        tags: string_array(value.get("tags")).unwrap_or_default(),
    }
}

fn parse_pack_plugin(
    value: &Value,
    index: usize,
    field: &str,
    errors: &mut Vec<String>,
) -> MilkRustPackPlugin {
    if !value.is_object() {
        errors.push(format!("{field}[{index}] must be an object"));
    }
    MilkRustPackPlugin {
        id: required_string(value.get("id"), &format!("{field}[{index}].id"), errors),
        kind: optional_string(value.get("kind")).unwrap_or_else(|| "data".to_string()),
        entry: required_string(
            value.get("entry"),
            &format!("{field}[{index}].entry"),
            errors,
        ),
    }
}

fn validate_id(field: &str, id: &str, errors: &mut Vec<String>) {
    if id.trim().is_empty() {
        errors.push(format!("{field} must not be empty"));
        return;
    }
    if !id
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || char == '-' || char == '_' || char == '.')
    {
        errors.push(format!(
            "{field} contains invalid characters; use letters, numbers, '.', '-', or '_'"
        ));
    }
}

fn validate_relative_pack_path(path: &str, label: &str, errors: &mut Vec<String>) {
    if path.trim().is_empty() {
        errors.push(format!("{label} path must not be empty"));
        return;
    }
    let path = Path::new(path);
    if path.is_absolute() {
        errors.push(format!("{label} path must be relative"));
        return;
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        errors.push(format!("{label} path must stay inside the pack"));
    }
}

fn normalize_source_format(source_format: &str, file: &str) -> String {
    let normalized = source_format.trim().to_ascii_lowercase().replace('_', "-");
    if !normalized.is_empty() {
        return normalized;
    }
    let lower_file = file.to_ascii_lowercase();
    if lower_file.ends_with(".milk2") {
        "milk2".to_string()
    } else if lower_file.ends_with(".json") {
        "butterchurn-json".to_string()
    } else {
        "milk".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "milkrust-pack-test-{}-{name}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn manifest_source() -> &'static str {
        r#"{
          "schemaVersion": 1,
          "id": "sample-pack",
          "name": "Sample Pack",
          "version": "0.1.0",
          "author": "MilkRust",
          "license": "CC0-1.0",
          "requiredMilkRustVersion": "0.1.0",
          "sourceUrls": ["https://example.invalid/milkrust"],
              "presets": [
                {
                  "id": "warm-lines",
                  "title": "Warm Lines",
                  "file": "presets/warm-lines.milk",
                  "sourceFormat": "milk",
                  "tags": ["lines", "fixture"],
                  "thumbnail": "thumbnails/warm-lines.png"
                }
          ],
          "textures": [
            { "id": "noise", "file": "textures/noise.png", "aliases": ["noise_lq"] }
          ],
          "fragments": [
            { "id": "warm-shape", "kind": "shape", "file": "fragments/warm-shape.json" }
          ],
          "plugins": [
            { "id": "playlist-defaults", "kind": "data", "entry": "plugins/playlist.json" }
          ],
          "playlist": ["warm-lines"]
        }"#
    }

    fn preset_source() -> &'static str {
        "name=Warm Lines\n\
         decay=0.9\n\
         wave_r=0.8\n\
         wave_g=0.35\n\
         wave_b=0.2\n\
         wave_a=0.9\n"
    }

    #[test]
    fn parses_manifest_metadata() {
        let manifest = parse_milkrust_pack_manifest(manifest_source()).unwrap();
        assert_eq!(manifest.id, "sample-pack");
        assert_eq!(manifest.presets[0].file, "presets/warm-lines.milk");
        assert_eq!(manifest.presets[0].source_format, "milk");
        assert_eq!(manifest.textures[0].aliases, vec!["noise_lq"]);
    }

    #[test]
    fn validates_pack_directory_and_presets() {
        let dir = temp_dir("valid");
        fs::create_dir_all(dir.join("presets")).unwrap();
        fs::create_dir_all(dir.join("textures")).unwrap();
        fs::create_dir_all(dir.join("fragments")).unwrap();
        fs::create_dir_all(dir.join("plugins")).unwrap();
        fs::create_dir_all(dir.join("thumbnails")).unwrap();
        fs::write(dir.join("manifest.json"), manifest_source()).unwrap();
        fs::write(dir.join("presets/warm-lines.milk"), preset_source()).unwrap();
        fs::write(dir.join("textures/noise.png"), []).unwrap();
        fs::write(dir.join("fragments/warm-shape.json"), "{}").unwrap();
        fs::write(dir.join("plugins/playlist.json"), "{}").unwrap();

        let report = validate_milkrust_pack_dir(&dir);
        let _ = fs::remove_dir_all(&dir);

        assert!(report.valid, "{:?}", report.errors);
        assert_eq!(report.preset_count, 1);
        assert_eq!(report.presets[0].title, "Warm Lines");
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("thumbnail")));
    }

    #[test]
    fn rejects_paths_that_escape_pack() {
        let source = r#"{
          "id": "bad-pack",
          "name": "Bad Pack",
          "version": "0.1.0",
          "presets": [{ "id": "escape", "file": "../escape.milk" }]
        }"#;
        let report = validate_milkrust_pack_source(".", "manifest.json", source);
        assert!(!report.valid);
        assert!(report
            .errors
            .iter()
           .any(|error| error.contains("must stay inside the pack")));
    }

    #[test]
    fn optional_string_returns_some_for_valid_string() {
        let v = serde_json::json!("hello");
        let result = optional_string(Some(&v));
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn optional_string_returns_none_for_non_string() {
        let v = serde_json::json!(42);
        let result = optional_string(Some(&v));
        assert_eq!(result, None);
    }

    #[test]
    fn optional_u64_returns_some_for_valid_number() {
        let v = serde_json::json!(3);
        let result = optional_u64(Some(&v));
        assert_eq!(result, Some(3u64));
    }
    #[test]
    fn required_string_returns_value_for_valid_input() {
        let v = serde_json::json!("hello");
        let mut err = Vec::new();
        let result = required_string(Some(&v), "field", &mut err);
        assert_eq!(result, "hello".to_string());
        assert!(err.is_empty());
    }

    #[test]
    fn required_string_adds_error_for_empty_string() {
        let v = serde_json::json!("");
        let mut err = Vec::new();
        let result = required_string(Some(&v), "field", &mut err);
        assert_eq!(result, String::new());
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("field"));
    }

    #[test]
    fn required_string_adds_error_for_null() {
        let v = serde_json::Value::Null;
        let mut err = Vec::new();
        let result = required_string(Some(&v), "field", &mut err);
        assert_eq!(result, String::new());
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("field"));
    }
    #[test]
    fn string_array_parses_valid_array() {
        let v = serde_json::json!(["a", "b", "c"]);
        let result = string_array(Some(&v));
        assert_eq!(result, Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]));
    }

    #[test]
    fn string_array_filters_non_string_values() {
        let v = serde_json::json!(["a", 42, "b"]);
        let result = string_array(Some(&v));
        assert_eq!(result, Some(vec!["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn string_array_returns_none_for_non_array() {
        let v = serde_json::json!("not-array");
        let result = string_array(Some(&v));
        assert_eq!(result, None);
    }
    #[test]
    fn validate_id_accepts_valid_ids() {
        let mut err = Vec::new();
        validate_id("id", "valid-id_123.ok", &mut err);
        assert!(err.is_empty());
    }

    #[test]
    fn validate_id_rejects_empty_id() {
        let mut err = Vec::new();
        validate_id("id", "", &mut err);
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("id"));
    }

    #[test]
    fn validate_id_rejects_invalid_characters() {
        let mut err = Vec::new();
        validate_id("id", "bad id!", &mut err);
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("id"));
    }
    #[test]
    fn validate_relative_pack_path_rejects_absolute_paths() {
        let mut err = Vec::new();
        validate_relative_pack_path("/etc/passwd", "file", &mut err);
        assert_eq!(err.len(), 1);
    }

    #[test]
    fn validate_relative_pack_path_rejects_parent_dir_traversal() {
        let mut err = Vec::new();
        validate_relative_pack_path("../outside.milk", "file", &mut err);
        assert_eq!(err.len(), 1);
    }

    #[test]
    fn validate_relative_pack_path_accepts_relative_path() {
        let mut err = Vec::new();
        validate_relative_pack_path("presets/test.milk", "file", &mut err);
        assert!(err.is_empty());
    }
    #[test]
    fn normalize_source_format_normalizes_input() {
        assert_eq!(normalize_source_format("MILK2", ""), "milk2");
        assert_eq!(normalize_source_format("milk_2", ""), "milk-2");
        assert_eq!(normalize_source_format("BUTTERCHURN_JSON", ""), "butterchurn-json");
    }

    #[test]
    fn normalize_source_format_infer_from_file_extension() {
        assert_eq!(normalize_source_format("", ".milk"), "milk");
        assert_eq!(normalize_source_format("", ".MILK"), "milk");
        assert_eq!(normalize_source_format("", ".json"), "butterchurn-json");
        assert_eq!(normalize_source_format("", ".JSON"), "butterchurn-json");
        assert_eq!(normalize_source_format("", ".unknown"), "milk");
    }
    #[test]
    fn milkrust_pack_manifest_path_returns_correct_path() {
        // use a real temp dir so is_dir() is true
        let tmp = std::env::temp_dir().join("milkrust_test_pack_dir");
        let _ = std::fs::create_dir_all(&tmp);
        let p = milkrust_pack_manifest_path(&tmp);
        assert_eq!(p, tmp.join("manifest.json"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn milkrust_pack_manifest_path_handles_file_path() {
        let p = milkrust_pack_manifest_path(std::path::Path::new("/path/to/manifest.json"));
        assert_eq!(p, std::path::Path::new("/path/to/manifest.json"));
    }

    #[test]
    fn parse_pack_preset_defaults_missing_fields() {
        let v = serde_json::json!({ "id": "test", "file": "test.milk" });
        let mut err = Vec::new();
        let preset = parse_pack_preset(&v, 0, "presets", &mut err);
        assert_eq!(preset.id, "test");
        assert_eq!(preset.title, String::new());
        assert_eq!(preset.source_format, String::new());
        assert!(preset.tags.is_empty());
        assert_eq!(preset.thumbnail, String::new());
    }
    #[test]
    fn parse_pack_preset_accepts_camelcase_source_format() {
        let v = serde_json::json!({ "id": "test", "file": "t.milk", "sourceFormat": "milk2" });
        let mut err = Vec::new();
        let preset = parse_pack_preset(&v, 0, "presets", &mut err);
        assert_eq!(preset.source_format, "milk2");
    }

    #[test]
    fn parse_pack_texture_defaults_missing_aliases() {
        let v = serde_json::json!({ "id": "tex", "file": "t.png" });
        let mut err = Vec::new();
        let tex = parse_pack_texture(&v, 0, "textures", &mut err);
        assert_eq!(tex.id, "tex");
        assert_eq!(tex.aliases, Vec::<String>::new());
    }

    #[test]
    fn parse_pack_fragment_defaults_kind() {
        let v = serde_json::json!({ "id": "frag", "file": "f.json" });
        let mut err = Vec::new();
        let frag = parse_pack_fragment(&v, 0, "fragments", &mut err);
        assert_eq!(frag.kind, "preset");
    }

    #[test]
    fn parse_pack_plugin_defaults_kind() {
        let v = serde_json::json!({ "id": "plug", "entry": "e.json" });
        let mut err = Vec::new();
        let plug = parse_pack_plugin(&v, 0, "plugins", &mut err);
        assert_eq!(plug.kind, "data");
    }
    #[test]
    fn validation_report_to_json_includes_all_fields() {
        let report = MilkRustPackValidationReport {
            manifest_path: "/path/to/manifest.json".to_string(),
            pack_id: "my-pack".to_string(),
            pack_name: "My Pack".to_string(),
            version: "1.0.0".to_string(),
            valid: true,
            preset_count: 2,
            texture_count: 1,
            fragment_count: 0,
            plugin_count: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
            presets: Vec::new(),
        };
        let json = report.to_json();
        let j = json.as_object().expect("to_json should return an object");
        assert_eq!(j.get("manifestPath").unwrap().as_str().unwrap(), "/path/to/manifest.json");
        assert_eq!(j.get("packId").unwrap().as_str().unwrap(), "my-pack");
        assert_eq!(j.get("packName").unwrap().as_str().unwrap(), "My Pack");
        assert_eq!(j.get("version").unwrap().as_str().unwrap(), "1.0.0");
        assert_eq!(j.get("valid").unwrap().as_bool().unwrap(), true);
        assert_eq!(j.get("presetCount").unwrap().as_u64().unwrap(), 2);
        assert_eq!(j.get("textureCount").unwrap().as_u64().unwrap(), 1);
        assert_eq!(j.get("fragmentCount").unwrap().as_u64().unwrap(), 0);
        assert_eq!(j.get("pluginCount").unwrap().as_u64().unwrap(), 0);
    }
}

#[cfg(test)]
mod dependency_schema_tests {
    use super::*;

    #[test]
    fn texture_pack_is_explicit_and_dependencies_survive_parsing() {
        let textures = serde_json::json!({
            "id": "textures", "version": "1", "name": "Textures", "kind": "textures",
            "presets": [], "textures": [{"id": "cloud", "file": "textures/cloud.png"}]
        });
        let parsed = parse_milkrust_pack_manifest(&textures.to_string()).unwrap();
        assert_eq!(parsed.kind, "textures");
        assert!(parsed.presets.is_empty());
        let mut owner = serde_json::json!({
            "id": "owner", "name": "Owner", "version": "1",
            "dependencies": [{"id": "textures", "version": "1", "integritySha256": "a".repeat(64)}]
        });
        let parsed = parse_milkrust_pack_manifest(&owner.to_string()).unwrap();
        assert_eq!(parsed.dependencies[0].integrity_sha256, "a".repeat(64));
        owner["dependencies"][0]["id"] = Value::String("../escape".to_string());
        assert!(parse_milkrust_pack_manifest(&owner.to_string()).is_err());
        let mut cycle = textures;
        cycle["dependencies"] = serde_json::json!([{"id": "textures", "version": "1", "integritySha256": "a".repeat(64)}]);
        assert!(parse_milkrust_pack_manifest(&cycle.to_string()).is_err());
    }
}
