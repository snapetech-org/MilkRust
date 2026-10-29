//! MilkDrop 3's on-disk double-preset envelope. This module implements the
//! format, not a claim that every proprietary blend pattern is reproduced.
//! Source of format observations is recorded in docs/MILK2_FORMAT.md.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct SavedBlend {
    pub pattern: String,
    pub progress: f64,
    pub direction: i32,
    pub random: [Option<f64>; 5],
    /// Retain all header records for lossless export of unknown extensions.
    pub header: BTreeMap<String, String>,
    /// Embedded sprite/extensions after preset 2 are preserved, not executed.
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
        let mut saw_name = false;
        let disabled_sprites = self.header.get("sprite").map(String::as_str) == Some("0");
        let mut sprite_end: Option<String> = None;
        for line in self.trailer.lines() {
            let line = line.trim();
            let upper = line.to_ascii_uppercase();
            if let Some(end) = &sprite_end {
                if &upper == end { sprite_end = None; }
                else if upper.starts_with("[SPRITE") { return false; }
                continue;
            }
            if line.is_empty() || line.starts_with("//") || line.starts_with(';') { continue; }
            if let Some(index) = upper.strip_prefix("[SPRITE").and_then(|v| v.strip_suffix("_BEGIN]")) {
                // The explicit saved flag disables execution, not preservation.
                // Do not accept malformed or nested blocks as ordinary metadata.
                if !disabled_sprites || index.is_empty() || index.len()>6 || !index.bytes().all(|b| b.is_ascii_digit()) { return false; }
                sprite_end = Some(format!("[SPRITE{index}_END]"));
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { return false; };
            if !key.trim().eq_ignore_ascii_case("NAME") || saw_name || value.trim().len()>512 { return false; }
            saw_name = true;
        }
        sprite_end.is_none()
    }

    fn has_supported_header(&self) -> bool {
        self.header.iter().all(|(key,value)| match key.as_str() {
            "blending_pattern" | "blending_progress" | "blending_direction" |
            "random_1" | "random_2" | "random_3" | "random_4" | "random_5" => true,
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
        "cercle" | "circle" => 4,
        "plasma2" => 5,
        "plasma3" => 6,
        "snail" => 7,
        "snail2" => 8,
        "snail3" => 9,
        "triangle" => 10,
        "donuts" => 11,
        "corner" => 12,
        "patches" => 13,
        "checkerboard" => 14,
        "bubbles" => 15,
        "stars" => 16,
        "cisor" => 17,
        "wave" => 18,
        "curtain" => 19,
        "vertical" => 20,
        "horizontal" => 21,
        "linesvertical" => 22,
        "lineshorizontal" => 23,
        "square" => 24,
        "stars2" => 25,
        "clock" => 26,
        "nuclear" => 27,
        "arrow" => 28,
        "cross" => 29,
        "cross2" => 30,
        _ => return None,
    })
}

/// Shared GLSL for deterministic .milk2 spatial reveal masks. The equations
/// are MilkRust's implementation from documented pattern names, direction and
/// saved random values; upstream reference parity remains a qualification task.
pub const GLSL_TRANSITION_MASK: &str = r#"
uniform int u_transitionPattern;
uniform float u_transitionProgress;
uniform float u_transitionDirection;
uniform float u_transitionRandom1;
uniform float u_transitionRandom2;
uniform float u_transitionRandom3;
uniform float u_transitionRandom4;
uniform float u_transitionRandom5;
uniform vec2 u_transitionResolution;
float milkrust_transition_hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7)) + u_transitionRandom1 * 17.0 + u_transitionRandom5 * 31.0) * 43758.5453123);
}
float milkrust_transition_mask(vec2 uv) {
  float progress = clamp(u_transitionProgress, 0.0, 1.0);
  if (u_transitionPattern == 0) return 1.0;
  if (progress <= 0.0) return 0.0;
  if (progress >= 1.0) return 1.0;
  vec2 p = clamp(uv, vec2(0.0), vec2(1.0));
  vec2 centered = p - vec2(0.5);
  float radius = clamp(length(centered) * 1.41421356, 0.0, 1.0);
  float angle = fract(atan(centered.y, centered.x) * 0.15915494 + 0.5);
  float value = p.x;
  if (u_transitionPattern == 1) value = radius;
  else if (u_transitionPattern == 2 || u_transitionPattern == 20) value = p.x;
  else if (u_transitionPattern == 3) value = 0.5 + 0.24*sin(p.x*17.0+u_transitionRandom1*6.28318)*cos(p.y*13.0+u_transitionRandom2*6.28318) + 0.18*sin((p.x+p.y)*23.0+u_transitionRandom3*6.28318);
  else if (u_transitionPattern == 4) value = radius;
  else if (u_transitionPattern == 5) value = 0.5 + 0.25*sin(p.x*29.0+cos(p.y*17.0+u_transitionRandom1*6.28318)) + 0.2*cos(p.y*31.0+p.x*11.0+u_transitionRandom2*6.28318);
  else if (u_transitionPattern == 6) value = 0.5 + 0.25*cos((p.x-p.y)*27.0+u_transitionRandom3*6.28318) + 0.2*sin((p.x+p.y)*19.0+u_transitionRandom4*6.28318);
  else if (u_transitionPattern == 7) value = fract(angle + radius*2.0 + u_transitionRandom2);
  else if (u_transitionPattern == 8) value = fract(angle*2.0 + radius*3.0 + u_transitionRandom3);
  else if (u_transitionPattern == 9) value = fract(angle*3.0 - radius*4.0 + u_transitionRandom4);
  else if (u_transitionPattern == 10) value = clamp(max(abs(centered.x)*1.5, abs(centered.y)*1.7320508) * 1.1547005, 0.0, 1.0);
  else if (u_transitionPattern == 11) value = fract(radius*6.0 + u_transitionRandom5);
  else if (u_transitionPattern == 12) value = clamp(max(p.x, p.y), 0.0, 1.0);
  else if (u_transitionPattern == 13) {
    vec2 cell = floor(p*8.0); vec2 local = fract(p*8.0)-0.5;
    value = clamp(milkrust_transition_hash(cell)*0.72 + length(local)*0.42, 0.0, 1.0);
  }
  else if (u_transitionPattern == 14) {
    vec2 cell = floor(p*8.0); float parity = mod(cell.x+cell.y, 2.0);
    value = clamp(parity*0.58 + milkrust_transition_hash(cell)*0.32 + radius*0.1, 0.0, 1.0);
  }
  else if (u_transitionPattern == 15) {
    vec2 cell = floor(p*8.0); vec2 local = fract(p*8.0);
    vec2 center = vec2(milkrust_transition_hash(cell), milkrust_transition_hash(cell+vec2(19.0,7.0)));
    float distanceToBubble = length(local-center);
    value = clamp(milkrust_transition_hash(cell+vec2(5.0,13.0))*0.7 + distanceToBubble*0.9, 0.0, 1.0);
  }
  else if (u_transitionPattern == 16 || u_transitionPattern == 25) {
    vec2 cell = floor(p*10.0); vec2 local = fract(p*10.0)-0.5;
    float star = max(abs(local.x), abs(local.y)) + 0.35*min(abs(local.x), abs(local.y));
    float seed = milkrust_transition_hash(cell);
    value = clamp(seed*0.72 + star*(u_transitionPattern == 16 ? 0.62 : 0.9), 0.0, 1.0);
  }
  else if (u_transitionPattern == 17) value = clamp(abs(centered.x-centered.y)*1.41421356, 0.0, 1.0);
  else if (u_transitionPattern == 18) value = clamp(p.x + 0.16*sin(p.y*25.13274+u_transitionRandom1*6.28318), 0.0, 1.0);
  else if (u_transitionPattern == 19) value = clamp(p.x + 0.2*sin(p.y*12.56637+u_transitionRandom2*6.28318), 0.0, 1.0);
  else if (u_transitionPattern == 21) value = p.y;
  else if (u_transitionPattern == 22) value = fract(p.x*12.0 + u_transitionRandom3);
  else if (u_transitionPattern == 23) value = fract(p.y*12.0 + u_transitionRandom4);
  else if (u_transitionPattern == 24) value = clamp(max(abs(centered.x),abs(centered.y))*2.0, 0.0, 1.0);
  else if (u_transitionPattern == 26) value = angle;
  else if (u_transitionPattern == 27) value = fract(radius*7.0 + angle*0.75 + u_transitionRandom5);
  else if (u_transitionPattern == 28) value = clamp(max(p.x,abs(centered.y)*1.8),0.0,1.0);
  else if (u_transitionPattern == 29) value = clamp(max(abs(centered.x),abs(centered.y))*2.0,0.0,1.0);
  else if (u_transitionPattern == 30) {
    vec2 cell = floor(p*6.0); float parity = mod(cell.x+cell.y,2.0);
    float crossValue = clamp(max(abs(centered.x),abs(centered.y))*2.0,0.0,1.0);
    value = clamp(0.58*crossValue+0.42*parity,0.0,1.0);
  }
  if (u_transitionDirection < 0.0) value = 1.0-value;
  float edge = max(0.0015, 1.5/max(max(u_transitionResolution.x,u_transitionResolution.y),1.0));
  return 1.0-smoothstep(progress-edge, progress+edge, clamp(value,0.0,1.0));
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
            if parsed.abs() > 1.0e9 { return Err("milk2 saved random value exceeds its limit".into()); }
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
        let source = PAIR.replace("Fade", "Plasma") + "[SPRITE0_BEGIN]\nimg=x.png\n[SPRITE0_END]\n";
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
        let source=format!("sprite=0\n{PAIR}[SPRITE0_BEGIN]\nSpriteName=art.png\ncode_1=x=1;\n[SPRITE0_END]\nNAME=With disabled sprite\n");
        let (blend,presets)=parse(&source).unwrap();
        assert!(blend.supports_uniform_composite());
        assert_eq!(parse(&blend.serialize(&presets)).unwrap(),(blend,presets));
        for invalid in [source.replace("sprite=0","sprite=1"),source.replace("[SPRITE0_END]",""),source.replace("code_1=x=1;","[SPRITE1_BEGIN]")] {
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
