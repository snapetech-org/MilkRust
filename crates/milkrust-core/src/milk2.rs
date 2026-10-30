//! MilkDrop 3's on-disk double-preset envelope and saved blend controls.
//! Source semantics and compatibility boundaries are recorded in
//! docs/MILK2_FORMAT.md.
use std::collections::BTreeMap;

/// MilkDrop keeps saved blend header values between loads. A fresh host starts
/// with the values initialized by the format implementation.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderState {
    pub pattern: String,
    pub progress: f64,
    pub direction: i32,
    pub random: [f64; 5],
    pub randoms_active: bool,
}

impl Default for HeaderState {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            progress: 1.0,
            direction: 0,
            random: [0.0; 5],
            randoms_active: false,
        }
    }
}

impl HeaderState {
    /// Reject externally restored state that could make a later saved wrapper
    /// ambiguous or invalid. An empty pattern is the process-start default.
    pub fn validate(&self) -> Result<(), String> {
        if self.pattern.len() > 80 || (!self.pattern.is_empty() && blend_pattern_code(&self.pattern).is_none()) {
            return Err("invalid carried milk2 blending_pattern".into());
        }
        if !self.progress.is_finite() || !(0.0..=1.0).contains(&self.progress) {
            return Err("invalid carried milk2 blending_progress".into());
        }
        if self.random.iter().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) {
            return Err("invalid carried milk2 random register".into());
        }
        Ok(())
    }

    /// Capture only header globals explicitly written by an ordinary
    /// single-preset soft cut. Missing or invalid values leave the process
    /// state unchanged instead of resetting an inherited register.
    pub fn update_from_scope(&mut self, scope: &std::collections::BTreeMap<String, crate::MilkRustValue>) {
        if let Some(crate::MilkRustValue::Text(pattern)) = scope.get("blending_pattern") {
            if pattern.len() <= 80 && (pattern.is_empty() || blend_pattern_code(pattern).is_some()) {
                self.pattern.clone_from(pattern);
            }
        }
        if let Some(progress) = scope.get("blending_progress").and_then(crate::MilkRustValue::as_number) {
            if progress.is_finite() && (0.0..=1.0).contains(&progress) {
                self.progress = progress;
            }
        }
        if let Some(direction) = scope.get("blending_direction").and_then(crate::MilkRustValue::as_number) {
            if direction.is_finite() && direction.fract() == 0.0
                && direction >= i32::MIN as f64 && direction <= i32::MAX as f64 {
                self.direction = direction as i32;
            }
        }
        for index in 0..self.random.len() {
            let key = format!("random_{}", index + 1);
            if let Some(value) = scope.get(&key).and_then(crate::MilkRustValue::as_number) {
                if value.is_finite() && (0.0..=1.0).contains(&value) {
                    self.random[index] = value;
                    self.randoms_active = true;
                }
            }
        }
    }

    /// Resolve absent records against the current host state without changing
    /// which keys were actually present in the saved file.
    pub fn resolve(&self, blend: &mut SavedBlend) {
        if !blend.header.contains_key("blending_pattern") {
            blend.pattern.clone_from(&self.pattern);
        }
        if !blend.header.contains_key("blending_progress") {
            blend.progress = self.progress;
        }
        if !blend.header.contains_key("blending_direction") {
            blend.direction = self.direction;
        }
        for index in 0..blend.random.len() {
            if blend.random[index].is_none() {
                blend.resolved_random[index] = self.random[index];
            }
        }
        blend.randoms_active = self.randoms_active || blend.random.iter().any(|value| value.is_some());
    }

    /// Commit a loaded file's effective values for the next preset load.
    pub fn update(&mut self, blend: &SavedBlend) {
        self.pattern.clone_from(&blend.pattern);
        self.progress = blend.progress;
        self.direction = blend.direction;
        self.random = blend.resolved_random;
        self.randoms_active = blend.randoms_active;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedBlend {
    pub pattern: String,
    pub progress: f64,
    pub direction: i32,
    pub random: [Option<f64>; 5],
    /// Effective random values used for rendering. A fresh parse starts missing
    /// slots at zero; a persistent runtime may resolve them from prior state.
    pub resolved_random: [f64; 5],
    /// Whether saved random state has ever been initialized in this header
    /// history. The five values themselves may be inherited from prior files.
    pub randoms_active: bool,
    /// Retain all key/value header records for export of unknown extensions.
    pub header: BTreeMap<String, String>,
    /// Retain non-record preamble lines such as MilkDrop's Created/version and URL.
    pub preamble: Vec<String>,
    /// Trailing sprite blocks and metadata are retained for lossless export.
    pub trailer: String,
}

impl SavedBlend {
    /// MilkDrop3's documented saved patterns. These are lowered by MilkRust's
    /// GPU renderers to a deterministic spatial reveal mask; they do not claim
    /// pixel-identical parity with MilkDrop3's proprietary transition code.
    pub fn pattern_code(&self) -> Option<i32> {
        blend_pattern_code(&self.pattern)
    }

    /// Return this file's effective saved random registers.
    pub fn random_values(&self) -> [f64; 5] {
        self.resolved_random
    }

    /// Whether random registers were initialized by this file or inherited
    /// from prior header state.
    pub fn has_randoms(&self) -> bool {
        self.randoms_active
    }

    /// MilkDrop 3.2 adds a display-name record after the two preset blocks.
    /// It is metadata, not an executable sprite/shader extension, so it must
    /// not disable an otherwise supported uniform Zoom blend.
    fn has_only_supported_trailer_metadata(&self) -> bool {
        let enabled = self.header.get("sprite").is_some_and(|value| value == "1");
        let Ok(blocks) = crate::sprites::extract(&self.trailer, enabled) else { return false; };
        let mut saw_name = false;
        for line in blocks.body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with(';') { continue; }
            let Some((key, value)) = line.split_once('=') else { return false; };
            if !key.trim().eq_ignore_ascii_case("NAME") || saw_name || value.trim().len() > 512 { return false; }
            saw_name = true;
        }
        true
    }

    fn has_supported_header(&self) -> bool {
        self.header.iter().all(|(key, value)| match key.as_str() {
            "blending_pattern" => value.eq_ignore_ascii_case(&self.pattern),
            "blending_progress" => parse_header_float(value, "blending_progress")
                .is_ok_and(|number| (0.0..=1.0).contains(&number)),
            // The saved header stores direction as a signed integer. The
            // renderer's direction-sensitive patterns test exactly `-1`;
            // other integer values are valid and behave like the default
            // orientation.
            "blending_direction" => parse_header_integer(value, "blending_direction").is_ok(),
            "random_1" | "random_2" | "random_3" | "random_4" | "random_5" => parse_header_float(value, key)
                .is_ok_and(|number| (0.0..=1.0).contains(&number)),
            "sprite" => matches!(value.as_str(), "0" | "1"),
            _ => false,
        })
    }

    /// Uniform Zoom has no spatial mask. Other patterns must not silently turn
    /// into an ordinary temporal cross-fade.
    pub fn supports_uniform_composite(&self) -> bool {
        self.pattern.eq_ignore_ascii_case("zoom")
            && self.has_only_supported_trailer_metadata()
            && self.has_supported_header()
    }

    pub fn supports_rendered_pattern(&self) -> bool {
        self.pattern_code().is_some()
            && self.has_only_supported_trailer_metadata()
            && self.has_supported_header()
    }

    pub fn render_requirement(&self) -> Option<String> {
        if self.supports_rendered_pattern() { None } else {
            Some(format!("milk2-saved-blend-pattern:{}{}", self.pattern,
                if self.trailer.trim().is_empty() { "" } else { ":embedded-extension" }))
        }
    }

    pub fn serialize(&self, presets: &[String]) -> String {
        let mut header = self.header.clone();
        if header.contains_key("blending_pattern") { header.insert("blending_pattern".into(), self.pattern.clone()); }
        if header.contains_key("blending_progress") { header.insert("blending_progress".into(), self.progress.to_string()); }
        if header.contains_key("blending_direction") { header.insert("blending_direction".into(), self.direction.to_string()); }
        for (index, value) in self.random.iter().enumerate() {
            let key = format!("random_{}", index + 1);
            if header.contains_key(&key) {
                if let Some(value) = value { header.insert(key, value.to_string()); }
                else { header.remove(&key); }
            }
        }
        let mut result = self.preamble.iter().map(|line| format!("{line}\n")).collect::<String>();
        result.push_str(&header.into_iter().map(|(k, v)| format!("{k}={v}\n")).collect::<String>());
        for (index, preset) in presets.iter().enumerate() {
            let number = index + 1;
            result.push_str(&format!("[PRESET{number}_BEGIN]\n{}\n[PRESET{number}_END]\n", preset.trim_end()));
        }
        result.push_str(&self.trailer);
        result
    }
}

/// Stable IDs shared by the WebGL and native GLES/GL renderers.
pub fn blend_pattern_code(pattern: &str) -> Option<i32> {
    Some(match pattern.trim().to_ascii_lowercase().as_str() {
        "zoom" => 1,
        "side" => 2,
        "plasma" => 3,
        "plasma2" => 4,
        "plasma3" => 5,
        "cercle" => 6,
        "square" => 7,
        "snail" => 8,
        "snail2" => 9,
        "snail3" => 10,
        "triangle" => 11,
        "donuts" => 12,
        "corner" => 13,
        "patches" => 14,
        "checkerboard" => 15,
        "bubbles" => 16,
        "stars" => 17,
        "stars2" => 18,
        "clock" => 19,
        "nuclear" => 20,
        "arrow" => 21,
        "cisor" => 22,
        "wave" => 23,
        "curtain" => 24,
        "vertical" => 25,
        "horizontal" => 26,
        "linesvertical" => 27,
        "lineshorizontal" => 28,
        "cross" => 29,
        "cross2" => 30,
        _ => return None,
    })
}

/// Cache key for a rendered MD3 mask. GPU renderers use the same key so frozen
/// masks are generated once per saved blend and target size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionMaskKey {
    pub pattern: i32,
    pub progress: u32,
    pub direction: i32,
    pub random: [u32; 5],
    pub width: u32,
    pub height: u32,
}

impl TransitionMaskKey {
    pub fn new(pattern: i32, progress: f32, direction: i32, random: [f32; 5], width: u32, height: u32) -> Self {
        Self { pattern, progress: progress.to_bits(), direction, random: random.map(f32::to_bits), width, height }
    }
}

/// Dimensions of MilkDrop 3's saved blend mesh, including its boundary vertices.
pub const TRANSITION_GRID_WIDTH: u32 = 65;
pub const TRANSITION_GRID_HEIGHT: u32 = 49;

/// Generate the saved MD3 blend alpha at each 64x48 mesh vertex. Four bytes
/// encode each normalized 32-bit fixed-point value in an RGBA8UI texture so
/// both GPU hosts can apply the original triangle interpolation without first
/// quantizing the field to an output-sized R8 image.
pub fn generate_transition_vertex_field(key: TransitionMaskKey) -> Result<Vec<u8>, String> {
    crate::milk2_mask::generate(key)
}

/// GLSL shared by copy, translated HLSL, and generated per-pixel paths. The
/// native and WebGL hosts bind the same packed vertex field at the reserved
/// final texture unit and perform matching 64x48 triangle interpolation.
pub const GLSL_TRANSITION_MASK: &str = r#"
uniform int u_transitionPattern;
uniform usampler2D u_transitionMask;
highp float milkrust_transition_vertex(ivec2 coordinate) {
  highp uvec4 bytes = texelFetch(u_transitionMask, coordinate, 0);
  highp uint encoded = (bytes.r << 24u) | (bytes.g << 16u) | (bytes.b << 8u) | bytes.a;
  return float(encoded) / 4294967295.0;
}
float milkrust_transition_mask(vec2 uv) {
  if (u_transitionPattern == 0) return 1.0;
  vec2 position = clamp(uv, vec2(0.0), vec2(1.0)) * vec2(64.0, 48.0);
  ivec2 cell = ivec2(min(floor(position), vec2(63.0, 47.0)));
  vec2 fraction = position - vec2(cell);
  float tl = milkrust_transition_vertex(cell);
  float tr = milkrust_transition_vertex(cell + ivec2(1, 0));
  float bl = milkrust_transition_vertex(cell + ivec2(0, 1));
  float br = milkrust_transition_vertex(cell + ivec2(1, 1));
  return fraction.y <= fraction.x
    ? tl * (1.0 - fraction.x) + tr * (fraction.x - fraction.y) + br * fraction.y
    : tl * (1.0 - fraction.y) + bl * (fraction.y - fraction.x) + br * fraction.x;
}
"#;

pub fn has_wrapper(source: &str) -> bool {
    source.lines().enumerate().any(|(index, raw_line)| {
        let line = raw_line.trim();
        let line = if index == 0 { line.trim_start_matches('\u{feff}') } else { line };
        line.eq_ignore_ascii_case("[PRESET1_BEGIN]") || line.eq_ignore_ascii_case("[PRESET2_BEGIN]")
            || line.eq_ignore_ascii_case("[PRESET1_END]") || line.eq_ignore_ascii_case("[PRESET2_END]")
    })
}

/// A saved wrapper is present only when all four envelope markers exist.
/// Callers can use an incomplete .milk2 as an ordinary preset, matching the
/// original loader's NotDouble fallback.
pub fn has_complete_wrapper(source: &str) -> bool {
    let markers = ["[PRESET1_BEGIN]", "[PRESET1_END]", "[PRESET2_BEGIN]", "[PRESET2_END]"];
    let mut found = [false; 4];
    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        let line = if index == 0 { line.trim_start_matches('\u{feff}') } else { line };
        for (marker_index, marker) in markers.iter().enumerate() {
            if line.eq_ignore_ascii_case(marker) { found[marker_index] = true; }
        }
    }
    found.into_iter().all(|present| present)
}

/// Match the decimal prefix accepted by the Windows CRT's `std::stof`
/// without accepting a hexadecimal literal. MilkDrop files may have a harmless
/// suffix after the numeric value.
fn parse_header_float(value: &str, key: &str) -> Result<f64, String> {
    let value = value.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
    let bytes = value.as_bytes();
    let mut cursor = usize::from(bytes.first().is_some_and(|byte| *byte == b'+' || *byte == b'-'));
    let start = cursor;
    let lower = value[cursor..].to_ascii_lowercase();
    if lower.starts_with("infinity") { cursor += 8; }
    else if lower.starts_with("inf") || lower.starts_with("nan") { cursor += 3; }
    else {
        let integer_start = cursor;
        while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit()) { cursor += 1; }
        let mut digits = cursor - integer_start;
        if bytes.get(cursor) == Some(&b'.') {
            cursor += 1;
            let fraction_start = cursor;
            while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit()) { cursor += 1; }
            digits += cursor - fraction_start;
        }
        if digits == 0 { return Err(format!("invalid milk2 {key}")); }
        let mantissa_end = cursor;
        if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
            let mut exponent = cursor + 1;
            if matches!(bytes.get(exponent), Some(b'+' | b'-')) { exponent += 1; }
            let exponent_start = exponent;
            while bytes.get(exponent).is_some_and(|byte| byte.is_ascii_digit()) { exponent += 1; }
            cursor = if exponent > exponent_start { exponent } else { mantissa_end };
        }
    }
    if cursor <= start { return Err(format!("invalid milk2 {key}")); }
    let mut token = value[..cursor].to_string();
    let sign = usize::from(token.starts_with('+') || token.starts_with('-'));
    if token.as_bytes().get(sign) == Some(&b'.') { token.insert(sign, '0'); }
    let exponent = token.find(|ch| ch == 'e' || ch == 'E').unwrap_or(token.len());
    if exponent > 0 && token.as_bytes().get(exponent - 1) == Some(&b'.') { token.insert(exponent, '0'); }
    let number = token.parse::<f32>().map_err(|_| format!("invalid milk2 {key}"))?;
    if !number.is_finite() { return Err(format!("non-finite milk2 {key}")); }
    Ok(f64::from(number))
}

/// Match `std::stoi(text, nullptr, 10)`: consume a signed decimal prefix.
fn parse_header_integer(value: &str, key: &str) -> Result<i32, String> {
    let value = value.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
    let bytes = value.as_bytes();
    let mut cursor = usize::from(bytes.first().is_some_and(|byte| *byte == b'+' || *byte == b'-'));
    let start = cursor;
    while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit()) { cursor += 1; }
    if cursor == start { return Err(format!("invalid milk2 {key}")); }
    value[..cursor].parse::<i32>().map_err(|_| format!("invalid milk2 {key}"))
}

/// Parse ordered, complete blocks without confusing the [preset00] section
/// inside each block with the envelope itself. No filesystem or code execution.
pub fn parse(source: &str) -> Result<(SavedBlend, [String; 2]), String> {
    if source.len()>crate::input_limits::MAX_PRESET_BYTES { return Err("milk2 source exceeds 2 MiB".into()); }
    let normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut state = 0usize;
    let mut header = BTreeMap::new();
    let mut preamble = Vec::new();
    let mut presets = [String::new(), String::new()];
    let mut trailer = String::new();
    let markers = ["[PRESET1_BEGIN]", "[PRESET1_END]", "[PRESET2_BEGIN]", "[PRESET2_END]"];
    for line in normalized.lines() {
        let trimmed = line.trim();
        if let Some(marker) = markers.iter().position(|m| trimmed.eq_ignore_ascii_case(m)) {
            if marker != state { return Err("milk2 block markers are duplicate, nested or out of order".into()); }
            state += 1;
            continue;
        }
        match state {
            0 => {
                if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with(';') {
                    preamble.push(line.to_string());
                    continue;
                }
                let Some((key, value)) = trimmed.split_once('=') else {
                    // Real MD3 files begin with free-form Created/version and URL lines.
                    preamble.push(line.to_string());
                    continue;
                };
                let key = key.trim().to_ascii_lowercase();
                if key.is_empty() || key.len() > 256 {
                    return Err("milk2 header key is empty or oversized".into());
                }
                // The last occurrence wins, matching the original loader.
                header.insert(key, value.trim().to_string());
            }
            1 | 3 => {
                let index = if state == 1 { 0 } else { 1 };
                presets[index].push_str(line);
                presets[index].push('\n');
            }
            2 => {
                if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with(';') {
                    return Err("unexpected text between milk2 preset blocks".into());
                }
            }
            _ => { trailer.push_str(line); trailer.push('\n'); }
        }
    }
    if state != 4 || presets.iter().any(|s| s.trim().is_empty()) {
        return Err("milk2 requires two nonempty, complete preset blocks".into());
    }
    let pattern = header.get("blending_pattern").cloned().unwrap_or_default();
    if pattern.len() > 80 { return Err("invalid milk2 blending pattern".into()); }
    let number = |key: &str, default: f64| -> Result<f64, String> {
        match header.get(key) {
            Some(value) => parse_header_float(value, key),
            None => Ok(default),
        }
    };
    let progress = number("blending_progress", 1.0)?;
    if !(0.0..=1.0).contains(&progress) { return Err("milk2 progress is outside 0..1".into()); }
    let direction = match header.get("blending_direction") {
        Some(value) => parse_header_integer(value, "blending_direction")?,
        None => 0,
    };
    let mut random = [None; 5];
    let mut resolved_random = [0.0; 5];
    for (index, value) in random.iter_mut().enumerate() {
        let key = format!("random_{}", index + 1);
        if header.contains_key(&key) {
            let parsed = number(&key, 0.0)?;
            if !(0.0..=1.0).contains(&parsed) { return Err("milk2 saved random value is outside 0..1".into()); }
            *value = Some(parsed);
            resolved_random[index] = parsed;
        }
    }
    let randoms_active = random.iter().any(Option::is_some);
    Ok((SavedBlend { pattern, progress, direction, random, resolved_random, randoms_active, header, preamble, trailer }, presets))
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAIR: &str = "blending_pattern=Zoom\nblending_progress=0.3\nblending_direction=-1\nrandom_1=0.123\n[PRESET1_BEGIN]\nMILKDROP_PRESET_VERSION=201\n[preset00]\nname=First\nzoom=1\n[PRESET1_END]\n[PRESET2_BEGIN]\n[preset00]\nname=Second\nzoom=1.1\n[PRESET2_END]\n";
    #[test]
    fn saves_both_inner_sections_and_randoms() {
        let (blend, presets) = parse(PAIR).unwrap();
        assert_eq!(presets.len(), 2);
        assert!(presets[0].contains("MILKDROP_PRESET_VERSION=201"));
        assert_eq!(blend.random[0], Some(0.123));
        assert_eq!(blend.direction, -1);
        assert_eq!(parse(&blend.serialize(&presets)).unwrap(), (blend, presets));
    }

    #[test]
    fn direction_accepts_signed_integer_values_and_preserves_them() {
        for direction in [-17, -1, 0, 1, 23] {
            let source = PAIR.replace("blending_direction=-1", &format!("blending_direction={direction}"));
            let (blend, presets) = parse(&source).unwrap();
            assert_eq!(blend.direction, direction);
            assert!(blend.supports_rendered_pattern());
            assert_eq!(parse(&blend.serialize(&presets)).unwrap(), (blend, presets));
        }
        for invalid in ["0.5", "NaN", "2147483648"] {
            let source = PAIR.replace("blending_direction=-1", &format!("blending_direction={invalid}"));
            assert!(parse(&source).is_err());
        }
    }
    #[test]
    fn unknown_patterns_and_extensions_survive_but_do_not_claim_render_support() {
        let source = PAIR.replace("Zoom", "NotAPattern") + "[SPRITE1_BEGIN]\nimg=x.png\n[SPRITE1_END]\n";
        let (blend, presets) = parse(&source).unwrap();
        assert!(!blend.supports_uniform_composite());
        assert!(blend.serialize(&presets).contains("img=x.png"));
    }

    #[test]
    fn milkdrop_32_display_name_does_not_disable_zoom_and_round_trips() {
        let source = format!("{}NAME=Original author title\n", PAIR);
        let (blend, presets) = parse(&source).unwrap();
        assert!(blend.supports_uniform_composite());
        let serialized = blend.serialize(&presets);
        assert!(serialized.ends_with("NAME=Original author title\n"));
        assert_eq!(parse(&serialized).unwrap(), (blend, presets));
    }

    #[test]
    fn unknown_or_ambiguous_trailer_metadata_remains_gated() {
        let trailers = vec![
            "NAME=first\nNAME=second\n".to_string(),
            "sprite=external.png\n".to_string(),
            format!("NAME={}\n", "x".repeat(513)),
        ];
        for trailer in trailers {
            let source = format!("{PAIR}{trailer}");
            let (blend, _) = parse(&source).unwrap();
            assert!(!blend.supports_uniform_composite());
        }
    }
    #[test]
    fn explicit_disabled_sprite_blocks_are_preserved_without_execution() {
        let source=format!("sprite=0\n{PAIR}[SPRITE1_BEGIN]\nSpriteName=art.png\ncode_1=x=1;\n[SPRITE1_END]\nNAME=With disabled sprite\n");
        let (blend,presets)=parse(&source).unwrap();
        assert!(blend.supports_uniform_composite());
        assert_eq!(parse(&blend.serialize(&presets)).unwrap(),(blend,presets));
        assert!(parse(&source.replace("sprite=0", "sprite=1")).unwrap().0.supports_rendered_pattern());
        for invalid in [source.replace("[SPRITE1_END]",""),source.replace("code_1=x=1;","[SPRITE2_BEGIN]")] {
            assert!(!parse(&invalid).unwrap().0.supports_rendered_pattern());
        }
    }
    #[test]
    fn malformed_blocks_and_nonfinite_metadata_fail() {
        for source in [PAIR.replace("[PRESET1_END]", ""), PAIR.replace("[PRESET2_BEGIN]", "[PRESET1_BEGIN]"),
            PAIR.replace("progress=0.3", "progress=NaN"), PAIR.replace("progress=0.3", "progress=1.01"),
            PAIR.replace("direction=-1", "direction=not-an-integer")] {
            assert!(parse(&source).is_err());
        }
        let repeated = PAIR.replace("blending_pattern=Zoom\n", "blending_pattern=Plasma\nblending_pattern=Zoom\n");
        assert_eq!(parse(&repeated).unwrap().0.pattern, "Zoom");
    }

    #[test]
    fn omitted_header_registers_use_process_start_defaults_without_prior_state() {
        let source = PAIR.replace("blending_pattern=Zoom\n", "")
            .replace("blending_progress=0.3\n", "")
            .replace("blending_direction=-1\n", "")
            .replace("random_1=0.123\n", "");
        let (blend, _) = parse(&source).unwrap();
        assert_eq!(blend.pattern, "");
        assert_eq!(blend.progress, 1.0);
        assert_eq!(blend.direction, 0);
        assert_eq!(blend.random_values(), [0.0; 5]);
        assert!(blend.render_requirement().is_some());
    }

    #[test]
    fn only_the_canonical_saved_patterns_are_admitted() {
        assert_eq!(blend_pattern_code("Zoom"), Some(1));
        assert_eq!(blend_pattern_code("Fade"), None);
    }
}
