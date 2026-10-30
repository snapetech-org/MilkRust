use super::{create_milkrust_feedback_targets, delete_milkrust_feedback_targets,
    compile_milkrust_shader, link_milkrust_program, MilkRustFeedbackTargets,
    MILKRUST_TRANSLATED_VERTEX_SHADER};
use milkrust_core::resources::{self, TextureKind};
use std::collections::BTreeMap;
use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as Gl, WebGlTexture, WebGlSampler, WebGlProgram, WebGlBuffer};

pub(super) struct ShaderResources {
    gl: Gl,
    program: WebGlProgram,
    buffer: WebGlBuffer,
    samplers: Vec<WebGlSampler>,
    noise: BTreeMap<String, (WebGlTexture, u32)>,
    blur: Vec<MilkRustFeedbackTargets>,
    history: Option<WebGlTexture>,
    size: (i32, i32),
}

impl ShaderResources {
    pub fn new(gl: &Gl) -> Result<Self, JsValue> {
        let vertex = compile_milkrust_shader(gl, Gl::VERTEX_SHADER, MILKRUST_TRANSLATED_VERTEX_SHADER)?;
        let fragment = compile_milkrust_shader(gl, Gl::FRAGMENT_SHADER, resources::BLUR_FRAGMENT)
            .map_err(|e| { gl.delete_shader(Some(&vertex)); e })?;
        let program = link_milkrust_program(gl, &vertex, &fragment)?;
        let buffer = match gl.create_buffer() { Some(b) => b, None => {
            gl.delete_program(Some(&program)); return Err(JsValue::from_str("Blur buffer allocation failed"));
        }};
        let mut result = Self { gl: gl.clone(), program, buffer, samplers: Vec::new(),
            noise: BTreeMap::new(), blur: Vec::new(), history: None, size: (0, 0) };
        for index in 0..4 {
            let sampler = gl.create_sampler().ok_or_else(|| JsValue::from_str("Sampler allocation failed"))?;
            gl.sampler_parameteri(&sampler, Gl::TEXTURE_MIN_FILTER,
                if index & 1 != 0 { Gl::NEAREST_MIPMAP_NEAREST } else { Gl::LINEAR_MIPMAP_LINEAR } as i32);
            gl.sampler_parameteri(&sampler, Gl::TEXTURE_MAG_FILTER,
                if index & 1 != 0 { Gl::NEAREST } else { Gl::LINEAR } as i32);
            for axis in [Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T, Gl::TEXTURE_WRAP_R] {
                gl.sampler_parameteri(&sampler, axis, if index & 2 != 0 { Gl::REPEAT } else { Gl::CLAMP_TO_EDGE } as i32);
            }
            result.samplers.push(sampler);
        }
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&result.buffer));
        let vertices = js_sys::Float32Array::from([-1f32, -1., 1., -1., -1., 1., 1., 1.].as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &vertices, Gl::STATIC_DRAW);
        Ok(result)
    }

    pub fn prepare<'a>(&mut self, names: impl Iterator<Item = &'a String>, width: i32, height: i32) -> Result<(), JsValue> {
        if self.size != (width, height) {
            for target in self.blur.drain(..) { delete_milkrust_feedback_targets(&self.gl, &target); }
            self.size = (width, height);
        }
        let mut levels = 0;
        for name in names {
            match resources::texture_kind(name) {
                TextureKind::Blur(index) => levels = levels.max(index + 1),
                TextureKind::Noise2d(seed) | TextureKind::Noise3d(seed) => {
                    let key = resources::base_name(name).to_string();
                    if self.noise.contains_key(&key) { continue; }
                    let volume = resources::texture_kind(name).is_volume();
                    let (w, h, d, data) = resources::noise_texture(seed, volume);
                    let texture = self.gl.create_texture().ok_or_else(|| JsValue::from_str("Noise texture allocation failed"))?;
                    let target = if volume { Gl::TEXTURE_3D } else { Gl::TEXTURE_2D };
                    self.gl.active_texture(Gl::TEXTURE0);
                    self.gl.bind_texture(target, Some(&texture));
                    // Register before upload so error unwinding still deletes it.
                    self.noise.insert(key, (texture, target));
                    self.gl.pixel_storei(Gl::UNPACK_ALIGNMENT, 1);
                    if volume {
                        self.gl.tex_image_3d_with_opt_u8_array(target, 0, Gl::RGBA8 as i32, w as i32,
                            h as i32, d as i32, 0, Gl::RGBA, Gl::UNSIGNED_BYTE, Some(&data))?;
                    } else {
                        self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                            target, 0, Gl::RGBA8 as i32, w as i32, h as i32, 0, Gl::RGBA, Gl::UNSIGNED_BYTE, Some(&data))?;
                    }
                    self.gl.generate_mipmap(target);
                }
                _ => {}
            }
        }
        while self.blur.len() < levels {
            let scale = 1 << (self.blur.len() + 1);
            self.blur.push(create_milkrust_feedback_targets(&self.gl, (width / scale).max(1), (height / scale).max(1))?);
        }
        Ok(())
    }

    pub fn build(&mut self, frame: &milkrust_core::MilkRustFrame, history: &WebGlTexture) {
        self.history = Some(history.clone());
        let gl = &self.gl;
        gl.active_texture(Gl::TEXTURE0); gl.bind_sampler(0, None);
        gl.bind_texture(Gl::TEXTURE_2D, Some(history)); gl.generate_mipmap(Gl::TEXTURE_2D);
        gl.disable(Gl::BLEND);
        let mut source = history.clone();
        gl.use_program(Some(&self.program));
        gl.uniform1i(gl.get_uniform_location(&self.program, "previousFrame").as_ref(), 0);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.buffer));
        let position = gl.get_attrib_location(&self.program, "position");
        if position >= 0 {
            gl.enable_vertex_attrib_array(position as u32);
            gl.vertex_attrib_pointer_with_i32(position as u32, 2, Gl::FLOAT, false, 0, 0);
        }
        let mut source_size = self.size;
        for (index, level) in self.blur.iter().enumerate() {
            gl.viewport(0, 0, level.width, level.height);
            for (pass, target) in level.targets.iter().enumerate() {
                gl.bind_texture(Gl::TEXTURE_2D, Some(&source));
                gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&target.framebuffer));
                let [scale, bias] = resources::blur_transform(frame, index, pass);
                let (weights, offsets, normalization) = resources::blur_kernel(pass);
                let edge = resources::blur_edge(frame, index, pass);
                gl.uniform2f(gl.get_uniform_location(&self.program, "blurTransform").as_ref(), scale, bias);
                gl.uniform2f(gl.get_uniform_location(&self.program, "sourceTexel").as_ref(),
                    1.0/source_size.0 as f32, 1.0/source_size.1 as f32);
                gl.uniform2f(gl.get_uniform_location(&self.program, "blurStep").as_ref(),
                    if pass == 0 { 1.0/source_size.0 as f32 } else { 0.0 },
                    if pass == 1 { 1.0/source_size.1 as f32 } else { 0.0 });
                gl.uniform4f(gl.get_uniform_location(&self.program, "blurWeights").as_ref(),
                    weights[0], weights[1], weights[2], weights[3]);
                gl.uniform4f(gl.get_uniform_location(&self.program, "blurOffsets").as_ref(),
                    offsets[0], offsets[1], offsets[2], offsets[3]);
                gl.uniform1f(gl.get_uniform_location(&self.program, "blurWeightScale").as_ref(), normalization);
                gl.uniform4f(gl.get_uniform_location(&self.program, "blurEdge").as_ref(),
                    edge[0], edge[1], edge[2], edge[3]);
                gl.uniform1f(gl.get_uniform_location(&self.program, "blurHorizontal").as_ref(),
                    if pass == 0 { 1.0 } else { 0.0 });
                gl.bind_sampler(0, self.samplers.get(2));
                gl.draw_arrays(Gl::TRIANGLE_STRIP, 0, 4);
                source = target.texture.clone();
                source_size = (level.width, level.height);
            }
            gl.bind_texture(Gl::TEXTURE_2D, Some(&source)); gl.generate_mipmap(Gl::TEXTURE_2D);
        }
        gl.viewport(0, 0, self.size.0, self.size.1);
    }

    pub fn texture(&self, name: &str) -> Option<(WebGlTexture, u32)> {
        match resources::texture_kind(name) {
            TextureKind::Main => self.history.clone().map(|t| (t, Gl::TEXTURE_2D)),
            TextureKind::Blur(index) => self.blur.get(index).map(|t| (t.targets[1].texture.clone(), Gl::TEXTURE_2D)),
            TextureKind::Noise2d(_) | TextureKind::Noise3d(_) => self.noise.get(resources::base_name(name)).cloned(),
            TextureKind::Asset => None,
        }
    }
    pub fn dimensions(&self, name: &str) -> Option<(u32, u32)> {
        match resources::texture_kind(name) {
            TextureKind::Main => Some((self.size.0 as u32, self.size.1 as u32)),
            TextureKind::Blur(index) => self.blur.get(index).map(|target| (target.width as u32, target.height as u32)),
            TextureKind::Noise2d(seed) => { let side = if seed == 4 { 32 } else { 256 }; Some((side, side)) },
            TextureKind::Noise3d(_) => Some((32, 32)),
            TextureKind::Asset => None,
        }
    }
    pub fn bind_sampler(&self, unit: u32, name: &str) {
        let (nearest, repeat) = resources::sampler_state(name);
        self.gl.bind_sampler(unit, self.samplers.get(usize::from(nearest) + 2 * usize::from(repeat)));
    }
}

impl Drop for ShaderResources {
    fn drop(&mut self) {
        self.gl.delete_program(Some(&self.program)); self.gl.delete_buffer(Some(&self.buffer));
        for sampler in &self.samplers { self.gl.delete_sampler(Some(sampler)); }
        for (texture, _) in self.noise.values() { self.gl.delete_texture(Some(texture)); }
        for target in &self.blur { delete_milkrust_feedback_targets(&self.gl, target); }
    }
}
