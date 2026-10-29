//! Saved MilkDrop sprite blocks and their independent EEL lifetime.
//! No image is opened here: hosts resolve admitted, pack-local dependencies.
use std::collections::BTreeMap;
use crate::{MilkRustValue, evaluate_milkrust_equations, milkrust_scope_number};

/// MilkDrop's preset texture manager exposes 16 sprite slots. Its separate
/// external sprite image cache can hold 100 assets; those are not preset slots.
pub const MAX_SPRITES: usize = 16;
const MAX_SPRITE_RECORDS: usize = 64;
const DEFAULT_COLOR_KEY_ARGB: u32 = 0xff00_0000;
pub const SPRITE_BLEND_MODES: usize = 11;

fn parse_color_key(value: &str) -> Result<u32, String> {
    // The MD3 loader uses strtoul(..., base=0): 0x is hexadecimal, a leading
    // zero is octal, and other values are decimal. Keep the result within the
    // original 32-bit Windows unsigned-long domain before forcing opaque alpha.
    let (negative, unsigned) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else if let Some(value) = value.strip_prefix('+') {
        (false, value)
    } else {
        (false, value)
    };
    let (digits, radix) = if let Some(hex) = unsigned.strip_prefix("0x").or_else(|| unsigned.strip_prefix("0X")) {
        (hex, 16)
    } else if unsigned.len() > 1 && unsigned.starts_with('0') {
        (unsigned, 8)
    } else {
        (unsigned, 10)
    };
    if digits.is_empty() {
        return Err("Invalid sprite color key".into());
    }
    let mut parsed = u64::from_str_radix(digits, radix).ok().filter(|value| *value <= u64::from(u32::MAX))
        .ok_or("Invalid sprite color key")? as u32;
    if negative { parsed = parsed.wrapping_neg(); }
    Ok(0xff00_0000 | parsed)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub index: u32,
    /// Zero-based owner preset. A trailing sprite block in a `.milk2` belongs to PRESET2.
    pub preset_index: usize,
    pub texture: String,
    pub color_key: u32,
    pub layer: i32,
    pub blend_mode: i32,
    pub alpha: f64,
    pub burn_mode: f64,
    pub rotation_mode: f64,
    pub rotation_speed: f64,
    pub offset: [f64; 2],
    pub size_offset: [f64; 2],
    pub repeat_multiplier: [f64; 2],
    pub values: BTreeMap<String, MilkRustValue>,
    pub init: String,
    pub code: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Blocks {
    pub body: String,
    /// Original records, including disabled blocks, retained on export.
    pub source: String,
    pub definitions: Vec<Definition>,
}

fn marker(line: &str, suffix: &str) -> Option<u32> {
    let upper = line.trim().to_ascii_uppercase();
    let digits = upper.strip_prefix("[SPRITE")?.strip_suffix(suffix)?;
    if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit()) { return None; }
    digits.parse().ok()
}

/// Separate complete blocks without allowing their keys to become preset equations.
/// Last definition of an index wins, matching saved-file replacement semantics.
pub fn extract(source: &str, enabled: bool) -> Result<Blocks, String> {
    crate::input_limits::validate_preset_source(source)?;
    let mut result = Blocks::default();
    let mut definitions = BTreeMap::new();
    let mut current: Option<(u32, usize, bool)> = None;
    let mut preset_state = 0u8;
    let mut records = BTreeMap::new();
    let mut blocks = 0usize;
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    for raw in normalized.lines() {
        let line = raw.trim();
        if current.is_none() {
            match line.to_ascii_uppercase().as_str() {
                "[PRESET1_BEGIN]" => preset_state = 1,
                "[PRESET1_END]" => preset_state = 2,
                "[PRESET2_BEGIN]" => preset_state = 3,
                "[PRESET2_END]" => preset_state = 4,
                _ => {}
            }
        }
        if let Some(index) = marker(line, "_BEGIN]") {
            if current.is_some() { return Err("Nested embedded sprite block".into()); }
            blocks += 1;
            if blocks > MAX_SPRITE_RECORDS { return Err("More than 64 saved sprite records".into()); }
            // MD3 reads the trailing sprite block as PRESET2's sprite; blocks
            // inside either preset half are inert. Ordinary `.milk` blocks
            // belong to the only preset at index zero.
            let owner = if preset_state == 4 { 1 } else { 0 };
            let executable = enabled && !matches!(preset_state, 1 | 3);
            current = Some((index, owner, executable));
            records.clear();
        } else if let Some(index) = marker(line, "_END]") {
            let Some((active_index, owner, executable)) = current else {
                return Err("Mismatched embedded sprite block end".into());
            };
            if active_index != index { return Err("Mismatched embedded sprite block end".into()); }
            if executable { definitions.insert(index, definition(index, owner, &records)?); }
            result.source.push_str(raw);
            result.source.push('\n');
            current = None;
            continue;
        } else if line.to_ascii_uppercase().starts_with("[SPRITE") {
            return Err("Invalid embedded sprite marker".into());
        } else if current.is_some_and(|(_, _, executable)| executable) && !line.is_empty() && !line.starts_with([';', '#']) && !line.starts_with("//") {
            let (key, value) = line.split_once('=').ok_or("Invalid embedded sprite record")?;
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if key == "spritename" {
                // The legacy reader replaces the image name when it is
                // repeated, while the other saved sprite fields retain their
                // first value.
                records.insert(key, value);
            } else {
                records.entry(key).or_insert(value);
            }
        }
        let target = if current.is_some() { &mut result.source } else { &mut result.body };
        target.push_str(raw);
        target.push('\n');
    }
    if current.is_some() { return Err("Unterminated embedded sprite block".into()); }
    if definitions.len() > MAX_SPRITES { return Err("More than 16 active embedded sprites".into()); }
    result.definitions = definitions.into_values().collect();
    Ok(result)
}

/// The saved enable flag belongs to the envelope/header, not to sprite code.
pub fn enabled(source: &str) -> bool {
    let milk2 = source.lines().any(|line| line.trim().eq_ignore_ascii_case("[PRESET1_BEGIN]"));
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') { break; }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("sprite") {
                return value.trim() == "1";
            }
        }
    }
    // Ordinary `.milk` files execute saved sprite blocks by default. The
    // trailing block in a `.milk2` envelope is opt-in through `sprite=1`.
    !milk2
}

fn definition(index: u32, preset_index: usize, records: &BTreeMap<String, String>) -> Result<Definition, String> {
    if !(1..=MAX_SPRITES as u32).contains(&index) { return Err("Embedded sprite index must be between 1 and 16".into()); }
    let texture = records.get("spritename").or_else(|| records.get("img"))
        .ok_or("Embedded sprite has no image name")?.trim_matches(['\'', '"']).replace('\\', "/");
    if texture.is_empty() || texture.len() > 512 || texture.starts_with('/') || texture.contains(':')
        || texture.chars().any(char::is_control) || texture.split('/').any(|v| v == ".." || v.is_empty()) {
        return Err("Embedded sprite image must be a relative pack asset".into());
    }
    let color_key = match records.get("spritecolorkey").or_else(|| records.get("colorkey")) {
        None => DEFAULT_COLOR_KEY_ARGB,
        Some(value) => parse_color_key(value)?,
    };
    let mut values = BTreeMap::new();
    for (key, fallback) in [("x", 0.5), ("y", 0.5), ("sx", 1.0), ("sy", 1.0), ("rot", 0.0),
        ("repeatx", 1.0), ("repeaty", 1.0), ("r", 1.0), ("g", 1.0), ("b", 1.0),
        ("a", 1.0), ("burn", 1.0), ("flipx", 0.0), ("flipy", 0.0), ("done", 0.0)] {
        values.insert(key.into(), MilkRustValue::Number(fallback));
    }
    let mut layer = 0i32;
    // MilkDrop 3's saved default is ColorKey (mode 4). A per-frame value
    // outside 0..=10 falls back to mode 0 in the renderer.
    let mut blend_mode = 4i32;
    let mut alpha = 1.0;
    let mut burn_mode = 1.0;
    let mut rotation_mode = 0.0;
    let mut rotation_speed = 0.0;
    let mut repeat_multiplier = [1.0; 2];
    let mut offset = [0.0; 2];
    let mut size_offset = [-0.5; 2];
    let mut init = BTreeMap::new();
    let mut code = BTreeMap::new();
    for (key, value) in records {
        if matches!(key.as_str(), "spritename" | "img" | "spritecolorkey" | "colorkey") { continue; }
        if let Some((prefix, number)) = key.split_once('_').filter(|(p, _)| matches!(*p, "init" | "code")) {
            let number = number.parse::<u32>().ok().filter(|n| (1..=1024).contains(n)).ok_or("Invalid sprite equation line")?;
            if prefix == "init" { init.insert(number, value.as_str()); } else { code.insert(number, value.as_str()); }
            continue;
        }
        if matches!(key.as_str(), "spritex" | "spritey" | "spritesx" | "spritesy") {
            let number = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1000.0).ok_or("Invalid sprite offset")?;
            let axis = usize::from(key.ends_with('y'));
            if key.starts_with("sprites") { size_offset[axis] = number; } else { offset[axis] = number; }
            continue;
        }
        if matches!(key.as_str(), "spritelayer" | "layer") {
            layer = value.parse::<i32>().map_err(|_| "Invalid sprite layer")?;
            continue;
        }
        if matches!(key.as_str(), "spriteblend" | "blendmode") {
            blend_mode = value.parse::<i32>().map_err(|_| "Invalid sprite blend mode")?;
            continue;
        }
        if matches!(key.as_str(), "spriteburn" | "burnmode") {
            burn_mode = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9)
                .ok_or("Invalid sprite burn mode")?;
            continue;
        }
        if key == "spritealpha" {
            alpha = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9)
                .ok_or("Invalid sprite alpha")?;
            continue;
        }
        if key == "spriterot" {
            rotation_mode = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9)
                .ok_or("Invalid sprite rotation mode")?;
            continue;
        }
        if key == "spritespeed" {
            rotation_speed = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9)
                .ok_or("Invalid sprite rotation speed")?;
            continue;
        }
        if matches!(key.as_str(), "spriterepeatx" | "spriterepeaty") {
            let number = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9)
                .ok_or("Invalid sprite repeat multiplier")?;
            repeat_multiplier[usize::from(key.ends_with('y'))] = number;
            continue;
        }
        let variable = match key.as_str() {
            key => key.strip_prefix("sprite").unwrap_or(key),
        };
        if !values.contains_key(variable) { return Err(format!("Unsupported embedded sprite field: {key}")); }
        let number = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9).ok_or("Non-finite or oversized sprite value")?;
        values.insert(variable.into(), MilkRustValue::Number(number));
    }
    values.insert("blendmode".into(), MilkRustValue::Number(f64::from(blend_mode)));
    let join = |lines: BTreeMap<u32, &str>| lines.into_values().collect::<Vec<_>>().join("\n");
    let result = Definition { index, preset_index, texture, color_key, layer, blend_mode, alpha, burn_mode,
        rotation_mode, rotation_speed, offset, size_offset, repeat_multiplier, values, init: join(init), code: join(code) };
    crate::validate_expression_source(&result.init)?;
    crate::validate_expression_source(&result.code)?;
    Ok(result)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub preset_index: usize,
    pub texture: String,
    /// Sprite-local elapsed time used by animated GIF sprite textures.
    pub time: f64,
    pub color_key: u32,
    pub blend_mode: i32,
    /// Layer 0 draws over the composite; when burning, it also stamps feedback.
    /// Layers 1..4 draw into feedback at their ordered scene stage. Burning
    /// sprites in those layers are drawn twice into feedback.
    pub layer: usize,
    pub burn: bool,
    pub color: [f64; 4],
    pub center: [f64; 2],
    pub scale: [f64; 2],
    pub repeat: [f64; 2],
    /// EEL flip selectors mirror the sampled image around its center.
    pub flip: [bool; 2],
    pub rotation: f64,
    /// Last frame of a burning sprite receives MD3's aspect correction.
    pub final_burn: bool,
}

impl Frame {
    /// The saved key is ARGB; GL uniforms use RGBA order.
    pub fn color_key_rgba(&self) -> [f32; 4] {
        [((self.color_key >> 16) & 255) as f32 / 255.0,
         ((self.color_key >> 8) & 255) as f32 / 255.0,
         (self.color_key & 255) as f32 / 255.0,
         ((self.color_key >> 24) & 255) as f32 / 255.0]
    }

    /// Exact packed diffuse color supplied to the legacy D3D sprite vertex.
    /// Quantizing here avoids driver-dependent float rounding at the byte edge.
    pub fn vertex_color(&self) -> [f32; 4] {
        sprite_vertex_color(self.blend_mode, self.color)
            .map(|channel| f32::from(channel) / 255.0)
    }

    /// Image aspect is applied before rotation; screen aspect after rotation.
    /// UVs are bottom-up for the common GL texture upload convention.
    pub fn geometry(&self, image: (u32, u32), screen: (u32, u32)) -> (Vec<f64>, Vec<f64>) {
        let image_aspect = image.0.max(1) as f64 / image.1.max(1) as f64;
        let screen_aspect = screen.0.max(1) as f64 / screen.1.max(1) as f64;
        let (sin, cos) = self.rotation.sin_cos();
        let flip_x = if self.flip[0] { -1.0 } else { 1.0 };
        let flip_y = if self.flip[1] { -1.0 } else { 1.0 };
        let mut positions = Vec::with_capacity(8);
        let mut uvs = Vec::with_capacity(8);
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let local_x = x * self.scale[0] * image_aspect.min(1.0);
            let local_y = y * self.scale[1] / image_aspect.max(1.0);
            positions.push((local_x * cos - local_y * sin + self.center[0] * 2.0 - 1.0) / screen_aspect.min(1.0));
            positions.push((local_x * sin + local_y * cos + 1.0 - self.center[1] * 2.0) * screen_aspect.max(1.0));
            uvs.extend_from_slice(&[
                0.5 + x * 0.5 * self.repeat[0] * flip_x,
                0.5 + y * 0.5 * self.repeat[1] * flip_y,
            ]);
        }
        if self.final_burn {
            let burn_aspect = screen.0.max(1) as f64 / (screen.1.max(1) as f64 * 4.0 / 3.0);
            for position in positions.chunks_exact_mut(2) {
                if burn_aspect >= 1.0 { position[1] /= burn_aspect; }
                else { position[0] *= burn_aspect; }
            }
        }
        (positions, uvs)
    }
}

#[derive(Clone, Debug, Default)]
struct Instance { scope: BTreeMap<String, MilkRustValue>, initialized: bool, dead: bool }

#[derive(Clone, Debug, Default)]
pub struct Runtime { instances: Vec<Instance>, start: Option<f64>, previous: Option<f64>, frame: u64 }

impl Runtime {
    pub fn render(&mut self, definitions: &[Definition], parent: &BTreeMap<String, MilkRustValue>, time: f64, progress: f64) -> Result<Vec<Frame>, String> {
        self.render_with_parents(definitions, std::slice::from_ref(parent), time, progress)
    }

    pub fn render_with_parents(&mut self, definitions: &[Definition], parents: &[BTreeMap<String, MilkRustValue>], time: f64, progress: f64) -> Result<Vec<Frame>, String> {
        let checkpoint = self.clone();
        match self.render_with_parents_inner(definitions, parents, time, progress) {
            Ok(frames) => Ok(frames),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
        }
    }

    fn render_with_parents_inner(&mut self, definitions: &[Definition], parents: &[BTreeMap<String, MilkRustValue>], time: f64, progress: f64) -> Result<Vec<Frame>, String> {
        if !time.is_finite() { return Err("Non-finite sprite time".into()); }
        if !progress.is_finite() { return Err("Non-finite sprite progress".into()); }
        if definitions.len() > MAX_SPRITES { return Err("Sprite instance limit exceeded".into()); }
        self.instances.resize_with(definitions.len(), Instance::default);
        let start = *self.start.get_or_insert(time);
        let elapsed = (time - start).max(0.0);
        if !elapsed.is_finite() { return Err("Sprite elapsed time overflow".into()); }
        let delta = self.previous.map_or(0.0, |last| (time - last).clamp(0.0, 1.0));
        self.previous = Some(time);
        let fps = if delta > 0.0 { 1.0 / delta } else { 60.0 };
        let mut frames = Vec::new();
        for (definition, instance) in definitions.iter().zip(&mut self.instances) {
            // MD3 does not evaluate or draw a sprite on an unsupported layer.
            if !(0..=4).contains(&definition.layer) { continue; }
            if instance.dead { continue; }
            if !instance.initialized { instance.scope = definition.values.clone(); }
            let parent = parents.get(definition.preset_index).or_else(|| parents.first())
                .ok_or("Sprite has no owning preset scope")?;
            for key in ["bass", "mid", "treb", "bass_att", "mid_att", "treb_att"] {
                instance.scope.insert(key.into(), MilkRustValue::Number(milkrust_scope_number(parent, key, 0.0)));
            }
            for (key, value) in [("time", elapsed), ("frame", self.frame as f64), ("fps", fps), ("progress", progress)] {
                instance.scope.insert(key.into(), MilkRustValue::Number(value));
            }
            if !instance.initialized {
                instance.scope = evaluate_milkrust_equations(&definition.init, &instance.scope)?;
                instance.initialized = true;
            }
            instance.scope = evaluate_milkrust_equations(&definition.code, &instance.scope)?;
            let value = |key: &str| instance.scope.get(key).and_then(MilkRustValue::as_number).unwrap_or(0.0);
            for key in ["x", "y", "sx", "sy", "rot", "repeatx", "repeaty", "r", "g", "b", "a", "burn", "blendmode", "done", "flipx", "flipy"] {
                if !value(key).is_finite() { return Err(format!("Non-finite sprite variable: {key}")); }
            }
            // SpriteRot is an outer mode: 1/2 add/subtract absolute host time
            // times SpriteSpeed. EEL `rot` remains an independent persistent value.
            let spin = time * definition.rotation_speed;
            let rotation = value("rot") + if definition.rotation_mode == 1.0 { spin }
                else if definition.rotation_mode == 2.0 { -spin } else { 0.0 };
            if !rotation.is_finite() { return Err("Non-finite sprite rotation".into()); }
            let done = value("done") != 0.0;
            let burn = if definition.burn_mode == 0.0 { false }
                else if definition.burn_mode == 1.0 { true }
                else { value("burn") != 0.0 };
            let x = ((value("x") * 2.0 - 1.0).clamp(-1000.0, 1000.0) + definition.offset[0] + 1.0) * 0.5;
            let y = ((value("y") * 2.0 - 1.0).clamp(-1000.0, 1000.0) + definition.offset[1] + 1.0) * 0.5;
            // MilkDrop truncates the EEL value; values outside the complete
            // saved table select ordinary alpha blend mode 0.
            let requested_blend_mode = value("blendmode").trunc() as i32;
            let blend_mode = sprite_blend_mode(requested_blend_mode);
            frames.push(Frame {
                preset_index: definition.preset_index,
                texture: definition.texture.clone(), time: elapsed, color_key: definition.color_key,
                blend_mode, layer: definition.layer as usize, burn,
                // RGB is clamped before CPU-side D3DCOLOR byte packing;
                // script alpha is clamped before SpriteAlpha multiplies it,
                // then left unclamped for the legacy low-byte conversion.
                color: [value("r").clamp(0.0, 1.0), value("g").clamp(0.0, 1.0), value("b").clamp(0.0, 1.0), value("a").clamp(0.0, 1.0) * definition.alpha],
                center: [x, y],
                scale: [value("sx").clamp(-1000.0, 1000.0) + definition.size_offset[0],
                    value("sy").clamp(-1000.0, 1000.0) + definition.size_offset[1]],
                repeat: [value("repeatx").clamp(0.01, 100.0) * definition.repeat_multiplier[0],
                    value("repeaty").clamp(0.01, 100.0) * definition.repeat_multiplier[1]],
                flip: [value("flipx") != 0.0, value("flipy") != 0.0],
                rotation,
                final_burn: done && burn,
            });
            // The final frame may burn before the instance is retired.
            instance.dead = done;
        }
        frames.sort_by_key(|frame| frame.layer);
        self.frame = self.frame.saturating_add(1);
        Ok(frames)
    }
}

/// MilkDrop 3 has eleven saved sprite blend modes. Any value outside the
/// table selects mode 0 instead of clamping to an endpoint.
pub fn sprite_blend_mode(requested: i32) -> i32 {
    if (0..SPRITE_BLEND_MODES as i32).contains(&requested) { requested } else { 0 }
}

fn sprite_byte(value: f32) -> u8 {
    // D3D receives a 32-bit float channel, truncates to a signed integer, then
    // keeps the low byte. Rust's float-to-int cast has the same saturation
    // boundary as the legacy conversion; the integer-to-u8 cast keeps 8 bits.
    (value * 255.0) as i32 as u8
}

/// D3DCOLOR-compatible vertex byte quantization for the saved sprite table.
/// RGB is saturated before mode-specific transforms; alpha intentionally
/// wraps after `SpriteAlpha` multiplication.
pub fn sprite_vertex_color(mode: i32, rgba: [f64; 4]) -> [u8; 4] {
    let mode = sprite_blend_mode(mode);
    let channels = rgba.map(|value| value as f32);
    let rgb = [channels[0].clamp(0.0, 1.0), channels[1].clamp(0.0, 1.0), channels[2].clamp(0.0, 1.0)];
    let alpha = channels[3];
    match mode {
        1 | 2 => [sprite_byte(rgb[0] * alpha), sprite_byte(rgb[1] * alpha),
            sprite_byte(rgb[2] * alpha), 255],
        3 => [255; 4],
        7 => [255, 255, 255, sprite_byte(alpha)],
        8 => {
            let fold = |value: f32| if value > 0.5 { 1.0 - value } else { value };
            [sprite_byte(fold(rgb[0])), sprite_byte(fold(rgb[1])),
                sprite_byte(fold(rgb[2])), sprite_byte(alpha)]
        }
        _ => [sprite_byte(rgb[0]), sprite_byte(rgb[1]),
            sprite_byte(rgb[2]), sprite_byte(alpha)],
    }
}

/// CPU reference for the fixed-function D3D9 sprite stage and blend-factor
/// table, expressed in 0..=255 RGB units without final integer rounding.
pub fn sprite_blend_pixel(mode: i32, rgba: [f64; 4], texel: [u8; 4], dst: [u8; 3]) -> [f64; 3] {
    let mode = sprite_blend_mode(mode);
    let vertex = sprite_vertex_color(mode, rgba).map(|value| f64::from(value) / 255.0);
    let texture = texel.map(|value| f64::from(value) / 255.0);
    let destination = dst.map(|value| f64::from(value) / 255.0);
    let alpha = match mode {
        4 | 7 | 8 => vertex[3] * texture[3],
        _ => vertex[3],
    };
    let saturate = |value: f64| value.clamp(0.0, 1.0);
    let mut output = [0.0; 3];
    for channel in 0..3 {
        let source = match mode {
            3 => texture[channel],
            7 => saturate(1.0 - texture[channel]),
            9 => saturate(texture[channel] + vertex[channel] - 0.5),
            10 => saturate(2.0 * texture[channel] * vertex[channel]),
            _ => vertex[channel] * texture[channel],
        };
        let (source_factor, destination_factor) = match mode {
            1 => (1.0, 0.0),
            2 | 10 => (1.0, 1.0),
            3 => (source, 1.0 - source),
            5 => (destination[channel], 0.0),
            6 => (0.0, 1.0 - source),
            9 => (0.0, source),
            _ => (alpha, 1.0 - alpha),
        };
        output[channel] = saturate(source * source_factor + destination[channel] * destination_factor) * 255.0;
    }
    output
}

/// Common native/WebGL sprite compositing; destination is a distinct snapshot,
/// never the texture attached to the draw framebuffer. The eleven mode IDs
/// follow MilkDrop 3's fixed-function `DrawUserSprites` blend table.
pub const FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D spriteImage;
uniform sampler2D spriteBackground;
uniform vec2 spriteResolution;
uniform vec4 spriteTint;
uniform vec4 spriteColorKey;
uniform int spriteBlend;
in vec2 uv;
out vec4 outColor;
vec4 sprite_texel(ivec2 point, ivec2 size) {
    point = ((point % size) + size) % size;
    vec4 color = texelFetch(spriteImage, point, 0);
    // D3DX uses an ARGB key and replaces the whole matching texel with
    // transparent black before sampling.
    if (all(lessThanEqual(abs(color - spriteColorKey), vec4(0.5 / 255.0)))) color = vec4(0.0);
    return color;
}
vec4 sprite_sample(vec2 coordinate) {
    // Apply the key to source texels before linear filtering, as at decode.
    // Non-keyed texels keep ordinary RGBA interpolation, including hidden RGB.
    ivec2 size = textureSize(spriteImage, 0);
    vec2 position = coordinate * vec2(size) - vec2(0.5);
    ivec2 origin = ivec2(floor(position));
    vec2 weight = fract(position);
    return mix(mix(sprite_texel(origin,size), sprite_texel(origin+ivec2(1,0),size), weight.x),
        mix(sprite_texel(origin+ivec2(0,1),size), sprite_texel(origin+ivec2(1,1),size), weight.x), weight.y);
}

void main() {
    vec4 sampleColor = sprite_sample(uv);
    vec4 background = texture(spriteBackground, gl_FragCoord.xy / spriteResolution);
    int mode = spriteBlend < 0 || spriteBlend > 10 ? 0 : spriteBlend;
    vec4 diffuse = spriteTint;
    vec3 texel = sampleColor.rgb;
    vec3 stage = diffuse.rgb * texel;
    vec3 dst = background.rgb;
    float alpha = diffuse.a;
    if (mode == 4 || mode == 7 || mode == 8) alpha *= sampleColor.a;
    if (mode == 3) stage = texel;
    else if (mode == 7) stage = vec3(1.0) - texel;
    else if (mode == 9) stage = clamp(texel + diffuse.rgb - vec3(0.5), 0.0, 1.0);
    else if (mode == 10) stage = clamp(2.0 * texel * diffuse.rgb, 0.0, 1.0);
    vec3 result;
    if (mode == 1) result = stage; // decal: blending disabled
    else if (mode == 2) result = dst + stage;
    else if (mode == 3) result = stage * stage + dst * (vec3(1.0) - stage);
    else if (mode == 5) result = dst * stage;
    else if (mode == 6) result = dst * (vec3(1.0) - stage);
    else if (mode == 9) result = dst * stage;
    else if (mode == 10) result = dst + stage;
    else result = mix(dst, stage, alpha);
    outColor = vec4(clamp(result, 0.0, 1.0), background.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    fn block(records: &str) -> String {
        format!("[SPRITE1_BEGIN]\nSpriteName=sprites/burst.png\n{records}\n[SPRITE1_END]\n")
    }

    #[test]
    fn saved_rotation_mode_adds_absolute_time_without_mutating_eel_rot() {
        let definitions=extract(&block("SpriteRot=1\nSpriteSpeed=1"),true).unwrap().definitions;
        let mut runtime=Runtime::default();
        let first=runtime.render(&definitions,&BTreeMap::new(),0.0,0.0).unwrap();
        let second=runtime.render(&definitions,&BTreeMap::new(),0.5,0.0).unwrap();
        assert_eq!(first[0].rotation, 0.0);
        assert_eq!(second[0].rotation, 0.5);
    }
    #[test]
    fn rotation_register_can_cross_full_turns_and_retire_the_sprite() {
        let definitions = extract(&block("code_1=rot=rot+4;done=above(rot,7);"), true).unwrap().definitions;
        let mut runtime = Runtime::default();
        assert_eq!(runtime.render(&definitions, &BTreeMap::new(), 0.0, 0.0).unwrap()[0].rotation, 4.0);
        assert_eq!(runtime.render(&definitions, &BTreeMap::new(), 1.0/60.0, 0.0).unwrap()[0].rotation, 8.0);
        assert!(runtime.render(&definitions, &BTreeMap::new(), 2.0/60.0, 0.0).unwrap().is_empty());
    }

    #[test]
    fn saved_blend_and_layer_follow_the_preset_controls() {
        for (records, mode, layer) in [("SpriteBlend=9\nSpriteLayer=2", 9, 2),
            ("SpriteBlend=-2\nSpriteLayer=0", 0, 0), ("SpriteBlend=99\nSpriteLayer=4", 0, 4)] {
            let definitions = extract(&block(records), true).unwrap().definitions;
            let frames = Runtime::default().render(&definitions, &BTreeMap::new(), 0.0, 0.0).unwrap();
            assert_eq!(frames[0].blend_mode, mode);
            assert_eq!(frames[0].layer, layer);
        }
        let hidden = extract(&block("SpriteLayer=5\ncode_1=x=0;"), true).unwrap().definitions;
        assert!(Runtime::default().render(&hidden, &BTreeMap::new(), 0.0, 0.0).unwrap().is_empty());
    }

    #[test]
    fn duplicate_saved_fields_keep_first_values_except_sprite_name() {
        let source = "[SPRITE1_BEGIN]\nSpriteName=sprites/first.png\nSpriteLayer=2\nSpriteAlpha=0.25\nSpriteName=sprites/last.png\nSpriteLayer=4\nSpriteAlpha=0.75\n[SPRITE1_END]\n";
        let definitions = extract(source, true).unwrap().definitions;
        assert_eq!(definitions[0].texture, "sprites/last.png");
        assert_eq!(definitions[0].layer, 2);
        assert_eq!(definitions[0].alpha, 0.25);
    }

    #[test]
    fn saved_blend_defaults_to_four_and_runtime_supports_all_eleven_modes() {
        let default = extract(&block(""), true).unwrap().definitions;
        assert_eq!(default[0].blend_mode, 4);
        assert_eq!(Runtime::default().render(&default, &BTreeMap::new(), 0.0, 0.0).unwrap()[0].blend_mode, 4);
        for mode in 0..=10 {
            let definitions = extract(&block(&format!("SpriteBlend={mode}\ncode_1=blendmode={mode};")), true).unwrap().definitions;
            let frame = Runtime::default().render(&definitions, &BTreeMap::new(), 0.0, 0.0).unwrap().remove(0);
            assert_eq!(frame.blend_mode, mode);
        }
        let out_of_range = extract(&block("code_1=blendmode=99;"), true).unwrap().definitions;
        assert_eq!(Runtime::default().render(&out_of_range, &BTreeMap::new(), 0.0, 0.0).unwrap()[0].blend_mode, 0);
    }

    #[test]
    fn saved_blend_vertex_bytes_and_fixed_function_factors_match_the_table() {
        let rgba = [1.0, 1.0, 1.0, 0.5];
        assert_eq!(sprite_vertex_color(1, rgba), [127, 127, 127, 255]);
        assert_eq!(sprite_vertex_color(3, rgba), [255; 4]);
        assert_eq!(sprite_vertex_color(7, rgba), [255, 255, 255, 127]);
        assert_eq!(sprite_vertex_color(8, rgba), [0, 0, 0, 127]);

        let rgba = [1.0; 4];
        let texel = [64, 128, 192, 128];
        let dst = [32, 96, 160];
        let results = (0..=10).map(|mode| sprite_blend_pixel(mode, rgba, texel, dst)).collect::<Vec<_>>();
        assert_eq!(results[0], [64.0, 128.0, 192.0]);
        assert_eq!(results[1], [64.0, 128.0, 192.0]);
        assert_eq!(results[2], [96.0, 224.0, 255.0]);
        assert!(results[3].iter().zip([40.03, 112.06, 184.09]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[4].iter().zip([48.06, 112.06, 176.06]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[5].iter().zip([8.03, 48.19, 120.47]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[6].iter().zip([23.97, 47.81, 39.53]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[7].iter().zip([111.81, 111.56, 111.31]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[8].iter().zip([15.94, 47.81, 79.69]).all(|(a,b)| (a-b).abs()<0.1));
        assert!(results[9].iter().zip([23.97, 96.0, 160.0]).all(|(a,b)| (a-b).abs()<0.1));
        assert_eq!(results[10], [160.0, 255.0, 255.0]);
        assert_eq!(sprite_blend_mode(-1), 0);
        assert_eq!(sprite_blend_mode(11), 0);
    }

    #[test]
    fn blocks_do_not_leak_into_preset_equations_and_last_index_wins() {
        let source = format!("name=Host\n{}{}zoom=1\n", block("init_1=x=0;"), block("init_1=x=1;"));
        let blocks = extract(&source, true).unwrap();
        assert_eq!(blocks.body, "name=Host\nzoom=1\n");
        assert_eq!(blocks.definitions.len(), 1);
        assert_eq!(blocks.definitions[0].init, "x=1;");
        assert!(blocks.source.contains("init_1=x=0;"));
        let parsed = crate::parse_milkrust_preset_set(&source, false);
        assert!(parsed.presets[0].equations.init.is_empty());
        assert_eq!(parsed.embedded_sprites, blocks.definitions);
        let exported = crate::serialize_milkrust_preset_set(&parsed);
        assert_eq!(crate::parse_milkrust_preset_set(&exported, false).embedded_sprites, blocks.definitions);
    }

    #[test]
    fn bounds_and_paths_are_checked_before_allocating_or_loading() {
        for records in ["SpriteBlend=bad", "SpriteX=NaN", "SpriteLayer=bad", "code_999999=x=1;",
            "SpriteName=../escape.png", "SpriteName=C:\\escape.png", "SpriteName=https://host/image.png", "unknown=1"] {
            assert!(extract(&block(records), true).is_err(), "{records}");
        }
        assert!(extract(&block("").replace("[SPRITE1_END]", "[SPRITE2_END]"), true).is_err());
        assert!(extract(&block("[SPRITE1_BEGIN]"), true).is_err());
        let many = (1..=MAX_SPRITES + 1).map(|i| block("").replace("SPRITE1_", &format!("SPRITE{i}_"))).collect::<String>();
        assert!(extract(&many, true).is_err());
    }

    #[test]
    fn disabled_blocks_are_preserved_without_code_or_asset_admission() {
        let source = format!("sprite=0\nname=Disabled\n{}", block("SpriteName=../not-opened.png\ncode_1=unknown_function();"));
        let blocks = extract(&source, enabled(&source)).unwrap();
        assert!(blocks.definitions.is_empty());
        assert!(blocks.source.contains("unknown_function"));
        assert!(crate::validate_milkrust_webgl_import(&source).is_ok());
        assert!(crate::validate_milkrust_webgl_import(&source.replace("sprite=0", "sprite=1")).is_err());
    }

    #[test]
    fn numbered_code_has_persistent_private_state_and_one_final_burn_frame() {
        let blocks = extract(&block("SpriteBurn=1\ninit_1=counter=0;\ncode_10=done=above(counter,1);\ncode_2=counter=counter+1;x=counter/4;"), true).unwrap();
        let mut runtime = Runtime::default();
        let first = runtime.render(&blocks.definitions, &BTreeMap::new(), 100.0, 0.3).unwrap();
        assert_eq!(first[0].center[0], 0.25);
        let second = runtime.render(&blocks.definitions, &BTreeMap::new(), 100.5, 0.3).unwrap();
        assert_eq!(second[0].center[0], 0.5);
        assert!(second[0].burn);
        assert!(runtime.render(&blocks.definitions, &BTreeMap::new(), 101.0, 0.3).unwrap().is_empty());
    }

    #[test]
    fn saved_offsets_and_negative_scales_survive_script_assignments() {
        let blocks = extract(&block("SpriteX=0.4\nSpriteSX=-1.5\ncode_1=x=0.5;sx=1;"), true).unwrap();
        let frames = Runtime::default().render(&blocks.definitions, &BTreeMap::new(), 10.0, 0.5).unwrap();
        assert_eq!(frames[0].center[0], 0.7);
        assert_eq!(frames[0].scale[0], -0.5);
        let (positions, uvs) = frames[0].geometry((200, 100), (100, 100));
        assert!(positions[0] > positions[2]);
        assert_eq!(uvs, vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn all_saved_modes_are_explicit_and_runtime_errors_propagate() {
        for mode in 0..=10 {
            let blocks = extract(&block(&format!("SpriteBlend={mode}\nSpriteLayer=4")), true).unwrap();
            let frames = Runtime::default().render(&blocks.definitions, &BTreeMap::new(), 0.0, 0.5).unwrap();
            assert_eq!(frames[0].blend_mode, mode);
            assert_eq!(frames[0].layer, 4);
        }
        let source = format!("name=Bad\n{}", block("code_1=unknown_sprite_function();"));
        let mut runtime = crate::MilkRustFrameSetRuntime::default();
        assert!(runtime.render_source(&source, 0.0, 0.0, 0.0, 0.0).entries.is_empty());
        assert!(runtime.render_error().is_some());
        let valid = format!("name=Good\n{}", block(""));
        assert_eq!(runtime.render_source(&valid, 1.0, 0.0, 0.0, 0.0).sprites.len(), 1);
        assert!(runtime.render_error().is_none());
    }
}
