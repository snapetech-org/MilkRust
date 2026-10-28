//! Native GL/GLES execution of MilkRust's frame graph. Feedback and final
//! composite are distinct passes; each double-preset layer owns two targets.
use std::{collections::BTreeMap, ffi::{c_void, CString}, ptr};
use milkrust_core::{get_milkrust_texture_name_aliases, MilkRustFrame, MilkRustFrameSet,
    MilkRustPrimitiveMode, MilkRustShaderStage, MilkRustTexturedPrimitiveMode};
use crate::gl::*;

const VERTEX: &str = "#version 300 es\nin vec2 position; in vec2 sourceUv; in vec4 vertexColor; out vec2 uv; out vec4 tint; void main(){ uv=sourceUv; tint=vertexColor; gl_Position=vec4(position,0,1); gl_PointSize=1.0; }";
const COPY: &str = "#version 300 es\nprecision highp float; uniform sampler2D previousFrame; uniform float feedback; uniform float outputAlpha; in vec2 uv; out vec4 outColor; void main(){outColor=vec4(texture(previousFrame,uv).rgb*feedback,outputAlpha);}";
const COLOR: &str = "#version 300 es\nprecision highp float; in vec4 tint; out vec4 outColor; void main(){outColor=tint;}";
const TEXTURED: &str = "#version 300 es\nprecision highp float; uniform sampler2D previousFrame; in vec2 uv; in vec4 tint; out vec4 outColor; void main(){outColor=texture(previousFrame,uv)*tint;}";
const MAX_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_TEXTURE_PIXELS: u64 = 32 * 1024 * 1024;
const MAX_VERTICES: usize = 262_144;

struct Program { gl: Gl, id: u32 }
impl Drop for Program { fn drop(&mut self) { unsafe { (self.gl.delete_program)(self.id); } } }
struct Shader { gl: Gl, id: u32 }
impl Drop for Shader { fn drop(&mut self) { unsafe { (self.gl.delete_shader)(self.id); } } }
struct Texture { gl: Gl, id: u32, pixels: u64 }
impl Drop for Texture { fn drop(&mut self) { unsafe { (self.gl.delete_textures)(1, &self.id); } } }
struct Target { gl: Gl, id: u32, texture: Texture }
impl Drop for Target { fn drop(&mut self) { unsafe { (self.gl.delete_framebuffers)(1, &self.id); } } }
struct Layer {
    targets: [Target; 2], read: usize,
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
        let program = Self { gl, id };
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
        CString::new(name).ok().map_or(-1, |name| unsafe { (self.gl.uniform_location)(self.id, name.as_ptr()) })
    }
    fn bind(&self) { unsafe { (self.gl.use_program)(self.id); } }
    fn float(&self, name: &str, value: f32) { unsafe { (self.gl.uniform_1f)(self.location(name), value); } }
    fn integer(&self, name: &str, value: i32) { unsafe { (self.gl.uniform_1i)(self.location(name), value); } }
    fn vector2(&self, name: &str, x: f32, y: f32) { unsafe { (self.gl.uniform_2f)(self.location(name), x, y); } }
    fn vector3(&self, name: &str, x: f32, y: f32, z: f32) { unsafe { (self.gl.uniform_3f)(self.location(name), x, y, z); } }
    fn vector4(&self, name: &str, x: f32, y: f32, z: f32, w: f32) { unsafe { (self.gl.uniform_4f)(self.location(name), x, y, z, w); } }
    fn floats(&self, name: &str, values: &[f32]) { unsafe { (self.gl.uniform_1fv)(self.location(name), values.len() as i32, values.as_ptr()); } }
}

impl Texture {
    fn new(gl: Gl, width: u32, height: u32, data: Option<&[u8]>) -> Result<Self, String> {
        let mut id = 0;
        unsafe { (gl.gen_textures)(1, &mut id); }
        if id == 0 { return Err("GL could not allocate a texture".into()); }
        let texture = Self { gl, id, pixels: u64::from(width) * u64::from(height) };
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
    gl: Gl, gles: bool, copy: Program, color: Program, textured: Program,
    buffer: u32, vao: u32, max_texture: i32, texture_units: usize,
    width: u32, height: u32, capture: Option<(u32, u32, Target)>, layers: Vec<Layer>, textures: BTreeMap<String, Texture>,
}
impl Drop for Renderer {
    fn drop(&mut self) { unsafe { (self.gl.delete_buffers)(1, &self.buffer); (self.gl.delete_vertex_arrays)(1, &self.vao); } }
}

impl Renderer {
    /// # Safety
    /// gl must refer to the current dedicated context for this entire lifetime.
    pub unsafe fn new(gl: Gl, gles: bool) -> Result<Self, String> {
        let copy = Program::new(gl, gles, COPY)?;
        let color = Program::new(gl, gles, COLOR)?;
        let textured = Program::new(gl, gles, TEXTURED)?;
        let (mut buffer, mut vao, mut max_texture, mut units) = (0, 0, 0, 0);
        unsafe {
            (gl.get_integer)(0x0D33, &mut max_texture); (gl.get_integer)(0x8872, &mut units);
            (gl.gen_buffers)(1, &mut buffer); (gl.gen_vertex_arrays)(1, &mut vao);
        }
        let result = Self { gl, gles, copy, color, textured, buffer, vao, max_texture, texture_units: units.max(0) as usize,
            width: 0, height: 0, capture: None, layers: Vec::new(), textures: BTreeMap::new() };
        if buffer == 0 || vao == 0 || max_texture < 64 || units < 4 { return Err("Insufficient OpenGL context resources".into()); }
        unsafe {
            (gl.disable)(0x0B71); (gl.disable)(0x0B44); (gl.disable)(0x0C11); // depth, cull, scissor
            if !gles { (gl.enable)(0x8642); } // PROGRAM_POINT_SIZE
        }
        Ok(result)
    }

    pub fn reset(&mut self) { self.layers.clear(); self.textures.clear(); self.capture = None; self.width = 0; self.height = 0; }

    pub fn texture(&mut self, name: &str, width: u32, height: u32, bytes: &[u8]) -> Result<(), String> {
        dimensions(width, height, self.max_texture)?;
        if name.is_empty() || name.len() > 512 || name.contains('\0') || bytes.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err("Invalid texture name or RGBA byte length".into());
        }
        let folded = name.to_ascii_lowercase();
        let existing = self.textures.get(&folded).map_or(0, |texture| texture.pixels);
        if (self.textures.len() >= 64 && existing == 0) || self.textures.values().map(|t| t.pixels).sum::<u64>() - existing
            + u64::from(width) * u64::from(height) > MAX_TEXTURE_PIXELS {
            return Err("Named textures exceed 64 assets or 32 Mi-pixels".into());
        }
        // GL UV origin is bottom-left. Hosts supply ordinary top-down image rows.
        let stride = width as usize * 4;
        let mut flipped = Vec::with_capacity(bytes.len());
        for row in bytes.chunks_exact(stride).rev() { flipped.extend_from_slice(row); }
        let texture = Texture::new(self.gl, width, height, Some(&flipped))?;
        self.textures.insert(folded, texture);
        Ok(())
    }

    fn named_texture(&self, name: &str) -> Result<u32, String> {
        let aliases = get_milkrust_texture_name_aliases(name);
        for name in aliases {
            if let Some(texture) = self.textures.get(&name.to_ascii_lowercase()) { return Ok(texture.id); }
        }
        Err(format!("Missing texture: {name}"))
    }

    fn prepare(&mut self, frames: &MilkRustFrameSet, width: u32, height: u32) -> Result<(), String> {
        dimensions(width, height, self.max_texture)?;
        if frames.entries.is_empty() || frames.entries.len() > 2 { return Err("Expected one or two preset layers".into()); }
        let mut vertices = 0usize;
        for entry in &frames.entries {
            let frame = &entry.frame;
            vertices += frame.warp_mesh.as_ref().map_or(6, |mesh| mesh.positions.len() / 2);
            vertices += frame.primitives.iter().map(|p| p.vertices.len()/2).sum::<usize>();
            vertices += frame.textured_primitives.iter().map(|p| p.vertices.len()/2).sum::<usize>();
            if vertices > MAX_VERTICES { return Err("Frame geometry exceeds 262144 vertices".into()); }
            for stage in [&frame.shader_stages.warp, &frame.shader_stages.composite] {
                if stage.required && stage.source.is_empty() { return Err("Required shader could not be translated".into()); }
                if stage.texture_samplers.len() + 1 > self.texture_units { return Err("Shader exceeds available texture units".into()); }
                for name in &stage.texture_samplers { self.named_texture(name)?; }
            }
            for primitive in &frame.textured_primitives {
                if !primitive.texture_name.is_empty() { self.named_texture(&primitive.texture_name)?; }
            }
        }
        if width != self.width || height != self.height || self.layers.len() != frames.entries.len() {
            let mut layers = Vec::with_capacity(frames.entries.len());
            for _ in &frames.entries {
                layers.push(Layer { targets: [Target::new(self.gl, width, height)?, Target::new(self.gl, width, height)?],
                    read: 0, warp: None, composite: None });
            }
            self.layers = layers; self.width = width; self.height = height;
        }
        for (layer, entry) in self.layers.iter_mut().zip(&frames.entries) {
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
        unsafe { (self.gl.active_texture)(TEXTURE0 + unit as u32); (self.gl.bind_texture)(TEXTURE_2D, texture); }
    }

    fn uniforms(&self, program: &Program, frame: &MilkRustFrame, stage: &MilkRustShaderStage,
        time: f64, alpha: f32, feedback: f32) -> Result<(), String> {
        program.bind(); program.integer("previousFrame", 0);
        program.float("feedback", feedback); program.float("outputAlpha", alpha);
        program.float("time", time as f32); program.float("sampleRate", 44_100.0);
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
        for (index, name) in stage.texture_samplers.iter().enumerate() {
            self.bind_texture(index + 1, self.named_texture(name)?);
            program.integer(&format!("shaderTexture{index}"), (index + 1) as i32);
        }
        unsafe { (self.gl.active_texture)(TEXTURE0); }
        Ok(())
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
            unsafe { (self.gl.bind_framebuffer)(FRAMEBUFFER, layer.targets[write].id); (self.gl.disable)(BLEND); }
            self.bind_texture(0, layer.targets[layer.read].texture.id);
            let warp = layer.warp.as_ref().map_or(&self.copy, |(_, program)| program);
            self.uniforms(warp, frame, &frame.shader_stages.warp, time, 1.0, (1.0-frame.background_alpha).clamp(0.0, 1.0) as f32)?;
            self.draw(0x0004, &feedback_vertices(frame)?)?;
            unsafe { (self.gl.enable)(BLEND); (self.gl.blend_func_separate)(SRC_ALPHA, ONE_MINUS_SRC_ALPHA, 1, ONE_MINUS_SRC_ALPHA); }
            for primitive in &frame.textured_primitives {
                self.textured.bind(); self.textured.integer("previousFrame", 0);
                self.bind_texture(0, if primitive.texture_name.is_empty() { layer.targets[layer.read].texture.id }
                    else { self.named_texture(&primitive.texture_name)? });
                let vertices = interleave(&primitive.vertices, &primitive.uvs, &[], primitive.color)?;
                let mode = match primitive.mode { MilkRustTexturedPrimitiveMode::Quad => 0x0006, MilkRustTexturedPrimitiveMode::TriangleFan => 0x0006 };
                self.draw(mode, &vertices)?;
            }
            for primitive in &frame.primitives {
                self.color.bind();
                let mode = match primitive.mode { MilkRustPrimitiveMode::LineStrip => 0x0003, MilkRustPrimitiveMode::Lines => 0x0001,
                    MilkRustPrimitiveMode::Points => 0x0000, MilkRustPrimitiveMode::TriangleFan => 0x0006, MilkRustPrimitiveMode::Triangles => 0x0004 };
                self.draw(mode, &interleave(&primitive.vertices, &[], &primitive.vertex_colors, primitive.color)?)?;
            }
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
            self.uniforms(composite, frame, &frame.shader_stages.composite, time, if index == 0 { 1.0 } else { entry.blend_alpha.clamp(0.0, 1.0) as f32 }, 1.0)?;
            self.draw(0x0004, &quad())?;
            self.layers[index].read = write;
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
    if let Some(mesh) = &frame.warp_mesh { return interleave(&mesh.positions, &mesh.source_uvs, &[], [1.0; 4]); }
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
