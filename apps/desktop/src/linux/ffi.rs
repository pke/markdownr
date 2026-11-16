//! The C interfaces the Linux shell uses, declared here (stable C ABIs of the system's
//! libraries), so building needs no headers and no binding crates. Linked against the
//! runtime libraries by name (build.rs: `-l:libcairo.so.2` …), so no -dev packages either.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use core::ffi::{c_char, c_double, c_int, c_long, c_uint, c_ulong, c_void};

// ------------------------------------------------------------------- Xlib

pub type Display = c_void;
pub type Window = c_ulong;
pub type Drawable = c_ulong;
pub type Pixmap = c_ulong;
pub type Atom = c_ulong;
pub type KeySym = c_ulong;
pub type Cursor = c_ulong;
pub type Time = c_ulong;
pub type XIM = *mut c_void;
pub type XIC = *mut c_void;
pub type GC = *mut c_void;
pub type Visual = c_void;

/// XEvent: a union of 24 longs; fields are read by offset (see `mod.rs`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XEvent {
    pub pad: [c_long; 24],
}

impl XEvent {
    pub fn zeroed() -> Self {
        XEvent { pad: [0; 24] }
    }
    pub fn kind(&self) -> c_int {
        self.i32(0)
    }
    pub fn i32(&self, off: usize) -> c_int {
        // SAFETY: off < 192 and 4-aligned by the callers.
        unsafe { *((self.pad.as_ptr() as *const u8).add(off) as *const c_int) }
    }
    pub fn u32(&self, off: usize) -> c_uint {
        self.i32(off) as c_uint
    }
    pub fn ulong(&self, off: usize) -> c_ulong {
        // SAFETY: off < 192 and 8-aligned by the callers.
        unsafe { *((self.pad.as_ptr() as *const u8).add(off) as *const c_ulong) }
    }
    pub fn set_i32(&mut self, off: usize, v: c_int) {
        unsafe { *((self.pad.as_mut_ptr() as *mut u8).add(off) as *mut c_int) = v };
    }
    pub fn set_ulong(&mut self, off: usize, v: c_ulong) {
        unsafe { *((self.pad.as_mut_ptr() as *mut u8).add(off) as *mut c_ulong) = v };
    }
}

pub const KEY_PRESS: c_int = 2;
pub const BUTTON_PRESS: c_int = 4;
pub const BUTTON_RELEASE: c_int = 5;
pub const MOTION_NOTIFY: c_int = 6;
pub const LEAVE_NOTIFY: c_int = 8;
pub const FOCUS_IN: c_int = 9;
pub const FOCUS_OUT: c_int = 10;
pub const EXPOSE: c_int = 12;
pub const CONFIGURE_NOTIFY: c_int = 22;
pub const SELECTION_REQUEST: c_int = 30;
pub const SELECTION_NOTIFY: c_int = 31;
pub const CLIENT_MESSAGE: c_int = 33;

pub const KEY_PRESS_MASK: c_long = 1;
pub const BUTTON_PRESS_MASK: c_long = 1 << 2;
pub const BUTTON_RELEASE_MASK: c_long = 1 << 3;
pub const LEAVE_WINDOW_MASK: c_long = 1 << 5;
pub const POINTER_MOTION_MASK: c_long = 1 << 6;
pub const EXPOSURE_MASK: c_long = 1 << 15;
pub const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
pub const SUBSTRUCTURE_NOTIFY_MASK: c_long = 1 << 19;
pub const SUBSTRUCTURE_REDIRECT_MASK: c_long = 1 << 20;
pub const FOCUS_CHANGE_MASK: c_long = 1 << 21;

pub const SHIFT_MASK: c_uint = 1;
pub const CONTROL_MASK: c_uint = 4;

pub const XA_ATOM: Atom = 4;
pub const XA_CARDINAL: Atom = 6;
pub const XA_STRING: Atom = 31;


unsafe extern "C" {
    pub fn XOpenDisplay(name: *const c_char) -> *mut Display;
    pub fn XCloseDisplay(d: *mut Display) -> c_int;
    pub fn XDefaultScreen(d: *mut Display) -> c_int;
    pub fn XRootWindow(d: *mut Display, screen: c_int) -> Window;
    pub fn XDefaultVisual(d: *mut Display, screen: c_int) -> *mut Visual;
    pub fn XDefaultDepth(d: *mut Display, screen: c_int) -> c_int;
    pub fn XCreateSimpleWindow(d: *mut Display, parent: Window, x: c_int, y: c_int, w: c_uint, h: c_uint, border: c_uint, border_px: c_ulong, bg: c_ulong) -> Window;
    pub fn XSetWindowBackgroundPixmap(d: *mut Display, w: Window, pixmap: Pixmap) -> c_int;
    pub fn XSelectInput(d: *mut Display, w: Window, mask: c_long) -> c_int;
    pub fn XMapWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XInternAtom(d: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    pub fn XSetWMProtocols(d: *mut Display, w: Window, protocols: *const Atom, n: c_int) -> c_int;
    pub fn XChangeProperty(d: *mut Display, w: Window, property: Atom, kind: Atom, format: c_int, mode: c_int, data: *const u8, n: c_int) -> c_int;
    pub fn XDeleteProperty(d: *mut Display, w: Window, property: Atom) -> c_int;
    pub fn XNextEvent(d: *mut Display, ev: *mut XEvent) -> c_int;
    pub fn XPending(d: *mut Display) -> c_int;
    pub fn XConnectionNumber(d: *mut Display) -> c_int;
    pub fn XFlush(d: *mut Display) -> c_int;
    pub fn XCreatePixmap(d: *mut Display, drawable: Drawable, w: c_uint, h: c_uint, depth: c_uint) -> Pixmap;
    pub fn XFreePixmap(d: *mut Display, p: Pixmap) -> c_int;
    pub fn XCreateGC(d: *mut Display, drawable: Drawable, mask: c_ulong, values: *mut c_void) -> GC;
    pub fn XCopyArea(d: *mut Display, src: Drawable, dst: Drawable, gc: GC, sx: c_int, sy: c_int, w: c_uint, h: c_uint, dx: c_int, dy: c_int) -> c_int;
    pub fn XCreateFontCursor(d: *mut Display, shape: c_uint) -> Cursor;
    pub fn XDefineCursor(d: *mut Display, w: Window, c: Cursor) -> c_int;
    pub fn XSendEvent(d: *mut Display, w: Window, propagate: c_int, mask: c_long, ev: *mut XEvent) -> c_int;
    pub fn XSetSelectionOwner(d: *mut Display, selection: Atom, owner: Window, time: Time) -> c_int;
    pub fn XResourceManagerString(d: *mut Display) -> *const c_char;
    pub fn XSetLocaleModifiers(m: *const c_char) -> *mut c_char;
    pub fn XOpenIM(d: *mut Display, db: *mut c_void, name: *mut c_char, class: *mut c_char) -> XIM;
    pub fn XCreateIC(im: XIM, ...) -> XIC;
    pub fn XSetICFocus(ic: XIC);
    pub fn XUnsetICFocus(ic: XIC);
    pub fn XFilterEvent(ev: *mut XEvent, w: Window) -> c_int;
    pub fn Xutf8LookupString(ic: XIC, ev: *mut XEvent, buf: *mut c_char, len: c_int, keysym: *mut KeySym, status: *mut c_int) -> c_int;
    pub fn XLookupString(ev: *mut XEvent, buf: *mut c_char, len: c_int, keysym: *mut KeySym, compose: *mut c_void) -> c_int;
    pub fn XDestroyWindow(d: *mut Display, w: Window) -> c_int;
}

unsafe extern "C" {
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    pub fn poll(fds: *mut PollFd, n: c_ulong, timeout: c_int) -> c_int;
    pub fn time(t: *mut i64) -> i64;
    pub fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
    pub fn inotify_init1(flags: c_int) -> c_int;
    pub fn inotify_add_watch(fd: c_int, path: *const c_char, mask: u32) -> c_int;
}

pub const IN_NONBLOCK: c_int = 0o4000;
pub const IN_CLOEXEC: c_int = 0o2000000;
/// Written and closed, moved in or out, created, deleted: a file saved in place or by
/// replacing it, and files coming and going.
pub const IN_CHANGES: u32 = 0x8 | 0x40 | 0x80 | 0x100 | 0x200;

pub const LC_CTYPE: c_int = 0;

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

#[repr(C)]
#[derive(Default)]
pub struct Tm {
    pub sec: c_int,
    pub min: c_int,
    pub hour: c_int,
    pub mday: c_int,
    pub mon: c_int,
    pub year: c_int,
    pub wday: c_int,
    pub yday: c_int,
    pub isdst: c_int,
    pub gmtoff: c_long,
    pub zone: usize,
}

// ------------------------------------------------------------------ cairo

pub type cairo_t = c_void;
pub type cairo_surface_t = c_void;
pub type cairo_pattern_t = c_void;
pub type cairo_font_face_t = c_void;
pub type cairo_scaled_font_t = c_void;
pub type cairo_font_options_t = c_void;

#[repr(C)]
pub struct cairo_matrix_t {
    pub xx: c_double,
    pub yx: c_double,
    pub xy: c_double,
    pub yy: c_double,
    pub x0: c_double,
    pub y0: c_double,
}

#[repr(C)]
#[derive(Default)]
pub struct cairo_text_extents_t {
    pub x_bearing: c_double,
    pub y_bearing: c_double,
    pub width: c_double,
    pub height: c_double,
    pub x_advance: c_double,
    pub y_advance: c_double,
}

#[repr(C)]
pub struct cairo_glyph_t {
    pub index: c_ulong,
    pub x: c_double,
    pub y: c_double,
}

pub const CAIRO_FORMAT_ARGB32: c_int = 0;
pub const CAIRO_FILTER_GOOD: c_int = 1;

unsafe extern "C" {
    pub fn cairo_xlib_surface_create(d: *mut Display, drawable: Drawable, visual: *mut Visual, w: c_int, h: c_int) -> *mut cairo_surface_t;
    pub fn cairo_image_surface_create(format: c_int, w: c_int, h: c_int) -> *mut cairo_surface_t;
    pub fn cairo_image_surface_get_data(s: *mut cairo_surface_t) -> *mut u8;
    pub fn cairo_image_surface_get_stride(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_image_surface_get_width(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_image_surface_get_height(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_image_surface_create_from_png_stream(read: unsafe extern "C" fn(*mut c_void, *mut u8, c_uint) -> c_int, closure: *mut c_void) -> *mut cairo_surface_t;
    pub fn cairo_surface_status(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_surface_mark_dirty(s: *mut cairo_surface_t);
    pub fn cairo_surface_flush(s: *mut cairo_surface_t);
    pub fn cairo_surface_set_device_scale(s: *mut cairo_surface_t, x: c_double, y: c_double);
    pub fn cairo_surface_destroy(s: *mut cairo_surface_t);
    pub fn cairo_create(s: *mut cairo_surface_t) -> *mut cairo_t;
    pub fn cairo_destroy(cr: *mut cairo_t);
    pub fn cairo_save(cr: *mut cairo_t);
    pub fn cairo_restore(cr: *mut cairo_t);
    pub fn cairo_set_matrix(cr: *mut cairo_t, m: *const cairo_matrix_t);
    pub fn cairo_translate(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_scale(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_rotate(cr: *mut cairo_t, radians: c_double);
    pub fn cairo_set_source_rgba(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double, a: c_double);
    pub fn cairo_set_source_surface(cr: *mut cairo_t, s: *mut cairo_surface_t, x: c_double, y: c_double);
    pub fn cairo_get_source(cr: *mut cairo_t) -> *mut cairo_pattern_t;
    pub fn cairo_pattern_set_filter(p: *mut cairo_pattern_t, filter: c_int);
    pub fn cairo_paint(cr: *mut cairo_t);
    pub fn cairo_paint_with_alpha(cr: *mut cairo_t, a: c_double);
    pub fn cairo_new_path(cr: *mut cairo_t);
    pub fn cairo_new_sub_path(cr: *mut cairo_t);
    pub fn cairo_move_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_line_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_curve_to(cr: *mut cairo_t, x1: c_double, y1: c_double, x2: c_double, y2: c_double, x3: c_double, y3: c_double);
    pub fn cairo_arc(cr: *mut cairo_t, xc: c_double, yc: c_double, r: c_double, a1: c_double, a2: c_double);
    pub fn cairo_close_path(cr: *mut cairo_t);
    pub fn cairo_rectangle(cr: *mut cairo_t, x: c_double, y: c_double, w: c_double, h: c_double);
    pub fn cairo_fill(cr: *mut cairo_t);
    pub fn cairo_push_group(cr: *mut cairo_t);
    pub fn cairo_pop_group_to_source(cr: *mut cairo_t);
    pub fn cairo_set_font_face(cr: *mut cairo_t, f: *mut cairo_font_face_t);
    pub fn cairo_set_font_size(cr: *mut cairo_t, size: c_double);
    pub fn cairo_show_glyphs(cr: *mut cairo_t, glyphs: *const cairo_glyph_t, n: c_int);
    pub fn cairo_ft_font_face_create_for_pattern(pattern: *mut FcPattern) -> *mut cairo_font_face_t;
    pub fn cairo_scaled_font_create(face: *mut cairo_font_face_t, font_matrix: *const cairo_matrix_t, ctm: *const cairo_matrix_t, options: *const cairo_font_options_t) -> *mut cairo_scaled_font_t;
    pub fn cairo_scaled_font_glyph_extents(f: *mut cairo_scaled_font_t, glyphs: *const cairo_glyph_t, n: c_int, extents: *mut cairo_text_extents_t);
    pub fn cairo_font_options_create() -> *mut cairo_font_options_t;
    pub fn cairo_font_options_set_hint_metrics(o: *mut cairo_font_options_t, v: c_int);
    pub fn cairo_font_options_set_hint_style(o: *mut cairo_font_options_t, v: c_int);
    pub fn cairo_font_options_set_antialias(o: *mut cairo_font_options_t, v: c_int);
}

// ------------------------------------------------------------------ Pango

pub type PangoContext = c_void;
pub type PangoFontMap = c_void;
pub type PangoLayout = c_void;
pub type PangoLayoutIter = c_void;
pub type PangoFontDescription = c_void;
pub type PangoAttrList = c_void;

/// The public head of PangoLayoutLine.
#[repr(C)]
pub struct PangoLayoutLine {
    pub layout: *mut PangoLayout,
    pub start_index: c_int,
    pub length: c_int,
}

/// The public head of PangoAttribute.
#[repr(C)]
pub struct PangoAttribute {
    pub klass: *const c_void,
    pub start_index: c_uint,
    pub end_index: c_uint,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct PangoRectangle {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

pub const PANGO_SCALE: f32 = 1024.0;
pub const PANGO_WRAP_WORD: c_int = 0;
pub const PANGO_WRAP_WORD_CHAR: c_int = 2;
pub const PANGO_STYLE_ITALIC: c_int = 2;
pub const PANGO_UNDERLINE_SINGLE: c_int = 1;

unsafe extern "C" {
    pub fn pango_cairo_font_map_get_default() -> *mut PangoFontMap;
    pub fn pango_font_map_create_context(m: *mut PangoFontMap) -> *mut PangoContext;
    pub fn pango_cairo_context_set_font_options(c: *mut PangoContext, o: *const cairo_font_options_t);
    pub fn pango_layout_new(c: *mut PangoContext) -> *mut PangoLayout;
    pub fn pango_layout_copy(l: *mut PangoLayout) -> *mut PangoLayout;
    pub fn pango_layout_set_text(l: *mut PangoLayout, text: *const c_char, len: c_int);
    pub fn pango_layout_set_attributes(l: *mut PangoLayout, a: *mut PangoAttrList);
    pub fn pango_layout_set_font_description(l: *mut PangoLayout, d: *const PangoFontDescription);
    pub fn pango_layout_set_width(l: *mut PangoLayout, w: c_int);
    pub fn pango_layout_set_wrap(l: *mut PangoLayout, w: c_int);
    pub fn pango_layout_set_alignment(l: *mut PangoLayout, a: c_int);
    pub fn pango_layout_get_extents(l: *mut PangoLayout, ink: *mut PangoRectangle, logical: *mut PangoRectangle);
    pub fn pango_layout_xy_to_index(l: *mut PangoLayout, x: c_int, y: c_int, index: *mut c_int, trailing: *mut c_int) -> c_int;
    pub fn pango_layout_index_to_pos(l: *mut PangoLayout, index: c_int, pos: *mut PangoRectangle);
    pub fn pango_layout_get_iter(l: *mut PangoLayout) -> *mut PangoLayoutIter;
    pub fn pango_layout_iter_free(i: *mut PangoLayoutIter);
    pub fn pango_layout_iter_next_line(i: *mut PangoLayoutIter) -> c_int;
    pub fn pango_layout_iter_get_line_readonly(i: *mut PangoLayoutIter) -> *mut PangoLayoutLine;
    pub fn pango_layout_iter_get_line_extents(i: *mut PangoLayoutIter, ink: *mut PangoRectangle, logical: *mut PangoRectangle);
    pub fn pango_layout_iter_get_baseline(i: *mut PangoLayoutIter) -> c_int;
    pub fn pango_layout_line_get_x_ranges(line: *mut PangoLayoutLine, start: c_int, end: c_int, ranges: *mut *mut c_int, n: *mut c_int);
    pub fn pango_font_description_new() -> *mut PangoFontDescription;
    pub fn pango_font_description_set_family(d: *mut PangoFontDescription, family: *const c_char);
    pub fn pango_font_description_set_absolute_size(d: *mut PangoFontDescription, size: c_double);
    pub fn pango_font_description_set_weight(d: *mut PangoFontDescription, w: c_int);
    pub fn pango_font_description_set_style(d: *mut PangoFontDescription, s: c_int);
    pub fn pango_attr_list_new() -> *mut PangoAttrList;
    pub fn pango_attr_list_insert(l: *mut PangoAttrList, a: *mut PangoAttribute);
    pub fn pango_attr_list_unref(l: *mut PangoAttrList);
    pub fn pango_attr_weight_new(w: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_style_new(s: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_family_new(family: *const c_char) -> *mut PangoAttribute;
    pub fn pango_attr_foreground_new(r: u16, g: u16, b: u16) -> *mut PangoAttribute;
    pub fn pango_attr_foreground_alpha_new(a: u16) -> *mut PangoAttribute;
    pub fn pango_attr_strikethrough_new(on: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_underline_new(u: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_size_new_absolute(size: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_font_features_new(features: *const c_char) -> *mut PangoAttribute;
    pub fn pango_attr_shape_new(ink: *const PangoRectangle, logical: *const PangoRectangle) -> *mut PangoAttribute;
    pub fn pango_attr_line_height_new(factor: c_double) -> *mut PangoAttribute;
    pub fn pango_attr_line_height_new_absolute(height: c_int) -> *mut PangoAttribute;
    pub fn pango_cairo_show_layout(cr: *mut cairo_t, l: *mut PangoLayout);
    pub fn g_object_unref(o: *mut c_void);
    pub fn g_free(p: *mut c_void);
    pub fn g_error_free(e: *mut c_void);
}

// ------------------------------------------------------------- gdk-pixbuf

pub type GdkPixbuf = c_void;

unsafe extern "C" {
    pub fn gdk_pixbuf_new_from_file(path: *const c_char, error: *mut *mut c_void) -> *mut GdkPixbuf;
    pub fn gdk_pixbuf_apply_embedded_orientation(p: *mut GdkPixbuf) -> *mut GdkPixbuf;
    pub fn gdk_pixbuf_get_width(p: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_get_height(p: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_get_rowstride(p: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_get_n_channels(p: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_get_pixels(p: *const GdkPixbuf) -> *mut u8;
}

// ------------------------------------------------------------- fontconfig

pub type FcPattern = c_void;

unsafe extern "C" {
    pub fn FcNameParse(name: *const u8) -> *mut FcPattern;
    pub fn FcConfigSubstitute(config: *mut c_void, p: *mut FcPattern, kind: c_int) -> c_int;
    pub fn FcDefaultSubstitute(p: *mut FcPattern);
    pub fn FcFontMatch(config: *mut c_void, p: *mut FcPattern, result: *mut c_int) -> *mut FcPattern;
    pub fn FcPatternGetString(p: *const FcPattern, object: *const c_char, n: c_int, s: *mut *const u8) -> c_int;
    pub fn FcPatternGetInteger(p: *const FcPattern, object: *const c_char, n: c_int, i: *mut c_int) -> c_int;
    pub fn FcPatternDestroy(p: *mut FcPattern);
}
