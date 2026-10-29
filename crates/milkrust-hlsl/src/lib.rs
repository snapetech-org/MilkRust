//! Safe boundary around the pinned legacy HLSL parser and GLSL ES 3 emitter.
//! Compilation limits are independent of driver shader execution limits.
mod preprocess;
pub use preprocess::{preprocess, tokens as tokenize};

use std::cell::RefCell;
use std::collections::VecDeque;

pub const PROFILE: &str = "milkrust-hlslparser-es300-v1";
pub const MAX_SOURCE: usize = 256 * 1024;
pub const MAX_OUTPUT: usize = 1024 * 1024;

type Cached = (String, String, Result<String, String>);
thread_local! {
    // No cross-thread compiler state; cache bounds include source and output.
    static CACHE: RefCell<VecDeque<Cached>> = RefCell::new(VecDeque::new());
}

pub fn compile(source: &str, entry: &str) -> Result<String, String> {
    if source.len() > MAX_SOURCE || source.contains('\0') || entry.len() > 64
        || entry.is_empty() || !entry.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err("Invalid HLSL source or entry point".into());
    }
    if let Some(found) = CACHE.with(|cache| cache.borrow().iter()
        .find(|(s, e, _)| s == source && e == entry).map(|(_, _, r)| r.clone())) {
        return found;
    }
    let result = platform_compile(source, entry);
    // Do not cache a browser readiness failure: initialization is asynchronous.
    if result.as_ref().err().is_some_and(|e| e.contains("not initialized")) { return result; }
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        while cache.len() >= 8 { cache.pop_front(); }
        cache.push_back((source.into(), entry.into(), result.clone()));
    });
    result
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_compile(source: &str, entry: &str) -> Result<String, String> {
    use std::ffi::{c_char, CString};
    extern "C" {
        fn milkrust_hlsl_compile(source: *const c_char, size: usize, entry: *const c_char,
            output: *mut c_char, capacity: usize, error: *mut c_char, error_capacity: usize) -> i32;
    }
    let entry = CString::new(entry).map_err(|_| "Invalid entry point")?;
    let mut output = vec![0u8; MAX_OUTPUT + 1];
    let mut error = vec![0u8; 8192];
    // SAFETY: all pointers remain valid for this synchronous call; lengths are
    // exact, buffers do not alias, the ABI catches all C++ exceptions and never
    // retains pointers. The returned length is checked before reading output.
    let size = unsafe { milkrust_hlsl_compile(source.as_ptr().cast(), source.len(),
        entry.as_ptr(), output.as_mut_ptr().cast(), output.len(),
        error.as_mut_ptr().cast(), error.len()) };
    if size < 0 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(String::from_utf8_lossy(&error[..end]).into_owned());
    }
    let size = usize::try_from(size).map_err(|_| "Invalid compiler output size")?;
    if size > MAX_OUTPUT { return Err("Compiler output limit exceeded".into()); }
    output.truncate(size);
    String::from_utf8(output).map_err(|_| "Compiler produced invalid UTF-8".into())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::compile;

    #[test]
    fn numeric_language_contract_compiles() {
        let source = include_str!("../tests/fixtures/numeric.hlsl");
        compile(source, "main").expect("numeric control must reach GLSL generation");
        // GPU assertions live in smoke-hlsl.mjs; source acceptance alone is
        // deliberately not treated as evidence of numeric/render correctness.
    }

    #[test]
    fn legacy_intrinsic_contract_compiles() {
        compile(include_str!("../tests/fixtures/intrinsics.hlsl"), "main")
            .expect("legacy intrinsic control must reach GLSL generation");
    }

    #[test]
    fn declaration_and_scoped_constant_contract_compiles() {
        compile(include_str!("../tests/fixtures/declarations.hlsl"), "main")
            .expect("declaration control must reach GLSL generation");
    }

    #[test]
    fn malformed_declarations_fail_with_bounded_diagnostics() {
        for body in [
            "float values[uint(0xffffffffu)];",
            "const int n=int(1e20); float values[n];",
            "const int n=n; float values[n];",
            "const int n=int(uv.x); float values[n];",
            "float values[]={};",
            "float values[2]={1,2,3};",
            "vector<float,5> v;",
            "typedef float Tone; typedef int Tone;",
            "[unroll(0)] for(int i=0;i<2;i++) {}",
        ] {
            let source = format!("float4 main(float2 uv:TEXCOORD0):COLOR0 {{ {body} return 1; }}");
            assert!(compile(&source, "main").is_err(), "invalid declaration accepted: {body}");
        }
    }

    #[test]
    fn token_and_numeric_limits_return_errors() {
        let identifier = "a".repeat(300);
        for body in [format!("float {identifier}=1;"), "float v=1e100;".into(),
            "uint v=0x100000000u;".into(), "int v=4294967296;".into()] {
            let source = format!("float4 main(float2 uv:TEXCOORD0):COLOR0 {{ {body} return 1; }}");
            assert!(compile(&source, "main").is_err());
        }
        for bound in ["3%0", "1<<32", "1<<-1"] {
            let source = format!("float4 main(float2 uv:TEXCOORD0):COLOR0 {{ float data[{bound}]; return 1; }}");
            assert!(compile(&source, "main").is_err());
        }
    }

    #[test]
    fn invalid_component_selections_are_rejected() {
        for statement in [
            "float2 v=0; float x=v.z;",
            "float4 v=0; float2 x=v.xr;",
            "float2x2 m=0; float x=m._00;",
            "float2x2 m=0; float2 x=m._11_m01;",
            "float2x2 m=0; m._11_11=float2(1,2);",
        ] {
            let source = format!("float4 main(float2 uv:TEXCOORD0):COLOR0 {{ {statement} return 1; }}");
            assert!(compile(&source, "main").is_err(), "invalid selection accepted: {statement}");
        }
    }

    #[test]
    fn constant_failures_do_not_cross_the_abi() {
        for bound in ["1 / 0", "2147483647 + 1", "65536 * 65536"] {
            let source = format!("float4 main(float2 uv:TEXCOORD0):COLOR0 {{ float data[{bound}]; return float4(uv,0,1); }}");
            assert!(compile(&source, "main").is_err(), "unsafe bound was accepted: {bound}");
        }
        assert!(compile("float4 main(float2 uv:TEXCOORD0):COLOR0 { return float4(uv,0,1); }", "main").is_ok());
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(module = "/src/compiler.js")]
    extern "C" {
        #[wasm_bindgen(catch, js_name = compileHlsl)]
        pub fn compile(source: &str, entry: &str) -> Result<String, JsValue>;
        #[wasm_bindgen(js_name = initializeHlsl)]
        pub fn initialize(url: &str) -> js_sys::Promise;
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::initialize;

#[cfg(target_arch = "wasm32")]
fn platform_compile(source: &str, entry: &str) -> Result<String, String> {
    browser::compile(source, entry).map_err(|e| e.as_string().unwrap_or_else(||
        js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string())
            .unwrap_or_else(|| "HLSL compiler failed".into())))
}
