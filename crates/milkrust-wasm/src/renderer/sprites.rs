use wasm_bindgen::prelude::*;
use web_sys::{WebGl2RenderingContext as Gl, WebGlTexture};

pub struct SpritePass {
    gl: Gl,
    program: web_sys::WebGlProgram,
    buffer: web_sys::WebGlBuffer,
    sampler: web_sys::WebGlSampler,
    snapshot: WebGlTexture,
    dimensions: (i32, i32),
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
        let Some(buffer) = gl.create_buffer() else {
            gl.delete_program(Some(&program)); return Err(JsValue::from_str("Sprite buffer unavailable"));
        };
        let Some(sampler) = gl.create_sampler() else {
            gl.delete_buffer(Some(&buffer)); gl.delete_program(Some(&program));
            return Err(JsValue::from_str("Sprite sampler unavailable"));
        };
        let Some(snapshot) = gl.create_texture() else {
            gl.delete_sampler(Some(&sampler)); gl.delete_buffer(Some(&buffer)); gl.delete_program(Some(&program));
            return Err(JsValue::from_str("Sprite snapshot unavailable"));
        };
        for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] { gl.sampler_parameteri(&sampler, parameter, Gl::LINEAR as i32); }
        for parameter in [Gl::TEXTURE_WRAP_S, Gl::TEXTURE_WRAP_T] { gl.sampler_parameteri(&sampler, parameter, Gl::REPEAT as i32); }
        Ok(Self { gl, program, buffer, sampler, snapshot, dimensions: (0, 0) })
    }

    pub fn draw(&mut self, sprite: &milkrust_core::sprites::Frame, image: &WebGlTexture, image_size: (u32, u32)) -> Result<(), JsValue> {
        let gl = &self.gl;
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
        gl.bind_texture(Gl::TEXTURE_2D, Some(image));
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
        gl.uniform1i(uniform("spriteBlend").as_ref(), sprite.blend_mode);
        let (positions, uvs) = sprite.geometry(image_size, (size.0 as u32, size.1 as u32));
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
        self.gl.delete_buffer(Some(&self.buffer));
        self.gl.delete_sampler(Some(&self.sampler));
        self.gl.delete_texture(Some(&self.snapshot));
    }
}
