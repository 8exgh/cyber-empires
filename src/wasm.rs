//! C ABI exported to the JavaScript host (no wasm-bindgen).

use crate::app::{App, SCREEN_H, SCREEN_W};
use std::cell::RefCell;

#[link(wasm_import_module = "env")]
extern "C" {
    fn js_store(slot: i32, ptr: *const u8, len: usize);
    fn js_fetch(slot: i32, ptr: *mut u8, max: usize) -> i32;
}

struct JsStorage;
impl crate::app::Storage for JsStorage {
    fn store(&mut self, slot: usize, data: &[u8]) {
        unsafe { js_store(slot as i32, data.as_ptr(), data.len()) }
    }
    fn fetch(&mut self, slot: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; 32768];
        let n = unsafe { js_fetch(slot as i32, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            None
        } else {
            buf.truncate(n as usize);
            Some(buf)
        }
    }
}

thread_local! {
    static APP: RefCell<Option<Box<App>>> = RefCell::new(None);
    static LAST_ERROR: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        if b.is_none() {
            let mut app = Box::new(App::new());
            app.storage = Box::new(JsStorage);
            *b = Some(app);
        }
        f(b.as_mut().unwrap())
    })
}

/// Allocate a buffer of `size` bytes owned by the module; JS fills it then passes it to ce_add_asset.
#[no_mangle]
pub extern "C" fn ce_alloc(size: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(size.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Register an asset file. Takes ownership of the data buffer previously obtained from ce_alloc.
#[no_mangle]
pub extern "C" fn ce_add_asset(name_ptr: *const u8, name_len: usize, data_ptr: *mut u8, data_len: usize) {
    let name = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len)) }.to_string();
    let data = unsafe { Vec::from_raw_parts(data_ptr, data_len, data_len.max(1)) };
    with_app(|a| a.add_asset(&name, data));
}

/// Start the game. Returns 0 on success, otherwise the length of the error text (see ce_error_ptr).
#[no_mangle]
pub extern "C" fn ce_start(seed: u32) -> i32 {
    match with_app(|a| a.start(seed)) {
        Ok(()) => 0,
        Err(e) => {
            let n = e.len() as i32;
            LAST_ERROR.with(|l| *l.borrow_mut() = e.into_bytes());
            n
        }
    }
}

#[no_mangle]
pub extern "C" fn ce_error_ptr() -> *const u8 {
    LAST_ERROR.with(|l| l.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn ce_mouse(x: i32, y: i32, buttons: i32) {
    with_app(|a| {
        a.input.mouse_x = x;
        a.input.mouse_y = y;
        a.input.mouse_down = buttons & 1 != 0;
        a.input.mouse_right = buttons & 2 != 0;
    });
}

#[no_mangle]
pub extern "C" fn ce_key(code: i32, down: i32) {
    with_app(|a| {
        if (0..256).contains(&code) {
            a.input.keys[code as usize] = down != 0;
        }
    });
}

#[no_mangle]
pub extern "C" fn ce_typed(ch: i32) {
    with_app(|a| {
        if (32..127).contains(&ch) {
            a.input.typed.push(ch as u8);
        }
    });
}

#[no_mangle]
pub extern "C" fn ce_frame(dt: f32) {
    with_app(|a| a.frame(dt));
}

#[no_mangle]
pub extern "C" fn ce_framebuffer() -> *const u8 {
    with_app(|a| a.rgba.as_ptr())
}

#[no_mangle]
pub extern "C" fn ce_width() -> i32 {
    SCREEN_W as i32
}

#[no_mangle]
pub extern "C" fn ce_height() -> i32 {
    SCREEN_H as i32
}

#[no_mangle]
pub extern "C" fn ce_cursor_visible() -> i32 {
    with_app(|a| a.cursor_visible as i32)
}
