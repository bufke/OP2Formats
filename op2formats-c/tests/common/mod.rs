//! Helpers for calling the C interface from the tests the way C does.

#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char};
use std::ptr::null_mut;

use op2formats::*;

pub fn last_error() -> String {
    let mut buf = [0 as c_char; 512];
    unsafe { op2f_last_error(buf.as_mut_ptr(), buf.len()) };
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

/// Asserts a call succeeded, showing the library's message when it didn't.
pub fn ok(status: Op2fStatus) {
    assert_eq!(status, Op2fStatus::Ok, "{}", last_error());
}

/// `$OP2_DIR`, or `None` (and a note) when it isn't set; `OP2_REQUIRE_DATA` makes that a failure.
pub fn game_dir() -> Option<String> {
    let Some(dir) = std::env::var_os("OP2_DIR").filter(|d| !d.is_empty()) else {
        assert!(std::env::var_os("OP2_REQUIRE_DATA").is_none(), "OP2_REQUIRE_DATA is set but OP2_DIR isn't");
        eprintln!("skipping: set OP2_DIR to an Outpost 2 install");
        return None;
    };
    Some(dir.into_string().expect("OP2_DIR is UTF-8"))
}

pub fn game() -> Option<*mut Op2fGame> {
    let dir = c(&game_dir()?);
    let mut game = null_mut();
    ok(unsafe { op2f_game_open(dir.as_ptr(), &mut game) });
    Some(game)
}

pub fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// Reads a string through the length query and then a buffer of exactly that size.
pub fn string(mut f: impl FnMut(*mut c_char, usize, *mut usize) -> Op2fStatus) -> String {
    let mut len = 0;
    ok(f(null_mut(), 0, &mut len));
    let mut buf = vec![0 as c_char; len + 1];
    ok(f(buf.as_mut_ptr(), buf.len(), &mut len));
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

/// A fixed-size C string field, as text.
pub fn field(chars: &[c_char]) -> String {
    unsafe { CStr::from_ptr(chars.as_ptr()) }.to_string_lossy().into_owned()
}

/// Every string of a string list, freeing it.
pub fn list(l: *mut Op2fStringList) -> Vec<String> {
    let out = (0..unsafe { op2f_string_list_count(l) }).map(|i| string(|b, c, n| unsafe { op2f_string_list_get(l, i, b, c, n) })).collect();
    unsafe { op2f_string_list_free(l) };
    out
}

/// The bytes of a block, freeing it.
pub fn bytes(b: *mut Op2fBytes) -> Vec<u8> {
    let out = unsafe { std::slice::from_raw_parts(op2f_bytes_data(b), op2f_bytes_len(b)) }.to_vec();
    unsafe { op2f_bytes_free(b) };
    out
}

/// An image's width, height, hotspot and pixels, freeing it.
pub fn image(i: *mut Op2fImage) -> (u32, u32, (i32, i32), Vec<u8>) {
    let (w, h) = unsafe { (op2f_image_width(i), op2f_image_height(i)) };
    let hotspot = unsafe { (op2f_image_hotspot_x(i), op2f_image_hotspot_y(i)) };
    let rgba = unsafe { std::slice::from_raw_parts(op2f_image_rgba(i), (w * h * 4) as usize) }.to_vec();
    unsafe { op2f_image_free(i) };
    (w, h, hotspot, rgba)
}
