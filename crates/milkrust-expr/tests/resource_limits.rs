use std::collections::BTreeMap;
use milkrust_expr::{CompiledMilkRustExpression, evaluate_milkrust_expression};

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
