// Takes a picture of Markdownr's window on X11 from outside (the Linux counterpart of
// tools/mdr-shot.ps1): finds the window by its WM_CLASS ("markdownr"), optionally sends it
// keys (synthetic KeyPress events to that window only), waits, and saves that window's
// contents (XGetImage on it, never the screen) as PNG via cairo.
//
//   rustc --edition 2024 -O tools/mdr-shot-x11.rs -o mdr-shot-x11 -C link-arg=-l:libX11.so.6 -C link-arg=-l:libcairo.so.2
//   ./mdr-shot-x11 out.png [--wait-ms 1500] [--keys F1,ctrl+t,...] [--type text] [--scroll N]
#![allow(non_snake_case)]
use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_ulong, c_void};

#[repr(C)]
struct XWindowAttributes {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    rest: [c_long; 32],
}

unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XDefaultRootWindow(d: *mut c_void) -> c_ulong;
    fn XQueryTree(d: *mut c_void, w: c_ulong, root: *mut c_ulong, parent: *mut c_ulong, children: *mut *mut c_ulong, n: *mut c_uint) -> c_int;
    fn XFree(p: *mut c_void) -> c_int;
    fn XGetWindowProperty(d: *mut c_void, w: c_ulong, prop: c_ulong, off: c_long, len: c_long, delete: c_int, req: c_ulong, actual: *mut c_ulong, format: *mut c_int, n: *mut c_ulong, after: *mut c_ulong, data: *mut *mut u8) -> c_int;
    fn XInternAtom(d: *mut c_void, name: *const c_char, only: c_int) -> c_ulong;
    fn XGetWindowAttributes(d: *mut c_void, w: c_ulong, a: *mut XWindowAttributes) -> c_int;
    fn XGetImage(d: *mut c_void, w: c_ulong, x: c_int, y: c_int, width: c_uint, height: c_uint, planes: c_ulong, format: c_int) -> *mut c_void;
    fn XKeysymToKeycode(d: *mut c_void, k: c_ulong) -> u8;
    fn XStringToKeysym(s: *const c_char) -> c_ulong;
    fn XSendEvent(d: *mut c_void, w: c_ulong, propagate: c_int, mask: c_long, ev: *mut [c_long; 24]) -> c_int;
    fn XFlush(d: *mut c_void) -> c_int;
    fn XSync(d: *mut c_void, discard: c_int) -> c_int;
    fn cairo_image_surface_create_for_data(data: *mut u8, format: c_int, w: c_int, h: c_int, stride: c_int) -> *mut c_void;
    fn cairo_surface_write_to_png(s: *mut c_void, path: *const c_char) -> c_int;
}

fn class_of(d: *mut c_void, w: c_ulong, wm_class: c_ulong) -> Vec<u8> {
    let (mut actual, mut format, mut n, mut after, mut data) = (0, 0, 0, 0, std::ptr::null_mut());
    unsafe {
        if XGetWindowProperty(d, w, wm_class, 0, 64, 0, 0, &mut actual, &mut format, &mut n, &mut after, &mut data) != 0 || data.is_null() {
            return Vec::new();
        }
        let v = std::slice::from_raw_parts(data, n as usize).to_vec();
        XFree(data as *mut _);
        v
    }
}

fn find(d: *mut c_void, w: c_ulong, wm_class: c_ulong) -> Option<c_ulong> {
    if class_of(d, w, wm_class).starts_with(b"markdownr\0") {
        return Some(w);
    }
    let (mut root, mut parent, mut kids, mut n) = (0, 0, std::ptr::null_mut(), 0);
    unsafe {
        if XQueryTree(d, w, &mut root, &mut parent, &mut kids, &mut n) == 0 {
            return None;
        }
        let list = if kids.is_null() { Vec::new() } else { std::slice::from_raw_parts(kids, n as usize).to_vec() };
        if !kids.is_null() {
            XFree(kids as *mut _);
        }
        list.into_iter().rev().find_map(|k| find(d, k, wm_class))
    }
}

fn key(d: *mut c_void, w: c_ulong, spec: &str) {
    let mut state = 0u32;
    let mut name = spec;
    while let Some((m, rest)) = name.split_once('+').filter(|(m, r)| !r.is_empty() && matches!(m.to_ascii_lowercase().as_str(), "ctrl" | "shift")) {
        state |= if m.eq_ignore_ascii_case("ctrl") { 4 } else { 1 };
        name = rest;
    }
    let sym = unsafe { XStringToKeysym(CString::new(name).unwrap().as_ptr()) };
    let code = unsafe { XKeysymToKeycode(d, sym) };
    let mut ev = [0 as c_long; 24];
    let p = ev.as_mut_ptr() as *mut u8;
    unsafe {
        *(p as *mut c_int) = 2; // KeyPress
        *(p.add(16) as *mut c_int) = 1;
        *(p.add(32) as *mut c_ulong) = w;
        *(p.add(80) as *mut c_uint) = state;
        *(p.add(84) as *mut c_uint) = code as c_uint;
        *(p.add(88) as *mut c_int) = 1;
        XSendEvent(d, w, 1, 1, &mut ev);
    }
}

fn wheel(d: *mut c_void, w: c_ulong, down: bool) {
    let mut ev = [0 as c_long; 24];
    let p = ev.as_mut_ptr() as *mut u8;
    unsafe {
        *(p as *mut c_int) = 4; // ButtonPress
        *(p.add(16) as *mut c_int) = 1;
        *(p.add(32) as *mut c_ulong) = w;
        *(p.add(64) as *mut c_int) = 300;
        *(p.add(68) as *mut c_int) = 300;
        *(p.add(84) as *mut c_uint) = if down { 5 } else { 4 };
        *(p.add(88) as *mut c_int) = 1;
        XSendEvent(d, w, 1, 4, &mut ev);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).expect("out.png");
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let wait: u64 = opt("--wait-ms").and_then(|v| v.parse().ok()).unwrap_or(1500);
    unsafe {
        let d = XOpenDisplay(std::ptr::null());
        assert!(!d.is_null(), "no display");
        let wm_class = XInternAtom(d, c"WM_CLASS".as_ptr(), 0);
        let mut win = None;
        for _ in 0..200 {
            win = find(d, XDefaultRootWindow(d), wm_class);
            if win.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let w = win.expect("no markdownr window");
        std::thread::sleep(std::time::Duration::from_millis(wait / 2));
        for k in opt("--keys").unwrap_or_default().split(',').filter(|k| !k.is_empty()) {
            key(d, w, k);
            XFlush(d);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        for c in opt("--type").unwrap_or_default().chars() {
            key(d, w, &c.to_string());
            XFlush(d);
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        let scroll: i32 = opt("--scroll").and_then(|v| v.parse().ok()).unwrap_or(0);
        for _ in 0..scroll.abs() {
            wheel(d, w, scroll > 0);
        }
        XSync(d, 0);
        std::thread::sleep(std::time::Duration::from_millis(wait / 2));
        let mut a: XWindowAttributes = std::mem::zeroed();
        XGetWindowAttributes(d, w, &mut a);
        let img = XGetImage(d, w, 0, 0, a.width as c_uint, a.height as c_uint, !0, 2);
        assert!(!img.is_null(), "XGetImage failed");
        let data = *((img as *mut u8).add(16) as *mut *mut u8);
        let stride = *((img as *mut u8).add(44) as *mut c_int);
        let s = cairo_image_surface_create_for_data(data, 1, a.width, a.height, stride);
        let path = CString::new(out.as_str()).unwrap();
        assert_eq!(cairo_surface_write_to_png(s, path.as_ptr()), 0, "write png");
        println!("{out}: {}x{}", a.width, a.height);
    }
}
