mod sprites;
mod shader_resources;
use shader_resources::ShaderResources;
use std::{cell::{Cell, RefCell}, collections::BTreeMap, rc::Rc};

use milkrust_core::{
    clamp_unit, create_repeated_milkrust_colors, get_milkrust_texture_name_aliases,
    milk2, MilkRustCompositeFrame, MilkRustFrame, MilkRustFrameSet, MilkRustPrimitiveMode, MilkRustTexturedPrimitiveMode,
    MilkRustShaderStage, MilkRustPrimitive, MilkRustTexturedPrimitive, MilkRustDrawCommand,
};
use wasm_bindgen::{prelude::*, JsCast};

#[derive(Clone, Copy)]
struct TransitionUniforms {
    pattern: i32,
    progress: f32,
    direction: f32,
    random: [f32; 5],
}

impl TransitionUniforms {
    fn from_entry(entry: &MilkRustCompositeFrame) -> Self {
        Self {
            pattern: milk2::blend_pattern_code(&entry.blend_pattern).unwrap_or(0),
            progress: clamp_unit(entry.blend_alpha) as f32,
            direction: entry.blend_direction as f32,
            random: entry.blend_random.map(|value| value as f32),
        }
    }

    fn fade() -> Self {
        Self { pattern: 0, progress: 1.0, direction: 1.0, random: [0.0; 5] }
    }

    fn output_alpha(self, clear_screen: bool) -> f32 {
        if clear_screen || self.pattern != 0 { 1.0 }
        else { self.progress }
    }
}

fn set_transition_uniforms(
    gl: &web_sys::WebGl2RenderingContext,
    program: &web_sys::WebGlProgram,
    transition: TransitionUniforms,
    width: i32,
    height: i32,
) {
    let uniform = |name: &str| gl.get_uniform_location(program, name);
    gl.uniform1i(uniform("u_transitionPattern").as_ref(), transition.pattern);
    gl.uniform1f(uniform("u_transitionProgress").as_ref(), transition.progress);
    gl.uniform1f(uniform("u_transitionDirection").as_ref(), transition.direction);
    for (index, value) in transition.random.iter().enumerate() {
        gl.uniform1f(uniform(&format!("u_transitionRandom{}", index + 1)).as_ref(), *value);
    }
    gl.uniform2f(uniform("u_transitionResolution").as_ref(), width as f32, height as f32);
}

pub enum MilkRustRenderer {
    WebGl(MilkRustWebGlRendererSet),
    Canvas {
        canvas: web_sys::HtmlCanvasElement,
        context: web_sys::CanvasRenderingContext2d,
    },
}

impl MilkRustRenderer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::WebGl(_) => "Rust WebGL2 renderer active",
            Self::Canvas { .. } => "Canvas renderer fallback active",
        }
    }

    pub fn reset(&self) {
        if let Self::WebGl(renderer_set) = self { renderer_set.renderers.borrow_mut().clear(); }
    }

    pub fn render_frame_set(&self, frame_set: &MilkRustFrameSet, time: f64) -> Result<bool, JsValue> {
        if frame_set.entries.is_empty() {
            return Err(JsValue::from_str("No preset frames to render"));
        }
        match self {
            Self::WebGl(renderer_set) => renderer_set.render_frame_set(frame_set, time),
            Self::Canvas { canvas, context } => {
                if !frame_set.sprites.is_empty() || frame_set.entries.iter().any(|entry| entry.frame.shader_stages.warp.required ||
                    entry.frame.shader_stages.composite.required || entry.frame.warp_mesh.is_some() ||
                    !entry.frame.textured_primitives.is_empty() ||
                    milk2::blend_pattern_code(&entry.blend_pattern).is_some_and(|pattern| pattern != 0)) {
                    return Err(JsValue::from_str("Canvas preview cannot execute preset shaders, warp meshes, textured primitives or spatial .milk2 transitions"));
                }
                for (index, entry) in frame_set.entries.iter().enumerate() {
                    context.save();
                    context.set_global_alpha(clamp_unit(entry.blend_alpha));
                    if index == 0 {
                        render_milkrust_canvas_frame(context, canvas, &entry.frame, time);
                    } else {
                        render_milkrust_canvas_overlay_frame(context, canvas, &entry.frame, time);
                    }
                    context.restore();
                }
                Ok(true)
            }
        }
    }
}

pub struct MilkRustWebGlRendererSet {
    gl: web_sys::WebGl2RenderingContext,
    renderers: RefCell<Vec<MilkRustWebGlRenderer>>,
    texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
}

struct MilkRustWebGlRenderer {
    sprite_pass: RefCell<Option<sprites::SpritePass>>,
    transition_mask: RefCell<Option<(milk2::TransitionMaskKey, web_sys::WebGlTexture)>>,
    shader_resources: RefCell<ShaderResources>,
    buffer: web_sys::WebGlBuffer,
    feedback_targets: RefCell<MilkRustFeedbackTargets>,
    gl: web_sys::WebGl2RenderingContext,
    primitive_buffer: web_sys::WebGlBuffer,
    primitive_color_buffer: web_sys::WebGlBuffer,
    primitive_program: web_sys::WebGlProgram,
    placeholder_texture: web_sys::WebGlTexture,
    transition_placeholder_texture: web_sys::WebGlTexture,
    named_textures: RefCell<BTreeMap<String, MilkRustTextureAsset>>,
    texture_pixels: Rc<Cell<u64>>,
    max_texture_units: usize,
    program: web_sys::WebGlProgram,
    textured_position_buffer: web_sys::WebGlBuffer,
    textured_program: web_sys::WebGlProgram,
    textured_uv_buffer: web_sys::WebGlBuffer,
    translated_warp_program: RefCell<Option<MilkRustTranslatedProgram>>,
    translated_composite_program: RefCell<Option<MilkRustTranslatedProgram>>,
    texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
    u_color: Option<web_sys::WebGlUniformLocation>,
    u_counts: Option<web_sys::WebGlUniformLocation>,
    u_display_only: Option<web_sys::WebGlUniformLocation>,
    u_feedback: Option<web_sys::WebGlUniformLocation>,
    u_motion: Option<web_sys::WebGlUniformLocation>,
    u_output_alpha: Option<web_sys::WebGlUniformLocation>,
    u_previous_frame: Option<web_sys::WebGlUniformLocation>,
    u_primitive_point_size: Option<web_sys::WebGlUniformLocation>,
    u_resolution: Option<web_sys::WebGlUniformLocation>,
    u_textured_sampler: Option<web_sys::WebGlUniformLocation>,
    u_time: Option<web_sys::WebGlUniformLocation>,
    u_warp_color: Option<web_sys::WebGlUniformLocation>,
    u_warp_feedback: Option<web_sys::WebGlUniformLocation>,
    u_warp_output_alpha: Option<web_sys::WebGlUniformLocation>,
    u_warp_previous_frame: Option<web_sys::WebGlUniformLocation>,
    warp_position_buffer: web_sys::WebGlBuffer,
    warp_program: web_sys::WebGlProgram,
    warp_uv_buffer: web_sys::WebGlBuffer,
}

struct MilkRustTranslatedProgram {
    program: web_sys::WebGlProgram,
    source: String,
    mesh: bool,
    uniforms: BTreeMap<String, Option<web_sys::WebGlUniformLocation>>,
}

struct MilkRustTextureAsset {
    gl: web_sys::WebGl2RenderingContext,
    texture: web_sys::WebGlTexture,
    additional_frames: Vec<web_sys::WebGlTexture>,
    delays_seconds: Vec<f64>,
    repeat_count: Option<u16>,
    dimensions: Rc<Cell<(u32, u32)>>,
    // None while loading, Some(Ok) after upload, Some(Err) after decode/upload failure.
    state: Rc<RefCell<Option<Result<(), String>>>>,
    total_pixels: Rc<Cell<u64>>,
    reserved_pixels: u64,
    worker: Option<web_sys::Worker>,
    worker_url: Option<String>,
    worker_timeout_id: Option<i32>,
    _onmessage: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    _onerror: Option<Closure<dyn FnMut(web_sys::ErrorEvent)>>,
    _timeout: Option<Closure<dyn FnMut()>>,
}

impl MilkRustTextureAsset {
    fn texture_at(&self, seconds: f64) -> web_sys::WebGlTexture {
        if self.additional_frames.is_empty() || self.delays_seconds.len() != self.additional_frames.len() + 1 {
            return self.texture.clone();
        }
        let index = milkrust_core::texture::animation_frame_index(&self.delays_seconds, self.repeat_count, seconds);
        if index == 0 { self.texture.clone() } else { self.additional_frames[index - 1].clone() }
    }
}

impl Drop for MilkRustTextureAsset {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.set_onmessage(None);
            worker.set_onerror(None);
            worker.terminate();
        }
        if let Some(timeout_id) = self.worker_timeout_id {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(timeout_id);
            }
        }
        if let Some(url) = &self.worker_url {
            revoke_image_decoder_worker_url(url);
        }
        self.gl.delete_texture(Some(&self.texture));
        for texture in &self.additional_frames { self.gl.delete_texture(Some(texture)); }
        self.total_pixels.set(self.total_pixels.get().saturating_sub(self.reserved_pixels));
    }
}

struct MilkRustFeedbackTargets {
    height: i32,
    read_index: usize,
    targets: [MilkRustFeedbackTarget; 2],
    width: i32,
}

struct MilkRustFeedbackTarget {
    framebuffer: web_sys::WebGlFramebuffer,
    texture: web_sys::WebGlTexture,
}

pub fn milkrust_renderer(
    canvas: &web_sys::HtmlCanvasElement,
    texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
) -> Result<MilkRustRenderer, JsValue> {
    if let Some(context) = canvas.get_context("webgl2")? {
        if let Ok(gl) = context.dyn_into::<web_sys::WebGl2RenderingContext>() {
            if let Ok(renderer) = MilkRustWebGlRendererSet::new(gl, texture_assets) {
                return Ok(MilkRustRenderer::WebGl(renderer));
            }
        }
    }
    let context: web_sys::CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("2D canvas is unavailable"))?
        .dyn_into()?;
    Ok(MilkRustRenderer::Canvas {
        canvas: canvas.clone(),
        context,
    })
}

impl MilkRustWebGlRendererSet {
    fn new(
        gl: web_sys::WebGl2RenderingContext,
        texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
    ) -> Result<Self, JsValue> {
        Ok(Self {
            gl: gl.clone(),
            renderers: RefCell::new(Vec::new()),
            texture_assets,
        })
    }

    fn render_frame_set(&self, frame_set: &MilkRustFrameSet, time: f64) -> Result<bool, JsValue> {
        if self.gl.is_context_lost() { return Err(JsValue::from_str("WebGL context lost; reload the preset after recovery")); }
        self.ensure_renderer_count(frame_set.entries.len())?;
        let renderers = self.renderers.borrow();
        // Compile every required stage before changing feedback or the canvas.
        // A failed shader never silently selects a built-in visual instead.
        for (renderer, entry) in renderers.iter().zip(&frame_set.entries) {
            renderer.prepare_stages(&entry.frame)?;
            if !renderer.prepare_textures(&entry.frame)? { return Ok(false); }
            for sprite in &frame_set.sprites {
                if renderer.named_texture_for(&sprite.texture)?.is_none() { return Ok(false); }
            }
        }
        for (index, entry) in frame_set.entries.iter().enumerate() {
            if let Some(renderer) = renderers.get(index) {
                renderer.render_with_options(
                    entry,
                    time,
                    index == 0,
                    &frame_set.sprites,
                )?;
            }
        }
        self.gl.bind_framebuffer(web_sys::WebGl2RenderingContext::FRAMEBUFFER, None);
        for sprite in frame_set.sprites.iter().filter(|sprite| sprite.layer == 0) {
            let renderer = renderers.get(sprite.preset_index)
                .ok_or_else(|| JsValue::from_str("Sprite owner has no active preset renderer"))?;
            // Layer zero alone has a separate draw over the combined composite.
            renderer.draw_sprite(sprite)?;
        }
        Ok(true)
    }

    fn ensure_renderer_count(&self, count: usize) -> Result<(), JsValue> {
        if count == 0 || count > 2 { return Err(JsValue::from_str("Expected one or two independent preset render targets")); }
        let mut renderers = self.renderers.borrow_mut();
        renderers.truncate(count);
        while renderers.len() < count {
            let renderer = MilkRustWebGlRenderer::new(self.gl.clone(), self.texture_assets.clone())?;
            renderers.push(renderer);
        }
        Ok(())
    }
}

impl MilkRustWebGlRenderer {
    fn new(
        gl: web_sys::WebGl2RenderingContext,
        texture_assets: Rc<RefCell<BTreeMap<String, String>>>,
    ) -> Result<Self, JsValue> {
        let max_texture_units = gl.get_parameter(web_sys::WebGl2RenderingContext::MAX_TEXTURE_IMAGE_UNITS)?
            .as_f64().unwrap_or(0.0) as usize;
        let vertex_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            MILKRUST_WEBGL_VERTEX_SHADER,
        )?;
        let fragment_source = format!(
            "#version 300 es\nprecision highp float;\n{}\n{}\n{}",
            milk2::GLSL_TRANSITION_MASK,
            milkrust_core::classic::GLSL,
            MILKRUST_WEBGL_FRAGMENT_BODY,
        );
        let fragment_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            &fragment_source,
        )?;
        let program = link_milkrust_program(&gl, &vertex_shader, &fragment_shader)?;
        gl.use_program(Some(&program));
        let primitive_vertex_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            MILKRUST_PRIMITIVE_VERTEX_SHADER,
        )?;
        let primitive_fragment_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            MILKRUST_PRIMITIVE_FRAGMENT_SHADER,
        )?;
        let primitive_program =
            link_milkrust_program(&gl, &primitive_vertex_shader, &primitive_fragment_shader)?;
        let textured_vertex_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            MILKRUST_TEXTURED_VERTEX_SHADER,
        )?;
        let textured_fragment_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            MILKRUST_TEXTURED_FRAGMENT_SHADER,
        )?;
        let textured_program =
            link_milkrust_program(&gl, &textured_vertex_shader, &textured_fragment_shader)?;
        let warp_vertex_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            MILKRUST_WARP_GRID_VERTEX_SHADER,
        )?;
        let warp_fragment_shader = compile_milkrust_shader(
            &gl,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            MILKRUST_WARP_GRID_FRAGMENT_SHADER,
        )?;
        let warp_program = link_milkrust_program(&gl, &warp_vertex_shader, &warp_fragment_shader)?;

        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL buffer allocation failed"))?;
        gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));
        let vertices =
            js_sys::Float32Array::from(&[-1.0_f32, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0][..]);
        gl.buffer_data_with_array_buffer_view(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            &vertices,
            web_sys::WebGl2RenderingContext::STATIC_DRAW,
        );
        let position = gl.get_attrib_location(&program, "position");
        if position >= 0 {
            gl.enable_vertex_attrib_array(position as u32);
            gl.vertex_attrib_pointer_with_i32(
                position as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        let primitive_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL primitive buffer allocation failed"))?;
        let primitive_color_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL primitive color buffer allocation failed"))?;
        let textured_position_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL textured position buffer allocation failed"))?;
        let textured_uv_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL textured UV buffer allocation failed"))?;
        let placeholder_texture = create_milkrust_placeholder_texture(&gl)?;
        let transition_placeholder_texture = create_milkrust_transition_placeholder_texture(&gl)?;
        let warp_position_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL warp position buffer allocation failed"))?;
        let warp_uv_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("WebGL warp UV buffer allocation failed"))?;
        let feedback_targets = RefCell::new(create_milkrust_feedback_targets(
            &gl,
            gl.drawing_buffer_width().max(1),
            gl.drawing_buffer_height().max(1),
        )?);

        Ok(Self {
            sprite_pass: RefCell::new(None),
            transition_mask: RefCell::new(None),
            u_color: gl.get_uniform_location(&program, "u_color"),
            u_counts: gl.get_uniform_location(&program, "u_counts"),
            u_display_only: gl.get_uniform_location(&program, "u_displayOnly"),
            u_feedback: gl.get_uniform_location(&program, "u_feedback"),
            u_motion: gl.get_uniform_location(&program, "u_motion"),
            u_output_alpha: gl.get_uniform_location(&program, "u_outputAlpha"),
            u_previous_frame: gl.get_uniform_location(&program, "u_previousFrame"),
            u_primitive_point_size: gl
                .get_uniform_location(&primitive_program, "u_primitivePointSize"),
            u_resolution: gl.get_uniform_location(&program, "u_resolution"),
            u_textured_sampler: gl.get_uniform_location(&textured_program, "u_texture"),
            u_time: gl.get_uniform_location(&program, "u_time"),
            u_warp_color: gl.get_uniform_location(&warp_program, "u_color"),
            u_warp_feedback: gl.get_uniform_location(&warp_program, "u_feedback"),
            u_warp_output_alpha: gl.get_uniform_location(&warp_program, "u_outputAlpha"),
            u_warp_previous_frame: gl.get_uniform_location(&warp_program, "u_previousFrame"),
            warp_position_buffer,
            warp_program,
            warp_uv_buffer,
            buffer,
            feedback_targets,
            gl: gl.clone(),
            named_textures: RefCell::new(BTreeMap::new()),
            texture_pixels: Rc::new(Cell::new(0)),
            max_texture_units,
            primitive_buffer,
            primitive_color_buffer,
            primitive_program,
            placeholder_texture,
            transition_placeholder_texture,
            program,
            textured_position_buffer,
            textured_program,
            textured_uv_buffer,
            texture_assets,
            shader_resources: RefCell::new(ShaderResources::new(&gl)?),
            translated_warp_program: RefCell::new(None),
            translated_composite_program: RefCell::new(None),
        })
    }

    fn render_with_options(
        &self,
        entry: &MilkRustCompositeFrame,
        time: f64,
        clear_screen: bool,
        sprites: &[milkrust_core::sprites::Frame],
    ) -> Result<(), JsValue> {
        let frame = &entry.frame;
        let transition = TransitionUniforms::from_entry(entry);
        let (warp_program, composite_program) = self.prepare_stages(frame)?;
        self.gl.use_program(Some(&self.program));
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.buffer),
        );
        let drawing_width = self.gl.drawing_buffer_width().max(1);
        let drawing_height = self.gl.drawing_buffer_height().max(1);
        self.prepare_transition_mask(entry, drawing_width as u32, drawing_height as u32)?;
        let mut targets = self.feedback_targets.borrow_mut();
        if targets.width != drawing_width || targets.height != drawing_height {
            let next_targets = create_milkrust_feedback_targets(&self.gl, drawing_width, drawing_height)?;
            delete_milkrust_feedback_targets(&self.gl, &targets);
            *targets = next_targets;
        }
        let read_index = targets.read_index;
        if let Some(motion) = &frame.motion_vectors {
            self.gl.bind_framebuffer(web_sys::WebGl2RenderingContext::FRAMEBUFFER, Some(&targets.targets[read_index].framebuffer));
            self.gl.viewport(0, 0, drawing_width, drawing_height);
            self.draw_primitives(std::slice::from_ref(motion));
        }
        let write_index = 1 - read_index;
        self.shader_resources.borrow_mut().prepare(frame.shader_stages.warp.texture_samplers.iter()
            .chain(&frame.shader_stages.composite.texture_samplers), drawing_width, drawing_height)?;
        self.shader_resources.borrow_mut().build(frame, &targets.targets[read_index].texture);
        self.gl.bind_sampler(0, None);

        self.gl
            .active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            Some(&targets.targets[read_index].texture),
        );
        self.gl.bind_framebuffer(
            web_sys::WebGl2RenderingContext::FRAMEBUFFER,
            Some(&targets.targets[write_index].framebuffer),
        );
        self.gl.disable(web_sys::WebGl2RenderingContext::BLEND);
        if frame.warp_mesh.is_some() {
            self.draw_warp_mesh(frame, drawing_width, drawing_height, warp_program.as_ref(), time)?;
        } else if let Some(program) = warp_program {
            self.draw_translated_quad(&program, frame, &frame.shader_stages.warp,
                time, drawing_width, drawing_height, true, 1.0, TransitionUniforms::fade())?;
        } else {
            self.draw_feedback_quad(frame, time, drawing_width, drawing_height, false, 1.0, TransitionUniforms::fade());
        }
        let mut ordered_sprite_layers = false;
        for command in frame.draw_commands() {
            match command {
                MilkRustDrawCommand::Primitive(index) => self.draw_primitives(std::slice::from_ref(frame.primitives.get(index).ok_or_else(|| JsValue::from_str("Invalid primitive draw index"))?)),
                MilkRustDrawCommand::Textured(index) => self.draw_textured_primitives(std::slice::from_ref(frame.textured_primitives.get(index).ok_or_else(|| JsValue::from_str("Invalid textured draw index"))?), &targets.targets[read_index].texture, time)?,
                MilkRustDrawCommand::SpriteLayer(sprite_layer) => {
                    ordered_sprite_layers = true;
                    for sprite in sprites.iter().filter(|sprite| sprite.preset_index == entry.index
                        && sprite.layer == usize::from(sprite_layer)) {
                        self.draw_sprite(sprite)?;
                        if sprite.burn { self.draw_sprite(sprite)?; }
                    }
                }
            }
        }
        if !ordered_sprite_layers {
            // Preserve legacy FrameSet producers that have no phase markers.
            for sprite in sprites.iter().filter(|sprite| sprite.preset_index == entry.index && sprite.layer > 0) {
                self.draw_sprite(sprite)?;
                if sprite.burn { self.draw_sprite(sprite)?; }
            }
        }
        self.shader_resources.borrow_mut().build(frame, &targets.targets[write_index].texture);

        self.gl
            .bind_framebuffer(web_sys::WebGl2RenderingContext::FRAMEBUFFER, None);
        self.gl.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(
            web_sys::WebGl2RenderingContext::TEXTURE_2D,
            Some(&targets.targets[write_index].texture),
        );
        if clear_screen {
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl
                .clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);
        }
        let output_alpha = transition.output_alpha(clear_screen);
        let should_blend = !clear_screen;
        if should_blend {
            self.gl.enable(web_sys::WebGl2RenderingContext::BLEND);
            let (source_factor, destination_factor) =
                milkrust_webgl_composite_blend_factors(&entry.composite_mode);
            self.gl.blend_func_separate(source_factor, destination_factor, web_sys::WebGl2RenderingContext::ONE, destination_factor);
        }
        if let Some(program) = composite_program {
            self.draw_translated_quad(&program, frame, &frame.shader_stages.composite,
                time, drawing_width, drawing_height, false, output_alpha, transition)?;
        } else { self.draw_feedback_quad(
            frame,
            time,
            drawing_width,
            drawing_height,
            true,
            output_alpha,
            transition,
        ); }
        if should_blend {
            self.gl.disable(web_sys::WebGl2RenderingContext::BLEND);
        }
        self.gl.bind_framebuffer(web_sys::WebGl2RenderingContext::FRAMEBUFFER, Some(&targets.targets[write_index].framebuffer));
        for sprite in sprites.iter().filter(|sprite| sprite.preset_index == entry.index && sprite.burn && sprite.layer == 0) { self.draw_sprite(sprite)?; }
        self.gl.bind_framebuffer(web_sys::WebGl2RenderingContext::FRAMEBUFFER, None);
        targets.read_index = write_index;
        if self.gl.get_error() != web_sys::WebGl2RenderingContext::NO_ERROR {
            return Err(JsValue::from_str("WebGL failed while drawing a preset stage"));
        }
        Ok(())
    }

    fn draw_warp_mesh(&self, frame: &MilkRustFrame, drawing_width: i32, drawing_height: i32,
        translated: Option<&web_sys::WebGlProgram>, time: f64) -> Result<(), JsValue> {
        let Some(mesh) = frame.warp_mesh.as_ref() else {
            return Err(JsValue::from_str("Invalid or absent warp mesh"));
        };
        if mesh.positions.len() < 6 || mesh.positions.len() != mesh.source_uvs.len() {
            return Err(JsValue::from_str("Invalid or absent warp mesh"));
        }
        let program = translated.unwrap_or(&self.warp_program);
        self.gl.use_program(Some(program));
        self.gl.viewport(0, 0, drawing_width, drawing_height);
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl
            .clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);
        let positions = mesh
            .positions
            .iter()
            .map(|value| *value as f32)
            .collect::<Vec<_>>();
        let source_uvs = mesh
            .source_uvs
            .iter()
            .map(|value| *value as f32)
            .collect::<Vec<_>>();
        let positions_array = js_sys::Float32Array::from(positions.as_slice());
        let source_uvs_array = js_sys::Float32Array::from(source_uvs.as_slice());
        let position = self.gl.get_attrib_location(program, "position");
        let source_uv = self.gl.get_attrib_location(program, "sourceUv");
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.warp_position_buffer),
        );
        self.gl.buffer_data_with_array_buffer_view(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            &positions_array,
            web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
        );
        if position >= 0 {
            self.gl.enable_vertex_attrib_array(position as u32);
            self.gl.vertex_attrib_pointer_with_i32(
                position as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.warp_uv_buffer),
        );
        self.gl.buffer_data_with_array_buffer_view(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            &source_uvs_array,
            web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
        );
        if source_uv >= 0 {
            self.gl.enable_vertex_attrib_array(source_uv as u32);
            self.gl.vertex_attrib_pointer_with_i32(
                source_uv as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        if translated.is_some() {
            self.set_translated_uniforms(program, frame, &frame.shader_stages.warp,
                time, drawing_width, drawing_height, true, 1.0, TransitionUniforms::fade())?;
        } else {
        let (r, g, b) = frame.wave_color;
        self.gl.uniform3f(
            self.u_warp_color.as_ref(),
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        );
        self.gl.uniform1f(
            self.u_warp_feedback.as_ref(),
            (1.0 - frame.background_alpha).clamp(0.0, 1.0) as f32,
        );
        self.gl.uniform1f(self.u_warp_output_alpha.as_ref(), 1.0);
        self.gl.uniform1i(self.u_warp_previous_frame.as_ref(), 0);
        self.gl.bind_sampler(0, None);
        self.gl.uniform1f(self.gl.get_uniform_location(&self.warp_program, "classicWrap").as_ref(), milkrust_core::classic::uniforms(frame).2);
        }
        self.gl.draw_arrays(
            web_sys::WebGl2RenderingContext::TRIANGLES,
            0,
            (positions.len() / 2) as i32,
        );
        Ok(())
    }

    fn prepare_transition_mask(&self, entry: &MilkRustCompositeFrame, width: u32, height: u32) -> Result<(), JsValue> {
        let Some(pattern) = milk2::blend_pattern_code(&entry.blend_pattern).filter(|pattern| *pattern != 0) else {
            if let Some((_, texture)) = self.transition_mask.borrow_mut().take() {
                self.gl.delete_texture(Some(&texture));
            }
            return Ok(());
        };
        let key = milk2::TransitionMaskKey::new(pattern, entry.blend_alpha as f32, entry.blend_direction,
            entry.blend_random.map(|value| value as f32), width, height);
        if self.transition_mask.borrow().as_ref().is_some_and(|(cached, _)| *cached == key) { return Ok(()); }
        let vertices = milk2::generate_transition_vertex_field(key).map_err(|error| JsValue::from_str(&error))?;
        let (mask_width, mask_height) = (milk2::TRANSITION_GRID_WIDTH, milk2::TRANSITION_GRID_HEIGHT);
        let texture = self.gl.create_texture().ok_or_else(|| JsValue::from_str("WebGL transition mask allocation failed"))?;
        self.gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        self.gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_ALIGNMENT, 1);
        self.gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER, web_sys::WebGl2RenderingContext::NEAREST as i32);
        self.gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER, web_sys::WebGl2RenderingContext::NEAREST as i32);
        self.gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_WRAP_S, web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
        self.gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
            web_sys::WebGl2RenderingContext::TEXTURE_WRAP_T, web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
        let upload = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            web_sys::WebGl2RenderingContext::TEXTURE_2D, 0, 0x8D7C,
            mask_width as i32, mask_height as i32, 0, 0x8D99,
            web_sys::WebGl2RenderingContext::UNSIGNED_BYTE, Some(&vertices));
        self.gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_ALIGNMENT, 4);
        if let Err(error) = upload {
            self.gl.delete_texture(Some(&texture));
            return Err(error);
        }
        if self.gl.get_error() != web_sys::WebGl2RenderingContext::NO_ERROR {
            self.gl.delete_texture(Some(&texture));
            return Err(JsValue::from_str("WebGL transition vertex field upload failed"));
        }
        if let Some((_, previous)) = self.transition_mask.borrow_mut().replace((key, texture)) {
            self.gl.delete_texture(Some(&previous));
        }
        Ok(())
    }

    fn bind_transition_mask(&self, program: &web_sys::WebGlProgram, transition: TransitionUniforms) {
        let unit = self.max_texture_units.saturating_sub(1) as u32;
        let texture = if transition.pattern == 0 {
            &self.transition_placeholder_texture
        } else {
            self.transition_mask.borrow().as_ref().map(|(_, texture)| texture).unwrap_or(&self.transition_placeholder_texture)
        };
        self.gl.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0 + unit);
        self.gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(texture));
        self.gl.bind_sampler(unit, None);
        self.gl.uniform1i(self.gl.get_uniform_location(program, "u_transitionMask").as_ref(), unit as i32);
        self.gl.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
    }

    fn prepare_stages(&self, frame: &MilkRustFrame) -> Result<(Option<web_sys::WebGlProgram>, Option<web_sys::WebGlProgram>), JsValue> {
        let units = self.max_texture_units;
        if frame.shader_stages.warp.texture_samplers.len() + 2 > units ||
            frame.shader_stages.composite.texture_samplers.len() + 2 > units {
            return Err(JsValue::from_str("Preset exceeds the GPU texture-unit limit after reserving history and transition-mask units"));
        }
        let warp = self.translated_program_for(&frame.shader_stages.warp, &self.translated_warp_program,
            frame.warp_mesh.is_some()).map_err(|e| JsValue::from_str(&format!("Warp shader: {}", e.as_string().unwrap_or_default())))?;
        let composite = self.translated_program_for(&frame.shader_stages.composite, &self.translated_composite_program,
            false).map_err(|e| JsValue::from_str(&format!("Composite shader: {}", e.as_string().unwrap_or_default())))?;
        Ok((warp, composite))
    }

    fn translated_program_for(&self, stage: &MilkRustShaderStage,
        cache: &RefCell<Option<MilkRustTranslatedProgram>>, mesh: bool) -> Result<Option<web_sys::WebGlProgram>, JsValue> {
        if !stage.required { return Ok(None); }
        let source = &stage.source;
        if source.trim().is_empty() {
            return Err(JsValue::from_str("the required shader could not be translated"));
        }
        if let Some(cached) = cache.borrow().as_ref() {
            if cached.source == *source && cached.mesh == mesh {
                return Ok(Some(cached.program.clone()));
            }
        }
        let vertex_shader = compile_milkrust_shader(
            &self.gl,
            web_sys::WebGl2RenderingContext::VERTEX_SHADER,
            if mesh { MILKRUST_TRANSLATED_MESH_VERTEX_SHADER } else { MILKRUST_TRANSLATED_VERTEX_SHADER },
        )?;
        let fragment_shader = compile_milkrust_shader(
            &self.gl,
            web_sys::WebGl2RenderingContext::FRAGMENT_SHADER,
            source,
        ).map_err(|error| { self.gl.delete_shader(Some(&vertex_shader)); error })?;
        let program = link_milkrust_program(&self.gl, &vertex_shader, &fragment_shader)?;
        if let Some(previous) = cache.borrow_mut().take() { self.gl.delete_program(Some(&previous.program)); }
        *cache.borrow_mut() = Some(MilkRustTranslatedProgram {
            program: program.clone(),
            source: source.to_string(),
            mesh,
            uniforms: BTreeMap::new(),
        });
        Ok(Some(program))
    }

    fn set_translated_uniforms(
        &self,
        program: &web_sys::WebGlProgram,
        frame: &MilkRustFrame,
        stage: &MilkRustShaderStage,
        time: f64,
        drawing_width: i32,
        drawing_height: i32,
        warp: bool,
        output_alpha: f32,
        transition: TransitionUniforms,
    ) -> Result<(), JsValue> {
        let (r, g, b) = frame.wave_color;
        let feedback = (1.0 - frame.background_alpha).clamp(0.0, 1.0) as f32;
        let mut cached = (if warp { &self.translated_warp_program } else { &self.translated_composite_program }).borrow_mut();
        let mut uniform = |name: &str| {
            if let Some(cached) = cached.as_mut().filter(|cached| cached.program == *program) {
                cached.uniforms.entry(name.to_string()).or_insert_with(|| self.gl.get_uniform_location(program, name)).clone()
            } else { self.gl.get_uniform_location(program, name) }
        };
        self.gl.uniform3f(
            uniform("color").as_ref(),
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        );
        self.gl.uniform1i(uniform("previousFrame").as_ref(), 0);
        self.shader_resources.borrow().bind_sampler(0, "sampler_main");
        for (name, v) in milkrust_core::resources::uniform_vectors(frame, time, drawing_width.max(1) as u32, drawing_height.max(1) as u32) {
            self.gl.uniform4f(uniform(name).as_ref(), v[0], v[1], v[2], v[3]);
        }
        for (name, matrix) in milkrust_core::resources::uniform_matrices(frame, time) {
            self.gl.uniform_matrix3x4fv_with_f32_array(uniform(name).as_ref(), false, &matrix);
        }
        for index in 0..stage.texture_samplers.len() {
            let texture_unit = index + 1;
            self.gl
                .active_texture(web_sys::WebGl2RenderingContext::TEXTURE0 + texture_unit as u32);
            let name = &stage.texture_samplers[index];
            let builtin = self.shader_resources.borrow().texture(name);
            let (texture, target) = match builtin {
                Some(binding) => binding,
                None => (self.named_texture_for_at(name, time)?.ok_or_else(||
                    JsValue::from_str(&format!("Missing shader texture: {name}")))?,
                    web_sys::WebGl2RenderingContext::TEXTURE_2D),
            };
            self.gl.bind_texture(target, Some(&texture));
            let dimensions = self.shader_resources.borrow().dimensions(name).or_else(|| {
                let assets = self.named_textures.borrow();
                get_milkrust_texture_name_aliases(name).iter().find_map(|alias|
                    assets.get(alias).map(|asset| asset.dimensions.get()))
            });
            let (width, height) = dimensions.filter(|(w,h)| *w > 0 && *h > 0)
                .ok_or_else(|| JsValue::from_str("Shader texture dimensions are unavailable"))?;
            self.gl.uniform4f(uniform(&format!("texsize_{}", milkrust_core::resources::base_name(name))).as_ref(),
                width as f32, height as f32, 1.0/width as f32, 1.0/height as f32);
            if !milkrust_core::resources::texture_kind(name).is_builtin() { self.gl.generate_mipmap(target); }
            self.shader_resources.borrow().bind_sampler(texture_unit as u32, name);
            self.gl.uniform1i(
                uniform(&format!("shaderTexture{index}")).as_ref(),
                texture_unit as i32,
            );
        }
        self.gl
            .active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        self.gl.uniform1f(uniform("feedback").as_ref(), feedback);
        self.gl.uniform1f(uniform("outputAlpha").as_ref(), output_alpha);
        set_transition_uniforms(&self.gl, program, transition, drawing_width, drawing_height);
        self.bind_transition_mask(program, transition);
        self.gl.uniform1f(uniform("isWarp").as_ref(), if warp { 1.0 } else { 0.0 });
        self.gl.uniform4f(uniform("motion").as_ref(), frame.rotation as f32, frame.zoom as f32,
            frame.dx as f32, frame.dy as f32);
        self.gl.uniform1f(uniform("time").as_ref(), time.rem_euclid(10000.0) as f32);
        self.gl.uniform1f(uniform("sampleRate").as_ref(), 44_100.0);
        self.gl.uniform2f(
            uniform("resolution").as_ref(),
            drawing_width as f32,
            drawing_height as f32,
        );
        self.gl.uniform2f(
            uniform("pixelSize").as_ref(),
            1.0 / drawing_width.max(1) as f32,
            1.0 / drawing_height.max(1) as f32,
        );
        self.gl.uniform1f(
            uniform("aspect").as_ref(),
            drawing_width as f32 / drawing_height.max(1) as f32,
        );
        self.gl.uniform4f(
            uniform("texsize").as_ref(),
            drawing_width as f32,
            drawing_height as f32,
            1.0 / drawing_width.max(1) as f32,
            1.0 / drawing_height.max(1) as f32,
        );
        self.gl
            .uniform1f(uniform("bass").as_ref(), frame.bass as f32);
        self.gl
            .uniform1f(uniform("bass_att").as_ref(), frame.bass as f32);
        self.gl.uniform1f(uniform("mid").as_ref(), frame.mid as f32);
        self.gl
            .uniform1f(uniform("mid_att").as_ref(), frame.mid as f32);
        self.gl
            .uniform1f(uniform("treb").as_ref(), frame.treble as f32);
        self.gl
            .uniform1f(uniform("treb_att").as_ref(), frame.treble as f32);
        let fft = frame
            .fft_bins
            .iter()
            .map(|value| *value as f32)
            .collect::<Vec<_>>();
        let waveform = frame
            .waveform_bins
            .iter()
            .map(|value| *value as f32)
            .collect::<Vec<_>>();
        self.gl
            .uniform1fv_with_f32_array(uniform("fftBins").as_ref(), &fft);
        self.gl
            .uniform1fv_with_f32_array(uniform("waveformBins").as_ref(), &waveform);
        for (index, value) in frame.q_registers.iter().enumerate() {
            self.gl
                .uniform1f(uniform(&format!("q{}", index + 1)).as_ref(), *value as f32);
        }
        for (name, value) in &frame.shader_values {
            if name != "preset_seed" { self.gl.uniform1f(uniform(name).as_ref(), *value as f32); }
        }
        for (level, [low, high]) in milkrust_core::resources::blur_ranges(frame).iter().copied().enumerate() {
            self.gl.uniform1f(uniform(&format!("blur{}_min", level+1)).as_ref(), low);
            self.gl.uniform1f(uniform(&format!("blur{}_max", level+1)).as_ref(), high);
        }

        Ok(())
    }

    fn draw_translated_quad(
        &self,
        program: &web_sys::WebGlProgram,
        frame: &MilkRustFrame,
        stage: &MilkRustShaderStage,
        time: f64,
        drawing_width: i32,
        drawing_height: i32,
        warp: bool,
        output_alpha: f32,
        transition: TransitionUniforms,
    ) -> Result<(), JsValue> {
        self.gl.use_program(Some(program));
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.buffer),
        );
        let position = self.gl.get_attrib_location(program, "position");
        if position >= 0 {
            self.gl.enable_vertex_attrib_array(position as u32);
            self.gl.vertex_attrib_pointer_with_i32(
                position as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        self.gl.viewport(0, 0, drawing_width, drawing_height);
        if warp {
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);
        }
        self.set_translated_uniforms(program, frame, stage, time, drawing_width, drawing_height, warp, output_alpha, transition)?;
        self.gl
            .draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);
        Ok(())
    }

    fn draw_feedback_quad(
        &self,
        frame: &MilkRustFrame,
        time: f64,
        drawing_width: i32,
        drawing_height: i32,
        display_only: bool,
        output_alpha: f32,
        transition: TransitionUniforms,
    ) {
        self.gl.use_program(Some(&self.program));
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.buffer),
        );
        let position = self.gl.get_attrib_location(&self.program, "position");
        if position >= 0 {
            self.gl.enable_vertex_attrib_array(position as u32);
            self.gl.vertex_attrib_pointer_with_i32(
                position as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        self.gl.viewport(0, 0, drawing_width, drawing_height);
        if !display_only {
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl
                .clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);
        }
        let (r, g, b) = frame.wave_color;
        let feedback = (1.0 - frame.background_alpha).clamp(0.0, 1.0) as f32;
        self.gl.bind_sampler(0, None);
        let (echo, filters, wrap) = milkrust_core::classic::uniforms(frame);
        self.gl.uniform4fv_with_f32_array(self.gl.get_uniform_location(&self.program, "classicEcho").as_ref(), &echo);
        self.gl.uniform4fv_with_f32_array(self.gl.get_uniform_location(&self.program, "classicFilters").as_ref(), &filters);
        self.gl.uniform1f(self.gl.get_uniform_location(&self.program, "classicWrap").as_ref(), wrap);
        self.gl.uniform1i(self.u_previous_frame.as_ref(), 0);
        self.gl.uniform1f(
            self.u_display_only.as_ref(),
            if display_only { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(self.u_feedback.as_ref(), feedback);
        self.gl
            .uniform1f(self.u_output_alpha.as_ref(), output_alpha);
        set_transition_uniforms(&self.gl, &self.program, transition, drawing_width, drawing_height);
        self.bind_transition_mask(&self.program, transition);
        self.gl.uniform2f(
            self.u_resolution.as_ref(),
            drawing_width as f32,
            drawing_height as f32,
        );
        self.gl.uniform1f(self.u_time.as_ref(), time as f32);
        self.gl.uniform4f(
            self.u_color.as_ref(),
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            feedback,
        );
        self.gl.uniform4f(
            self.u_motion.as_ref(),
            frame.rotation as f32,
            frame.zoom as f32,
            frame.dx as f32,
            frame.dy as f32,
        );
        self.gl.uniform2f(
            self.u_counts.as_ref(),
            frame.shape_count as f32,
            frame.waveform_count as f32,
        );
        self.gl
            .draw_arrays(web_sys::WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);
    }

    fn draw_primitives(&self, primitives: &[MilkRustPrimitive]) {
        if primitives.is_empty() {
            return;
        }
        self.gl.enable(web_sys::WebGl2RenderingContext::BLEND);
        self.gl.blend_func_separate(
            web_sys::WebGl2RenderingContext::SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );
        self.gl.use_program(Some(&self.primitive_program));
        self.gl.bind_buffer(
            web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
            Some(&self.primitive_buffer),
        );
        let position = self
            .gl
            .get_attrib_location(&self.primitive_program, "position");
        let color = self
            .gl
            .get_attrib_location(&self.primitive_program, "color");
        if position >= 0 {
            self.gl.enable_vertex_attrib_array(position as u32);
            self.gl.vertex_attrib_pointer_with_i32(
                position as u32,
                2,
                web_sys::WebGl2RenderingContext::FLOAT,
                false,
                0,
                0,
            );
        }
        for primitive in primitives {
            self.gl.blend_func_separate(web_sys::WebGl2RenderingContext::SRC_ALPHA,
                if primitive.additive { web_sys::WebGl2RenderingContext::ONE } else { web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA },
                web_sys::WebGl2RenderingContext::ONE, web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA);
            if primitive.vertices.len() < 4 {
                continue;
            }
            let vertices = primitive
                .vertices
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let vertex_array = js_sys::Float32Array::from(vertices.as_slice());
            self.gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.primitive_buffer));
            self.gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &vertex_array,
                web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
            );
            let vertex_count = vertices.len() / 2;
            let colors = if primitive.vertex_colors.len() == vertex_count * 4 {
                primitive.vertex_colors.clone()
            } else {
                create_repeated_milkrust_colors(vertex_count, primitive.color)
            };
            let colors = colors.iter().map(|value| *value as f32).collect::<Vec<_>>();
            let color_array = js_sys::Float32Array::from(colors.as_slice());
            self.gl.bind_buffer(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                Some(&self.primitive_color_buffer),
            );
            self.gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &color_array,
                web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
            );
            if color >= 0 {
                self.gl.enable_vertex_attrib_array(color as u32);
                self.gl.vertex_attrib_pointer_with_i32(
                    color as u32,
                    4,
                    web_sys::WebGl2RenderingContext::FLOAT,
                    false,
                    0,
                    0,
                );
            }
            self.gl.uniform1f(
                self.u_primitive_point_size.as_ref(),
                if primitive.mode == MilkRustPrimitiveMode::Points {
                    2.0
                } else {
                    1.0
                },
            );
            let mode = match primitive.mode {
                MilkRustPrimitiveMode::LineStrip => web_sys::WebGl2RenderingContext::LINE_STRIP,
                MilkRustPrimitiveMode::Lines => web_sys::WebGl2RenderingContext::LINES,
                MilkRustPrimitiveMode::Points => web_sys::WebGl2RenderingContext::POINTS,
                MilkRustPrimitiveMode::TriangleFan => {
                    web_sys::WebGl2RenderingContext::TRIANGLE_FAN
                }
                MilkRustPrimitiveMode::Triangles => web_sys::WebGl2RenderingContext::TRIANGLES,
            };
            self.gl.draw_arrays(mode, 0, vertex_count as i32);
        }
        self.gl.disable(web_sys::WebGl2RenderingContext::BLEND);
        self.gl.use_program(Some(&self.program));
    }

    fn draw_sprite(&self, sprite: &milkrust_core::sprites::Frame) -> Result<(), JsValue> {
        let texture = self.named_texture_for_at(&sprite.texture, sprite.time)?.ok_or_else(|| JsValue::from_str("Sprite image is still loading"))?;
        let dimensions = {
            let textures = self.named_textures.borrow();
            get_milkrust_texture_name_aliases(&sprite.texture).iter().find_map(|alias| textures.get(alias))
                .map(|asset| asset.dimensions.get())
                .ok_or_else(|| JsValue::from_str("Sprite image dimensions are unavailable"))?
        };
        let mut pass = self.sprite_pass.borrow_mut();
        if pass.is_none() { *pass = Some(sprites::SpritePass::new(self.gl.clone())?); }
        pass.as_mut().unwrap().draw(sprite, &texture, dimensions)
    }

    fn prepare_textures(&self, frame: &MilkRustFrame) -> Result<bool, JsValue> {
        let mut ready = true;
        for name in frame.shader_stages.warp.texture_samplers.iter()
            .chain(&frame.shader_stages.composite.texture_samplers)
            .chain(frame.textured_primitives.iter().map(|primitive| &primitive.texture_name)) {
            if name.is_empty() || milkrust_core::resources::texture_kind(name).is_builtin() { continue; }
            if self.named_texture_for(name)?.is_none() { ready = false; }
        }
        Ok(ready)
    }

    fn named_texture_for(&self, raw_name: &str) -> Result<Option<web_sys::WebGlTexture>, JsValue> {
        self.named_texture_for_at(raw_name, 0.0)
    }

    fn named_texture_for_at(&self, raw_name: &str, seconds: f64) -> Result<Option<web_sys::WebGlTexture>, JsValue> {
        if raw_name.is_empty() { return Ok(Some(self.placeholder_texture.clone())); }
        for alias in get_milkrust_texture_name_aliases(raw_name) {
            if let Some(asset) = self.named_textures.borrow().get(&alias) {
                return match asset.state.borrow().as_ref() {
                    None => Ok(None),
                    Some(Ok(())) => Ok(Some(asset.texture_at(seconds))),
                    Some(Err(error)) => Err(JsValue::from_str(&format!("Texture {raw_name}: {error}"))),
                };
            }
            let data_url = self.texture_assets.borrow().get(&alias).cloned();
            let Some(data_url) = data_url else {
                continue;
            };
            if self.named_textures.borrow().len() >= 100 {
                return Err(JsValue::from_str("Preset exceeds the 100-texture cache limit"));
            }
            let asset = create_milkrust_texture_from_data_url(&self.gl, &data_url, self.texture_pixels.clone())?;
            self.named_textures
                .borrow_mut()
                .insert(alias, asset);
            return Ok(None);
        }
        Err(JsValue::from_str(&format!("Missing texture: {raw_name}")))
    }

    fn draw_textured_primitives(&self, primitives: &[MilkRustTexturedPrimitive], previous_frame: &web_sys::WebGlTexture, time: f64) -> Result<(), JsValue> {
        if primitives.is_empty() {
            return Ok(());
        }
        self.gl.enable(web_sys::WebGl2RenderingContext::BLEND);
        self.gl.blend_func_separate(
            web_sys::WebGl2RenderingContext::SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );
        self.gl.use_program(Some(&self.textured_program));
        self.gl
            .active_texture(web_sys::WebGl2RenderingContext::TEXTURE1);
        self.gl.uniform1i(self.u_textured_sampler.as_ref(), 1);
        self.gl.bind_sampler(1, None);
        let position = self
            .gl
            .get_attrib_location(&self.textured_program, "position");
        let source_uv = self
            .gl
            .get_attrib_location(&self.textured_program, "sourceUv");
        for primitive in primitives {
            self.gl.blend_func_separate(web_sys::WebGl2RenderingContext::SRC_ALPHA,
                if primitive.additive { web_sys::WebGl2RenderingContext::ONE } else { web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA },
                web_sys::WebGl2RenderingContext::ONE, web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA);
            if primitive.vertices.len() < 6 || primitive.vertices.len() != primitive.uvs.len() {
                continue;
            }
            self.gl
                .active_texture(web_sys::WebGl2RenderingContext::TEXTURE1);
            let texture = if primitive.texture_name.is_empty() { previous_frame.clone() } else {
                self.named_texture_for_at(&primitive.texture_name, time)?
                    .ok_or_else(|| JsValue::from_str("Texture became unavailable before drawing"))?
            };
            self.gl
                .bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
            let vertices = primitive
                .vertices
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let uvs = primitive
                .uvs
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>();
            let vertex_array = js_sys::Float32Array::from(vertices.as_slice());
            let uv_array = js_sys::Float32Array::from(uvs.as_slice());
            self.gl.bind_buffer(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                Some(&self.textured_position_buffer),
            );
            self.gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &vertex_array,
                web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
            );
            if position >= 0 {
                self.gl.enable_vertex_attrib_array(position as u32);
                self.gl.vertex_attrib_pointer_with_i32(
                    position as u32,
                    2,
                    web_sys::WebGl2RenderingContext::FLOAT,
                    false,
                    0,
                    0,
                );
            }
            self.gl.bind_buffer(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                Some(&self.textured_uv_buffer),
            );
            self.gl.buffer_data_with_array_buffer_view(
                web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                &uv_array,
                web_sys::WebGl2RenderingContext::DYNAMIC_DRAW,
            );
            if source_uv >= 0 {
                self.gl.enable_vertex_attrib_array(source_uv as u32);
                self.gl.vertex_attrib_pointer_with_i32(
                    source_uv as u32,
                    2,
                    web_sys::WebGl2RenderingContext::FLOAT,
                    false,
                    0,
                    0,
                );
            }
            let color = self.gl.get_attrib_location(&self.textured_program, "color");
            if color >= 0 {
                let rgba = if primitive.vertex_colors.len() == vertices.len() * 2 { primitive.vertex_colors.clone() }
                    else { create_repeated_milkrust_colors(vertices.len()/2, primitive.color) };
                let rgba = rgba.iter().map(|v| *v as f32).collect::<Vec<_>>();
                self.gl.bind_buffer(web_sys::WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.primitive_color_buffer));
                self.gl.buffer_data_with_array_buffer_view(web_sys::WebGl2RenderingContext::ARRAY_BUFFER,
                    &js_sys::Float32Array::from(rgba.as_slice()), web_sys::WebGl2RenderingContext::DYNAMIC_DRAW);
                self.gl.enable_vertex_attrib_array(color as u32);
                self.gl.vertex_attrib_pointer_with_i32(color as u32, 4, web_sys::WebGl2RenderingContext::FLOAT, false, 0, 0);
            }
            let draw_mode = match primitive.mode {
                MilkRustTexturedPrimitiveMode::Quad
                | MilkRustTexturedPrimitiveMode::TriangleFan => {
                    web_sys::WebGl2RenderingContext::TRIANGLE_FAN
                }
            };
            self.gl
                .draw_arrays(draw_mode, 0, (vertices.len() / 2) as i32);
        }
        self.gl
            .active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        self.gl.disable(web_sys::WebGl2RenderingContext::BLEND);
        self.gl.use_program(Some(&self.program));
        Ok(())
    }
}

impl Drop for MilkRustWebGlRenderer {
    fn drop(&mut self) {
        delete_milkrust_feedback_targets(&self.gl, self.feedback_targets.get_mut());
        for buffer in [&self.buffer, &self.primitive_buffer, &self.primitive_color_buffer,
            &self.textured_position_buffer, &self.textured_uv_buffer, &self.warp_position_buffer, &self.warp_uv_buffer] {
            self.gl.delete_buffer(Some(buffer));
        }
        for program in [&self.program, &self.primitive_program, &self.textured_program, &self.warp_program] {
            self.gl.delete_program(Some(program));
        }
        for cache in [&mut self.translated_warp_program, &mut self.translated_composite_program] {
            if let Some(program) = cache.get_mut().take() { self.gl.delete_program(Some(&program.program)); }
        }
        self.gl.delete_texture(Some(&self.placeholder_texture));
        self.gl.delete_texture(Some(&self.transition_placeholder_texture));
        if let Some((_, texture)) = self.transition_mask.get_mut().take() { self.gl.delete_texture(Some(&texture)); }
        self.named_textures.get_mut().clear();
    }
}

fn delete_milkrust_feedback_targets(gl: &web_sys::WebGl2RenderingContext, targets: &MilkRustFeedbackTargets) {
    for target in &targets.targets {
        gl.delete_framebuffer(Some(&target.framebuffer));
        gl.delete_texture(Some(&target.texture));
    }
}

fn milkrust_webgl_composite_blend_factors(mode: &str) -> (u32, u32) {
    match mode {
        "additive" => (
            web_sys::WebGl2RenderingContext::SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE,
        ),
        "screen" => (
            web_sys::WebGl2RenderingContext::ONE,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_COLOR,
        ),
        "multiply" => (
            web_sys::WebGl2RenderingContext::DST_COLOR,
            web_sys::WebGl2RenderingContext::ZERO,
        ),
        _ => (
            web_sys::WebGl2RenderingContext::SRC_ALPHA,
            web_sys::WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        ),
    }
}

fn create_milkrust_feedback_targets(
    gl: &web_sys::WebGl2RenderingContext,
    width: i32,
    height: i32,
) -> Result<MilkRustFeedbackTargets, JsValue> {
    let width = width.max(1);
    let height = height.max(1);
    if width > 4096 || height > 4096 || i64::from(width) * i64::from(height) > 8 * 1024 * 1024 {
        return Err(JsValue::from_str("Render targets exceed 4096 pixels per side or 8 megapixels"));
    }
    let first = create_milkrust_feedback_target(gl, width, height)?;
    let second = match create_milkrust_feedback_target(gl, width, height) {
        Ok(target) => target,
        Err(error) => {
            gl.delete_framebuffer(Some(&first.framebuffer));
            gl.delete_texture(Some(&first.texture));
            return Err(error);
        }
    };
    Ok(MilkRustFeedbackTargets {
        height,
        read_index: 0,
        targets: [first, second],
        width,
    })
}

fn create_milkrust_feedback_target(
    gl: &web_sys::WebGl2RenderingContext,
    width: i32,
    height: i32,
) -> Result<MilkRustFeedbackTarget, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("WebGL feedback texture allocation failed"))?;
    gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER,
        web_sys::WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER,
        web_sys::WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_S,
        web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_T,
        web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        0,
        web_sys::WebGl2RenderingContext::RGBA as i32,
        width,
        height,
        0,
        web_sys::WebGl2RenderingContext::RGBA,
        web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
        None,
    )?;

    let framebuffer = gl
        .create_framebuffer()
        .ok_or_else(|| JsValue::from_str("WebGL feedback framebuffer allocation failed"))?;
    gl.bind_framebuffer(
        web_sys::WebGl2RenderingContext::FRAMEBUFFER,
        Some(&framebuffer),
    );
    gl.framebuffer_texture_2d(
        web_sys::WebGl2RenderingContext::FRAMEBUFFER,
        web_sys::WebGl2RenderingContext::COLOR_ATTACHMENT0,
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        Some(&texture),
        0,
    );
    let status = gl.check_framebuffer_status(web_sys::WebGl2RenderingContext::FRAMEBUFFER);
    if status != web_sys::WebGl2RenderingContext::FRAMEBUFFER_COMPLETE {
        gl.delete_framebuffer(Some(&framebuffer));
        gl.delete_texture(Some(&texture));
        return Err(JsValue::from_str(
            "WebGL feedback framebuffer is incomplete",
        ));
    }
    gl.viewport(0, 0, width, height);
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);

    Ok(MilkRustFeedbackTarget {
        framebuffer,
        texture,
    })
}

fn create_milkrust_placeholder_texture(
    gl: &web_sys::WebGl2RenderingContext,
) -> Result<web_sys::WebGlTexture, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("WebGL placeholder texture allocation failed"))?;
    let pixels = [0u8, 0, 0, 255];
    gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER,
        web_sys::WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER,
        web_sys::WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_S,
        web_sys::WebGl2RenderingContext::REPEAT as i32,
    );
    gl.tex_parameteri(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_T,
        web_sys::WebGl2RenderingContext::REPEAT as i32,
    );
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        0,
        web_sys::WebGl2RenderingContext::RGBA as i32,
        1,
        1,
        0,
        web_sys::WebGl2RenderingContext::RGBA,
        web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
        Some(&pixels),
    )?;
    Ok(texture)
}

fn create_milkrust_transition_placeholder_texture(
    gl: &web_sys::WebGl2RenderingContext,
) -> Result<web_sys::WebGlTexture, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("WebGL transition placeholder allocation failed"))?;
    let pixels = [0u8; 4];
    gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
    gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MIN_FILTER, web_sys::WebGl2RenderingContext::NEAREST as i32);
    gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_MAG_FILTER, web_sys::WebGl2RenderingContext::NEAREST as i32);
    gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_S, web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(web_sys::WebGl2RenderingContext::TEXTURE_2D,
        web_sys::WebGl2RenderingContext::TEXTURE_WRAP_T, web_sys::WebGl2RenderingContext::CLAMP_TO_EDGE as i32);
    if let Err(error) = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        web_sys::WebGl2RenderingContext::TEXTURE_2D, 0, 0x8D7C, 1, 1, 0, 0x8D99,
        web_sys::WebGl2RenderingContext::UNSIGNED_BYTE, Some(&pixels)) {
        gl.delete_texture(Some(&texture));
        return Err(error);
    }
    if gl.get_error() != web_sys::WebGl2RenderingContext::NO_ERROR {
        gl.delete_texture(Some(&texture));
        return Err(JsValue::from_str("WebGL transition placeholder upload failed"));
    }
    Ok(texture)
}

fn create_image_decoder_worker() -> Result<(web_sys::Worker, String), JsValue> {
    if let Some(url) = crate::TEXTURE_DECODER_WORKER_URL.with(|value| value.borrow().clone()) {
        if let Ok(worker) = web_sys::Worker::new(&url) {
            return Ok((worker, url));
        }
        // A same-origin URL can still be rejected by worker-src or fail to
        // load. The fallback remains subject to the page's blob: CSP policy.
    }

    // Direct wasm-bindgen users can omit the static package URL; keep a bounded
    // blob-worker fallback for that integration style. The web SDK sets the
    // same-origin static worker URL when possible so strict CSPs can avoid `blob:`.
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(include_str!("../texture_decoder_worker.js")));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    match web_sys::Worker::new(&url) {
        Ok(worker) => Ok((worker, url)),
        Err(error) => {
            let _ = web_sys::Url::revoke_object_url(&url);
            Err(error)
        }
    }
}

fn revoke_image_decoder_worker_url(url: &str) {
    if url.starts_with("blob:") { let _ = web_sys::Url::revoke_object_url(url); }
}

fn create_milkrust_texture_from_data_url(
    gl: &web_sys::WebGl2RenderingContext,
    data_url: &str,
    total_pixels: Rc<Cell<u64>>,
) -> Result<MilkRustTextureAsset, JsValue> {
    use milkrust_core::texture::{self, Format};
    let source = texture::from_data_url(data_url).map_err(|e| JsValue::from_str(&e))?;
    let (width, height) = (source.width, source.height);
    let pixels = source.decoded_pixel_count().map_err(|e| JsValue::from_str(&e))?;
    if total_pixels.get() + pixels > 32 * 1024 * 1024 {
        return Err(JsValue::from_str("Preset exceeds its 32-megapixel decoded texture budget"));
    }
    let texture = create_milkrust_placeholder_texture(gl)?;
    // Reserve before asynchronous decoding: concurrent onload callbacks cannot
    // each admit an image against the same unused allocation budget.
    total_pixels.set(total_pixels.get() + pixels);
    let mut asset = MilkRustTextureAsset {
        gl: gl.clone(), texture, additional_frames: Vec::new(), delays_seconds: Vec::new(), repeat_count: None,
        dimensions: Rc::new(Cell::new((width, height))),
        state: Rc::new(RefCell::new(None)), total_pixels, reserved_pixels: pixels,
        worker: None, worker_url: None, worker_timeout_id: None,
        _onmessage: None, _onerror: None, _timeout: None,
    };
    if matches!(source.format, Format::Gif) {
        let animation = source.decode_gif().map_err(|e| JsValue::from_str(&e))?;
        if animation.decoded_pixels() != pixels {
            return Err(JsValue::from_str("GIF frame count changed after pixel reservation"));
        }
        for (index, frame) in animation.frames.iter().enumerate() {
            let texture = if index == 0 { asset.texture.clone() } else { create_milkrust_placeholder_texture(gl)? };
            let stride = width as usize * 4;
            let mut flipped = Vec::with_capacity(frame.pixels.len());
            for row in frame.pixels.chunks_exact(stride).rev() { flipped.extend_from_slice(row); }
            gl.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
            gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
            gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_ALIGNMENT, 1);
            let upload = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                web_sys::WebGl2RenderingContext::TEXTURE_2D, 0,
                web_sys::WebGl2RenderingContext::RGBA as i32, width as i32, height as i32, 0,
                web_sys::WebGl2RenderingContext::RGBA, web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
                Some(&flipped),
            );
            gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_ALIGNMENT, 4);
            if let Err(error) = upload {
                if index != 0 { gl.delete_texture(Some(&texture)); }
                return Err(error);
            }
            gl.generate_mipmap(web_sys::WebGl2RenderingContext::TEXTURE_2D);
            if gl.get_error() != web_sys::WebGl2RenderingContext::NO_ERROR {
                if index != 0 { gl.delete_texture(Some(&texture)); }
                return Err(JsValue::from_str("Animated GIF texture upload failed"));
            }
            if index != 0 { asset.additional_frames.push(texture); }
        }
        asset.delays_seconds = animation.delays_seconds;
        asset.repeat_count = animation.repeat_count;
        *asset.state.borrow_mut() = Some(Ok(()));
        return Ok(asset);
    }
    if matches!(source.format, Format::Dds | Format::Tga) {
        let decoded = source.decode_legacy().map_err(|e| JsValue::from_str(&e))?;
        let stride = width as usize * 4;
        let mut flipped = Vec::with_capacity(decoded.pixels.len());
        for row in decoded.pixels.chunks_exact(stride).rev() { flipped.extend_from_slice(row); }
        gl.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&asset.texture));
        gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_ALIGNMENT, 1);
        gl.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 0);
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            web_sys::WebGl2RenderingContext::TEXTURE_2D, 0,
            web_sys::WebGl2RenderingContext::RGBA as i32, width as i32, height as i32, 0,
            web_sys::WebGl2RenderingContext::RGBA, web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
            Some(&flipped),
        )?;
        if gl.get_error() != web_sys::WebGl2RenderingContext::NO_ERROR {
            return Err(JsValue::from_str("Legacy texture GPU upload failed"));
        }
        *asset.state.borrow_mut() = Some(Ok(()));
        return Ok(asset);
    }

    // Browser codecs run inside a disposable Worker. A malformed compressed
    // image may stall JS or allocate unexpectedly; terminating the worker
    // bounds renderer-thread waiting, but cannot contain a browser-process OOM.
    let (worker, worker_url) = create_image_decoder_worker()?;
    asset.worker = Some(worker.clone());
    asset.worker_url = Some(worker_url.clone());

    let gl_for_load = gl.clone();
    let texture_for_load = asset.texture.clone();
    let dimensions_for_load = asset.dimensions.clone();
    let state_for_load = asset.state.clone();
    let worker_for_load = worker.clone();
    let url_for_load = worker_url.clone();
    let timeout_for_load = Rc::new(Cell::new(None));
    let timeout_id_for_load = timeout_for_load.clone();
    let onmessage = Closure::<dyn FnMut(web_sys::MessageEvent)>::wrap(Box::new(move |event| {
        if let Some(timeout_id) = timeout_id_for_load.get() {
            if let Some(window) = web_sys::window() { window.clear_timeout_with_handle(timeout_id); }
        }
        worker_for_load.set_onmessage(None);
        worker_for_load.set_onerror(None);
        worker_for_load.terminate();
        revoke_image_decoder_worker_url(&url_for_load);

        // An image decode can finish between context restoration and the next
        // frame's rebuild. Never bind an object from the retired GPU generation.
        if gl_for_load.is_context_lost() || !gl_for_load.is_texture(Some(&texture_for_load)) {
            *state_for_load.borrow_mut() = Some(Err("Texture upload belongs to a lost or retired graphics context".into()));
            if let Ok(value) = js_sys::Reflect::get(&event.data(), &JsValue::from_str("bitmap")) {
                if let Ok(bitmap) = value.dyn_into::<web_sys::ImageBitmap>() { bitmap.close(); }
            }
            return;
        }
        let data = event.data();
        let decode_error = js_sys::Reflect::get(&data, &JsValue::from_str("error"))
            .ok().and_then(|value| value.as_string());
        if let Some(error) = decode_error {
            *state_for_load.borrow_mut() = Some(Err(error));
            return;
        }
        let bitmap = match js_sys::Reflect::get(&data, &JsValue::from_str("bitmap"))
            .ok().and_then(|value| value.dyn_into::<web_sys::ImageBitmap>().ok()) {
            Some(bitmap) => bitmap,
            None => {
                *state_for_load.borrow_mut() = Some(Err("Image decoder returned no bitmap".into()));
                return;
            }
        };
        // EXIF orientation can swap dimensions without changing the pixel budget.
        let actual = (bitmap.width(), bitmap.height());
        if actual != (width, height) && actual != (height, width) {
            *state_for_load.borrow_mut() = Some(Err("Decoded dimensions disagree with admitted texture header".into()));
            bitmap.close();
            return;
        }
        gl_for_load.active_texture(web_sys::WebGl2RenderingContext::TEXTURE0);
        gl_for_load.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(&texture_for_load));
        gl_for_load.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 0);
        let result = gl_for_load.tex_image_2d_with_u32_and_u32_and_image_bitmap(
            web_sys::WebGl2RenderingContext::TEXTURE_2D, 0,
            web_sys::WebGl2RenderingContext::RGBA as i32, web_sys::WebGl2RenderingContext::RGBA,
            web_sys::WebGl2RenderingContext::UNSIGNED_BYTE, &bitmap,
        );
        gl_for_load.pixel_storei(web_sys::WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 0);
        bitmap.close();
        let result = result.map_err(|error| error.as_string().unwrap_or_else(|| "GPU upload failed".into()))
            .and_then(|()| if gl_for_load.get_error() == web_sys::WebGl2RenderingContext::NO_ERROR { Ok(()) }
                else { Err("GPU texture upload failed".into()) });
        if result.is_ok() { dimensions_for_load.set(actual); }
        *state_for_load.borrow_mut() = Some(result);
    }));
    let state_for_error = asset.state.clone();
    let worker_for_error = worker.clone();
    let url_for_error = worker_url.clone();
    let timeout_id_for_error = timeout_for_load.clone();
    let onerror = Closure::<dyn FnMut(web_sys::ErrorEvent)>::wrap(Box::new(move |_event| {
        if let Some(timeout_id) = timeout_id_for_error.get() {
            if let Some(window) = web_sys::window() { window.clear_timeout_with_handle(timeout_id); }
        }
        worker_for_error.set_onmessage(None);
        worker_for_error.set_onerror(None);
        worker_for_error.terminate();
        revoke_image_decoder_worker_url(&url_for_error);
        *state_for_error.borrow_mut() = Some(Err("Image decoder worker failed".into()));
    }));
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

    let timeout_worker = worker.clone();
    let timeout_url = worker_url;
    let timeout_state = asset.state.clone();
    let timeout_id = timeout_for_load.clone();
    let timeout = Closure::<dyn FnMut()>::wrap(Box::new(move || {
        timeout_worker.set_onmessage(None);
        timeout_worker.set_onerror(None);
        timeout_worker.terminate();
        revoke_image_decoder_worker_url(&timeout_url);
        *timeout_state.borrow_mut() = Some(Err("Image decode timed out".into()));
        timeout_id.set(None);
    }));
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Browser window is unavailable"))?;
    let timeout_handle = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        timeout.as_ref().unchecked_ref(), 8000,
    )?;
    timeout_for_load.set(Some(timeout_handle));
    asset.worker_timeout_id = Some(timeout_handle);
    asset._onmessage = Some(onmessage);
    asset._onerror = Some(onerror);
    asset._timeout = Some(timeout);
    let mime = match source.format {
        Format::Png => "image/png", Format::Jpeg => "image/jpeg", Format::Gif => "image/gif",
        Format::Bmp => "image/bmp", Format::WebP => "image/webp",
        Format::Dds | Format::Tga => return Err(JsValue::from_str("Legacy images require the bounded Rust decoder")),
    };
    let encoded = js_sys::Uint8Array::from(source.bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&encoded);
    let blob_options = web_sys::BlobPropertyBag::new();
    blob_options.set_type(mime);
    let image_blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &blob_options)?;
    let request = js_sys::Object::new();
    js_sys::Reflect::set(request.as_ref(), &JsValue::from_str("blob"), image_blob.as_ref())?;
    js_sys::Reflect::set(request.as_ref(), &JsValue::from_str("width"), &JsValue::from_f64(f64::from(width)))?;
    js_sys::Reflect::set(request.as_ref(), &JsValue::from_str("height"), &JsValue::from_f64(f64::from(height)))?;
    worker.post_message(request.as_ref())?;
    Ok(asset)
}

fn compile_milkrust_shader(
    gl: &web_sys::WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<web_sys::WebGlShader, JsValue> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| JsValue::from_str("WebGL shader allocation failed"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, web_sys::WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let error = gl.get_shader_info_log(&shader).unwrap_or_else(|| "MilkRust WebGL shader compile failed".to_string());
        gl.delete_shader(Some(&shader));
        Err(JsValue::from_str(&error))
    }
}

fn link_milkrust_program(
    gl: &web_sys::WebGl2RenderingContext,
    vertex_shader: &web_sys::WebGlShader,
    fragment_shader: &web_sys::WebGlShader,
) -> Result<web_sys::WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("WebGL program allocation failed"))?;
    gl.attach_shader(&program, vertex_shader);
    gl.attach_shader(&program, fragment_shader);
    gl.link_program(&program);
    gl.detach_shader(&program, vertex_shader);
    gl.detach_shader(&program, fragment_shader);
    gl.delete_shader(Some(vertex_shader));
    gl.delete_shader(Some(fragment_shader));
    if gl
        .get_program_parameter(&program, web_sys::WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        let error = gl.get_program_info_log(&program).unwrap_or_else(|| "MilkRust WebGL program link failed".to_string());
        gl.delete_program(Some(&program));
        Err(JsValue::from_str(&error))
    }
}

const MILKRUST_WEBGL_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
out vec2 v_uv;
void main() {
  v_uv = position * 0.5 + vec2(0.5);
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_TRANSLATED_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
uniform vec4 motion;
uniform float isWarp;
out vec2 uv;
out highp vec2 originalUv;
void main() {
  originalUv = position * 0.5 + vec2(0.5);
  uv = position * 0.5 + vec2(0.5);
  if (isWarp > 0.5) {
    float c = cos(motion.x), s = sin(motion.x);
    uv = mat2(c, -s, s, c) * (uv - vec2(0.5) - motion.zw) / max(motion.y, 0.001) + vec2(0.5);
  }
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_TRANSLATED_MESH_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
in vec2 sourceUv;
out vec2 uv;
out highp vec2 originalUv;
void main() {
  originalUv = position * 0.5 + vec2(0.5);
  uv = sourceUv;
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_WEBGL_FRAGMENT_BODY: &str = r#"
uniform vec4 u_motion;
uniform sampler2D u_previousFrame;
uniform float u_feedback;
uniform float u_displayOnly;
uniform float u_outputAlpha;
in vec2 v_uv;
out vec4 outColor;
void main() {
  vec2 centered = (v_uv - vec2(0.5)) / max(u_motion.y, 0.001);
  float s = sin(u_motion.x), c = cos(u_motion.x);
  vec2 feedbackUv = vec2(c*centered.x-s*centered.y, s*centered.x+c*centered.y) + vec2(0.5) + u_motion.zw;
  vec2 sampleUv = u_displayOnly > 0.5 ? v_uv : feedbackUv;
  float retention = u_displayOnly > 0.5 ? 1.0 : clamp(u_feedback, 0.0, 1.0);
  vec2 boundedUv = classicWrap > 0.5 ? fract(sampleUv) : clamp(sampleUv, 0.0, 1.0);
  vec3 color = u_displayOnly > 0.5 ? milkrust_classic_display(u_previousFrame, v_uv) : texture(u_previousFrame, boundedUv).rgb * retention;
  outColor = vec4(color, u_outputAlpha);
  outColor.a *= milkrust_transition_mask(v_uv);
}
"#;

const MILKRUST_PRIMITIVE_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
in vec4 color;
uniform float u_primitivePointSize;
out vec4 v_color;
void main() {
  v_color = color;
  gl_PointSize = u_primitivePointSize;
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_PRIMITIVE_FRAGMENT_SHADER: &str = r#"#version 300 es
precision highp float;
in vec4 v_color;
out vec4 outColor;
void main() {
  outColor = v_color;
}
"#;

const MILKRUST_TEXTURED_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
in vec2 sourceUv;
in vec4 color;
out vec4 v_color;
out vec2 v_uv;
void main() {
  v_uv = sourceUv;
  v_color = color;
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_TEXTURED_FRAGMENT_SHADER: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_texture;
in vec4 v_color;
in vec2 v_uv;
out vec4 outColor;
void main() {
  vec4 texel = texture(u_texture, v_uv);
  outColor = texel * v_color;
}
"#;

const MILKRUST_WARP_GRID_VERTEX_SHADER: &str = r#"#version 300 es
in vec2 position;
in vec2 sourceUv;
out vec2 v_sourceUv;
void main() {
  v_sourceUv = sourceUv;
  gl_Position = vec4(position, 0.0, 1.0);
}
"#;

const MILKRUST_WARP_GRID_FRAGMENT_SHADER: &str = r#"#version 300 es
precision highp float;
uniform vec3 u_color;
uniform float u_feedback;
uniform float classicWrap;
uniform float u_outputAlpha;
uniform sampler2D u_previousFrame;
in vec2 v_sourceUv;
out vec4 outColor;
void main() {
  vec3 previous = texture(u_previousFrame, (classicWrap > 0.5 ? fract(v_sourceUv) : clamp(v_sourceUv, vec2(0.0), vec2(1.0)))).rgb;
  vec3 base = previous * clamp(u_feedback, 0.0, 1.0);
  outColor = vec4(base, u_outputAlpha);
}
"#;

fn render_milkrust_canvas_frame(
    context: &web_sys::CanvasRenderingContext2d,
    canvas: &web_sys::HtmlCanvasElement,
    frame: &MilkRustFrame,
    time: f64,
) {
    let width = canvas.width() as f64;
    let height = canvas.height() as f64;
    context.set_fill_style_str(&format!("rgba(9, 13, 18, {:.3})", frame.background_alpha));
    context.fill_rect(0.0, 0.0, width, height);

    let (r, g, b) = frame.wave_color;
    let center_x = width * (0.5 + frame.dx);
    let center_y = height * (0.5 + frame.dy);
    let base = width.min(height) * frame.wave_radius * frame.zoom;
    context.save();
    let _ = context.translate(center_x, center_y);
    let _ = context.rotate(frame.rotation + time * 0.08);
    context.set_stroke_style_str(&format!("rgba({r}, {g}, {b}, 0.92)"));
    context.set_line_width(2.0);
    context.begin_path();
    for index in 0..192 {
        let unit = index as f64 / 192.0;
        let angle = unit * std::f64::consts::TAU;
        let radius = base
            * (0.72
                + 0.12 * (time * 1.9 + angle * 3.0).sin()
                + 0.08 * (time * 2.7 + angle * 7.0).cos());
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        if index == 0 {
            context.move_to(x, y);
        } else {
            context.line_to(x, y);
        }
    }
    context.close_path();
    context.stroke();

    context.set_fill_style_str(&format!("rgba({r}, {g}, {b}, 0.14)"));
    context.fill();

    context.set_stroke_style_str("rgba(255, 209, 102, 0.34)");
    context.set_line_width(1.0);
    for ring in 1..=4 {
        context.begin_path();
        let ring_radius = base * ring as f64 * 0.18 * (1.0 + 0.03 * (time + ring as f64).sin());
        let _ = context.arc(0.0, 0.0, ring_radius, 0.0, std::f64::consts::TAU);
        context.stroke();
    }
    for shape in 0..frame.shape_count.min(6) {
        let sides = 3 + (shape % 5);
        let radius = base * (0.16 + shape as f64 * 0.035);
        let spin = time * (0.4 + shape as f64 * 0.09);
        context.begin_path();
        for side in 0..=sides {
            let angle = spin + side as f64 / sides as f64 * std::f64::consts::TAU;
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            if side == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.set_stroke_style_str(&format!("rgba({r}, {g}, {b}, 0.42)"));
        context.stroke();
    }
    context.restore();

    context.set_stroke_style_str("rgba(216, 232, 225, 0.36)");
    context.set_line_width(1.5);
    for wave in 0..frame.waveform_count.clamp(1, 3) {
        context.begin_path();
        for index in 0..128 {
            let x = width * index as f64 / 127.0;
            let amp = (time * (5.0 + wave as f64) + index as f64 * 0.19).sin()
                * (0.18 + 0.08 * wave as f64 + 0.18 * (time * 2.0).sin().abs());
            let baseline = 0.78 - wave as f64 * 0.16;
            let y = height * (baseline + amp * 0.28);
            if index == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.stroke();
    }
    render_milkrust_canvas_primitives(context, width, height, frame);
    render_milkrust_canvas_textured_primitives(context, width, height, frame);
}

fn render_milkrust_canvas_overlay_frame(
    context: &web_sys::CanvasRenderingContext2d,
    canvas: &web_sys::HtmlCanvasElement,
    frame: &MilkRustFrame,
    time: f64,
) {
    let width = canvas.width() as f64;
    let height = canvas.height() as f64;
    let (r, g, b) = frame.wave_color;
    context.save();
    let _ = context.translate(width * (0.5 + frame.dx), height * (0.5 + frame.dy));
    let _ = context.rotate(frame.rotation + time * 0.05);
    context.set_stroke_style_str(&format!("rgba({r}, {g}, {b}, 0.68)"));
    context.set_line_width(1.5);
    let base = width.min(height) * frame.wave_radius * frame.zoom;
    for ring in 0..frame.shape_count.clamp(1, 5) {
        context.begin_path();
        let radius = base * (0.22 + ring as f64 * 0.11);
        let _ = context.arc(0.0, 0.0, radius, 0.0, std::f64::consts::TAU);
        context.stroke();
    }
    context.restore();
    render_milkrust_canvas_primitives(context, width, height, frame);
    render_milkrust_canvas_textured_primitives(context, width, height, frame);
}

fn milkrust_canvas_color(color: [f64; 4]) -> String {
    format!(
        "rgba({:.0}, {:.0}, {:.0}, {:.3})",
        clamp_unit(color[0]) * 255.0,
        clamp_unit(color[1]) * 255.0,
        clamp_unit(color[2]) * 255.0,
        clamp_unit(color[3])
    )
}

fn milkrust_clip_to_canvas(vertex: &[f64], width: f64, height: f64) -> (f64, f64) {
    (
        (vertex.first().copied().unwrap_or_default() * 0.5 + 0.5) * width,
        (1.0 - (vertex.get(1).copied().unwrap_or_default() * 0.5 + 0.5)) * height,
    )
}

fn render_milkrust_canvas_primitives(
    context: &web_sys::CanvasRenderingContext2d,
    width: f64,
    height: f64,
    frame: &MilkRustFrame,
) {
    for primitive in &frame.primitives {
        if primitive.vertices.len() < 4 {
            continue;
        }
        let color = milkrust_canvas_color(primitive.color);
        match primitive.mode {
            MilkRustPrimitiveMode::LineStrip => {
                context.begin_path();
                for (index, vertex) in primitive.vertices.chunks(2).enumerate() {
                    let (x, y) = milkrust_clip_to_canvas(vertex, width, height);
                    if index == 0 {
                        context.move_to(x, y);
                    } else {
                        context.line_to(x, y);
                    }
                }
                context.set_stroke_style_str(&color);
                context.set_line_width(1.5);
                context.stroke();
            }
            MilkRustPrimitiveMode::Lines => {
                context.set_stroke_style_str(&color);
                context.set_line_width(1.0);
                for line in primitive.vertices.chunks(4) {
                    if line.len() < 4 {
                        continue;
                    }
                    let (x1, y1) = milkrust_clip_to_canvas(&line[0..2], width, height);
                    let (x2, y2) = milkrust_clip_to_canvas(&line[2..4], width, height);
                    context.begin_path();
                    context.move_to(x1, y1);
                    context.line_to(x2, y2);
                    context.stroke();
                }
            }
            MilkRustPrimitiveMode::Points => {
                context.set_fill_style_str(&color);
                for vertex in primitive.vertices.chunks(2) {
                    let (x, y) = milkrust_clip_to_canvas(vertex, width, height);
                    context.begin_path();
                    let _ = context.arc(x, y, 2.0, 0.0, std::f64::consts::TAU);
                    context.fill();
                }
            }
            MilkRustPrimitiveMode::TriangleFan => {
                context.begin_path();
                for (index, vertex) in primitive.vertices.chunks(2).enumerate() {
                    let (x, y) = milkrust_clip_to_canvas(vertex, width, height);
                    if index == 0 {
                        context.move_to(x, y);
                    } else {
                        context.line_to(x, y);
                    }
                }
                context.close_path();
                context.set_fill_style_str(&color);
                context.fill();
            }
            MilkRustPrimitiveMode::Triangles => {
                context.set_fill_style_str(&color);
                for triangle in primitive.vertices.chunks(6) {
                    if triangle.len() < 6 {
                        continue;
                    }
                    context.begin_path();
                    let (x1, y1) = milkrust_clip_to_canvas(&triangle[0..2], width, height);
                    let (x2, y2) = milkrust_clip_to_canvas(&triangle[2..4], width, height);
                    let (x3, y3) = milkrust_clip_to_canvas(&triangle[4..6], width, height);
                    context.move_to(x1, y1);
                    context.line_to(x2, y2);
                    context.line_to(x3, y3);
                    context.close_path();
                    context.fill();
                }
            }
        }
    }
}

fn render_milkrust_canvas_textured_primitives(
    context: &web_sys::CanvasRenderingContext2d,
    width: f64,
    height: f64,
    frame: &MilkRustFrame,
) {
    for primitive in &frame.textured_primitives {
        if primitive.vertices.len() < 6 {
            continue;
        }
        context.begin_path();
        for (index, vertex) in primitive.vertices.chunks(2).enumerate() {
            let (x, y) = milkrust_clip_to_canvas(vertex, width, height);
            if index == 0 {
                context.move_to(x, y);
            } else {
                context.line_to(x, y);
            }
        }
        context.close_path();
        context.set_fill_style_str(&milkrust_canvas_color(primitive.color));
        context.fill();
        context.set_stroke_style_str("rgba(255, 255, 255, 0.22)");
        context.set_line_width(1.0);
        context.stroke();
    }
}
