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
