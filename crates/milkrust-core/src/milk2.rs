//! MilkDrop 3's on-disk double-preset envelope and saved blend controls.
//! Source semantics and compatibility boundaries are recorded in
//! docs/MILK2_FORMAT.md.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct SavedBlend {
    pub pattern: String,
    pub progress: f64,
    pub direction: i32,
    pub random: [Option<f64>; 5],
    /// Retain all header records for lossless export of unknown extensions.
    pub header: BTreeMap<String, String>,
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

    /// MilkDrop 3.2 adds a display-name record after the two preset blocks.
    /// It is metadata, not an executable sprite/shader extension, so it must
    /// not disable an otherwise supported uniform Fade blend.
    fn has_only_supported_trailer_metadata(&self) -> bool {
        let enabled = self.header.get("sprite").map(String::as_str) == Some("1");
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
            "blending_progress" => value.parse::<f64>()
                .is_ok_and(|number| number.is_finite() && (0.0..=1.0).contains(&number)),
            "blending_direction" => value.parse::<f64>()
                .is_ok_and(|number| number.is_finite() && (number == -1.0 || number == 1.0)),
            "random_1" | "random_2" | "random_3" | "random_4" | "random_5" => value.parse::<f64>()
                .is_ok_and(|number| number.is_finite() && (0.0..=1.0).contains(&number)),
            "sprite" => matches!(value.as_str(), "0" | "1"),
            _ => false,
        })
    }

    /// Uniform Fade has no spatial mask. Other patterns must not silently turn
    /// into an ordinary temporal cross-fade.
    pub fn supports_uniform_composite(&self) -> bool {
        self.pattern.eq_ignore_ascii_case("fade")
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
        header.insert("blending_pattern".into(), self.pattern.clone());
        header.insert("blending_progress".into(), self.progress.to_string());
        header.insert("blending_direction".into(), self.direction.to_string());
        for (index, value) in self.random.iter().enumerate() {
            let key = format!("random_{}", index + 1);
            if let Some(value) = value { header.insert(key, value.to_string()); }
            else { header.remove(&key); }
        }
        let mut result = header.into_iter().map(|(k, v)| format!("{k}={v}\n")).collect::<String>();
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
        "fade" => 0,
        "zoom" => 1,
        "side" => 2,
        "plasma" => 3,
        "plasma2" => 4,
        "plasma3" => 5,
        "cercle" | "circle" => 6,
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

/// Generate the saved MD3 blend alpha field from its 30 `(a,c)` vertex
/// patterns and the preset's fixed random block. The result is one R8 byte per
/// output pixel. The interpolation follows the saved 64x48 blend mesh.
pub fn generate_transition_mask(key: TransitionMaskKey) -> Result<Vec<u8>, String> {
    crate::milk2_mask::generate(key)
}

/// GLSL shared by copy, translated HLSL, and generated per-pixel paths. The
/// native and WebGL hosts bind the same precomputed R8 field at the reserved
/// final texture unit.
pub const GLSL_TRANSITION_MASK: &str = r#"
uniform int u_transitionPattern;
uniform sampler2D u_transitionMask;
float milkrust_transition_mask(vec2 uv) {
  if (u_transitionPattern == 0) return 1.0;
  return texture(u_transitionMask, clamp(uv, vec2(0.0), vec2(1.0))).r;
}
"#;

pub fn has_wrapper(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        line.eq_ignore_ascii_case("[PRESET1_BEGIN]") || line.eq_ignore_ascii_case("[PRESET2_BEGIN]")
            || line.eq_ignore_ascii_case("[PRESET1_END]") || line.eq_ignore_ascii_case("[PRESET2_END]")
    })
}

/// Parse ordered, complete blocks without confusing the [preset00] section
/// inside each block with the envelope itself. No filesystem or code execution.
pub fn parse(source: &str) -> Result<(SavedBlend, [String; 2]), String> {
    if source.len()>crate::input_limits::MAX_PRESET_BYTES { return Err("milk2 source exceeds 2 MiB".into()); }
    let normalized = source.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut state = 0usize;
    let mut header = BTreeMap::new();
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
                if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with(';') { continue; }
                let (key, value) = trimmed.split_once('=').ok_or("milk2 header record has no value")?;
                let key = key.trim().to_ascii_lowercase();
                if key.is_empty() || key.len() > 256 || header.insert(key, value.trim().to_string()).is_some() {
                    return Err("milk2 header key is empty, oversized or duplicated".into());
                }
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
    let pattern = header.get("blending_pattern").cloned().unwrap_or_else(|| "Fade".into());
    if pattern.is_empty() || pattern.len() > 80 { return Err("invalid milk2 blending pattern".into()); }
    let number = |key: &str, default: f64| -> Result<f64, String> {
        let value = match header.get(key) {
            Some(value) => value.parse::<f64>().map_err(|_| format!("invalid milk2 {key}"))?,
            None => default,
        };
        if !value.is_finite() { return Err(format!("non-finite milk2 {key}")); }
        Ok(value)
    };
    let progress = number("blending_progress", 0.5)?;
    if !(0.0..=1.0).contains(&progress) { return Err("milk2 progress is outside 0..1".into()); }
    let direction = number("blending_direction", 1.0)?;
    if direction != -1.0 && direction != 1.0 { return Err("milk2 direction must be -1 or 1".into()); }
    let mut random = [None; 5];
    for (index, value) in random.iter_mut().enumerate() {
        let key = format!("random_{}", index + 1);
        if header.contains_key(&key) {
            let parsed = number(&key, 0.0)?;
            if !(0.0..=1.0).contains(&parsed) { return Err("milk2 saved random value is outside 0..1".into()); }
            *value = Some(parsed);
        }
    }
    Ok((SavedBlend { pattern, progress, direction: direction as i32, random, header, trailer }, presets))
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAIR: &str = "blending_pattern=Fade\nblending_progress=0.3\nblending_direction=-1\nrandom_1=0.123\n[PRESET1_BEGIN]\nMILKDROP_PRESET_VERSION=201\n[preset00]\nname=First\nzoom=1\n[PRESET1_END]\n[PRESET2_BEGIN]\n[preset00]\nname=Second\nzoom=1.1\n[PRESET2_END]\n";
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
    fn unknown_patterns_and_extensions_survive_but_do_not_claim_render_support() {
        let source = PAIR.replace("Fade", "Plasma") + "[SPRITE1_BEGIN]\nimg=x.png\n[SPRITE1_END]\n";
        let (blend, presets) = parse(&source).unwrap();
        assert!(!blend.supports_uniform_composite());
        assert!(blend.serialize(&presets).contains("img=x.png"));
    }

    #[test]
    fn milkdrop_32_display_name_does_not_disable_fade_and_round_trips() {
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
            PAIR.replace("direction=-1", "direction=0"), PAIR.replace("pattern=Fade", "pattern=Fade\nblending_pattern=Zoom")] {
            assert!(parse(&source).is_err());
        }
    }
}
