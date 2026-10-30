//! Numeric operations for declaratively converted Butterchurn equations.
//!
//! Keep these separate from EEL's epsilon comparisons, integer remainder and
//! guarded math. Conversion never installs a JavaScript runtime. IEEE nonfinite
//! values remain observable inside these expressions; render inputs still use
//! the host's finite-value validation.
//! Reference: https://tc39.es/ecma262/2024/#sec-numeric-types-number
//! and https://tc39.es/ecma262/2024/#sec-math-object

fn arity(name: &str) -> Option<usize> {
    match name {
        "bcjstruth" | "bcjsbitnot" | "bcjsmathabs" | "bcjsmathacos"
        | "bcjsmathacosh" | "bcjsmathasin" | "bcjsmathasinh" | "bcjsmathatan"
        | "bcjsmathatanh" | "bcjsmathcbrt" | "bcjsmathceil" | "bcjsmathclz32"
        | "bcjsmathcos" | "bcjsmathcosh" | "bcjsmathexp" | "bcjsmathexpm1"
        | "bcjsmathfloor" | "bcjsmathfround" | "bcjsmathlog" | "bcjsmathlog10"
        | "bcjsmathlog1p" | "bcjsmathlog2" | "bcjsmathround" | "bcjsmathsign"
        | "bcjsmathsin" | "bcjsmathsinh" | "bcjsmathsqrt" | "bcjsmathtan"
        | "bcjsmathtanh" | "bcjsmathtrunc" => Some(1),
        "bcjsequal" | "bcjsremainder" | "bcjsdivide" | "bcjsbitand"
        | "bcjsbitor" | "bcjsbitxor" | "bcjsshiftleft" | "bcjsshiftright"
        | "bcjsunsignedshiftright" | "bcjsmathatan2" | "bcjsmathimul"
        | "bcjsmathmin" | "bcjsmathmax" | "bcjsmathpow" => Some(2),
        _ => None,
    }
}

pub fn supports_function(name: &str) -> bool {
    arity(&name.to_ascii_lowercase()).is_some()
}

fn uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let remainder = value.trunc() % 4_294_967_296.0;
    let positive = if remainder < 0.0 { remainder + 4_294_967_296.0 } else { remainder };
    positive as u32
}

fn round(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 || value == value.trunc() {
        return value;
    }
    if (-0.5..0.0).contains(&value) {
        return -0.0;
    }
    // Avoid x + 0.5 rounding before floor at large magnitudes or just below a tie.
    let floor = value.floor();
    floor + if value - floor >= 0.5 { 1.0 } else { 0.0 }
}

fn minimum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() { -0.0 } else { 0.0 }
    } else {
        a.min(b)
    }
}

fn maximum(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() { 0.0 } else { -0.0 }
    } else {
        a.max(b)
    }
}

pub(super) fn evaluate(name: &str, args: &[f64]) -> Option<Result<f64, String>> {
    let count = arity(name)?;
    if args.len() != count {
        return Some(Err(format!("{name} requires {count} numeric arguments")));
    }
    let a = args[0];
    let b = args.get(1).copied().unwrap_or(0.0);
    let value = match name {
        "bcjstruth" => (a != 0.0 && !a.is_nan()) as u8 as f64,
        "bcjsequal" => (a == b) as u8 as f64,
        "bcjsremainder" => a % b,
        "bcjsdivide" => a / b,
        "bcjsbitand" => ((uint32(a) & uint32(b)) as i32) as f64,
        "bcjsbitor" => ((uint32(a) | uint32(b)) as i32) as f64,
        "bcjsbitxor" => ((uint32(a) ^ uint32(b)) as i32) as f64,
        "bcjsbitnot" => (!uint32(a) as i32) as f64,
        "bcjsshiftleft" => (uint32(a).wrapping_shl(uint32(b) & 31) as i32) as f64,
        "bcjsshiftright" => ((uint32(a) as i32) >> (uint32(b) & 31)) as f64,
        "bcjsunsignedshiftright" => (uint32(a) >> (uint32(b) & 31)) as f64,
        "bcjsmathabs" => a.abs(),
        "bcjsmathacos" => a.acos(),
        "bcjsmathacosh" => a.acosh(),
        "bcjsmathasin" => a.asin(),
        "bcjsmathasinh" => a.asinh(),
        "bcjsmathatan" => a.atan(),
        "bcjsmathatanh" => a.atanh(),
        "bcjsmathatan2" => a.atan2(b),
        "bcjsmathcbrt" => a.cbrt(),
        "bcjsmathceil" => a.ceil(),
        "bcjsmathclz32" => uint32(a).leading_zeros() as f64,
        "bcjsmathcos" => a.cos(),
        "bcjsmathcosh" => a.cosh(),
        "bcjsmathexp" => a.exp(),
        "bcjsmathexpm1" => a.exp_m1(),
        "bcjsmathfloor" => a.floor(),
        "bcjsmathfround" => (a as f32) as f64,
        "bcjsmathimul" => (uint32(a).wrapping_mul(uint32(b)) as i32) as f64,
        "bcjsmathlog" => a.ln(),
        "bcjsmathlog10" => a.log10(),
        "bcjsmathlog1p" => a.ln_1p(),
        "bcjsmathlog2" => a.log2(),
        "bcjsmathmax" => maximum(a, b),
        "bcjsmathmin" => minimum(a, b),
        "bcjsmathpow" => {
            if b == 0.0 { 1.0 }
            else if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) { f64::NAN }
            else { a.powf(b) }
        }
        "bcjsmathround" => round(a),
        "bcjsmathsign" => {
            if a == 0.0 || a.is_nan() { a } else { a.signum() }
        }
        "bcjsmathsin" => a.sin(),
        "bcjsmathsinh" => a.sinh(),
        "bcjsmathsqrt" => a.sqrt(),
        "bcjsmathtan" => a.tan(),
        "bcjsmathtanh" => a.tanh(),
        "bcjsmathtrunc" => a.trunc(),
        _ => return None,
    };
    Some(Ok(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate_milkrust_equations, evaluate_milkrust_expression, MilkRustValue};
    use std::collections::BTreeMap;

    fn expression(source: &str) -> f64 {
        evaluate_milkrust_expression(source, &BTreeMap::new()).unwrap()
    }

    #[test]
    fn conversion_preserves_truth_equality_and_nonfinite_intermediates() {
        assert_eq!(expression("bcjsTruth(0.00000001)"), 1.0);
        assert_eq!(expression("bcjsEqual(1,1.000001)"), 0.0);
        assert_eq!(expression("bcjsRemainder(-7.5,2)"), -1.5);
        assert_eq!(expression("bcjsDivide(1,0.000001)"), 1_000_000.0);
        assert_eq!(expression("bcjsTruth(bcjsDivide(1,0))"), 1.0);
        assert_eq!(expression("bcjsTruth(bcjsMathsqrt(-1))"), 0.0);
        assert_eq!(expression("bcjsEqual(bcjsMathsqrt(-1),bcjsMathsqrt(-1))"), 0.0);
        assert!(expression("bcjsMathmin(1,bcjsDivide(0,0))").is_nan());
        assert_eq!(expression("bcjsMathpow(bcjsDivide(0,0),0)"), 1.0);
    }

    #[test]
    fn shifts_and_bits_follow_32_bit_conversion() {
        assert_eq!(expression("bcjsBitOr(4294967297,0)"), 1.0);
        assert_eq!(expression("bcjsBitOr(2147483648,0)"), -2147483648.0);
        assert_eq!(expression("bcjsUnsignedShiftRight(-1,0)"), 4294967295.0);
        assert_eq!(expression("bcjsShiftLeft(1,33)"), 2.0);
        assert_eq!(expression("bcjsShiftLeft(1,-1)"), -2147483648.0);
        assert_eq!(expression("bcjsShiftRight(-8,34)"), -2.0);
        assert_eq!(expression("bcjsBitXor(7,3)"), 4.0);
        assert_eq!(expression("bcjsMathimul(4294967295,5)"), -5.0);
        assert_eq!(uint32(f64::INFINITY), 0);
        assert_eq!(uint32(-4294967297.9), u32::MAX);
    }

    #[test]
    fn rounding_and_zero_signs_match_numeric_js() {
        assert_eq!(expression("bcjsMathround(3.5)"), 4.0);
        assert_eq!(expression("bcjsMathround(-3.5)"), -3.0);
        assert!(expression("bcjsMathround(-0.5)").is_sign_negative());
        assert!(expression("bcjsMathsign(-0)").is_sign_negative());
        assert!(expression("bcjsMathmin(0,-0)").is_sign_negative());
        assert!(expression("bcjsMathmax(-0,0)").is_sign_positive());
        assert_eq!(round(f64::from_bits(0.5f64.to_bits() - 1)), 0.0);
        assert_eq!(round(4_503_599_627_370_497.0), 4_503_599_627_370_497.0);
    }

    #[test]
    fn generated_sequence_and_lazy_branch_keep_evaluation_order() {
        let result = evaluate_milkrust_equations(
            "assign(x,1); assign(index,0); \
             if(exec2(assign(old,x),exec2(assign(x,x+1),assign(last,old))),last,last); \
             assign(y,if(bcjsTruth(assign(saved,0)),assign(x,99),saved)); \
             if(exec2(assign(address,index),exec2(assign(index,1),assign(last,assign(megabuf(address),7)))),last,last)",
            &BTreeMap::new(),
        ).unwrap();
        assert_eq!(result["x"], MilkRustValue::Number(2.0));
        assert_eq!(result["y"], MilkRustValue::Number(0.0));
        assert_eq!(result["megabuf_0"], MilkRustValue::Number(7.0));
        assert!(!result.contains_key("megabuf_1"));
        assert!(evaluate_milkrust_expression("bcjsDivide(1)", &BTreeMap::new()).is_err());
    }
}
