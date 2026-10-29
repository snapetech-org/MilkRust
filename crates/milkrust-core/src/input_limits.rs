//! Allocation preflight for preset text. Rejected inputs stay in compatibility
//! reports; no sparse indexed object array is allocated before this check.

pub const MAX_PRESET_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_INDEXED_ENTRIES: usize = 64;
/// MilkDrop 3 exposes four custom shape and four custom wave slots (indices 0–3).
pub const MILKDROP3_CUSTOM_SHAPE_COUNT: usize = 4;
pub const MILKDROP3_CUSTOM_WAVE_COUNT: usize = 4;

pub fn validate_preset_source(source: &str) -> Result<(), String> {
    if source.len() > MAX_PRESET_BYTES || source.as_bytes().contains(&0) {
        return Err("Preset text is binary or exceeds 2 MiB".to_string());
    }
    let mut record_count = 0usize;
    for (line_number, raw_line) in source.split(['\r', '\n']).enumerate() {
        if line_number >= 32_768 || raw_line.len() > 128 * 1024 {
            return Err("Preset exceeds its line count or 128 KiB line limit".to_string());
        }
        let line = raw_line.trim();
        if line.starts_with(';') || line.starts_with("//") { continue; }
        let Some((raw_key, _)) = line.split_once('=') else { continue; };
        record_count += 1;
        if record_count > 16_384 || raw_key.trim().len() > 256 {
            return Err("Preset exceeds its record count or 256-byte key limit".to_string());
        }
        let key = raw_key.trim().to_ascii_lowercase();
        for prefix in ["shapecode_", "shape_", "shape", "spritecode_", "sprite_", "sprite", "wavecode_", "wave_"] {
            let Some(rest) = key.strip_prefix(prefix) else { continue; };
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 || !rest[digits..].starts_with('_') { continue; }
            if rest[..digits].parse::<usize>().map_or(true, |index| index >= MAX_INDEXED_ENTRIES) {
                return Err(format!("Indexed object key {key} exceeds the 64-entry limit"));
            }
        }
    }
    Ok(())
}
