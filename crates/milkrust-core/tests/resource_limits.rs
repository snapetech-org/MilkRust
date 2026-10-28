use std::collections::BTreeMap;
use milkrust_core::{CompiledMilkRustExpression, evaluate_milkrust_expression};

#[test]
fn nested_work_is_bounded_but_compilation_does_not_execute_it() {
    let scope = BTreeMap::new();
    for source in ["loop(200000,loop(200000,1))", "while(1)", "memset(0,1,200000)"] {
        let compiled = CompiledMilkRustExpression::compile(source).expect("syntax only");
        let error = compiled.evaluate(&scope).expect_err("work must be bounded");
        assert!(error.to_string().contains("work budget"), "{error}");
    }
}

#[test]
fn deep_input_is_rejected_without_recursing_to_stack_exhaustion() {
    let nested = format!("{}1{}", "(".repeat(128), ")".repeat(128));
    let unary = format!("{}1", "-".repeat(128));
    for source in [nested, unary] {
        let error = CompiledMilkRustExpression::compile(&source).unwrap_err();
        assert!(error.to_string().contains("nesting limit"), "{error}");
    }
}

#[test]
fn byte_and_token_budgets_reject_before_large_parse_allocations() {
    let bytes = "1".repeat(64 * 1024 + 1);
    assert!(CompiledMilkRustExpression::compile(&bytes).unwrap_err().to_string().contains("64 KiB"));
    let tokens = format!("{}1", "1+".repeat(8193));
    assert!(CompiledMilkRustExpression::compile(&tokens).unwrap_err().to_string().contains("token limit"));
}

#[test]
fn ordinary_work_and_lazy_branches_keep_their_results() {
    let scope = BTreeMap::new();
    assert_eq!(evaluate_milkrust_expression("loop(10,a+=1)", &scope).unwrap(), 10.0);
    assert_eq!(evaluate_milkrust_expression("if(0,loop(200000,1),7)", &scope).unwrap(), 7.0);
    // A failed evaluation must not poison the next independent expression.
    assert!(evaluate_milkrust_expression("while(1)", &scope).is_err());
    assert_eq!(evaluate_milkrust_expression("3+4", &scope).unwrap(), 7.0);
}

#[test]
fn sparse_and_overflowing_object_indices_remain_visible_as_rejections() {
    use milkrust_core::{build_milkrust_compatibility_entry, parse_milkrust_preset_set, validate_milkrust_webgl_import};
    for key in ["shapecode_999999999_enabled", "wave_184467440737095516160_enabled", "sprite64_enabled"] {
        let source = format!("[preset00]\nname=bounded\n{key}=1");
        let parsed = parse_milkrust_preset_set(&source, false);
        assert_eq!(parsed.presets.len(), 1);
        assert!(parsed.presets[0].shapes.is_empty());
        assert!(parsed.presets[0].waves.is_empty());
        assert!(parsed.presets[0].sprites.is_empty());
        assert!(!parsed.presets[0].parse_errors.is_empty());
        assert!(validate_milkrust_webgl_import(&source).is_err());
        let report = build_milkrust_compatibility_entry("bounded", "bounded.milk", &source, false);
        assert!(!report.supported);
        assert!(!report.webgpu_supported);
        assert!(!report.preset_reports[0].parse_errors.is_empty());
    }
    let valid = parse_milkrust_preset_set("shapecode_63_enabled=1", false);
    assert!(valid.presets[0].parse_errors.is_empty());
    assert_eq!(valid.presets[0].shapes.len(), 64);
}

#[test]
fn oversized_and_binary_text_cannot_fall_back_to_fragment_parsing() {
    use milkrust_core::{parse_milkrust_fragment, parse_milkrust_preset_set};
    for source in ["x".repeat(2 * 1024 * 1024 + 1), "name=bad\0preset".to_string()] {
        let parsed = parse_milkrust_preset_set(&source, false);
        assert!(!parsed.presets[0].parse_errors.is_empty());
        assert!(parsed.presets[0].source.is_empty());
        let fragment = parse_milkrust_fragment(&source, "local.shape", "shape");
        assert!(!fragment.parse_errors.is_empty());
        assert!(fragment.entries.is_empty());
    }
}
