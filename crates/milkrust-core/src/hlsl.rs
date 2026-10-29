//! MilkDrop shader environment for the typed legacy HLSL compiler.
use crate::milk2;
use std::collections::BTreeMap;

pub(crate) fn sampler_names(source: &str) -> Result<Vec<String>, String> {
    let source = milkrust_hlsl::preprocess(source)?;
    let tokens = milkrust_hlsl::tokenize(&source)?;
    let mut names = Vec::new();
    let mut sampler_parameters = Vec::new();
    let mut parens = 0i32;
    let mut braces = 0i32;
    let mut functions = Vec::new();
    let structures: Vec<&String> = tokens.windows(2).filter(|pair| pair[0] == "struct")
        .map(|pair| &pair[1]).collect();
    let is_return_type = |name: &String| {
        structures.contains(&name) || ["void", "float", "half", "int", "uint", "bool", "double"]
            .iter().any(|base| name.strip_prefix(*base).is_some_and(|suffix|
                suffix.is_empty() || (suffix.len() <= 3 && suffix.bytes().all(|b| matches!(b, b'1'..=b'4' | b'x')))))
    };
    for (index, token) in tokens.iter().enumerate() {
        if token == "(" && parens == 0 && braces == 0 && index >= 2
            && is_return_type(&tokens[index - 2]) {
            functions.push(tokens[index - 1].clone());
        }
        if token == "(" { parens += 1; }
        if token == ")" { parens -= 1; }
        if token == "{" { braces += 1; }
        if token == "}" { braces -= 1; }
        if parens > 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D") {
            if let Some(name) = tokens.get(index + 1) { sampler_parameters.push(name); }
        } else if parens == 0 && braces == 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D") {
            if let Some(name) = tokens.get(index + 1).filter(|n| !crate::is_milkrust_main_sampler(n)) {
                if !names.contains(name) { names.push(name.clone()); }
            }
        }
    }
    // Only referenced resources are reflected, never arbitrary identifiers in
    // unknown function calls. Sampler parameters belong to their function.
    for call in tokens.windows(4) {
        if ["tex", "tex2d", "tex3d", "tex2D", "tex2Dlod", "tex2Dbias", "tex2Dgrad", "tex2Dproj", "tex3D", "tex3Dlod", "tex3Dbias"].contains(&call[0].as_str())
            && call[1] == "(" && call[3] == "," && !crate::is_milkrust_main_sampler(&call[2])
            && !sampler_parameters.contains(&&call[2]) && !names.contains(&call[2]) {
            names.push(call[2].clone());
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if functions.contains(token) && tokens.get(index + 1).map(String::as_str) == Some("(") {
            let mut depth = 1;
            for argument in tokens.iter().skip(index + 2) {
                if argument == "(" { depth += 1; }
                if argument == ")" { depth -= 1; if depth == 0 { break; } }
                if argument.starts_with("sampler_") && !sampler_parameters.contains(&argument)
                    && !crate::is_milkrust_main_sampler(argument) && !names.contains(argument) {
                    names.push(argument.clone());
                }
            }
        }
    }
    for level in 1..=3 {
        let name = format!("sampler_blur{level}");
        if tokens.iter().any(|t| t == &format!("GetBlur{level}")) && !names.contains(&name) { names.push(name); }
    }
    for token in &tokens {
        if let Some(base) = token.strip_prefix("texsize_") {
            let name = format!("sampler_{base}");
            if base != "main" && !crate::resources::VECTOR_UNIFORMS.contains(&token.as_str()) && !names.contains(&name) { names.push(name); }
        }
    }
    if names.len() > 15 { return Err("Shader exceeds 15 additional samplers".into()); }
    Ok(names)
}

pub(crate) fn needs_environment(source: &str) -> bool {
    let Ok(tokens) = milkrust_hlsl::tokenize(source) else { return true; };
    tokens.iter().any(|name| crate::resources::MATRIX_UNIFORMS.contains(&name.as_str())
        || crate::resources::VECTOR_UNIFORMS.contains(&name.as_str())
        || crate::resources::SCALAR_UNIFORMS.contains(&name.as_str())
        || name.starts_with("mip_") || name.starts_with("texsize_")
        || name.strip_prefix('q').and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| (1..=64).contains(&n))
        || ["rad", "ang", "aspect", "uv_orig", "hue_shader", "GetMain", "GetPixel", "GetBlur1", "GetBlur2", "GetBlur3", "lum",
            "M_PI", "M_PI_2", "M_INV_PI_2", "M_INV_PI", "tex2d", "tex3d"].contains(&name.as_str()))
}

pub(crate) fn compile(source: &str, legacy_feedback: bool) -> Result<String, String> {
    if source.len() > 128 * 1024 { return Err("Shader source exceeds 128 KiB".into()); }
    let source = milkrust_hlsl::preprocess(source)?;
    let samplers = sampler_names(&source)?;
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    aliases.insert("tex".into(), "tex2D".into());
    aliases.insert("tex2d".into(), "tex2D".into());
    aliases.insert("tex3d".into(), "tex3D".into());
    aliases.insert("aspect".into(), "hlslAspect".into());
    aliases.insert("uv".into(), "milkrust_uv_value".into());
    aliases.insert("uv_orig".into(), "milkrust_uv_orig_value".into());
    aliases.insert("texsize_main".into(), "texsize".into());
    for q in 0..64 {
        aliases.insert(format!("q{}", q+1), format!("_q{}.{}", char::from(b'a'+(q/4) as u8), ["x", "y", "z", "w"][q%4]));
    }
    for (name, value) in [("M_PI", "3.141592653589793"), ("M_PI_2", "6.283185307179586"),
        ("M_INV_PI_2", "0.159154943091895"), ("M_INV_PI", "0.318309886183791"), ("mip_x", "_c12.x"), ("mip_y", "_c12.y"),
        ("mip_xy", "_c12.xy"), ("mip_avg", "_c12.z")] { aliases.insert(name.into(), value.into()); }
    for name in ["sampler_main"] {
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
    for pair in tokens.windows(2) {
        if matches!(pair[0].as_str(), "float" | "double" | "half") && matches!(pair[1].as_str(), "M_PI" | "M_PI_2" | "M_INV_PI_2" | "M_INV_PI") {
            aliases.remove(&pair[1]);
        }
    }
    if tokens.iter().any(|t| t.starts_with("milkrust_") || t.starts_with("gl_") || t.starts_with("rast_") || t.starts_with("frag_")) {
        return Err("Shader uses a reserved compiler identifier".into());
    }
    for token in &mut tokens { if let Some(alias) = aliases.get(token) { *token = alias.clone(); } }
    // Sampler declarations saved in a preamble are represented by the exact
    // host binding below. Remove only complete, top-level bound declarations.
    let mut clean = Vec::new();
    let mut index = 0;
    let mut depth = 0i32;
    let mut parens = 0i32;
    while index < tokens.len() {
        let token = &tokens[index];
        if depth == 0 && parens == 0 && matches!(token.as_str(), "sampler" | "sampler2D" | "sampler3D")
            && tokens.get(index + 1).is_some_and(|name| name == "previousFrame" || name.starts_with("shaderTexture")) {
            let name = &tokens[index + 1];
            let expected_volume = name.strip_prefix("shaderTexture").and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| samplers.get(n)).is_some_and(|n| crate::resources::texture_kind(n).is_volume());
            if (token == "sampler3D") != expected_volume && token != "sampler" { return Err("Sampler dimension disagrees with its bound texture".into()); }
            index += 2;
            while index < tokens.len() && tokens[index] != ";" {
                if tokens[index] == "{" || tokens[index] == "=" || tokens[index] == "," { return Err("Custom sampler state declarations are unsupported".into()); }
                index += 1;
            }
            if index == tokens.len() { return Err("Unterminated sampler declaration".into()); }
            index += 1; continue;
        }
        if token == "{" { depth += 1; } else if token == "}" { depth -= 1; }
        if token == "(" { parens += 1; } else if token == ")" { parens -= 1; }
        clean.push(token.clone()); index += 1;
    }
    tokens = clean;
    let explicit_entry = tokens.iter().any(|t| t == "milkrust_user_entry");
    let mut preamble = String::from("float3 color; sampler2D previousFrame; float feedback; float outputAlpha; float time; float sampleRate; float fftBins[64]; float waveformBins[64]; float2 resolution; float2 pixelSize; float4 hlslAspect; float4 texsize;\n");
    preamble.push_str("static float2 milkrust_uv_value; static float2 milkrust_uv_orig_value; static float x; static float y; static float rad; static float ang; static float3 ret; static float3 hue_shader;\n");
    for name in ["bass", "bass_att", "mid", "mid_att", "treb", "treb_att"] { preamble.push_str(&format!("float {name};\n")); }
    for q in 1..=64 { preamble.push_str(&format!("float q{q};\n")); }
    for (index, name) in samplers.iter().enumerate() {
        let kind = if crate::resources::texture_kind(name).is_volume() { "sampler3D" } else { "sampler2D" };
        preamble.push_str(&format!("{kind} shaderTexture{index};\n"));
    }
    for name in crate::resources::SCALAR_UNIFORMS { preamble.push_str(&format!("float {name};\n")); }
    for name in crate::resources::VECTOR_UNIFORMS { preamble.push_str(&format!("float4 {name};\n")); }
    for name in crate::resources::MATRIX_UNIFORMS { preamble.push_str(&format!("float4x3 {name};\n")); }
    let sizes: std::collections::BTreeSet<_> = samplers.iter().map(|name| format!("texsize_{}", crate::resources::base_name(name))).collect();
    for name in sizes {
        if !crate::resources::VECTOR_UNIFORMS.contains(&name.as_str()) { preamble.push_str(&format!("float4 {name};\n")); }
    }
    for level in 1..=3 {
        if let Some(index) = samplers.iter().position(|name| name == &format!("sampler_blur{level}")) {
            let (scale, bias) = match level { 1 => ("_c5.x", "_c5.y"), 2 => ("_c5.z", "_c5.w"), _ => ("_c6.x", "_c6.y") };
            preamble.push_str(&format!("float3 GetBlur{level}(float2 p) {{ return tex2D(shaderTexture{index}, p).rgb * {scale} + {bias}; }}\n"));
        }
    }
    preamble.push_str("float3 GetMain(float2 p) { return tex2D(previousFrame, p).rgb; }\nfloat3 GetPixel(float2 p) { return GetMain(p); }\nfloat lum(float3 c) { return dot(c, float3(0.32, 0.49, 0.29)); }\n");
    preamble.push_str("float get_fft(float p) { return fftBins[(int)(clamp(p, 0.0, 1.0)*63.0)]; }\nfloat get_fft_hz(float hz) { return get_fft(hz / max(sampleRate * 0.5, 1.0)); }\nfloat get_waveform(float p) { return waveformBins[(int)(clamp(p,0.0,1.0)*63.0)]; }\n");
    let entry;
    if explicit_entry {
        // The adapter maps legacy UV pairs, polar coordinates and hue input,
        // plus system fragment coordinates. Other semantics fail below.
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
float3 milkrust_body() {{
    {body}
    return ret;
}}
float4 milkrust_entry(float2 milkrust_uv : TEXCOORD0) : COLOR0 {{
    return float4(milkrust_body(), 1.0);
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
    let mut initializers = Vec::new();
    for line in glsl.lines() {
        let trimmed = line.trim();
        // Keep the host binding spelling, then copy into mutable per-fragment
        // storage. Helpers and user assignments see the same local registers.
        if let Some(declaration) = trimmed.strip_prefix("uniform ").and_then(|v| v.strip_suffix(';')) {
            let parts: Vec<_> = declaration.split_whitespace().collect();
            if parts.len() == 2 {
                let (kind, name) = (parts[0], parts[1]);
                let mutable = crate::resources::SCALAR_UNIFORMS.contains(&name)
                    || crate::resources::VECTOR_UNIFORMS.contains(&name)
                    || crate::resources::MATRIX_UNIFORMS.contains(&name)
                    || ["color", "time", "texsize", "hlslAspect", "bass", "mid", "treb", "bass_att", "mid_att", "treb_att"].contains(&name)
                    || name.strip_prefix('q').and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| (1..=64).contains(&n));
                if mutable {
                    let suffix = name.trim_start_matches('_');
                    output.push_str(&format!("{line}\n{kind} milkrust_value_{suffix};\nvoid milkrust_init_{suffix}() {{ milkrust_value_{suffix} = {name}; }}\n#define {name} milkrust_value_{suffix}\n"));
                    initializers.push(format!("milkrust_init_{suffix}();"));
                    continue;
                }
            }
        }
        if trimmed.starts_with("in ") {
            if trimmed == "in vec2 frag_TEXCOORD0;" {
                output.push_str("in vec2 uv;\nvec2 milkrust_input_uv() { return uv; }\n#define frag_TEXCOORD0 milkrust_input_uv()\n");
                has_uv = true; continue;
            }
            if trimmed == "in vec4 frag_TEXCOORD0;" {
                output.push_str("in vec2 uv;\n#define frag_TEXCOORD0 vec4(uv, originalUv)\n");
                has_uv = true; continue;
            }
            if trimmed == "in vec2 frag_TEXCOORD1;" { output.push_str("#define frag_TEXCOORD1 vec2(rad, ang)\n"); continue; }
            if trimmed == "in vec4 frag_COLOR0;" { output.push_str("#define frag_COLOR0 vec4(hue_shader, 1.0)\n"); continue; }
            if trimmed == "in vec3 frag_COLOR0;" { output.push_str("#define frag_COLOR0 hue_shader\n"); continue; }
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
    output.insert_str(output.find('\n').map_or(0, |i| i+1), "in highp vec2 originalUv;\n");
    output.push_str(milk2::GLSL_TRANSITION_MASK);
    // The driver may optimize an unused source uniform out. In GLSL it still
    // exists in the declaration stream emitted from our preamble.
    output.push_str("\nvoid main() {\n");
    output.push_str(&initializers.join("\n"));
    output.push_str("\nmilkrust_uv_value=uv; milkrust_uv_orig_value=originalUv; x=uv.x; y=uv.y;\nvec2 milkrust_centered=(originalUv*2.0-1.0)*hlslAspect.xy;\nrad=length(milkrust_centered)/length(hlslAspect.xy); ang=atan(milkrust_centered.y,milkrust_centered.x); if(ang<0.0)ang+=6.283185307179586;\nhue_shader=mix(mix(hue_corner4.xyz,hue_corner3.xyz,originalUv.x),mix(hue_corner2.xyz,hue_corner1.xyz,originalUv.x),originalUv.y); ret=color;\nmilkrust_generated_main();\n");
    if feedback { output.push_str("rast_FragData[0].rgb = mix(rast_FragData[0].rgb, texture(previousFrame, clamp(uv, vec2(0.0), vec2(1.0))).rgb, feedback);\n"); }
    output.push_str("rast_FragData[0].a *= outputAlpha * milkrust_transition_mask(uv);\n}\n");
    if output.len() > 256 * 1024 { return Err("Generated shader exceeds renderer source limit".into()); }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_milkdrop_environment_and_distinct_main_sampler_modes() {
        let source = "shader_body { float3 p=mul(float4(uv,0,1),rot_s1); ret=GetMain(uv)+GetPixel(uv)+GetBlur2(uv); ret+=tex2d(sampler_PC_main,uv).rgb; ret+=float3(rad,ang/M_PI_2,mip_avg)+_c3.xyz+_qa.xyz+p+lum(ret); }";
        assert!(needs_environment(source));
        let output = compile(source, false).unwrap();
        assert!(output.contains("mat3x4 rot_s1"));
        assert!(output.contains("_c5"));
        assert_eq!(sampler_names(source).unwrap(), vec!["sampler_PC_main", "sampler_blur2"]);
    }
    #[test]
    fn helpers_share_mutable_registers_and_fragment_coordinates() {
        let source = "void tint(){q1+=0.25; hue_shader*=2; ret+=float3(uv_orig,rad);}\nshader_body { tint(); ret+=float3(_qa.x,q1,ang); }";
        let result = compile(source, false).unwrap();
        assert!(result.contains("#define _qa milkrust_value_qa"));
        assert!(result.contains("milkrust_init_qa();"));
        assert!(result.contains("milkrust_uv_orig_value=originalUv"));
        assert!(result.contains("hue_shader=mix"));
        assert_eq!(sampler_names("ret=float3(texsize_paper.xy,1);").unwrap(), vec!["sampler_paper"]);
    }
    #[test]
    fn explicit_legacy_entries_receive_uv_pairs_hue_and_polar_coordinates() {
        assert!(compile("float4 main(float4 p:TEXCOORD0,float2 a:TEXCOORD1,float4 hue:COLOR0):COLOR0 { return float4(p.zw,a.x,1)*hue; }",false).is_ok());
    }
    #[test]
    fn compiles_typed_helpers_arrays_matrices_and_loops() {
        let result = compile("#define COUNT 3\nfloat3 tint(float3 x) { float3x3 m = float3x3(1,0,0,0,1,0,0,0,1); return mul(m,x); }\nshader_body { float a[3]; a[0]=0.1; a[1]=0.2; a[2]=0.3; for(int i=0;i<COUNT;i++){ ret += tint(float3(a[i],a[i],a[i])); } }", false).unwrap();
        assert!(result.contains("milkrust_generated_main"));
        assert!(result.contains("milkrust_transition_mask"));
        assert!(result.contains("for ("));
    }
    #[test]
    fn preserves_entry_input_and_rejects_unbound_semantics() {
        assert!(compile("float4 main(float2 uv:TEXCOORD0):COLOR0 { return float4(uv,0,1); }", false).is_ok());
        assert!(compile("float4 main(float3 p:NORMAL):COLOR0 { return float4(p,1); }", false).is_err());
    }
    #[test]
    fn discovers_expanded_sampler_bindings() {
        assert_eq!(sampler_names("#define T sampler_cloud\nshader_body { ret=tex2D(T,uv).rgb; }").unwrap(), vec!["sampler_cloud"]);
        assert!(sampler_names("ret=mytex2D(sampler_ignored,uv);").unwrap().is_empty());
        assert_eq!(sampler_names("float3 sample(sampler2D s,float2 p){return tex2D(s,p).rgb;} shader_body {ret=sample(sampler_cloud,uv);}").unwrap(), vec!["sampler_cloud"]);
    }
}
