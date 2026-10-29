//! Ordered post-warp scene drawing. Shape fills/outlines, custom waves, the
//! classic wave, sprites, center fade and borders retain their relative order.
use std::collections::BTreeMap;
use crate::{audio, waveform, MilkRustDrawCommand, MilkRustIndexedEntry, MilkRustPresetDocument,
    MilkRustPrimitive, MilkRustPrimitiveMode, MilkRustTexturedPrimitive, MilkRustTexturedPrimitiveMode,
    MilkRustValue, milkrust_scope_number as number, milkrust_entry_number as entry,
    milkrust_entry_flag as flag};

#[derive(Default)]
pub(crate) struct Scene {
    pub primitives: Vec<MilkRustPrimitive>,
    pub textures: Vec<MilkRustTexturedPrimitive>,
    pub order: Vec<MilkRustDrawCommand>,
    pub motion: Option<MilkRustPrimitive>,
    vertices: usize,
}

impl Scene {
    fn admit(&mut self, vertices: usize) -> bool {
        self.vertices = self.vertices.saturating_add(vertices);
        if self.vertices > 262_144 {
            milkrust_expr::limits::record_frame_error("Scene exceeds its aggregate vertex limit"); false
        } else { true }
    }
    fn colored(&mut self, primitive: MilkRustPrimitive) {
        if !self.admit(primitive.vertices.len() / 2) { return; }
        self.order.push(MilkRustDrawCommand::Primitive(self.primitives.len()));
        self.primitives.push(primitive);
    }
    fn textured(&mut self, primitive: MilkRustTexturedPrimitive) {
        if !self.admit(primitive.vertices.len() / 2) { return; }
        self.order.push(MilkRustDrawCommand::Textured(self.textures.len()));
        self.textures.push(primitive);
    }
    fn shape(&mut self, shape: &MilkRustIndexedEntry, scope: &BTreeMap<String, MilkRustValue>, rgb: [f64;3]) {
        if !flag(shape, &["enabled", "benabled"]) { return; }
        for key in ["x", "y", "rad", "ang", "tex_zoom", "tex_ang", "sides"] {
            if shape.base_values.get(key).and_then(MilkRustValue::as_number).is_some_and(|v| !v.is_finite() || v.abs() > 1.0e9) {
                milkrust_expr::limits::record_frame_error(&format!("Shape variable exceeds its finite limit: {key}")); return;
            }
        }
        let vertices = crate::create_milkrust_shape_fill_vertices(shape);
        let color = [entry(shape, &["r"], rgb[0]), entry(shape, &["g"], rgb[1]), entry(shape, &["b"], rgb[2]), entry(shape, &["a"], 1.0)].map(|v| v.clamp(0.0,1.0));
        let additive = flag(shape, &["additive", "badditive"]);
        let colors = crate::create_milkrust_shape_fill_colors(shape, rgb);
        if crate::is_milkrust_shape_textured(shape) {
            self.textured(MilkRustTexturedPrimitive { additive, color, vertex_colors: colors,
                mode: MilkRustTexturedPrimitiveMode::TriangleFan, texture_name: crate::milkrust_texture_name(shape),
                uvs: crate::create_milkrust_shape_texture_uvs(shape), vertices });
        } else {
            self.colored(MilkRustPrimitive { additive, color, mode: MilkRustPrimitiveMode::TriangleFan,
                vertex_colors: colors, vertices });
        }
        let alpha = entry(shape, &["border_a"], 0.0).clamp(0.0,1.0);
        if alpha > 0.0 {
            let color = [entry(shape, &["border_r", "r"], rgb[0]), entry(shape, &["border_g", "g"], rgb[1]),
                entry(shape, &["border_b", "b"], rgb[2]), alpha].map(|v| v.clamp(0.0,1.0));
            for primitive in waveform::stroke(crate::create_milkrust_shape_vertices(shape), Vec::new(), color,
                false, flag(shape, &["thick", "thickoutline"]), additive, false, scope) { self.colored(primitive); }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(preset: &mut MilkRustPresetDocument, scope: &mut BTreeMap<String, MilkRustValue>,
    samples: &[f64], spectrum: &[f64], stereo: Option<&audio::Stereo>, rgb: [f64; 3], mesh: Option<&crate::MilkRustWarpMesh>) -> Scene {
    let mut scene = Scene::default();
    let motion_alpha = number(scope, "mv_a", 0.0).clamp(0.0,1.0);
    let motion = crate::warp::motion_vectors(scope, mesh);
    if motion_alpha > 0.0 && motion.len() >= 4 {
        scene.vertices = motion.len() / 2;
        scene.motion = Some(MilkRustPrimitive { additive: false, color: [number(scope,"mv_r",1.0), number(scope,"mv_g",1.0),
            number(scope,"mv_b",1.0), motion_alpha].map(|v| v.clamp(0.0,1.0)),
            mode: MilkRustPrimitiveMode::Lines, vertex_colors: Vec::new(), vertices: motion });
    }
    for shape in &mut preset.shapes {
        if !flag(shape, &["enabled", "benabled"]) { continue; }
        let count = entry(shape, &["instances", "num_inst"], 1.0).floor().clamp(0.0,1024.0) as usize;
        for instance in 0..count {
            if scene.vertices > 262_144 { break; }
            let mut inputs = scope.clone();
            inputs.insert("instance".into(), MilkRustValue::Number(instance as f64));
            let evaluated = crate::evaluate_milkrust_entry_stateful(shape, &inputs, crate::MILKRUST_SHAPE_VALUE_KEYS);
            crate::merge_milkrust_global_memory(scope, &evaluated.runtime_values);
            scene.shape(&evaluated, scope, rgb);
        }
    }
    for wave in &mut preset.waves {
        if !flag(wave, &["enabled", "benabled"]) { continue; }
        let mut evaluated = crate::evaluate_milkrust_entry_stateful(wave, scope, crate::MILKRUST_WAVE_VALUE_KEYS);
        crate::merge_milkrust_global_memory(scope, &evaluated.runtime_values);
        for primitive in waveform::custom(&mut evaluated, samples, spectrum, stereo, scope, rgb) { scene.colored(primitive); }
        wave.point_values = evaluated.point_values;
    }
    for primitive in waveform::classic(samples, spectrum, stereo, scope, rgb) { scene.colored(primitive); }
    for sprite in &mut preset.sprites {
        let evaluated = crate::evaluate_milkrust_entry_stateful(sprite, scope, crate::MILKRUST_SPRITE_VALUE_KEYS);
        crate::merge_milkrust_global_memory(scope, &evaluated.runtime_values);
        let vertices = crate::create_milkrust_sprite_vertices(&evaluated);
        let uvs = crate::create_milkrust_sprite_texture_uvs(&evaluated);
        if vertices.len() >= 8 && vertices.len() == uvs.len() {
            scene.textured(MilkRustTexturedPrimitive { additive: flag(&evaluated, &["additive","badditive"]),
                color: [entry(&evaluated,&["r"],rgb[0]), entry(&evaluated,&["g"],rgb[1]), entry(&evaluated,&["b"],rgb[2]), entry(&evaluated,&["a"],1.0)].map(|v| v.clamp(0.0,1.0)),
                mode: MilkRustTexturedPrimitiveMode::Quad, vertex_colors: Vec::new(),
                texture_name: crate::milkrust_texture_name(&evaluated), uvs, vertices });
        }
    }
    if let Some(center) = crate::classic::darken_center(scope) { scene.colored(center); }
    for (prefix, inset) in [("ob",0.0), ("ib",number(scope,"ob_size",0.0).clamp(0.0,1.0)*2.0)] {
        let alpha = number(scope, &format!("{prefix}_a"), 0.0).clamp(0.0,1.0);
        let vertices = crate::create_milkrust_screen_border_vertices(number(scope,&format!("{prefix}_size"),0.0),inset);
        if alpha > 0.0 && vertices.len() >= 6 {
            let mut color = [0.0;4];
            for (i, channel) in ["r","g","b"].iter().enumerate() { color[i] = number(scope,&format!("{prefix}_{channel}"),rgb[i]).clamp(0.0,1.0); }
            color[3] = alpha;
            scene.colored(MilkRustPrimitive { additive: false, color, mode: MilkRustPrimitiveMode::Triangles,
                vertex_colors: Vec::new(), vertices });
        }
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapes_keep_instance_scopes_texture_gradient_and_draw_order() {
        let source = "[preset00]\nwave_a=0\nper_frame_1=q1=7;\nshapecode_0_enabled=1\nshapecode_0_num_inst=3\nshapecode_0_textured=1\nshapecode_0_r=1\nshapecode_0_r2=0\nshapecode_0_border_a=1\nshape_0_init1=t1=0.2;counter=0;\nshape_0_per_frame1=counter+=1;x=t1+instance*0.2;y=0.25;q1=99;\n";
        let mut runtime = crate::MilkRustFrameSetRuntime::default();
        runtime.set_viewport(200,100).unwrap();
        let set = runtime.try_render_source_with_audio_and_input(source,0.0,0.0,0.0,0.0,&[0.0;512],&[],crate::MilkRustInputState::default()).unwrap();
        let frame = &set.entries[0].frame;
        assert_eq!(frame.textured_primitives.len(), 3);
        assert_eq!(frame.q_registers[0], 7.0);
        assert_eq!(frame.draw_order[..2], [MilkRustDrawCommand::Textured(0),MilkRustDrawCommand::Primitive(0)]);
        let first = &frame.textured_primitives[0];
        assert!((first.vertices[0]+0.6).abs()<1.0e-9);
        assert_eq!(first.vertices[1],0.5);
        assert_eq!(first.vertex_colors[0],1.0); assert_eq!(first.vertex_colors[4],0.0);
        let shape = &runtime.preset_documents[0].shapes[0];
        assert_eq!(shape.runtime_values["counter"], MilkRustValue::Number(3.0));
        assert!(!shape.base_values.contains_key("counter"));
    }
    #[test]
    fn custom_points_receive_current_spectrum_and_viewport_then_clear_silence() {
        let source="[preset00]\nwave_a=0\nwavecode_0_enabled=1\nwavecode_0_samples=2\nwavecode_0_bUseDots=1\nwave_0_per_point1=x=sample*pixelsx/canvas_width;y=0.5;a=get_fft(0.5);\n";
        let mut runtime=crate::MilkRustFrameSetRuntime::default();
        runtime.set_viewport(320,180).unwrap();
        let live=runtime.try_render_source_with_audio_and_input(source,0.0,0.0,0.0,0.0,&[0.0;512],&[0.4;256],crate::MilkRustInputState::default()).unwrap();
        assert!((live.entries[0].frame.primitives[0].vertex_colors[3]-0.4).abs()<1.0e-9);
        let silent=runtime.try_render_source_with_audio_and_input(source,1.0,0.0,0.0,0.0,&[],&[],crate::MilkRustInputState::default()).unwrap();
        assert_eq!(silent.entries[0].frame.primitives[0].vertex_colors[3],0.0);
    }
    #[test]
    fn point_temporaries_survive_frames_without_mutating_frame_vm() {
        let source="[preset00]\nwave_a=0\nwavecode_0_enabled=1\nwavecode_0_samples=2\nwave_0_per_point1=counter+=1;x=sample;y=0.5;gmegabuf(1)=counter;\n";
        let mut runtime=crate::MilkRustFrameSetRuntime::default();
        for (time, expected) in [(0.0,2.0),(1.0,4.0)] {
            runtime.try_render_source_with_audio_and_input(source,time,0.0,0.0,0.0,&[0.0;512],&[],crate::MilkRustInputState::default()).unwrap();
            let wave=&runtime.preset_documents[0].waves[0];
            assert_eq!(wave.point_values["counter"],MilkRustValue::Number(expected));
            assert!(!wave.runtime_values.contains_key("counter"));
            assert_eq!(runtime.scopes[0]["gmegabuf_1"],MilkRustValue::Number(expected));
        }
    }
    #[test]
    fn custom_init_temporaries_reset_while_private_variables_persist() {
        let mut parsed=crate::parse_milkrust_preset_set("[preset00]\nwavecode_0_enabled=1\nwave_0_init1=t1=4;counter=10;\nwave_0_per_frame1=t1+=1;counter+=1;r=counter/100;\n",false);
        let wave=&mut parsed.presets[0].waves[0];
        for expected in [11.0,12.0] {
            let evaluated=crate::evaluate_milkrust_entry_stateful(wave,&BTreeMap::new(),crate::MILKRUST_WAVE_VALUE_KEYS);
            assert_eq!(evaluated.runtime_values["counter"],MilkRustValue::Number(expected));
            assert_eq!(evaluated.runtime_values["t1"],MilkRustValue::Number(5.0));
        }
    }
}
