fn main() {
    println!("cargo:rerun-if-changed=vendor/hlslparser");
    println!("cargo:rerun-if-changed=src/bridge.cpp");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        // The browser loads the same compiler as an Emscripten ES module.
        return;
    }
    let mut build = cc::Build::new();
    build.cpp(true).std("c++11").warnings(false)
        .include("vendor/hlslparser/src").file("src/bridge.cpp");
    for name in ["Engine", "CodeWriter", "HLSLTokenizer", "HLSLParser", "HLSLTree", "GLSLGenerator"] {
        build.file(format!("vendor/hlslparser/src/{name}.cpp"));
    }
    build.flag_if_supported("/EHsc").compile("milkrust_hlsl");
}
