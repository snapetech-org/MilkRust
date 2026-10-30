use wasm_bindgen::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use web_sys::{WebGl2RenderingContext as Gl, WebGlFramebuffer, WebGlTexture};

const KEY_FRAGMENT: &str = "#version 300 es\nprecision highp float; uniform sampler2D spriteSource; uniform vec3 spriteColorKey; out vec4 outColor; void main(){ivec2 p=ivec2(gl_FragCoord.xy);vec4 c=texelFetch(spriteSource,p,0);if(c.a>=1.0-0.5/255.0&&all(lessThan(abs(c.rgb-spriteColorKey),vec3(0.5/255.0))))c=vec4(0.0);outColor=c;}";

pub struct SpritePass {
    gl: Gl,
    program: web_sys::WebGlProgram,
    key_program: web_sys::WebGlProgram,
    buffer: web_sys::WebGlBuffer,
    sampler: web_sys::WebGlSampler,
    snapshot: WebGlTexture,
    dimensions: (i32, i32),
    keyed: BTreeMap<(String, u32), (WebGlTexture, WebGlFramebuffer, u64)>,
}

impl SpritePass {
    pub fn new(gl: Gl) -> Result<Self, JsValue> {
        let vertex = super::compile_milkrust_shader(&gl, Gl::VERTEX_SHADER,
            "#version 300 es\nin vec2 position; in vec2 sourceUv; out vec2 uv; void main(){uv=sourceUv;gl_Position=vec4(position,0,1);}")?;
        let fragment = match super::compile_milkrust_shader(&gl, Gl::FRAGMENT_SHADER, milkrust_core::sprites::FRAGMENT) {
            Ok(fragment) => fragment,
            Err(error) => { gl.delete_shader(Some(&vertex)); return Err(error); }
        };
        let program = super::link_milkrust_program(&gl, &vertex, &fragment)?;
        let key_vertex = super::compile_milkrust_shader(&gl, Gl::VERTEX_SHADER,
            "#version 300 es\nin vec2 position; void main(){gl_Position=vec4(position,0,1);}")?;
        let key_fragment = match super::compile_milkrust_shader(&gl, Gl::FRAGMENT_SHADER, KEY_FRAGMENT) {
            Ok(fragment) => fragment,
            Err(error) => { gl.delete_shader(Some(&key_vertex)); gl.delete_program(Some(&program)); return Err(error); }
        };
        let key_program = super::link_milkrust_program(&gl, &key_vertex, &key_fragment)?;
        let Some(buffer) = gl.create_buffer() else {
            gl.delete_program(Some(&key_program)); gl.delete_program(Some(&program)); return Err(JsValue::from_str("Sprite buffer unavailable"));
        };
        let Some(sampler) = gl.create_sampler() else {
            gl.delete_buffer(Some(&buffer)); gl.delete_program(Some(&key_program)); gl.delete_program(Some(&program));
            return Err(JsValue::from_str("Sprite sampler unavailable"));
        };
        let Some(snapshot) = gl.create_texture() else {
            gl.delete_sampler(Some(&sampler)); gl.delete_buffer(Some(&buffer)); gl.delete_program(Some(&key_program)); gl.delete_program(Some(&program));
            return Err(JsValue::from_str("Sprite snapshot unavailable"));
        };
        for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] { gl.sampler_parameteri(&sampler, parameter, Gl::LINEAR as i32); }
        for parameter in [Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T] { gl.sampler_parameteri(&sampler, parameter, Gl::REPEAT as i32); }
        Ok(Self { gl, program, key_program, buffer, sampler, snapshot, dimensions: (0, 0), keyed: BTreeMap::new() })
    }

    pub fn retain_keyed(&mut self, active: &BTreeSet<(String, u32)>) {
        let gl = self.gl.clone();
        self.keyed.retain(|key, (texture, framebuffer, _)| {
            if active.contains(key) { true } else {
                gl.delete_texture(Some(texture));
                gl.delete_framebuffer(Some(framebuffer));
                false
            }
        });
    }

    pub fn prepare_keyed(
        &mut self,
        name: &str,
        image: &WebGlTexture,
        image_size: (u32, u32),
        color_key: u32,
        source_pixels: u64,
    ) -> Result<(), JsValue> {
        let key = (name.to_ascii_lowercase(), color_key);
        if self.keyed.contains_key(&key) { return Ok(()); }
        if self.keyed.len() >= milkrust_core::sprites::MAX_SPRITES {
            return Err(JsValue::from_str("Sprite color-key derivative limit exceeded"));
        }
        let pixels = u64::from(image_size.0) * u64::from(image_size.1);
        let retained = self.keyed.values().map(|(_, _, pixels)| *pixels).sum::<u64>();
        if source_pixels.checked_add(retained).and_then(|value| value.checked_add(pixels))
            .is_none_or(|value| value > 32 * 1024 * 1024) {
            return Err(JsValue::from_str("Sprite color-keyed textures exceed the 32 Mi-pixel renderer budget"));
        }
        let texture = self.gl.create_texture().ok_or_else(|| JsValue::from_str("Sprite key texture unavailable"))?;
        let framebuffer = self.gl.create_framebuffer().ok_or_else(|| {
            self.gl.delete_texture(Some(&texture)); JsValue::from_str("Sprite key framebuffer unavailable")
        })?;
        let gl = &self.gl;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR_MIPMAP_LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::REPEAT as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::REPEAT as i32);
        if let Err(error) = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D, 0, Gl::RGBA8 as i32, image_size.0 as i32, image_size.1 as i32, 0,
            Gl::RGBA, Gl::UNSIGNED_BYTE, None) {
            gl.delete_texture(Some(&texture)); gl.delete_framebuffer(Some(&framebuffer)); return Err(error);
        }
        gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&framebuffer));
        gl.framebuffer_texture_2d(Gl::FRAMEBUFFER, Gl::COLOR_ATTACHMENT0, Gl::TEXTURE_2D, Some(&texture), 0);
        if gl.check_framebuffer_status(Gl::FRAMEBUFFER) != Gl::FRAMEBUFFER_COMPLETE {
            gl.bind_framebuffer(Gl::FRAMEBUFFER, None); gl.delete_texture(Some(&texture)); gl.delete_framebuffer(Some(&framebuffer));
            return Err(JsValue::from_str("Sprite key framebuffer is incomplete"));
        }
        gl.viewport(0, 0, image_size.0 as i32, image_size.1 as i32);
        gl.disable(Gl::BLEND);
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(image));
        gl.bind_sampler(0, None);
        gl.use_program(Some(&self.key_program));
        let uniform = |name: &str| gl.get_uniform_location(&self.key_program, name);
        gl.uniform1i(uniform("spriteSource").as_ref(), 0);
        gl.uniform3f(uniform("spriteColorKey").as_ref(),
            ((color_key >> 16) & 255) as f32 / 255.0,
            ((color_key >> 8) & 255) as f32 / 255.0,
            (color_key & 255) as f32 / 255.0);
        let points = [-1.0f32, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0];
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.buffer));
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(points.as_slice()), Gl::STREAM_DRAW);
        let position = gl.get_attrib_location(&self.key_program, "position");
        if position >= 0 {
            gl.enable_vertex_attrib_array(position as u32);
            gl.vertex_attrib_pointer_with_i32(position as u32, 2, Gl::FLOAT, false, 8, 0);
        }
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&texture));
        gl.generate_mipmap(Gl::TEXTURE_2D);
        gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        gl.viewport(0, 0, gl.drawing_buffer_width().max(1), gl.drawing_buffer_height().max(1));
        gl.use_program(Some(&self.program));
        gl.bind_sampler(0, None);
        if gl.get_error() != Gl::NO_ERROR {
            gl.delete_texture(Some(&texture)); gl.delete_framebuffer(Some(&framebuffer));
            return Err(JsValue::from_str("WebGL failed while preparing a keyed sprite mip chain"));
        }
        self.keyed.insert(key, (texture, framebuffer, pixels));
        Ok(())
    }

    pub fn draw(
        &mut self,
        sprite: &milkrust_core::sprites::Frame,
        image: &WebGlTexture,
        image_size: (u32, u32),
        restore_final_burn_geometry: bool,
        animated: bool,
    ) -> Result<(), JsValue> {
        let gl = &self.gl;
        let keyed = if !animated && sprite.color_key_enabled() {
            self.keyed.get(&(sprite.texture.to_ascii_lowercase(), sprite.color_key)).map(|(texture, _, _)| texture)
        } else { None };
        let size = (gl.drawing_buffer_width().max(1), gl.drawing_buffer_height().max(1));
        gl.active_texture(Gl::TEXTURE1);
        gl.bind_sampler(1, None);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.snapshot));
        if self.dimensions != size {
            if size.0 > 4096 || size.1 > 4096 || i64::from(size.0) * i64::from(size.1) > 8 * 1024 * 1024 {
                return Err(JsValue::from_str("Sprite snapshot dimensions exceed the render budget"));
            }
            gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D, 0, Gl::RGBA8 as i32, size.0, size.1, 0, Gl::RGBA, Gl::UNSIGNED_BYTE, None)?;
            for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] { gl.tex_parameteri(Gl::TEXTURE_2D, parameter, Gl::LINEAR as i32); }
            for parameter in [Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T] { gl.tex_parameteri(Gl::TEXTURE_2D, parameter, Gl::CLAMP_TO_EDGE as i32); }
            self.dimensions = size;
        }
        gl.copy_tex_sub_image_2d(Gl::TEXTURE_2D, 0, 0, 0, 0, 0, size.0, size.1);
        gl.disable(Gl::BLEND);
        gl.active_texture(Gl::TEXTURE0);
        let settings = if animated {
            milkrust_core::resources::SamplerSettings {
                min_point: false,
                mag_point: false,
                mip: milkrust_core::resources::MipFilter::None,
                wrap: [milkrust_core::resources::AddressMode::Wrap; 3],
            }
        } else {
            milkrust_core::resources::sampler_profile(&sprite.texture)
        };
        let min_filter = match (settings.min_point, settings.mip) {
            (true, milkrust_core::resources::MipFilter::None) => Gl::NEAREST,
            (false, milkrust_core::resources::MipFilter::None) => Gl::LINEAR,
            (true, milkrust_core::resources::MipFilter::Point) => Gl::NEAREST_MIPMAP_NEAREST,
            (false, milkrust_core::resources::MipFilter::Point) => Gl::LINEAR_MIPMAP_NEAREST,
            (true, milkrust_core::resources::MipFilter::Linear) => Gl::NEAREST_MIPMAP_LINEAR,
            (false, milkrust_core::resources::MipFilter::Linear) => Gl::LINEAR_MIPMAP_LINEAR,
        };
        gl.sampler_parameteri(&self.sampler, Gl::TEXTURE_MIN_FILTER, min_filter as i32);
        gl.sampler_parameteri(
            &self.sampler,
            Gl::TEXTURE_MAG_FILTER,
            if settings.mag_point { Gl::NEAREST as i32 } else { Gl::LINEAR as i32 },
        );
        for (axis, mode) in [
            (Gl::TEXTURE_WRAP_S, settings.wrap[0]),
            (Gl::TEXTURE_WRAP_T, settings.wrap[1]),
            (Gl::TEXTURE_WRAP_R, settings.wrap[2]),
        ] {
            let address = match mode {
                milkrust_core::resources::AddressMode::Clamp => Gl::CLAMP_TO_EDGE,
                milkrust_core::resources::AddressMode::Wrap => Gl::REPEAT,
                milkrust_core::resources::AddressMode::Mirror => Gl::MIRRORED_REPEAT,
            };
            gl.sampler_parameteri(&self.sampler, axis, address as i32);
        }
        gl.bind_texture(Gl::TEXTURE_2D, Some(keyed.unwrap_or(image)));
        gl.bind_sampler(0, Some(&self.sampler));
        gl.use_program(Some(&self.program));
        let uniform = |name: &str| gl.get_uniform_location(&self.program, name);
        gl.uniform1i(uniform("spriteImage").as_ref(), 0);
        gl.uniform1i(uniform("spriteBackground").as_ref(), 1);
        gl.uniform2f(uniform("spriteResolution").as_ref(), size.0 as f32, size.1 as f32);
        let tint = sprite.vertex_color();
        gl.uniform4f(uniform("spriteTint").as_ref(), tint[0], tint[1], tint[2], tint[3]);
        let color_key = sprite.color_key_rgba();
        gl.uniform4f(uniform("spriteColorKey").as_ref(), color_key[0], color_key[1], color_key[2], color_key[3]);
        gl.uniform1i(uniform("spriteColorKeyEnabled").as_ref(), if sprite.color_key_enabled() && !animated && keyed.is_none() { 1 } else { 0 });
        gl.uniform1i(uniform("spriteBlend").as_ref(), sprite.blend_mode);
        let (positions, uvs) = sprite.geometry_for_target(
            image_size,
            (size.0 as u32, size.1 as u32),
            restore_final_burn_geometry,
        );
        let mut vertices = Vec::with_capacity(16);
        for (position, uv) in positions.chunks_exact(2).zip(uvs.chunks_exact(2)) {
            vertices.extend_from_slice(&[position[0] as f32, position[1] as f32, uv[0] as f32, uv[1] as f32]);
        }
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.buffer));
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(vertices.as_slice()), Gl::STREAM_DRAW);
        for (name, offset) in [("position", 0), ("sourceUv", 8)] {
            let location = gl.get_attrib_location(&self.program, name);
            if location >= 0 {
                gl.enable_vertex_attrib_array(location as u32);
                gl.vertex_attrib_pointer_with_i32(location as u32, 2, Gl::FLOAT, false, 16, offset);
            }
        }
        gl.draw_arrays(Gl::TRIANGLE_FAN, 0, 4);
        gl.bind_sampler(0, None);
        if gl.get_error() != Gl::NO_ERROR { return Err(JsValue::from_str("WebGL sprite pass failed")); }
        Ok(())
    }
}

impl Drop for SpritePass {
    fn drop(&mut self) {
        self.gl.delete_program(Some(&self.program));
        self.gl.delete_program(Some(&self.key_program));
        self.gl.delete_buffer(Some(&self.buffer));
        self.gl.delete_sampler(Some(&self.sampler));
        self.gl.delete_texture(Some(&self.snapshot));
        for (_, (texture, framebuffer, _)) in &self.keyed {
            self.gl.delete_texture(Some(texture));
            self.gl.delete_framebuffer(Some(framebuffer));
        }
    }
}
