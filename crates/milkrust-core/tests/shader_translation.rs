//! Source regressions, not shader compilation or reference-render evidence.
use milkrust_core::{create_translated_milkrust_fragment_shader,
    create_translated_milkrust_wgsl_shader, get_milkrust_shader_texture_samplers,
    translate_milkrust_shader_expression};

#[test]
fn hlsl_calls_allow_whitespace_without_rewriting_user_identifiers() {
    let shader = create_translated_milkrust_fragment_shader(
        "float3 shader_bodyColor = float3 (0.1,0.2,0.3); ret = lerp (shader_bodyColor, float3 (1), frac (time));");
    assert!(shader.contains("vec3 shader_bodyColor = vec3(0.1,0.2,0.3);"));
    assert!(shader.contains("mix(shader_bodyColor, vec3(1), fract(time))"));
    let expression = translate_milkrust_shader_expression("ret = myfloat3(1);");
    assert_eq!(expression, "myfloat3(1)"); // Unsupported call remains a compile diagnostic.
    assert!(create_translated_milkrust_fragment_shader("float3Exposure=1;ret=float3(1);").is_empty());
}

#[test]
fn texture_discovery_and_translation_share_exact_call_tokens() {
    let source = "float3 c = tex2D \n (sampler_Wood, uv).rgb; ret = c + tex2D(sampler_main,uv).rgb;";
    assert_eq!(get_milkrust_shader_texture_samplers(source), vec!["sampler_Wood"]);
    let shader = create_translated_milkrust_fragment_shader(source);
    assert!(shader.contains("texture(shaderTexture0, uv)"));
    assert!(shader.contains("texture(previousFrame,uv)"));
    assert!(get_milkrust_shader_texture_samplers("ret=mytex2D(sampler_ignored,uv);").is_empty());
}

#[test]
fn conditional_lowering_preserves_case_and_balances_function_parentheses() {
    let expression = translate_milkrust_shader_expression(
        "shader_body { if (sin(time) > 0) { ret = tex2D(sampler_Wood, uv).rgb; } else { ret = float3(1); } }");
    assert!(expression.contains("(sin(time) > 0) ?"));
    assert!(expression.contains("texture(shaderTexture0, uv).rgb"));
    assert!(!translate_milkrust_shader_expression(
        "if (time>0) {ret=float3(1);} else {ret=float3(0);}").is_empty());
    assert_eq!(get_milkrust_shader_texture_samplers(
        "if (time>0) {ret=tex2D(sampler_Wood,uv).rgb;} else {ret=float3(1);}"), vec!["sampler_Wood"]);
    assert!(translate_milkrust_shader_expression(
        "if(time>0){ret=float3(1);time=0;}else{ret=float3(0);}").is_empty());
    assert!(translate_milkrust_shader_expression(
        "if(time>0){ret=float3(1);}else{ret=float3(0);}ret=float3(2);").is_empty());
}

#[test]
fn comments_separate_tokens_and_unclosed_comments_reject() {
    let shader = create_translated_milkrust_fragment_shader("float3/**/Color=float3(1);ret=Color;");
    assert!(shader.contains("vec3 Color = vec3(1);"));
    assert!(create_translated_milkrust_fragment_shader("ret=float3(1); /* unfinished").is_empty());
}

#[test]
fn hlsl_remainder_is_not_glsl_floor_modulo() {
    let source = "ret=float3(fmod(-1.5,1.0),fmod(1.5,-1.0),0);";
    let shader = create_translated_milkrust_fragment_shader(source);
    assert!(shader.contains("milkrust_hlsl_fmod(-1.5,1.0)"));
    assert!(shader.contains("return x-y*trunc(x/y);"));
    // An unimplemented WGSL overload must not emit an undefined helper call.
    assert!(create_translated_milkrust_wgsl_shader(source).is_empty());
}
