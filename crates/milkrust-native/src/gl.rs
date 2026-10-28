//! The only OpenGL pointer loader. Symbols are required up front so unsupported
//! contexts fail creation, rather than jumping through a null pointer later.
use std::ffi::{c_char, c_void, CString};
pub type Loader = unsafe extern "C" fn(*const c_char) -> *const c_void;

macro_rules! functions {
    ($( $field:ident : $name:literal ( $($arg:ty),* ) $(-> $ret:ty)?; )*) => {
        #[derive(Clone, Copy)]
        pub struct Gl { $(pub $field: unsafe extern "system" fn($($arg),*) $(-> $ret)?,)* }
        impl Gl {
            /// # Safety
            /// Loader must return valid function pointers from a current GL
            /// context with the exact standard ABI, and outlive this table.
            pub unsafe fn load(loader: Loader) -> Result<Self, String> {
                Ok(Self { $($field: {
                    let name = CString::new($name).map_err(|_| "Invalid GL symbol")?;
                    let raw = unsafe { loader(name.as_ptr()) };
                    if raw.is_null() || (raw as usize) < 4096 || raw as usize == usize::MAX {
                        return Err(format!("Required GL symbol unavailable: {}", $name));
                    }
                    // Each expansion names one concrete OpenGL function type.
                    unsafe { std::mem::transmute::<*const c_void, unsafe extern "system" fn($($arg),*) $(-> $ret)?>(raw) }
                },)* })
            }
        }
    }
}
functions! {
    create_shader: "glCreateShader" (u32) -> u32;
    shader_source: "glShaderSource" (u32, i32, *const *const c_char, *const i32);
    compile_shader: "glCompileShader" (u32);
    get_shader_iv: "glGetShaderiv" (u32, u32, *mut i32);
    get_shader_log: "glGetShaderInfoLog" (u32, i32, *mut i32, *mut c_char);
    delete_shader: "glDeleteShader" (u32);
    create_program: "glCreateProgram" () -> u32;
    attach_shader: "glAttachShader" (u32, u32);
    bind_attrib: "glBindAttribLocation" (u32, u32, *const c_char);
    link_program: "glLinkProgram" (u32);
    get_program_iv: "glGetProgramiv" (u32, u32, *mut i32);
    get_program_log: "glGetProgramInfoLog" (u32, i32, *mut i32, *mut c_char);
    delete_program: "glDeleteProgram" (u32);
    use_program: "glUseProgram" (u32);
    uniform_location: "glGetUniformLocation" (u32, *const c_char) -> i32;
    uniform_1i: "glUniform1i" (i32, i32);
    uniform_1f: "glUniform1f" (i32, f32);
    uniform_2f: "glUniform2f" (i32, f32, f32);
    uniform_3f: "glUniform3f" (i32, f32, f32, f32);
    uniform_4f: "glUniform4f" (i32, f32, f32, f32, f32);
    uniform_1fv: "glUniform1fv" (i32, i32, *const f32);
    gen_buffers: "glGenBuffers" (i32, *mut u32);
    bind_buffer: "glBindBuffer" (u32, u32);
    buffer_data: "glBufferData" (u32, isize, *const c_void, u32);
    delete_buffers: "glDeleteBuffers" (i32, *const u32);
    gen_vertex_arrays: "glGenVertexArrays" (i32, *mut u32);
    bind_vertex_array: "glBindVertexArray" (u32);
    delete_vertex_arrays: "glDeleteVertexArrays" (i32, *const u32);
    enable_attrib: "glEnableVertexAttribArray" (u32);
    vertex_attrib: "glVertexAttribPointer" (u32, i32, u32, u8, i32, *const c_void);
    gen_textures: "glGenTextures" (i32, *mut u32);
    bind_texture: "glBindTexture" (u32, u32);
    active_texture: "glActiveTexture" (u32);
    tex_image: "glTexImage2D" (u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
    tex_parameter: "glTexParameteri" (u32, u32, i32);
    pixel_store: "glPixelStorei" (u32, i32);
    delete_textures: "glDeleteTextures" (i32, *const u32);
    gen_framebuffers: "glGenFramebuffers" (i32, *mut u32);
    bind_framebuffer: "glBindFramebuffer" (u32, u32);
    framebuffer_texture: "glFramebufferTexture2D" (u32, u32, u32, u32, i32);
    check_framebuffer: "glCheckFramebufferStatus" (u32) -> u32;
    delete_framebuffers: "glDeleteFramebuffers" (i32, *const u32);
    read_pixels: "glReadPixels" (i32, i32, i32, i32, u32, u32, *mut c_void);
    viewport: "glViewport" (i32, i32, i32, i32);
    clear_color: "glClearColor" (f32, f32, f32, f32);
    clear: "glClear" (u32);
    enable: "glEnable" (u32);
    disable: "glDisable" (u32);
    blend_func_separate: "glBlendFuncSeparate" (u32, u32, u32, u32);
    draw_arrays: "glDrawArrays" (u32, i32, i32);
    get_error: "glGetError" () -> u32;
    get_integer: "glGetIntegerv" (u32, *mut i32);
}

pub const TEXTURE_2D: u32 = 0x0DE1;
pub const FRAMEBUFFER: u32 = 0x8D40;
pub const COLOR_BUFFER: u32 = 0x4000;
pub const BLEND: u32 = 0x0BE2;
pub const SRC_ALPHA: u32 = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const ARRAY_BUFFER: u32 = 0x8892;
pub const TEXTURE0: u32 = 0x84C0;
