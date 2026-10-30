//! Native MilkRust rendering boundary. Foreign pointers and GL calls are
//! isolated here; the rest of the workspace retains unsafe_code=forbid.
mod audio;
mod gl;
mod renderer;

use std::{cell::RefCell, collections::BTreeMap, panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicU64, Ordering}};
use milkrust_core::{MilkRustClockState, MilkRustFrameSetRuntime, MilkRustInputState, validate_milkrust_webgl_import_with_state};
use audio::Audio;
use renderer::Renderer;

const ERROR: i32 = -1;
const INVALID_HANDLE: i32 = -2;
const PANIC: i32 = -3;
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static SESSIONS: RefCell<BTreeMap<u64, Session>> = RefCell::new(BTreeMap::new());
    static LAST_ERROR: RefCell<String> = RefCell::new(String::new());
}
struct Session {
    renderer: Renderer,
    runtime: MilkRustFrameSetRuntime,
    audio: Audio,
    input: MilkRustInputState,
    source: String,
    texture_names: String,
    last_time: Option<f64>,
    failed: bool,
}
fn error(message: impl Into<String>) {
    let mut message = message.into();
    if message.len() > 8192 {
        let mut end = 8192; while !message.is_char_boundary(end) { end -= 1; }
        message.truncate(end);
    }
    LAST_ERROR.with(|slot| { if let Ok(mut slot) = slot.try_borrow_mut() { *slot = message; } });
}
fn call(handle: u64, operation: impl FnOnce(&mut Session) -> Result<(), String>) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| SESSIONS.with(|sessions| {
        let mut sessions = sessions.try_borrow_mut().map_err(|_| (ERROR, "Reentrant native session call".to_string()))?;
        let session = sessions.get_mut(&handle).ok_or((INVALID_HANDLE, "Invalid handle or wrong render thread".to_string()))?;
        operation(session).map_err(|message| (ERROR, message))
    })));
    match result {
        Ok(Ok(())) => { error(""); 0 }
        Ok(Err((code, message))) => { error(message); code }
        Err(_) => {
            // Do not continue a partially mutated renderer after a Rust panic.
            SESSIONS.with(|sessions| { if let Ok(mut sessions) = sessions.try_borrow_mut() {
                if let Some(session) = sessions.get_mut(&handle) { session.failed = true; }
            }});
            error("Native renderer panicked; destroy and recreate this session"); PANIC
        }
    }
}

/// # Safety
/// Pointer must be readable for length bytes, with no concurrent writes. The
/// public C contract bounds lengths before any slice or allocation is created.
unsafe fn bytes<'a>(pointer: *const u8, length: usize, maximum: usize) -> Result<&'a [u8], String> {
    if length > maximum || (length != 0 && pointer.is_null()) { return Err("Invalid native input buffer".into()); }
    if length == 0 { return Ok(&[]); }
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}

#[no_mangle]
pub extern "C" fn milkrust_abi_version() -> u32 { 2 }

/// # Safety
/// A current dedicated GL context and a valid standard GL loader are required.
/// The host must obey the thread/context lifetime contract in milkrust.h.
#[no_mangle]
pub unsafe extern "C" fn milkrust_create(loader: Option<gl::Loader>, gles: i32) -> u64 {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<u64, String> {
        if gles != 0 && gles != 1 { return Err("Unknown GL context profile".into()); }
        SESSIONS.with(|sessions| {
            let mut sessions = sessions.try_borrow_mut().map_err(|_| "Reentrant native session creation")?;
            if sessions.len() >= 4 { return Err("At most four native sessions per render thread".into()); }
            let gl = unsafe { gl::Gl::load(loader.ok_or("Missing OpenGL loader")?)? };
            let renderer = unsafe { Renderer::new(gl, gles == 1)? };
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            if handle == 0 { return Err("Native session identity exhausted".into()); }
            sessions.insert(handle, Session { renderer, runtime: MilkRustFrameSetRuntime::default(),
                audio: Audio::default(), input: MilkRustInputState::default(), source: String::new(),
                texture_names: String::new(), last_time: None, failed: false });
            Ok(handle)
        })
    }));
    match result { Ok(Ok(handle)) => { error(""); handle }, Ok(Err(message)) => { error(message); 0 },
        Err(_) => { error("Native renderer creation panicked"); 0 } }
}

/// # Safety
/// text_bytes must be readable for length bytes, at most 2 MiB. See milkrust.h.
#[no_mangle]
pub unsafe extern "C" fn milkrust_load(handle: u64, text_bytes: *const u8, length: usize) -> i32 {
    call(handle, |session| {
        let decoded = milkrust_core::text::decode_preset(unsafe { bytes(text_bytes, length, 2*1024*1024)? })?;
        let source = decoded.source.as_str();
        // Uses the same explicitly implemented stage family as WebGL. A failed
        // translation/unsupported saved pattern never becomes a fallback scene.
        validate_milkrust_webgl_import_with_state(source, session.runtime.milk2_header_state())?;
        let parsed = milkrust_core::parse_milkrust_preset_set(source, false);
        let mut textures = parsed.embedded_sprites.iter().map(|sprite| sprite.texture.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for preset in &parsed.presets {
            for shader in [&preset.warp_shader, &preset.comp_shader] {
                textures.extend(milkrust_core::get_milkrust_shader_texture_samplers(shader).into_iter()
                    .filter(|name| !milkrust_core::resources::texture_kind(name).is_builtin())
                    .map(|name| milkrust_core::resources::base_name(&name).to_string()));
            }
            for object in preset.shapes.iter().chain(&preset.sprites) {
                for key in ["texture", "tex", "tex_name", "texname", "image", "img", "file", "filename"] {
                    if let Some(value) = object.base_values.get(key) {
                        let name = value.as_text().trim().trim_matches(|ch| ch == '\'' || ch == '"').to_string();
                        if !name.is_empty() { textures.insert(name); break; }
                    }
                }
            }
        }
        if textures.len() > 100 || textures.iter().any(|name| name.len() > 512 || name.chars().any(char::is_control)) {
            return Err("Preset texture dependency list exceeds its limits".into());
        }
        session.texture_names = textures.into_iter().collect::<Vec<_>>().join("\n");
        session.renderer.reset();
        let header_state = session.runtime.milk2_header_state().clone();
        session.runtime = MilkRustFrameSetRuntime::default();
        session.runtime.restore_milk2_header_state(header_state)?;
        session.source = source.to_string(); session.last_time = None; session.failed = false;
        Ok(())
    })
}

/// # Safety
/// output is writable for capacity bytes, or null with capacity zero. Returns
/// required bytes including NUL; zero denotes a handle/error failure. A small
/// buffer is left untouched, so callers cannot mistake a truncated list.
#[no_mangle]
pub unsafe extern "C" fn milkrust_texture_names(handle: u64, output: *mut u8, capacity: usize) -> usize {
    let mut required = 0;
    let code = call(handle, |session| {
        required = session.texture_names.len()+1;
        if capacity >= required {
            if output.is_null() { return Err("Null texture-name output".into()); }
            unsafe { std::ptr::copy_nonoverlapping(session.texture_names.as_ptr(), output, required-1); *output.add(required-1) = 0; }
        }
        Ok(())
    });
    if code == 0 { required } else { 0 }
}

/// # Safety
/// Both byte buffers must be readable at the stated lengths; RGBA length must
/// be width*height*4 and at most 32 MiB. Texture names must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn milkrust_texture(handle: u64, name: *const u8, name_length: usize,
    width: u32, height: u32, rgba: *const u8, length: usize) -> i32 {
    call(handle, |session| {
        let name = std::str::from_utf8(unsafe { bytes(name, name_length, 512)? }).map_err(|_| "Texture name must be UTF-8")?;
        let rgba = unsafe { bytes(rgba, length, 32*1024*1024)? };
        session.renderer.texture(name, width, height, rgba)
    })
}

/// # Safety
/// Name and encoded GIF bytes must be readable; names are at most 512 bytes and
/// encoded data is at most 16 MiB. Decoded frames share the preset pixel budget.
#[no_mangle]
pub unsafe extern "C" fn milkrust_texture_gif(handle: u64, name: *const u8, name_length: usize,
    encoded: *const u8, length: usize) -> i32 {
    call(handle, |session| {
        let name = std::str::from_utf8(unsafe { bytes(name, name_length, 512)? }).map_err(|_| "Texture name must be UTF-8")?;
        let encoded = unsafe { bytes(encoded, length, milkrust_core::texture::MAX_ENCODED_BYTES)? };
        let (width, height) = milkrust_core::texture::inspect(milkrust_core::texture::Format::Gif, encoded)?;
        let source = milkrust_core::texture::ImageSource {
            format: milkrust_core::texture::Format::Gif, width, height, bytes: encoded.to_vec(),
        };
        let animation = source.decode_gif()?;
        let frames = animation.frames.iter().map(|frame| frame.pixels.as_slice()).collect::<Vec<_>>();
        session.renderer.texture_animation(name, width, height, &frames, &animation.delays_seconds,
            animation.repeat_count)
    })
}

/// Update local calendar/time registers used by EEL equations. All values are
/// validated before the runtime accepts them; the host should refresh per frame.
#[no_mangle]
pub extern "C" fn milkrust_set_clock(handle: u64, year: i32, month: u32, day: u32,
    weekday: u32, hour: u32, minute: u32, second: u32, milliseconds: u32) -> i32 {
    call(handle, |session| session.runtime.set_clock_state(MilkRustClockState {
        year, month, day, weekday, hour, minute, second, milliseconds,
    }))
}

/// Update normalized pointer input for following frames. Call on the render thread.
/// mouse_down is exactly 0 or 1; non-finite values are rejected.
#[no_mangle]
pub extern "C" fn milkrust_set_mouse(handle: u64, mouse_down: i32, mouse_x: f64,
    mouse_y: f64, mouse_dx: f64, mouse_dy: f64) -> i32 {
    call(handle, |session| {
        if !matches!(mouse_down, 0 | 1) || !mouse_x.is_finite() || !mouse_y.is_finite()
            || !mouse_dx.is_finite() || !mouse_dy.is_finite() {
            return Err("Invalid native mouse input".into());
        }
        session.input = MilkRustInputState {
            mouse_down: f64::from(mouse_down),
            mouse_x: mouse_x.clamp(0.0, 1.0),
            mouse_y: mouse_y.clamp(0.0, 1.0),
            mouse_dx,
            mouse_dy,
        };
        Ok(())
    })
}

/// # Safety
/// stereo must point to frames*2 aligned readable float samples (<=4096 frames).
#[no_mangle]
pub unsafe extern "C" fn milkrust_pcm(handle: u64, stereo: *const f32, frames: usize, generation: u32) -> i32 {
    call(handle, |session| {
        if frames > 4096 || (frames != 0 && (stereo.is_null() || stereo as usize % std::mem::align_of::<f32>() != 0)) {
            return Err("Invalid stereo PCM buffer".into());
        }
        let samples = if frames == 0 { &[] } else { unsafe { std::slice::from_raw_parts(stereo, frames*2) } };
        session.audio.push(samples, generation)
    })
}

fn render_session(session: &mut Session, elapsed: f64,
    draw: impl FnOnce(&mut Renderer, &milkrust_core::MilkRustFrameSet) -> Result<(), String>) -> Result<(), String> {
    if session.failed { return Err("Renderer stopped after a failure; reload the preset to retry".into()); }
    if session.source.is_empty() { return Err("No native preset is loaded".into()); }
    if !elapsed.is_finite() || elapsed < 0.0 || session.last_time.is_some_and(|previous| elapsed < previous) {
        return Err("Render time must be finite and monotonic".into());
    }
    let audio = session.audio.analyze(elapsed);
    session.runtime.set_stereo_audio(audio.stereo)?;
    let result = session.runtime.try_render_source_with_audio_and_input(&session.source, elapsed,
        audio.bands[0], audio.bands[1], audio.bands[2], &audio.waveform, &audio.spectrum, session.input)
        .and_then(|frames| draw(&mut session.renderer, &frames));
    if result.is_err() { session.failed = true; } else { session.last_time = Some(elapsed); }
    result
}

/// # Safety
/// framebuffer must belong to the current, creating GL context. See milkrust.h.
#[no_mangle]
pub unsafe extern "C" fn milkrust_render(handle: u64, framebuffer: u32, width: u32, height: u32, elapsed: f64) -> i32 {
    call(handle, |session| {
        session.runtime.set_viewport(width, height)?;
        render_session(session, elapsed, |renderer, frames| renderer.render(frames, framebuffer, width, height, elapsed))
    })
}

/// # Safety
/// output must be writable for exactly width*height*4 bytes without concurrent
/// access. The creating GL context must be current; the renderer owns the FBO.
#[no_mangle]
pub unsafe extern "C" fn milkrust_render_rgba(handle: u64, width: u32, height: u32, elapsed: f64,
    output: *mut u8, length: usize) -> i32 {
    call(handle, |session| {
        if width == 0 || height == 0 || width > 4096 || height > 4096
            || u64::from(width)*u64::from(height) > 8*1024*1024
            || length as u64 != u64::from(width)*u64::from(height)*4 || output.is_null() {
            return Err("Invalid RGBA output buffer".into());
        }
        session.runtime.set_viewport(width, height)?;
        let output = unsafe { std::slice::from_raw_parts_mut(output, length) };
        render_session(session, elapsed, |renderer, frames| renderer.capture(frames, width, height, elapsed, output))
    })
}

/// # Safety
/// The creating context must still be current. Stale and wrong-thread handles
/// fail without dereferencing freed session pointers.
#[no_mangle]
pub unsafe extern "C" fn milkrust_destroy(handle: u64) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| SESSIONS.with(|sessions| {
        sessions.try_borrow_mut().map_err(|_| ERROR)?.remove(&handle).ok_or(INVALID_HANDLE)?;
        Ok::<(), i32>(())
    }))) {
        Ok(Ok(())) => { error(""); 0 },
        Ok(Err(code)) => { error("Invalid, reentrant or wrong-thread destroy"); code },
        Err(_) => { error("Native renderer destruction panicked"); PANIC },
    }
}

/// # Safety
/// output must be writable for capacity bytes. Null is accepted for capacity 0.
#[no_mangle]
pub unsafe extern "C" fn milkrust_last_error(output: *mut u8, capacity: usize) -> usize {
    LAST_ERROR.with(|slot| {
        let message = slot.borrow();
        if capacity != 0 && !output.is_null() {
            let mut length = message.len().min(capacity-1);
            while !message.is_char_boundary(length) { length -= 1; }
            unsafe { std::ptr::copy_nonoverlapping(message.as_ptr(), output, length); *output.add(length) = 0; }
        }
        message.len()+1
    })
}
