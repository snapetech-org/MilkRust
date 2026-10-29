//! Saved MilkDrop sprite blocks and their independent EEL lifetime.
//! No image is opened here: hosts resolve admitted, pack-local dependencies.
use std::collections::BTreeMap;
use crate::{MilkRustValue, evaluate_milkrust_equations, milkrust_scope_number};

/// MilkDrop's preset texture manager exposes 16 sprite slots. Its separate
/// external sprite image cache can hold 100 assets; those are not preset slots.
pub const MAX_SPRITES: usize = 16;
const MAX_SPRITE_RECORDS: usize = 64;
const DEFAULT_COLOR_KEY_ARGB: u32 = 0xff00_0000;

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub index: u32,
    pub texture: String,
    pub color_key: u32,
    pub offset: [f64; 2],
    pub size_offset: [f64; 2],
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
    let mut current: Option<u32> = None;
    let mut records = BTreeMap::new();
    let mut blocks = 0usize;
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    for raw in normalized.lines() {
        let line = raw.trim();
        if let Some(index) = marker(line, "_BEGIN]") {
            if current.is_some() { return Err("Nested embedded sprite block".into()); }
            blocks += 1;
            if blocks > MAX_SPRITE_RECORDS { return Err("More than 64 saved sprite records".into()); }
            current = Some(index);
            records.clear();
        } else if let Some(index) = marker(line, "_END]") {
            if current != Some(index) { return Err("Mismatched embedded sprite block end".into()); }
            if enabled { definitions.insert(index, definition(index, &records)?); }
            result.source.push_str(raw);
            result.source.push('\n');
            current = None;
            continue;
        } else if line.to_ascii_uppercase().starts_with("[SPRITE") {
            return Err("Invalid embedded sprite marker".into());
        } else if current.is_some() && enabled && !line.is_empty() && !line.starts_with([';', '#']) && !line.starts_with("//") {
            let (key, value) = line.split_once('=').ok_or("Invalid embedded sprite record")?;
            records.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
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
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') { break; }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("sprite") { return value.trim() != "0"; }
        }
    }
    true
}

fn definition(index: u32, records: &BTreeMap<String, String>) -> Result<Definition, String> {
    if index >= MAX_SPRITES as u32 { return Err("Embedded sprite index exceeds MilkDrop's 16 preset slots".into()); }
    let texture = records.get("spritename").or_else(|| records.get("img"))
        .ok_or("Embedded sprite has no image name")?.trim_matches(['\'', '"']).replace('\\', "/");
    if texture.is_empty() || texture.len() > 512 || texture.starts_with('/') || texture.contains(':')
        || texture.chars().any(char::is_control) || texture.split('/').any(|v| v == ".." || v.is_empty()) {
        return Err("Embedded sprite image must be a relative pack asset".into());
    }
    let color_key = match records.get("spritecolorkey").or_else(|| records.get("colorkey")) {
        None => DEFAULT_COLOR_KEY_ARGB,
        Some(value) => {
            let hex = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")).unwrap_or(value);
            let parsed = u32::from_str_radix(hex, 16).ok().ok_or("Invalid sprite color key")?;
            if hex.len() > 6 { return Err("Invalid sprite color key".into()); }
            // MilkDrop passes 0xff000000 | ck to D3DX, forcing an opaque RGB
            // color key regardless of the source image's alpha.
            0xff00_0000 | parsed
        }
    };
    let mut values = BTreeMap::new();
    for (key, fallback) in [("x", 0.5), ("y", 0.5), ("sx", 1.0), ("sy", 1.0), ("rot", 0.0),
        ("speed", 0.0), ("repeatx", 1.0), ("repeaty", 1.0), ("r", 1.0), ("g", 1.0), ("b", 1.0),
        ("a", 1.0), ("burn", 1.0), ("blendmode", 0.0), ("layer", 0.0), ("flipx", 0.0), ("flipy", 0.0), ("done", 0.0)] {
        values.insert(key.into(), MilkRustValue::Number(fallback));
    }
    let mut offset = [0.0; 2];
    let mut size_offset = [0.0; 2];
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
        let variable = match key.as_str() {
            "spritealpha" => "a", "spriteblend" => "blendmode",
            key => key.strip_prefix("sprite").unwrap_or(key),
        };
        if !values.contains_key(variable) { return Err(format!("Unsupported embedded sprite field: {key}")); }
        let number = value.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9).ok_or("Non-finite or oversized sprite value")?;
        // A saved SpriteRot=1 is the UI's neutral default, not one radian.
        // Explicit script `rot` still uses radians and overrides this seed.
        let number = if key == "spriterot" && number == 1.0 { 0.0 } else { number };
        values.insert(variable.into(), MilkRustValue::Number(number));
    }
    let join = |lines: BTreeMap<u32, &str>| lines.into_values().collect::<Vec<_>>().join("\n");
    let result = Definition { index, texture, color_key, offset, size_offset, values, init: join(init), code: join(code) };
    validate_mode(&result.values)?;
    crate::validate_expression_source(&result.init)?;
    crate::validate_expression_source(&result.code)?;
    Ok(result)
}

fn validate_mode(scope: &BTreeMap<String, MilkRustValue>) -> Result<(), String> {
    for (key, maximum) in [("blendmode", 10.0), ("layer", 4.0)] {
        let value = scope.get(key).and_then(MilkRustValue::as_number).unwrap_or(0.0);
        if !value.is_finite() || value != value.floor() || !(0.0..=maximum).contains(&value) {
            return Err(format!("Unsupported sprite {key}: {value}"));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub texture: String,
    pub color_key: u32,
    pub blend_mode: i32,
    /// Saved layer 0 stamps feedback after composite; layers 1..4 before it.
    /// Non-burning sprites are screen overlays, ordered by this layer.
    pub layer: usize,
    pub burn: bool,
    pub color: [f64; 4],
    pub center: [f64; 2],
    pub scale: [f64; 2],
    pub repeat: [f64; 2],
    pub rotation: f64,
}

impl Frame {
    /// The saved key is ARGB; GL uniforms use RGBA order.
    pub fn color_key_rgba(&self) -> [f32; 4] {
        [((self.color_key >> 16) & 255) as f32 / 255.0,
         ((self.color_key >> 8) & 255) as f32 / 255.0,
         (self.color_key & 255) as f32 / 255.0,
         ((self.color_key >> 24) & 255) as f32 / 255.0]
    }

    /// Image aspect is applied before rotation; screen aspect after rotation.
    /// UVs are bottom-up for the common GL texture upload convention.
    pub fn geometry(&self, image: (u32, u32), screen: (u32, u32)) -> (Vec<f64>, Vec<f64>) {
        let image_aspect = image.0.max(1) as f64 / image.1.max(1) as f64;
        let screen_aspect = screen.0.max(1) as f64 / screen.1.max(1) as f64;
        let (sin, cos) = self.rotation.sin_cos();
        let mut positions = Vec::with_capacity(8);
        let mut uvs = Vec::with_capacity(8);
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let local_x = x * self.scale[0] * image_aspect.min(1.0);
            let local_y = y * self.scale[1] / image_aspect.max(1.0);
            positions.push((local_x * cos - local_y * sin + self.center[0] * 2.0 - 1.0) / screen_aspect.min(1.0));
            positions.push((local_x * sin + local_y * cos + 1.0 - self.center[1] * 2.0) * screen_aspect.max(1.0));
            uvs.extend_from_slice(&[0.5 + x * 0.5 * self.repeat[0], 0.5 + y * 0.5 * self.repeat[1]]);
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
        if !time.is_finite() { return Err("Non-finite sprite time".into()); }
        if definitions.len() > MAX_SPRITES { return Err("Sprite instance limit exceeded".into()); }
        self.instances.resize_with(definitions.len(), Instance::default);
        let start = *self.start.get_or_insert(time);
        let elapsed = (time - start).max(0.0);
        let delta = self.previous.map_or(0.0, |last| (time - last).clamp(0.0, 1.0));
        self.previous = Some(time);
        let fps = if delta > 0.0 { 1.0 / delta } else { 60.0 };
        let mut frames = Vec::new();
        for (definition, instance) in definitions.iter().zip(&mut self.instances) {
            if instance.dead { continue; }
            if !instance.initialized { instance.scope = definition.values.clone(); }
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
            for key in ["x", "y", "sx", "sy", "rot", "speed", "repeatx", "repeaty", "r", "g", "b", "a", "burn", "done", "flipx", "flipy", "blendmode", "layer"] {
                if !value(key).is_finite() { return Err(format!("Non-finite sprite variable: {key}")); }
            }
            // Preserve the script's accumulated angle. Wrapping the persistent
            // register breaks conditions such as done=above(rot,2*$pi).
            let rotation = value("rot") + value("speed").clamp(-1000.0, 1000.0) / fps;
            frames.push(Frame {
                texture: definition.texture.clone(), color_key: definition.color_key,
                blend_mode: value("blendmode").clamp(0.0, 10.0) as i32, layer: (value("layer") + 0.5).floor().clamp(0.0, 4.0) as usize, burn: value("burn") != 0.0,
                color: [value("r").clamp(0.0, 1.0), value("g").clamp(0.0, 1.0), value("b").clamp(0.0, 1.0), value("a").clamp(0.0, 1.0)],
                center: [value("x").clamp(-500.0, 500.0) + definition.offset[0] * 0.5,
                    value("y").clamp(-500.0, 500.0) + definition.offset[1] * 0.5],
                scale: [(value("sx") + definition.size_offset[0]).clamp(-1000.0, 1000.0) * if value("flipx") == 0.0 { 1.0 } else { -1.0 },
                    (value("sy") + definition.size_offset[1]).clamp(-1000.0, 1000.0) * if value("flipy") == 0.0 { 1.0 } else { -1.0 }],
                repeat: [value("repeatx").clamp(0.01, 100.0), value("repeaty").clamp(0.01, 100.0)],
                rotation,
            });
            // The final frame may burn before the instance is retired.
            instance.dead = value("done") != 0.0;
            instance.scope.insert("rot".into(), MilkRustValue::Number(rotation));
        }
        frames.sort_by_key(|frame| frame.layer);
        self.frame = self.frame.saturating_add(1);
        Ok(frames)
    }
}

/// Common native/WebGL sprite compositing; destination is a distinct snapshot,
/// never the texture attached to the draw framebuffer. Modes 5..10 implement
/// the named arithmetic operators; visual parity is tracked separately.
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
    vec3 raw = sampleColor.rgb;
    // The five classic modes have distinct alpha sources. In modes 0..3
    // texture alpha is ignored, including non-keyed transparent texels.
    if (spriteBlend >= 5 && sampleColor.a <= 0.0) discard;
    // Legacy vertex colors are packed to bytes by truncation. Modes using
    // opacity in RGB multiply BEFORE packing (not after separate quantization).
    vec4 tint = floor(clamp(spriteTint, 0.0, 1.0) * 255.0) / 255.0;
    vec3 premultiplied = raw * floor(clamp(spriteTint.rgb * spriteTint.a, 0.0, 1.0) * 255.0) / 255.0;
    vec3 src = raw * tint.rgb;
    vec3 dst = background.rgb;
    float alpha = sampleColor.a * tint.a;
    vec3 result;
    if (spriteBlend == 1) result = premultiplied;
    else if (spriteBlend == 2) result = dst + premultiplied;
    else if (spriteBlend == 3) result = raw * raw + dst * (vec3(1.0) - raw);
    else if (spriteBlend == 4) {
        result = mix(dst, src, alpha);
    }
    else if (spriteBlend == 5) result = dst * src;
    else if (spriteBlend == 6) result = dst * (vec3(1.0) - premultiplied);
    else if (spriteBlend == 7) result = vec3(1.0) - src;
    else if (spriteBlend == 8) result = vec3(0.0);
    else if (spriteBlend == 9) result = dst * min(vec3(1.0), vec3(0.5) + src);
    else if (spriteBlend == 10) result = dst + 2.0 * premultiplied;
    else result = mix(dst, src, tint.a);
    outColor = vec4(clamp(result, 0.0, 1.0), background.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    fn block(records: &str) -> String {
        format!("[SPRITE2_BEGIN]\nSpriteName=sprites/burst.png\n{records}\n[SPRITE2_END]\n")
    }

    #[test]
    fn neutral_saved_rotation_and_speed_do_not_accumulate_elapsed_twice() {
        let definitions=extract(&block("SpriteRot=1\nSpriteSpeed=1"),true).unwrap().definitions;
        let mut runtime=Runtime::default();
        let first=runtime.render(&definitions,&BTreeMap::new(),0.0,0.0).unwrap();
        let second=runtime.render(&definitions,&BTreeMap::new(),0.5,0.0).unwrap();
        assert!((first[0].rotation-1.0/60.0).abs()<1.0e-9);
        assert!((second[0].rotation-1.0/60.0-0.5).abs()<1.0e-9);
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
    fn scripted_mode_and_layer_use_legacy_integer_conversion_and_clamping() {
        for (code, mode, layer) in [("blendmode=9.8;layer=2.7;", 9, 3),
            ("blendmode=-2;layer=-1;", 0, 0), ("blendmode=99;layer=99;", 10, 4)] {
            let definitions = extract(&block(&format!("code_1={code}")), true).unwrap().definitions;
            let frames = Runtime::default().render(&definitions, &BTreeMap::new(), 0.0, 0.0).unwrap();
            assert_eq!(frames[0].blend_mode, mode);
            assert_eq!(frames[0].layer, layer);
        }
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
        for records in ["SpriteBlend=11", "SpriteX=NaN", "SpriteLayer=5", "code_999999=x=1;",
            "SpriteName=../escape.png", "SpriteName=C:\\escape.png", "SpriteName=https://host/image.png", "unknown=1"] {
            assert!(extract(&block(records), true).is_err(), "{records}");
        }
        assert!(extract(&block("").replace("[SPRITE2_END]", "[SPRITE3_END]"), true).is_err());
        assert!(extract(&block("[SPRITE1_BEGIN]"), true).is_err());
        let many = (0..=MAX_SPRITES).map(|i| block("").replace("SPRITE2_", &format!("SPRITE{i}_"))).collect::<String>();
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
