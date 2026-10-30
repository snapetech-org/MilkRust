//! Native GL/GLES execution of MilkRust's frame graph. Feedback and final
//! composite are distinct passes; each double-preset layer owns two targets.
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use milkrust_core::{milk2, get_milkrust_texture_name_aliases, MilkRustCompositeFrame, MilkRustFrame, MilkRustFrameSet,
    MilkRustPrimitive, MilkRustPrimitiveMode, MilkRustDrawCommand, MilkRustShaderStage, MilkRustTexturedPrimitive, MilkRustTexturedPrimitiveMode};
use crate::gl::*;
use milkrust_core::resources::{self, TextureKind};

fn renderer_noise_seed() -> u32 {
    static SERIAL: AtomicU32 = AtomicU32::new(0);
    let ticks = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let mut seed = ticks as u32
        ^ (ticks >> 32) as u32
        ^ std::process::id()
        ^ SERIAL.fetch_add(1, Ordering::Relaxed);
    seed ^= seed >> 16;
    seed = seed.wrapping_mul(0x7feb_352d);
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x846c_a68b);
    (seed ^ (seed >> 16)).max(1)
}

const VERTEX: &str = "#version 300 es\nin vec2 position; in vec2 sourceUv; in vec4 vertexColor; out vec2 uv; out highp vec2 originalUv; out vec4 tint; void main(){ originalUv=position*0.5+0.5; uv=sourceUv; tint=vertexColor; gl_Position=vec4(position,0,1); gl_PointSize=1.0; }";
const COPY_BODY: &str = "uniform sampler2D previousFrame; uniform float feedback; uniform float outputAlpha; uniform float classicDisplay; in vec2 uv; out vec4 outColor; void main(){vec2 sampleUv=classicWrap>0.5?fract(uv):clamp(uv,0.0,1.0); vec3 color=classicDisplay>0.5?milkrust_classic_display(previousFrame,uv):texture(previousFrame,sampleUv).rgb*feedback;outColor=vec4(color,outputAlpha*milkrust_transition_mask(uv));}";
const COLOR: &str = "#version 300 es\nprecision highp float; in vec4 tint; out vec4 outColor; void main(){outColor=tint;}";
const TEXTURED: &str = "#version 300 es\nprecision highp float; uniform sampler2D previousFrame; in vec2 uv; in vec4 tint; out vec4 outColor; void main(){outColor=texture(previousFrame,uv)*tint;}";
const SPRITE_KEY: &str = "#version 300 es\nprecision highp float; uniform sampler2D spriteSource; uniform vec3 spriteColorKey; out vec4 outColor; void main(){ivec2 p=ivec2(gl_FragCoord.xy);vec4 c=texelFetch(spriteSource,p,0);if(c.a>=1.0-0.5/255.0&&all(lessThan(abs(c.rgb-spriteColorKey),vec3(0.5/255.0))))c=vec4(0.0);outColor=c;}";
const MAX_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_TEXTURE_PIXELS: u64 = 32 * 1024 * 1024;
const MAX_VERTICES: usize = 262_144;

#[derive(Clone, Copy)]
struct TransitionUniforms { pattern: i32, progress: f32, direction: f32, random: [f32; 5] }
impl TransitionUniforms {
    fn from_entry(entry: &MilkRustCompositeFrame) -> Self {
        Self { pattern: milk2::blend_pattern_code(&entry.blend_pattern).unwrap_or(0),
            progress: entry.blend_alpha.clamp(0.0, 1.0) as f32,
            direction: entry.blend_direction as f32,
            random: entry.blend_random.map(|value| value as f32) }
    }
    fn fade() -> Self { Self { pattern: 0, progress: 1.0, direction: 1.0, random: [0.0; 5] } }
    fn output_alpha(self, first: bool) -> f32 {
        if first || self.pattern != 0 { 1.0 } else { self.progress }
    }
}

struct Program { gl: Gl, id: u32, locations: std::cell::RefCell<BTreeMap<String, i32>> }
impl Drop for Program { fn drop(&mut self) { unsafe { (self.gl.delete_program)(self.id); } } }
struct Shader { gl: Gl, id: u32 }
impl Drop for Shader { fn drop(&mut self) { unsafe { (self.gl.delete_shader)(self.id); } } }
struct Texture { gl: Gl, id: u32, pixels: u64, width: u32, height: u32 }
struct TextureAsset { frames: Vec<Texture>, delays_seconds: Vec<f64>, repeat_count: Option<u32>, pixels: u64, animated: bool }
impl TextureAsset {
    fn frame_at(&self, seconds: f64) -> &Texture {
        let index = milkrust_core::texture::animation_frame_index(&self.delays_seconds, self.repeat_count, seconds);
        &self.frames[index.min(self.frames.len() - 1)]
    }
}
type TransitionMask = (milk2::TransitionMaskKey, Texture);
struct Samplers { gl: Gl, ids: [u32; resources::SAMPLER_OBJECT_COUNT] }
impl Drop for Samplers { fn drop(&mut self) { unsafe { (self.gl.delete_samplers)(resources::SAMPLER_OBJECT_COUNT as i32, self.ids.as_ptr()); } } }
impl Drop for Texture { fn drop(&mut self) { unsafe { (self.gl.delete_textures)(1, &self.id); } } }
struct Target { gl: Gl, id: u32, texture: Texture }
impl Drop for Target { fn drop(&mut self) { unsafe { (self.gl.delete_framebuffers)(1, &self.id); } } }
struct Layer {
    targets: [Target; 2], read: usize, blur: Vec<[Target; 2]>,
    warp: Option<(String, Program)>, composite: Option<(String, Program)>,
}

fn dimensions(width: u32, height: u32, maximum: i32) -> Result<(), String> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 || width > maximum as u32
        || height > maximum as u32 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("Render/texture dimensions exceed the 4096-side or 8 Mi-pixel limit".into());
    }
    Ok(())
}

fn source_for_context(source: &str, gles: bool) -> String {
    if gles { source.to_owned() } else {
        source.replace("#version 300 es", "#version 150")
            .replace("precision highp float;", "").replace("precision mediump float;", "")
            .replace("precision highp int;", "").replace("precision highp sampler3D;", "")
    }
}

impl Program {
    fn new(gl: Gl, gles: bool, fragment: &str) -> Result<Self, String> {
        fn shader(gl: Gl, kind: u32, source: String) -> Result<Shader, String> {
            if source.len() > 256 * 1024 { return Err("Shader source exceeds 256 KiB".into()); }
            let text = CString::new(source).map_err(|_| "Shader contains a NUL byte")?;
            let id = unsafe { (gl.create_shader)(kind) };
            if id == 0 { return Err("GL could not allocate a shader".into()); }
            let shader = Shader { gl, id };
            unsafe {
                (gl.shader_source)(id, 1, &text.as_ptr(), ptr::null()); (gl.compile_shader)(id);
                let mut success = 0; (gl.get_shader_iv)(id, 0x8B81, &mut success);
                if success == 0 {
                    let mut log = [0u8; 4096]; let mut length = 0;
                    (gl.get_shader_log)(id, log.len() as i32, &mut length, log.as_mut_ptr().cast());
                    return Err(format!("GL shader compilation failed: {}", String::from_utf8_lossy(&log[..(length.max(0) as usize).min(log.len())])));
                }
            }
            Ok(shader)
        }
        let vertex = shader(gl, 0x8B31, source_for_context(VERTEX, gles))?;
        let fragment = shader(gl, 0x8B30, source_for_context(fragment, gles))?;
        let id = unsafe { (gl.create_program)() };
        if id == 0 { return Err("GL could not allocate a program".into()); }
        let program = Self { gl, id, locations: std::cell::RefCell::new(BTreeMap::new()) };
        unsafe {
            (gl.attach_shader)(id, vertex.id); (gl.attach_shader)(id, fragment.id);
            for (index, name) in ["position", "sourceUv", "vertexColor"].iter().enumerate() {
                let name = CString::new(*name).map_err(|_| "Invalid attribute")?;
                (gl.bind_attrib)(id, index as u32, name.as_ptr());
            }
            (gl.link_program)(id);
            let mut success = 0; (gl.get_program_iv)(id, 0x8B82, &mut success);
            if success == 0 {
                let mut log = [0u8; 4096]; let mut length = 0;
                (gl.get_program_log)(id, log.len() as i32, &mut length, log.as_mut_ptr().cast());
                return Err(format!("GL program link failed: {}", String::from_utf8_lossy(&log[..(length.max(0) as usize).min(log.len())])));
            }
        }
        Ok(program)
    }
    fn location(&self, name: &str) -> i32 {
        if let Some(location) = self.locations.borrow().get(name) { return *location; }
        let location = CString::new(name).ok().map_or(-1, |name| unsafe { (self.gl.uniform_location)(self.id, name.as_ptr()) });
        self.locations.borrow_mut().insert(name.to_string(), location);
        location
    }
    fn bind(&self) { unsafe { (self.gl.use_program)(self.id); } }
    fn float(&self, name: &str, value: f32) { unsafe { (self.gl.uniform_1f)(self.location(name), value); } }
    fn integer(&self, name: &str, value: i32) { unsafe { (self.gl.uniform_1i)(self.location(name), value); } }
    fn vector2(&self, name: &str, x: f32, y: f32) { unsafe { (self.gl.uniform_2f)(self.location(name), x, y); } }
    fn vector3(&self, name: &str, x: f32, y: f32, z: f32) { unsafe { (self.gl.uniform_3f)(self.location(name), x, y, z); } }
    fn vector4(&self, name: &str, x: f32, y: f32, z: f32, w: f32) { unsafe { (self.gl.uniform_4f)(self.location(name), x, y, z, w); } }
    fn floats(&self, name: &str, values: &[f32]) { unsafe { (self.gl.uniform_1fv)(self.location(name), values.len() as i32, values.as_ptr()); } }
    fn matrix3x4(&self, name: &str, values: &[f32; 12]) { unsafe { (self.gl.uniform_matrix3x4fv)(self.location(name), 1, 0, values.as_ptr()); } }
}

impl Texture {
    fn new(gl: Gl, width: u32, height: u32, data: Option<&[u8]>) -> Result<Self, String> {
        let mut id = 0;
        unsafe { (gl.gen_textures)(1, &mut id); }
        if id == 0 { return Err("GL could not allocate a texture".into()); }
        let texture = Self { gl, id, pixels: u64::from(width) * u64::from(height), width, height };
        unsafe {
            (gl.active_texture)(TEXTURE0); (gl.bind_texture)(TEXTURE_2D, id);
            (gl.pixel_store)(0x0CF5, 1); // UNPACK_ALIGNMENT
            (gl.tex_parameter)(TEXTURE_2D, 0x2801, 0x2601); // MIN_FILTER LINEAR
            (gl.tex_parameter)(TEXTURE_2D, 0x2800, 0x2601);
            (gl.tex_parameter)(TEXTURE_2D, 0x2802, 0x812F); // CLAMP_TO_EDGE
            (gl.tex_parameter)(TEXTURE_2D, 0x2803, 0x812F);
            (gl.tex_image)(TEXTURE_2D, 0, 0x8058, width as i32, height as i32, 0, 0x1908, 0x1401,
                data.map_or(ptr::null(), |bytes| bytes.as_ptr().cast()));
            if (gl.get_error)() != 0 { return Err("GL texture allocation/upload failed".into()); }
        }
        Ok(texture)
    }

    fn new_transition_mask(gl: Gl, data: &[u8]) -> Result<Self, String> {
        let (width, height) = (milk2::TRANSITION_GRID_WIDTH, milk2::TRANSITION_GRID_HEIGHT);
        if data.len() as u64 != u64::from(width) * u64::from(height) {
            return Err("Invalid transition alpha field length".into());
        }
        let mut id = 0;
        unsafe { (gl.gen_textures)(1, &mut id); }
        if id == 0 { return Err("GL could not allocate a transition mask".into()); }
        let texture = Self { gl, id, pixels: u64::from(width) * u64::from(height), width, height };
        unsafe {
            (gl.active_texture)(TEXTURE0); (gl.bind_texture)(TEXTURE_2D, id);
            (gl.pixel_store)(0x0CF5, 1);
            (gl.tex_parameter)(TEXTURE_2D, 0x2801, 0x2600); // NEAREST
            (gl.tex_parameter)(TEXTURE_2D, 0x2800, 0x2600);
            (gl.tex_parameter)(TEXTURE_2D, 0x2802, 0x812F);
            (gl.tex_parameter)(TEXTURE_2D, 0x2803, 0x812F);
            (gl.tex_image)(TEXTURE_2D, 0, 0x8229, width as i32, height as i32, 0, 0x1903, 0x1401, data.as_ptr().cast());
            if (gl.get_error)() != 0 { return Err("GL transition vertex field allocation/upload failed".into()); }
        }
        Ok(texture)
    }
}
impl Target {
    fn new(gl: Gl, width: u32, height: u32) -> Result<Self, String> {
        let texture = Texture::new(gl, width, height, None)?;
        let mut id = 0; unsafe { (gl.gen_framebuffers)(1, &mut id); }
        if id == 0 { return Err("GL could not allocate a framebuffer".into()); }
        let target = Self { gl, id, texture };
        unsafe {
            (gl.bind_framebuffer)(FRAMEBUFFER, id);
            (gl.framebuffer_texture)(FRAMEBUFFER, 0x8CE0, TEXTURE_2D, target.texture.id, 0);
            if (gl.check_framebuffer)(FRAMEBUFFER) != 0x8CD5 { return Err("GL framebuffer is incomplete".into()); }
            (gl.clear_color)(0.0, 0.0, 0.0, 1.0); (gl.clear)(COLOR_BUFFER);
        }
        Ok(target)
    }
}

pub struct Renderer {
    sprite_program: Program, sprite_key_program: Program, sprite_keyed: BTreeMap<(String, u32), Target>, sprite_snapshot: Option<Texture>,
    gl: Gl, gles: bool, copy: Program, color: Program, textured: Program, blur_program: Program, samplers: Samplers, noise: BTreeMap<String, (Texture, u32)>, noise_runtime_seed: u32,
    buffer: u32, vao: u32, max_texture: i32, texture_units: usize,
    width: u32, height: u32, capture: Option<(u32, u32, Target)>, layers: Vec<Layer>, textures: BTreeMap<String, TextureAsset>,
    transition_mask: Option<TransitionMask>,
}
impl Drop for Renderer {
    fn drop(&mut self) { unsafe { (self.gl.delete_buffers)(1, &self.buffer); (self.gl.delete_vertex_arrays)(1, &self.vao); } }
}

impl Renderer {
    /// # Safety
    /// gl must refer to the current dedicated context for this entire lifetime.
    pub unsafe fn new(gl: Gl, gles: bool) -> Result<Self, String> {
        let copy_source = format!("#version 300 es\nprecision highp float;\n{}\n{}\n{}",
            milk2::GLSL_TRANSITION_MASK, milkrust_core::classic::GLSL, COPY_BODY);
        let copy = Program::new(gl, gles, &copy_source)?;
        let color = Program::new(gl, gles, COLOR)?;
        let textured = Program::new(gl, gles, TEXTURED)?;
        let sprite_program = Program::new(gl, gles, milkrust_core::sprites::FRAGMENT)?;
        let sprite_key_program = Program::new(gl, gles, SPRITE_KEY)?;
        let blur_program = Program::new(gl, gles, resources::BLUR_FRAGMENT)?;
        let mut samplers = Samplers { gl, ids: [0; resources::SAMPLER_OBJECT_COUNT] };
        unsafe {
            (gl.gen_samplers)(resources::SAMPLER_OBJECT_COUNT as i32, samplers.ids.as_mut_ptr());
            for (index, id) in samplers.ids.iter().enumerate().take(resources::SAMPLER_VARIANTS) {
                if *id == 0 { return Err("GL could not allocate samplers".into()); }
                let settings = resources::sampler_parameters(index).ok_or("Invalid generated sampler profile")?;
                let min_filter = match (settings.min_point, settings.mip) {
                    (true, resources::MipFilter::None) => 0x2600,
                    (false, resources::MipFilter::None) => 0x2601,
                    (true, resources::MipFilter::Point) => 0x2700,
                    (false, resources::MipFilter::Point) => 0x2701,
                    (true, resources::MipFilter::Linear) => 0x2702,
                    (false, resources::MipFilter::Linear) => 0x2703,
                };
                (gl.sampler_parameter)(*id, 0x2801, min_filter);
                (gl.sampler_parameter)(*id, 0x2800, if settings.mag_point { 0x2600 } else { 0x2601 });
                for (axis, mode) in [0x2802, 0x2803, 0x8072].iter().copied().zip(settings.wrap.iter().copied()) {
                    let address = match mode {
                        resources::AddressMode::Wrap => 0x2901,
                        resources::AddressMode::Clamp => 0x812F,
                        resources::AddressMode::Mirror => 0x8370,
                    };
                    (gl.sampler_parameter)(*id, axis, address);
                }
            }
            let animated = samplers.ids[resources::ANIMATED_SPRITE_SAMPLER];
            if animated == 0 { return Err("GL could not allocate animated sprite sampler".into()); }
            (gl.sampler_parameter)(animated, 0x2801, 0x2601);
            (gl.sampler_parameter)(animated, 0x2800, 0x2601);
            for axis in [0x2802, 0x2803, 0x8072] { (gl.sampler_parameter)(animated, axis, 0x2901); }
        }
        let (mut buffer, mut vao, mut max_texture, mut units) = (0, 0, 0, 0);
        unsafe {
            (gl.get_integer)(0x0D33, &mut max_texture); (gl.get_integer)(0x8872, &mut units);
            (gl.gen_buffers)(1, &mut buffer); (gl.gen_vertex_arrays)(1, &mut vao);
        }
        let result = Self { sprite_program, sprite_key_program, sprite_keyed: BTreeMap::new(), sprite_snapshot: None, gl, gles, copy, color, textured, blur_program, samplers, noise: BTreeMap::new(), noise_runtime_seed: renderer_noise_seed(), buffer, vao, max_texture, texture_units: units.max(0) as usize,
            width: 0, height: 0, capture: None, layers: Vec::new(), textures: BTreeMap::new(), transition_mask: None };
        if buffer == 0 || vao == 0 || max_texture < milk2::TRANSITION_GRID_WIDTH as i32 || units < 4 {
            return Err("Insufficient OpenGL context resources".into());
        }
        unsafe {
            (gl.disable)(0x0B71); (gl.disable)(0x0B44); (gl.disable)(0x0C11); // depth, cull, scissor
            if !gles { (gl.enable)(0x8642); } // PROGRAM_POINT_SIZE
        }
        Ok(result)
    }

    pub fn reset(&mut self) { self.sprite_snapshot = None; self.sprite_keyed.clear(); self.layers.clear(); self.textures.clear(); self.noise.clear(); self.capture = None; self.transition_mask = None; self.width = 0; self.height = 0; }

    pub fn texture(&mut self, name: &str, width: u32, height: u32, bytes: &[u8]) -> Result<(), String> {
        self.upload_texture(name, width, height, &[bytes], &[0.1], None, false)
    }

    pub fn texture_frames(&mut self, name: &str, width: u32, height: u32,
        frames: &[&[u8]], delays_seconds: &[f64]) -> Result<(), String> {
        self.upload_texture(name, width, height, frames, delays_seconds, Some(0), true)
    }

    pub fn texture_animation(&mut self, name: &str, width: u32, height: u32,
        frames: &[&[u8]], delays_seconds: &[f64], repeat_count: Option<u32>) -> Result<(), String> {
        self.upload_texture(name, width, height, frames, delays_seconds, repeat_count, true)
    }

    fn upload_texture(&mut self, name: &str, width: u32, height: u32,
        frames: &[&[u8]], delays_seconds: &[f64], repeat_count: Option<u32>, animated: bool) -> Result<(), String> {
        dimensions(width, height, self.max_texture)?;
        if name.is_empty() || name.len() > 512 || name.contains('\0') || frames.is_empty()
            || frames.len() > milkrust_core::texture::MAX_ANIMATION_FRAMES || delays_seconds.len() != frames.len()
            || delays_seconds.iter().any(|delay| !delay.is_finite() || *delay <= 0.0) {
            return Err("Invalid texture name, frame count or animation timing".into());
        }
        let frame_pixels = u64::from(width) * u64::from(height);
        let pixels = frame_pixels.checked_mul(frames.len() as u64).ok_or("Texture frame pixel count overflow")?;
        if pixels > milkrust_core::texture::MAX_ANIMATED_TOTAL_PIXELS
            || frames.iter().any(|frame| frame.len() as u64 != frame_pixels * 4) {
            return Err("Texture frames exceed their decoded pixel or RGBA byte budget".into());
        }
        let folded = name.to_ascii_lowercase();
        let existing = self.textures.get(&folded).map_or(0, |texture| texture.pixels);
        if (self.textures.len() >= 100 && existing == 0) || self.textures.values().map(|t| t.pixels).sum::<u64>() - existing
            + pixels > MAX_TEXTURE_PIXELS {
            return Err("Named textures exceed 100 assets or 32 Mi-pixels".into());
        }
        let stride = width as usize * 4;
        let mut gpu_frames = Vec::with_capacity(frames.len());
        for pixels in frames {
            // GL UV origin is bottom-left. Hosts supply ordinary top-down rows.
            let mut flipped = Vec::with_capacity(pixels.len());
            for row in pixels.chunks_exact(stride).rev() { flipped.extend_from_slice(row); }
            gpu_frames.push(Texture::new(self.gl, width, height, Some(&flipped))?);
            if !animated { unsafe { (self.gl.generate_mipmap)(TEXTURE_2D); } }
        }
        // Replaced assets invalidate every keyed mip derivative; prepare()
        // rebuilds only those used by the next active frame set.
        self.sprite_keyed.clear();
        self.textures.insert(folded, TextureAsset { frames: gpu_frames, delays_seconds: delays_seconds.to_vec(), repeat_count, pixels, animated });
        Ok(())
    }

    fn named_texture(&self, name: &str) -> Result<u32, String> {
        self.named_texture_at(name, 0.0)
    }
    fn named_texture_asset(&self, name: &str) -> Result<&Texture, String> {
        let aliases = get_milkrust_texture_name_aliases(name);
        for name in aliases {
            if let Some(texture) = self.textures.get(&name.to_ascii_lowercase()) { return Ok(&texture.frames[0]); }
        }
        Err(format!("Missing texture: {name}"))
    }
    fn named_texture_at(&self, name: &str, seconds: f64) -> Result<u32, String> {
        self.named_texture_asset_at(name, seconds).map(|texture| texture.id)
    }
    fn named_texture_asset_at(&self, name: &str, seconds: f64) -> Result<&Texture, String> {
        let aliases = get_milkrust_texture_name_aliases(name);
        for name in aliases {
            if let Some(texture) = self.textures.get(&name.to_ascii_lowercase()) { return Ok(texture.frame_at(seconds)); }
        }
        Err(format!("Missing texture: {name}"))
    }

    fn named_texture_is_animated(&self, name: &str) -> bool {
        get_milkrust_texture_name_aliases(name).iter().find_map(|alias|
            self.textures.get(&alias.to_ascii_lowercase()).map(|asset| asset.animated)).unwrap_or(false)
    }

    fn prepare_keyed_sprite(&mut self, sprite: &milkrust_core::sprites::Frame) -> Result<(), String> {
        if !sprite.color_key_enabled() || self.named_texture_is_animated(&sprite.texture) { return Ok(()); }
        let cache_key = (sprite.texture.to_ascii_lowercase(), sprite.color_key);
        if self.sprite_keyed.contains_key(&cache_key) { return Ok(()); }
        if self.sprite_keyed.len() >= milkrust_core::sprites::MAX_SPRITES {
            return Err("Sprite color-key derivative limit exceeded".into());
        }
        let (source, width, height) = {
            let texture = self.named_texture_asset(&sprite.texture)?;
            (texture.id, texture.width, texture.height)
        };
        let pixels = u64::from(width) * u64::from(height);
        let retained = self.sprite_keyed.values().map(|target| target.texture.pixels).sum::<u64>();
        let source_pixels = self.textures.values().map(|asset| asset.pixels).sum::<u64>();
        if source_pixels.checked_add(retained).and_then(|total| total.checked_add(pixels))
            .is_none_or(|total| total > MAX_TEXTURE_PIXELS) {
            return Err("Sprite color-keyed textures exceed the 32 Mi-pixel renderer budget".into());
        }
        let target = Target::new(self.gl, width, height)?;
        unsafe {
            (self.gl.bind_framebuffer)(FRAMEBUFFER, target.id);
            (self.gl.viewport)(0, 0, width as i32, height as i32);
            (self.gl.disable)(BLEND);
        }
        self.bind_texture(0, source);
        let key = sprite.color_key_rgba();
        self.sprite_key_program.bind();
        self.sprite_key_program.integer("spriteSource", 0);
        self.sprite_key_program.vector3("spriteColorKey", key[0], key[1], key[2]);
        let draw_result = self.draw(0x0004, &quad());
        if draw_result.is_ok() { self.bind_texture(0, target.texture.id); }
        unsafe {
            if draw_result.is_ok() { (self.gl.generate_mipmap)(TEXTURE_2D); }
            (self.gl.bind_framebuffer)(FRAMEBUFFER, 0);
            (self.gl.viewport)(0, 0, self.width as i32, self.height as i32);
        }
        draw_result?;
        if unsafe { (self.gl.get_error)() } != 0 { return Err("OpenGL failed while preparing a keyed sprite mip chain".into()); }
        self.sprite_keyed.insert(cache_key, target);
        Ok(())
    }

    fn prepare(&mut self, frames: &MilkRustFrameSet, width: u32, height: u32) -> Result<(), String> {
        dimensions(width, height, self.max_texture)?;
        if frames.entries.is_empty() || frames.entries.len() > 2 { return Err("Expected one or two preset layers".into()); }
        let mask_key = frames.entries.get(1).and_then(|entry| {
            let pattern = milk2::blend_pattern_code(&entry.blend_pattern)?;
            (pattern != 0).then(|| milk2::TransitionMaskKey::new(pattern, entry.blend_alpha as f32,
                entry.blend_direction, entry.blend_random.map(|value| value as f32), width, height))
        });
        if let Some(key) = mask_key {
            if self.transition_mask.as_ref().is_none_or(|(cached, _)| *cached != key) {
                let vertices = milk2::generate_transition_vertex_field(key)?;
                self.transition_mask = Some((key, Texture::new_transition_mask(self.gl, &vertices)?));
            }
        } else {
            self.transition_mask = None;
        }
        if frames.sprites.len() > milkrust_core::sprites::MAX_SPRITES { return Err("Too many sprite frames".into()); }
        let active_keyed = frames.sprites.iter().filter(|sprite| sprite.color_key_enabled()
                && !self.named_texture_is_animated(&sprite.texture))
            .map(|sprite| (sprite.texture.to_ascii_lowercase(), sprite.color_key)).collect::<BTreeSet<_>>();
        self.sprite_keyed.retain(|key, _| active_keyed.contains(key));
        for sprite in &frames.sprites {
            self.named_texture(&sprite.texture)?;
            self.prepare_keyed_sprite(sprite)?;
        }
        if !frames.sprites.is_empty() && self.sprite_snapshot.as_ref().is_none_or(|texture| texture.width != width || texture.height != height) {
            self.sprite_snapshot = Some(Texture::new(self.gl, width, height, None)?);
        }
        let mut vertices = frames.sprites.len() * 4;
        for entry in &frames.entries {
            let frame = &entry.frame;
            vertices += frame.warp_mesh.as_ref().map_or(6, |mesh| mesh.positions.len() / 2);
            vertices += frame.motion_vectors.as_ref().map_or(0, |p| p.vertices.len()/2);
            vertices += frame.primitives.iter().map(|p| p.vertices.len()/2).sum::<usize>();
            vertices += frame.textured_primitives.iter().map(|p| p.vertices.len()/2).sum::<usize>();
            if vertices > MAX_VERTICES { return Err("Frame geometry exceeds 262144 vertices".into()); }
            for stage in [&frame.shader_stages.warp, &frame.shader_stages.composite] {
                if stage.required && stage.source.is_empty() { return Err("Required shader could not be translated".into()); }
                if stage.texture_samplers.len() + 2 > self.texture_units { return Err("Shader exceeds available texture units after reserving history and transition-mask units".into()); }
                for name in &stage.texture_samplers {
                    match resources::texture_kind(name) {
                        TextureKind::Asset => { self.named_texture(name)?; }
                        TextureKind::Noise2d(seed) | TextureKind::Noise3d(seed) => {
                            let key = resources::base_name(name).to_string();
                            if !self.noise.contains_key(&key) {
                                let volume = resources::texture_kind(name).is_volume();
                                let (w, h, d, data) = resources::noise_texture_with_runtime_seed(seed, volume, self.noise_runtime_seed);
                                let target = if volume { TEXTURE_3D } else { TEXTURE_2D };
                                let texture = if volume {
                                    let mut id = 0;
                                    unsafe { (self.gl.gen_textures)(1, &mut id); }
                                    if id == 0 { return Err("GL noise allocation failed".into()); }
                                    let texture = Texture { gl: self.gl, id, pixels: u64::from(w*h*d), width: w, height: h };
                                    unsafe {
                                        (self.gl.active_texture)(TEXTURE0); (self.gl.bind_texture)(target, id);
                                        (self.gl.tex_image_3d)(target, 0, 0x8058, w as i32, h as i32, d as i32, 0, 0x1908, 0x1401, data.as_ptr().cast());
                                    }
                                    texture
                                } else { Texture::new(self.gl, w, h, Some(&data))? };
                                // MilkDrop's built-in noise textures have only level zero.
                                unsafe { (self.gl.tex_parameter)(target, 0x813D, 0); } // TEXTURE_MAX_LEVEL
                                self.noise.insert(key, (texture, target));
                            }
                        }
                        _ => {}
                    }
                }
            }
            for primitive in &frame.textured_primitives {
                if !primitive.texture_name.is_empty() { self.named_texture(&primitive.texture_name)?; }
            }
        }
        if width != self.width || height != self.height || self.layers.len() != frames.entries.len() {
            let mut layers = Vec::with_capacity(frames.entries.len());
            for _ in &frames.entries {
                layers.push(Layer { targets: [Target::new(self.gl, width, height)?, Target::new(self.gl, width, height)?],
                    read: 0, blur: Vec::new(), warp: None, composite: None });
            }
            self.layers = layers; self.width = width; self.height = height;
        }
        for (layer, entry) in self.layers.iter_mut().zip(&frames.entries) {
            let blur_count = entry.frame.shader_stages.warp.texture_samplers.iter()
                .chain(&entry.frame.shader_stages.composite.texture_samplers)
                .filter_map(|name| match resources::texture_kind(name) { TextureKind::Blur(i) => Some(i + 1), _ => None })
                .max().unwrap_or(0);
            while layer.blur.len() < blur_count {
                let scale = 1u32 << (layer.blur.len() + 1);
                let (w, h) = ((width / scale).max(1), (height / scale).max(1));
                layer.blur.push([Target::new(self.gl, w, h)?, Target::new(self.gl, w, h)?]);
            }
            for (cache, stage) in [(&mut layer.warp, &entry.frame.shader_stages.warp), (&mut layer.composite, &entry.frame.shader_stages.composite)] {
                if !stage.required { *cache = None; continue; }
                if cache.as_ref().is_some_and(|(source, _)| source == &stage.source) { continue; }
                let program = Program::new(self.gl, self.gles, &stage.source)?;
                *cache = Some((stage.source.clone(), program));
            }
        }
        Ok(())
    }

    pub fn capture(&mut self, frames: &MilkRustFrameSet, width: u32, height: u32, time: f64,
        rgba: &mut [u8]) -> Result<(), String> {
        dimensions(width, height, self.max_texture)?;
        if rgba.len() as u64 != u64::from(width)*u64::from(height)*4 {
            return Err("RGBA output must be exactly width*height*4 bytes".into());
        }
        if self.capture.as_ref().map_or(true, |(w,h,_)| *w != width || *h != height) {
            self.capture = Some((width, height, Target::new(self.gl, width, height)?));
        }
        let target = self.capture.as_ref().ok_or("Missing capture target")?.2.id;
        self.render(frames, target, width, height, time)?;
        unsafe {
            (self.gl.pixel_store)(0x0D05, 1); // PACK_ALIGNMENT
            (self.gl.read_pixels)(0, 0, width as i32, height as i32, 0x1908, 0x1401, rgba.as_mut_ptr().cast());
            if (self.gl.get_error)() != 0 { return Err("GL frame readback failed".into()); }
        }
        // Return top-down tightly packed pixels to all Flutter texture hosts.
        let stride = width as usize*4;
        for y in 0..height as usize/2 {
            let (top, bottom) = rgba.split_at_mut((height as usize-1-y)*stride);
            top[y*stride..(y+1)*stride].swap_with_slice(&mut bottom[..stride]);
        }
        Ok(())
    }

    fn bind_texture(&self, unit: usize, texture: u32) {
        unsafe { (self.gl.active_texture)(TEXTURE0 + unit as u32); (self.gl.bind_sampler)(unit as u32, 0); (self.gl.bind_texture)(TEXTURE_2D, texture); }
    }

    fn uniforms(&self, program: &Program, frame: &MilkRustFrame, stage: &MilkRustShaderStage,
        time: f64, alpha: f32, feedback: f32, transition: TransitionUniforms, layer: &Layer, history: u32) -> Result<(), String> {
        program.bind(); program.integer("previousFrame", 0);
        let (echo, filters, wrap) = milkrust_core::classic::uniforms(frame);
        program.vector4("classicEcho", echo[0], echo[1], echo[2], echo[3]);
        program.vector4("classicFilters", filters[0], filters[1], filters[2], filters[3]);
        program.float("classicWrap", wrap);
        unsafe { (self.gl.bind_sampler)(0, self.samplers.ids[resources::LINEAR_REPEAT_SAMPLER]); }
        program.float("feedback", feedback); program.float("outputAlpha", alpha);
        program.integer("u_transitionPattern", transition.pattern);
        program.float("u_transitionProgress", transition.progress);
        program.float("u_transitionDirection", transition.direction);
        for (index, value) in transition.random.iter().enumerate() {
            program.float(&format!("u_transitionRandom{}", index + 1), *value);
        }
        program.vector2("u_transitionResolution", self.width as f32, self.height as f32);
        program.float("time", time.rem_euclid(10000.0) as f32); program.float("sampleRate", 44_100.0);
        program.vector2("resolution", self.width as f32, self.height as f32);
        program.vector2("pixelSize", 1.0/self.width as f32, 1.0/self.height as f32);
        program.float("aspect", self.width as f32/self.height as f32);
        program.vector4("texsize", self.width as f32, self.height as f32, 1.0/self.width as f32, 1.0/self.height as f32);
        let (r, g, b) = frame.wave_color;
        program.vector3("color", r as f32/255.0, g as f32/255.0, b as f32/255.0);
        for (name, value) in [("bass", frame.bass), ("bass_att", frame.bass), ("mid", frame.mid),
            ("mid_att", frame.mid), ("treb", frame.treble), ("treb_att", frame.treble)] { program.float(name, value as f32); }
        program.floats("fftBins", &frame.fft_bins.map(|v| v as f32));
        program.floats("waveformBins", &frame.waveform_bins.map(|v| v as f32));
        for (index, value) in frame.q_registers.iter().enumerate() { program.float(&format!("q{}", index + 1), *value as f32); }
        for (name, value) in &frame.shader_values { if name != "preset_seed" { program.float(name, *value as f32); } }
        for (level, [low, high]) in resources::blur_ranges(frame).iter().copied().enumerate() {
            program.float(&format!("blur{}_min", level+1), low);
            program.float(&format!("blur{}_max", level+1), high);
        }
        for (name, v) in resources::uniform_vectors(frame, time, self.width, self.height) { program.vector4(name, v[0], v[1], v[2], v[3]); }
        for (name, matrix) in resources::uniform_matrices(frame, time) { program.matrix3x4(name, &matrix); }
        for (index, name) in stage.texture_samplers.iter().enumerate() {
            let (texture, target) = match resources::texture_kind(name) {
                TextureKind::Main => (history, TEXTURE_2D),
                TextureKind::Blur(i) => (layer.blur.get(i).ok_or("Missing blur target")?[1].texture.id, TEXTURE_2D),
                TextureKind::Noise2d(_) | TextureKind::Noise3d(_) => {
                    let (texture, target) = self.noise.get(resources::base_name(name)).ok_or("Missing noise texture")?;
                    (texture.id, *target)
                }
                TextureKind::Asset => (self.named_texture_at(name, time)?, TEXTURE_2D),
            };
            let sampler_index = resources::sampler_index(name);
            let (width, height) = match resources::texture_kind(name) {
                TextureKind::Main => (self.width, self.height),
                TextureKind::Blur(i) => { let texture = &layer.blur[i][1].texture; (texture.width, texture.height) },
                TextureKind::Noise2d(_) | TextureKind::Noise3d(_) => {
                    let texture = &self.noise.get(resources::base_name(name)).ok_or("Missing noise dimensions")?.0;
                    (texture.width, texture.height)
                }
                TextureKind::Asset => { let texture = self.named_texture_asset(name)?; (texture.width, texture.height) },
            };
            program.vector4(&format!("texsize_{}", resources::base_name(name)), width as f32, height as f32, 1.0/width as f32, 1.0/height as f32);
            unsafe {
                (self.gl.active_texture)(TEXTURE0 + (index + 1) as u32);
                (self.gl.bind_texture)(target, texture);
                (self.gl.bind_sampler)((index + 1) as u32, self.samplers.ids[sampler_index]);
            }
            program.integer(&format!("shaderTexture{index}"), (index + 1) as i32);
        }
        let transition_unit = self.texture_units.saturating_sub(1);
        program.integer("u_transitionMask", transition_unit as i32);
        if transition.pattern != 0 {
            let texture = &self.transition_mask.as_ref().ok_or("Missing generated transition mask")?.1;
            unsafe {
                (self.gl.active_texture)(TEXTURE0 + transition_unit as u32);
                (self.gl.bind_sampler)(transition_unit as u32, 0);
                (self.gl.bind_texture)(TEXTURE_2D, texture.id);
            }
        }
        unsafe { (self.gl.active_texture)(TEXTURE0); }
        Ok(())
    }

    fn build_resources(&self, frame: &MilkRustFrame, layer: &Layer, history: u32) -> Result<(), String> {
        self.bind_texture(0, history);
        unsafe { (self.gl.generate_mipmap)(TEXTURE_2D); (self.gl.disable)(BLEND); }
        let mut source = history;
        let mut source_size = (self.width, self.height);
        for (level, targets) in layer.blur.iter().enumerate() {
            let scale = 1u32 << (level + 1);
            let (width, height) = ((self.width / scale).max(1), (self.height / scale).max(1));
            for (pass, target) in targets.iter().enumerate() {
                self.bind_texture(0, source);
                unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, target.id); (self.gl.viewport)(0, 0, width as i32, height as i32); }
                self.blur_program.bind(); self.blur_program.integer("previousFrame", 0);
                let [scale, bias] = resources::blur_transform(frame, level, pass);
                let (weights, offsets, normalization) = resources::blur_kernel(pass);
                let edge = resources::blur_edge(frame, level, pass);
                self.blur_program.vector2("blurTransform", scale, bias);
                self.blur_program.vector2("sourceTexel", 1.0/source_size.0 as f32, 1.0/source_size.1 as f32);
                self.blur_program.vector2("blurStep", if pass == 0 { 1.0/source_size.0 as f32 } else { 0.0 }, if pass == 1 { 1.0/source_size.1 as f32 } else { 0.0 });
                self.blur_program.vector4("blurWeights", weights[0], weights[1], weights[2], weights[3]);
                self.blur_program.vector4("blurOffsets", offsets[0], offsets[1], offsets[2], offsets[3]);
                self.blur_program.float("blurWeightScale", normalization);
                self.blur_program.vector4("blurEdge", edge[0], edge[1], edge[2], edge[3]);
                self.blur_program.float("blurHorizontal", if pass == 0 { 1.0 } else { 0.0 });
                unsafe { (self.gl.bind_sampler)(0, self.samplers.ids[resources::LINEAR_CLAMP_SAMPLER]); }
                self.draw(0x0004, &quad())?;
                source = target.texture.id;
                source_size = (width, height);
            }
            self.bind_texture(0, source);
            unsafe { (self.gl.generate_mipmap)(TEXTURE_2D); }
        }
        unsafe { (self.gl.viewport)(0, 0, self.width as i32, self.height as i32); }
        Ok(())
    }

    fn draw_primitive(&self, primitive: &MilkRustPrimitive) -> Result<(), String> {
        unsafe { (self.gl.enable)(BLEND); (self.gl.blend_func_separate)(SRC_ALPHA, if primitive.additive { 1 } else { ONE_MINUS_SRC_ALPHA }, 1, ONE_MINUS_SRC_ALPHA); }
        self.color.bind();
        let mode = match primitive.mode { MilkRustPrimitiveMode::LineStrip => 0x0003, MilkRustPrimitiveMode::Lines => 0x0001,
            MilkRustPrimitiveMode::Points => 0x0000, MilkRustPrimitiveMode::TriangleFan => 0x0006, MilkRustPrimitiveMode::Triangles => 0x0004 };
        self.draw(mode, &interleave(&primitive.vertices, &[], &primitive.vertex_colors, primitive.color)?)
    }

    fn draw_textured(&self, primitive: &MilkRustTexturedPrimitive, previous: u32, time: f64) -> Result<(), String> {
        unsafe { (self.gl.enable)(BLEND); (self.gl.blend_func_separate)(SRC_ALPHA, if primitive.additive { 1 } else { ONE_MINUS_SRC_ALPHA }, 1, ONE_MINUS_SRC_ALPHA); (self.gl.bind_sampler)(0, 0); }
        self.textured.bind(); self.textured.integer("previousFrame", 0);
        self.bind_texture(0, if primitive.texture_name.is_empty() { previous } else { self.named_texture_at(&primitive.texture_name, time)? });
        let vertices = interleave(&primitive.vertices, &primitive.uvs, &primitive.vertex_colors, primitive.color)?;
        let mode = match primitive.mode { MilkRustTexturedPrimitiveMode::Quad | MilkRustTexturedPrimitiveMode::TriangleFan => 0x0006 };
        self.draw(mode, &vertices)
    }

    fn draw(&self, mode: u32, vertices: &[f32]) -> Result<(), String> {
        if vertices.len() % 8 != 0 || vertices.len()/8 > MAX_VERTICES || vertices.iter().any(|v| !v.is_finite()) {
            return Err("Invalid or non-finite frame vertices".into());
        }
        unsafe {
            (self.gl.bind_vertex_array)(self.vao); (self.gl.bind_buffer)(ARRAY_BUFFER, self.buffer);
            (self.gl.buffer_data)(ARRAY_BUFFER, std::mem::size_of_val(vertices) as isize, vertices.as_ptr().cast(), 0x88E0);
            for (index, components, offset) in [(0, 2, 0usize), (1, 2, 2), (2, 4, 4)] {
                (self.gl.enable_attrib)(index); (self.gl.vertex_attrib)(index, components, 0x1406, 0, 32, (offset*4) as *const c_void);
            }
            (self.gl.draw_arrays)(mode, 0, (vertices.len()/8) as i32);
        }
        Ok(())
    }

    fn draw_sprite(&self, sprite: &milkrust_core::sprites::Frame, restore_final_burn_geometry: bool) -> Result<(), String> {
        let image = self.named_texture_asset_at(&sprite.texture, sprite.time)?;
        let animated = self.named_texture_is_animated(&sprite.texture);
        let mut image_id = image.id;
        let mut keyed_mips = false;
        if !animated && sprite.color_key_enabled() {
            let key = (sprite.texture.to_ascii_lowercase(), sprite.color_key);
            if let Some(target) = self.sprite_keyed.get(&key) {
                image_id = target.texture.id;
                keyed_mips = true;
            }
        }
        let background = self.sprite_snapshot.as_ref().ok_or("Missing sprite snapshot")?;
        // Copy from the currently bound render target; sampling the live target
        // would be undefined GL feedback, especially for multiple overlays.
        self.bind_texture(1, background.id);
        unsafe {
            (self.gl.copy_tex_sub_image)(TEXTURE_2D, 0, 0, 0, 0, 0, self.width as i32, self.height as i32);
            (self.gl.disable)(BLEND);
        }
        self.bind_texture(0, image_id);
        unsafe { (self.gl.bind_sampler)(0, self.samplers.ids[if animated { resources::ANIMATED_SPRITE_SAMPLER } else { resources::sampler_index(&sprite.texture) }]); }
        let program = &self.sprite_program;
        program.bind(); program.integer("spriteImage", 0); program.integer("spriteBackground", 1);
        program.vector2("spriteResolution", self.width as f32, self.height as f32);
        let tint = sprite.vertex_color();
        program.vector4("spriteTint", tint[0], tint[1], tint[2], tint[3]);
        let color_key = sprite.color_key_rgba();
        program.vector4("spriteColorKey", color_key[0], color_key[1], color_key[2], color_key[3]);
        program.integer("spriteColorKeyEnabled", if sprite.color_key_enabled() && !animated && !keyed_mips { 1 } else { 0 });
        program.integer("spriteBlend", sprite.blend_mode);
        let (positions, uvs) = sprite.geometry_for_target(
            (image.width, image.height),
            (self.width, self.height),
            restore_final_burn_geometry,
        );
        self.draw(0x0006, &interleave(&positions, &uvs, &[], [1.0; 4])?)?;
        unsafe { (self.gl.bind_sampler)(0, 0); }
        Ok(())
    }

    pub fn render(&mut self, frames: &MilkRustFrameSet, output: u32, width: u32, height: u32, time: f64) -> Result<(), String> {
        self.prepare(frames, width, height)?;
        unsafe {
            (self.gl.viewport)(0, 0, width as i32, height as i32);
            (self.gl.bind_framebuffer)(FRAMEBUFFER, output);
            if (self.gl.check_framebuffer)(FRAMEBUFFER) != 0x8CD5 { return Err("Output framebuffer is incomplete".into()); }
            (self.gl.clear_color)(0.0, 0.0, 0.0, 1.0); (self.gl.clear)(COLOR_BUFFER);
        }
        for (index, entry) in frames.entries.iter().enumerate() {
            let layer = &self.layers[index]; let frame = &entry.frame;
            let write = 1 - layer.read;
            if let Some(motion) = &frame.motion_vectors {
                unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, layer.targets[layer.read].id); }
                self.draw_primitive(motion)?;
            }
            self.build_resources(frame, layer, layer.targets[layer.read].texture.id)?;
            unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, layer.targets[write].id); (self.gl.disable)(BLEND); }
            self.bind_texture(0, layer.targets[layer.read].texture.id);
            let warp = layer.warp.as_ref().map_or(&self.copy, |(_, program)| program);
            self.uniforms(warp, frame, &frame.shader_stages.warp, time, 1.0,
                (1.0-frame.background_alpha).clamp(0.0, 1.0) as f32, TransitionUniforms::fade(), layer, layer.targets[layer.read].texture.id)?;
            warp.float("classicDisplay", 0.0);
            if !frame.shader_stages.warp.required { unsafe { (self.gl.bind_sampler)(0, 0); } }
            self.draw(0x0004, &feedback_vertices(frame)?)?;
            unsafe { (self.gl.enable)(BLEND); (self.gl.blend_func_separate)(SRC_ALPHA, ONE_MINUS_SRC_ALPHA, 1, ONE_MINUS_SRC_ALPHA); }
            let mut ordered_sprite_layers = false;
            for command in frame.draw_commands() {
                match command {
                    MilkRustDrawCommand::Primitive(index) => self.draw_primitive(frame.primitives.get(index).ok_or("Invalid primitive draw index")?)?,
                    MilkRustDrawCommand::Textured(index) => self.draw_textured(frame.textured_primitives.get(index).ok_or("Invalid textured draw index")?, layer.targets[layer.read].texture.id, time)?,
                    MilkRustDrawCommand::SpriteLayer(sprite_layer) => {
                        ordered_sprite_layers = true;
                        for sprite in frames.sprites.iter().filter(|sprite| sprite.preset_index == index
                            && sprite.layer == usize::from(sprite_layer)) {
                            self.draw_sprite(sprite, false)?;
                            if sprite.burn { self.draw_sprite(sprite, sprite.final_burn)?; }
                        }
                    }
                }
            }
            if !ordered_sprite_layers {
                // Preserve older FrameSet producers without scene-layer markers.
                for sprite in frames.sprites.iter().filter(|sprite| sprite.preset_index == index && sprite.layer > 0) {
                    self.draw_sprite(sprite, false)?;
                    if sprite.burn { self.draw_sprite(sprite, sprite.final_burn)?; }
                }
            }
            self.build_resources(frame, layer, layer.targets[write].texture.id)?;
            // The final composite samples this layer's completed feedback. It
            // never writes the display composite back into the feedback loop.
            unsafe {
                (self.gl.bind_framebuffer)(FRAMEBUFFER, output);
                if index == 0 { (self.gl.disable)(BLEND); } else {
                    (self.gl.enable)(BLEND);
                    let destination = if entry.composite_mode == "additive" { 1 } else { ONE_MINUS_SRC_ALPHA };
                    (self.gl.blend_func_separate)(SRC_ALPHA, destination, 1, destination);
                }
            }
            self.bind_texture(0, layer.targets[write].texture.id);
            let composite = layer.composite.as_ref().map_or(&self.copy, |(_, program)| program);
            let transition = TransitionUniforms::from_entry(entry);
            self.uniforms(composite, frame, &frame.shader_stages.composite, time,
                transition.output_alpha(index == 0), 1.0, transition, layer, layer.targets[write].texture.id)?;
            composite.float("classicDisplay", 1.0);
            if !frame.shader_stages.composite.required { unsafe { (self.gl.bind_sampler)(0, 0); } }
            self.draw(0x0004, &quad())?;
            // Layer 0 is an output overlay. When it burns, it also stamps the
            // feedback target after the composite has read that target.
            unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, layer.targets[write].id); }
            for sprite in frames.sprites.iter().filter(|sprite| sprite.preset_index == index
                && sprite.burn && sprite.layer == 0) {
                self.draw_sprite(sprite, false)?;
            }
            self.layers[index].read = write;
        }
        unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, output); }
        // Layer 0 overlays the composite. Layers 1–4 already entered the
        // feedback image at their ordered scene stage.
        for sprite in frames.sprites.iter().filter(|sprite| sprite.layer == 0) {
            self.draw_sprite(sprite, true)?;
        }
        unsafe {
            (self.gl.disable)(BLEND); (self.gl.active_texture)(TEXTURE0);
            if (self.gl.get_error)() != 0 { return Err("OpenGL failed while rendering a preset pass".into()); }
        }
        Ok(())
    }
}

fn interleave(positions: &[f64], uvs: &[f64], colors: &[f64], color: [f64; 4]) -> Result<Vec<f32>, String> {
    if positions.len()%2 != 0 || positions.len()/2 > MAX_VERTICES || (!uvs.is_empty() && uvs.len() != positions.len())
        || (!colors.is_empty() && colors.len() != positions.len()*2) { return Err("Inconsistent frame vertex arrays".into()); }
    let mut result = Vec::with_capacity(positions.len()*4);
    for index in 0..positions.len()/2 {
        result.extend_from_slice(&[positions[index*2] as f32, positions[index*2+1] as f32,
            uvs.get(index*2).copied().unwrap_or(0.0) as f32, uvs.get(index*2+1).copied().unwrap_or(0.0) as f32]);
        for (channel, fallback) in color.iter().enumerate() {
            result.push(colors.get(index*4+channel).copied().unwrap_or(*fallback) as f32);
        }
    }
    Ok(result)
}
fn quad() -> Vec<f32> {
    let points = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];
    let uvs = points.map(|v| (v+1.0)*0.5);
    // Fixed, structurally valid data; no input can reach this fallback.
    interleave(&points, &uvs, &[], [1.0; 4]).unwrap_or_default()
}
fn feedback_vertices(frame: &MilkRustFrame) -> Result<Vec<f32>, String> {
    if let Some(mesh) = &frame.warp_mesh {
        if mesh.positions.len() < 6 { return Err("Invalid or failed warp mesh".into()); }
        return interleave(&mesh.positions, &mesh.source_uvs, &[], [1.0; 4]); }
    let mut vertices = quad();
    let (sine, cosine) = frame.rotation.sin_cos();
    for vertex in vertices.chunks_exact_mut(8) {
        let x = (vertex[2] as f64 - 0.5) / frame.zoom.max(0.001);
        let y = (vertex[3] as f64 - 0.5) / frame.zoom.max(0.001);
        vertex[2] = (cosine*x - sine*y + 0.5 + frame.dx) as f32;
        vertex[3] = (sine*x + cosine*y + 0.5 + frame.dy) as f32;
    }
    Ok(vertices)
}

