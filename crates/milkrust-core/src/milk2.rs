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
    /// Only this family currently has an exact implemented spatial mask. Other
    /// patterns must not silently turn into an ordinary temporal cross-fade.
    pub fn supports_uniform_composite(&self) -> bool {
        self.pattern.eq_ignore_ascii_case("fade") && self.trailer.trim().is_empty()
            && self.header.keys().all(|key| matches!(key.as_str(),
                "blending_pattern" | "blending_progress" | "blending_direction" |
                "random_1" | "random_2" | "random_3" | "random_4" | "random_5"))
    }

    pub fn render_requirement(&self) -> Option<String> {
        if self.supports_uniform_composite() { None } else {
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
    fn malformed_blocks_and_nonfinite_metadata_fail() {
        for source in [PAIR.replace("[PRESET1_END]", ""), PAIR.replace("[PRESET2_BEGIN]", "[PRESET1_BEGIN]"),
            PAIR.replace("progress=0.3", "progress=NaN"), PAIR.replace("progress=0.3", "progress=1.01"),
            PAIR.replace("direction=-1", "direction=0"), PAIR.replace("pattern=Fade", "pattern=Fade\nblending_pattern=Zoom")] {
            assert!(parse(&source).is_err());
        }
    }
}
