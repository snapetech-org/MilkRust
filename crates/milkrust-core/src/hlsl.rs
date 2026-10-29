//! MilkDrop shader environment for the typed legacy HLSL compiler.
use crate::milk2;
use std::collections::BTreeMap;

pub(crate) fn sampler_names(source: &str) -> Result<Vec<String>, String> {
    let source = milkrust_hlsl::preprocess(source)?;
    let tokens = milkrust_hlsl::tokenize(&source)?;
    let mut names = Vec::new();
    for name in &tokens {
        if name.starts_with("sampler_") && !crate::is_milkrust_main_sampler(&name) && !names.contains(name) {
            names.push(name.clone());
        }
    }
    for level in 1..=3 {
        let name = format!("sampler_blur{level}");
        if tokens.iter().any(|t| t == &format!("GetBlur{level}")) && !names.contains(&name) { names.push(name); }
    }
    if names.len() > 15 { return Err("Shader exceeds 15 additional samplers".into()); }
    Ok(names)
}

pub(crate) fn compile(source: &str, legacy_feedback: bool) -> Result<String, String> {
    if source.len() > 128 * 1024 { return Err("Shader source exceeds 128 KiB".into()); }
    let source = milkrust_hlsl::preprocess(source)?;
    let samplers = sampler_names(&source)?;
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    aliases.insert("tex".into(), "tex2D".into());
    aliases.insert("aspect".into(), "hlslAspect".into());
    for name in ["sampler_main", "sampler_fc_main", "sampler_sampler_main"] {
        aliases.insert(name.into(), "previousFrame".into());
    }
    for (index, name) in samplers.iter().enumerate() { aliases.insert(name.clone(), format!("shaderTexture{index}")); }
    // GLSL spellings accepted by the original expression path are also valid
    // in snippets. Half/minimum precision is explicitly widened to f32.
    for (from, to) in [("vec2", "float2"), ("vec3", "float3"), ("vec4", "float4"),
        ("float1", "float"), ("half1", "float"), ("min16float", "float"),
        ("min16float2", "float2"), ("min16float3", "float3"), ("min16float4", "float4")] {
        aliases.insert(from.into(), to.into());
    }
    // Rename any user entry before the emitter adds its GLSL main.
    aliases.insert("main".into(), "milkrust_user_entry".into());
    let mut tokens = milkrust_hlsl::tokenize(&source)?;
    if tokens.iter().any(|t| t.starts_with("milkrust_") || t.starts_with("gl_") || t.starts_with("rast_") || t.starts_with("frag_")) {
        return Err("Shader uses a reserved compiler identifier".into());
    }
    for token in &mut tokens { if let Some(alias) = aliases.get(token) { *token = alias.clone(); } }
    let explicit_entry = tokens.iter().any(|t| t == "milkrust_user_entry");
    let mut preamble = String::from("float3 color; sampler2D previousFrame; float feedback; float outputAlpha; float time; float sampleRate; float fftBins[64]; float waveformBins[64]; float2 resolution; float2 pixelSize; float4 hlslAspect; float4 texsize;\n");
    for name in ["bass", "bass_att", "mid", "mid_att", "treb", "treb_att"] { preamble.push_str(&format!("float {name};\n")); }
    for q in 1..=64 { preamble.push_str(&format!("float q{q};\n")); }
    for (index, name) in samplers.iter().enumerate() {
        let kind = if crate::resources::texture_kind(name).is_volume() { "sampler3D" } else { "sampler2D" };
        preamble.push_str(&format!("{kind} shaderTexture{index};\n"));
    }
    for name in crate::resources::SCALAR_UNIFORMS { preamble.push_str(&format!("float {name};\n")); }
    for name in crate::resources::VECTOR_UNIFORMS { preamble.push_str(&format!("float4 {name};\n")); }
    for level in 1..=3 {
        if let Some(index) = samplers.iter().position(|name| name == &format!("sampler_blur{level}")) {
            preamble.push_str(&format!("float3 GetBlur{level}(float2 p) {{ return tex2D(shaderTexture{index}, p).rgb; }}\n"));
        }
    }
    preamble.push_str("float get_fft(float p) { return fftBins[(int)(clamp(p, 0.0, 1.0)*63.0)]; }\nfloat get_fft_hz(float hz) { return get_fft(hz / max(sampleRate * 0.5, 1.0)); }\nfloat get_waveform(float p) { return waveformBins[(int)(clamp(p,0.0,1.0)*63.0)]; }\n");
    let entry;
    if explicit_entry {
        // The emitter maps only COLOR0 and TEXCOORD0 (plus system fragment
        // coordinates) to this render graph. Other semantics fail below.
        entry = "milkrust_user_entry";
        preamble.push_str(&tokens.join(" "));
    } else {
        entry = "milkrust_entry";
        let body = if let Some(marker) = tokens.iter().position(|t| t == "shader_body") {
            preamble.push_str(&tokens[..marker].join(" "));
            if tokens.get(marker + 1).map(String::as_str) != Some("{") || tokens.last().map(String::as_str) != Some("}") {
                return Err("Malformed shader_body".into());
            }
            tokens[marker + 2..tokens.len() - 1].join(" ")
        } else if tokens.first().map(String::as_str) == Some("{") && tokens.last().map(String::as_str) == Some("}") {
            tokens[1..tokens.len() - 1].join(" ")
        } else { tokens.join(" ") };
        preamble.push_str(&format!(r#"
float3 milkrust_body(float2 uv) {{
    float2 uv_orig = uv;
    float x = uv.x; float y = uv.y;
    float2 centered = uv - float2(0.5, 0.5);
    float rad = length(centered); float ang = atan2(centered.y, centered.x);
    float3 ret = color;
    {body}
    return ret;
}}
float4 milkrust_entry(float2 milkrust_uv : TEXCOORD0) : COLOR0 {{
    return float4(milkrust_body(milkrust_uv), 1.0);
}}
"#));
    }
    let glsl = milkrust_hlsl::compile(&preamble, entry)?;
    adapt_output(&glsl, legacy_feedback)
}

fn adapt_output(glsl: &str, feedback: bool) -> Result<String, String> {
    // Validate the generated interface before changing it. No unknown varying
    // or render target is silently initialized with zero.
    let mut output = String::new();
    let mut has_uv = false;
    let mut has_output = false;
    let mut has_main = false;
    for line in glsl.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("in ") {
            if trimmed == "in vec2 frag_TEXCOORD0;" {
                output.push_str("in vec2 uv;\n#define frag_TEXCOORD0 uv\n");
                has_uv = true; continue;
            }
            return Err(format!("Unsupported pixel shader input: {trimmed}"));
        }
        if trimmed.starts_with("out ") {
            if trimmed != "out vec4 rast_FragData[1];" { return Err(format!("Unsupported pixel shader output: {trimmed}")); }
            has_output = true;
        }
        if trimmed == "void main() {" {
            output.push_str("void milkrust_generated_main() {\n");
            has_main = true; continue;
        }
        output.push_str(line); output.push('\n');
    }
    if !has_output || !has_main { return Err("Shader entry must write COLOR0".into()); }
    if !has_uv { output.push_str("in vec2 uv;\n"); }
    output.push_str(milk2::GLSL_TRANSITION_MASK);
    // The driver may optimize an unused source uniform out. In GLSL it still
    // exists in the declaration stream emitted from our preamble.
    output.push_str("\nvoid main() { milkrust_generated_main();\n");
    if feedback { output.push_str("rast_FragData[0].rgb = mix(rast_FragData[0].rgb, texture(previousFrame, clamp(uv, vec2(0.0), vec2(1.0))).rgb, feedback);\n"); }
    output.push_str("rast_FragData[0].a *= outputAlpha * milkrust_transition_mask(uv);\n}\n");
    if output.len() > 256 * 1024 { return Err("Generated shader exceeds renderer source limit".into()); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_typed_helpers_arrays_matrices_and_loops() {
        let result = compile("#define COUNT 3\nfloat3 tint(float3 x) { float3x3 m = float3x3(1,0,0,0,1,0,0,0,1); return mul(m,x); }\nshader_body { float a[3]; a[0]=0.1; a[1]=0.2; a[2]=0.3; for(int i=0;i<COUNT;i++){ ret += tint(float3(a[i],a[i],a[i])); } }", false).unwrap();
        assert!(result.contains("milkrust_generated_main"));
        assert!(result.contains("milkrust_transition_mask"));
        assert!(result.contains("for ("));
    }
    #[test]
    fn preserves_entry_input_and_rejects_unbound_semantics() {
        assert!(compile("float4 main(float2 p:TEXCOORD0):COLOR0 { return float4(p,0,1); }", false).is_ok());
        assert!(compile("float4 main(float3 p:NORMAL):COLOR0 { return float4(p,1); }", false).is_err());
    }
    #[test]
    fn discovers_expanded_sampler_bindings() {
        assert_eq!(sampler_names("#define T sampler_cloud\nshader_body { ret=tex2D(T,uv).rgb; }").unwrap(), vec!["sampler_cloud"]);
    }
}
