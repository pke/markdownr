//! The C interfaces the macOS shell uses: the Objective-C runtime (AppKit is driven by
//! messages, no binding crate), Core Foundation, Core Graphics, Core Text, ImageIO and
//! libdispatch, declared here.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]

use core::ffi::{c_char, c_void};

pub type Id = *mut c_void;
pub type Sel = *const c_void;
pub type Class = *mut c_void;
pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFAttributedStringRef = *const c_void;
pub type CFMutableAttributedStringRef = *mut c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFNumberRef = *const c_void;
pub type CFURLRef = *const c_void;
pub type CFDataRef = *const c_void;
pub type CFIndex = isize;
pub type CGFloat = f64;
pub type CGContextRef = *mut c_void;
pub type CGColorRef = *const c_void;
pub type CGImageRef = *const c_void;
pub type CGPathRef = *const c_void;
pub type CTFontRef = *const c_void;
pub type CTLineRef = *const c_void;
pub type CTTypesetterRef = *const c_void;
pub type CTRunDelegateRef = *const c_void;
pub type CGGlyph = u16;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        CGRect { origin: CGPoint { x, y }, size: CGSize { width: w, height: h } }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CGAffineTransform {
    pub a: CGFloat,
    pub b: CGFloat,
    pub c: CGFloat,
    pub d: CGFloat,
    pub tx: CGFloat,
    pub ty: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

#[repr(C)]
pub struct CTRunDelegateCallbacks {
    pub version: CFIndex,
    pub dealloc: unsafe extern "C" fn(*mut c_void),
    pub ascent: unsafe extern "C" fn(*mut c_void) -> CGFloat,
    pub descent: unsafe extern "C" fn(*mut c_void) -> CGFloat,
    pub width: unsafe extern "C" fn(*mut c_void) -> CGFloat,
}

#[link(name = "objc")]
unsafe extern "C" {
    pub fn objc_getClass(name: *const c_char) -> Class;
    pub fn sel_registerName(name: *const c_char) -> Sel;
    pub fn objc_msgSend();
    #[cfg(target_arch = "x86_64")]
    pub fn objc_msgSend_stret();
    pub fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra: usize) -> Class;
    pub fn objc_registerClassPair(cls: Class);
    pub fn class_addMethod(cls: Class, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFAllocatorDefault: *const c_void;
    pub static kCFTypeDictionaryKeyCallBacks: c_void;
    pub static kCFTypeDictionaryValueCallBacks: c_void;
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFStringCreateWithBytes(alloc: *const c_void, bytes: *const u8, n: CFIndex, encoding: u32, external: bool) -> CFStringRef;
    pub fn CFStringGetLength(s: CFStringRef) -> CFIndex;
    pub fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: CFIndex, encoding: u32) -> bool;
    pub fn CFAttributedStringCreateMutable(alloc: *const c_void, max: CFIndex) -> CFMutableAttributedStringRef;
    pub fn CFAttributedStringReplaceString(s: CFMutableAttributedStringRef, range: CFRange, replacement: CFStringRef);
    pub fn CFAttributedStringSetAttribute(s: CFMutableAttributedStringRef, range: CFRange, name: CFStringRef, value: CFTypeRef);
    pub fn CFNumberCreate(alloc: *const c_void, kind: CFIndex, value: *const c_void) -> CFNumberRef;
    pub fn CFURLCreateFromFileSystemRepresentation(alloc: *const c_void, path: *const u8, len: CFIndex, dir: bool) -> CFURLRef;
    pub fn CFURLCreateWithBytes(alloc: *const c_void, bytes: *const u8, len: CFIndex, encoding: u32, base: CFURLRef) -> CFURLRef;
    pub fn CFDictionaryCreate(alloc: *const c_void, keys: *const CFTypeRef, values: *const CFTypeRef, n: CFIndex, kcb: *const c_void, vcb: *const c_void) -> CFDictionaryRef;
    pub static kCFBooleanTrue: CFTypeRef;
}

pub const kCFStringEncodingUTF8: u32 = 0x0800_0100;
pub const kCFNumberSInt32Type: CFIndex = 3;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGColorCreateSRGB(r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat) -> CGColorRef;
    pub fn CGContextSaveGState(c: CGContextRef);
    pub fn CGContextRestoreGState(c: CGContextRef);
    pub fn CGContextGetCTM(c: CGContextRef) -> CGAffineTransform;
    pub fn CGContextConcatCTM(c: CGContextRef, t: CGAffineTransform);
    pub fn CGContextTranslateCTM(c: CGContextRef, x: CGFloat, y: CGFloat);
    pub fn CGContextScaleCTM(c: CGContextRef, x: CGFloat, y: CGFloat);
    pub fn CGContextRotateCTM(c: CGContextRef, angle: CGFloat);
    pub fn CGContextSetRGBFillColor(c: CGContextRef, r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat);
    pub fn CGContextFillRect(c: CGContextRef, r: CGRect);
    pub fn CGContextFillEllipseInRect(c: CGContextRef, r: CGRect);
    pub fn CGContextBeginPath(c: CGContextRef);
    pub fn CGContextMoveToPoint(c: CGContextRef, x: CGFloat, y: CGFloat);
    pub fn CGContextAddLineToPoint(c: CGContextRef, x: CGFloat, y: CGFloat);
    pub fn CGContextAddCurveToPoint(c: CGContextRef, x1: CGFloat, y1: CGFloat, x2: CGFloat, y2: CGFloat, x: CGFloat, y: CGFloat);
    pub fn CGContextClosePath(c: CGContextRef);
    pub fn CGContextAddPath(c: CGContextRef, p: CGPathRef);
    pub fn CGContextFillPath(c: CGContextRef);
    pub fn CGPathCreateWithRoundedRect(r: CGRect, rx: CGFloat, ry: CGFloat, t: *const CGAffineTransform) -> CGPathRef;
    pub fn CGPathRelease(p: CGPathRef);
    pub fn CGContextSetAlpha(c: CGContextRef, a: CGFloat);
    pub fn CGContextBeginTransparencyLayer(c: CGContextRef, aux: CFDictionaryRef);
    pub fn CGContextEndTransparencyLayer(c: CGContextRef);
    pub fn CGContextSetTextMatrix(c: CGContextRef, t: CGAffineTransform);
    pub fn CGContextSetTextPosition(c: CGContextRef, x: CGFloat, y: CGFloat);
    pub fn CGContextDrawImage(c: CGContextRef, r: CGRect, image: CGImageRef);
    pub fn CGContextSetInterpolationQuality(c: CGContextRef, q: i32);
    pub fn CGImageGetWidth(i: CGImageRef) -> usize;
    pub fn CGImageGetHeight(i: CGImageRef) -> usize;
    pub fn CGImageRelease(i: CGImageRef);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    pub static kCTFontAttributeName: CFStringRef;
    pub static kCTForegroundColorAttributeName: CFStringRef;
    pub static kCTUnderlineStyleAttributeName: CFStringRef;
    pub static kCTSuperscriptAttributeName: CFStringRef;
    pub static kCTRunDelegateAttributeName: CFStringRef;
    pub fn CTFontCreateUIFontForLanguage(kind: u32, size: CGFloat, language: CFStringRef) -> CTFontRef;
    pub fn CTFontCreateWithName(name: CFStringRef, size: CGFloat, matrix: *const CGAffineTransform) -> CTFontRef;
    pub fn CTFontCreateCopyWithSymbolicTraits(font: CTFontRef, size: CGFloat, matrix: *const CGAffineTransform, value: u32, mask: u32) -> CTFontRef;
    pub fn CTFontCreateCopyWithAttributes(font: CTFontRef, size: CGFloat, matrix: *const CGAffineTransform, attrs: *const c_void) -> CTFontRef;
    pub fn CTFontCreateWithGraphicsFont(font: *const c_void, size: CGFloat, matrix: *const CGAffineTransform, attrs: *const c_void) -> CTFontRef;
    pub fn CTFontDrawGlyphs(font: CTFontRef, glyphs: *const CGGlyph, positions: *const CGPoint, n: usize, c: CGContextRef);
    pub fn CTFontGetAscent(font: CTFontRef) -> CGFloat;
    pub fn CTFontGetDescent(font: CTFontRef) -> CGFloat;
    pub fn CTTypesetterCreateWithAttributedString(s: CFAttributedStringRef) -> CTTypesetterRef;
    pub fn CTTypesetterSuggestLineBreak(t: CTTypesetterRef, start: CFIndex, width: f64) -> CFIndex;
    pub fn CTTypesetterCreateLine(t: CTTypesetterRef, range: CFRange) -> CTLineRef;
    pub fn CTLineGetTypographicBounds(l: CTLineRef, ascent: *mut CGFloat, descent: *mut CGFloat, leading: *mut CGFloat) -> f64;
    pub fn CTLineGetTrailingWhitespaceWidth(l: CTLineRef) -> f64;
    pub fn CTLineGetStringIndexForPosition(l: CTLineRef, p: CGPoint) -> CFIndex;
    pub fn CTLineGetOffsetForStringIndex(l: CTLineRef, index: CFIndex, secondary: *mut CGFloat) -> CGFloat;
    pub fn CTLineDraw(l: CTLineRef, c: CGContextRef);
    pub fn CTRunDelegateCreate(callbacks: *const CTRunDelegateCallbacks, refcon: *mut c_void) -> CTRunDelegateRef;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    pub static kCGImageSourceShouldCacheImmediately: CFStringRef;
    pub static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
    pub fn CGImageSourceCreateWithURL(url: CFURLRef, options: CFDictionaryRef) -> *const c_void;
    pub fn CGImageSourceCreateImageAtIndex(src: *const c_void, index: usize, options: CFDictionaryRef) -> CGImageRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGDataProviderCreateWithURL(url: CFURLRef) -> *const c_void;
    pub fn CGFontCreateWithDataProvider(p: *const c_void) -> *const c_void;
}

unsafe extern "C" {
    pub static _dispatch_main_q: c_void;
    pub fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: unsafe extern "C" fn(*mut c_void));
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    pub static NSPasteboardTypeString: Id;
}

/// A selector by name (a C string literal).
pub fn sel(name: &core::ffi::CStr) -> Sel {
    unsafe { sel_registerName(name.as_ptr()) }
}

pub fn class(name: &core::ffi::CStr) -> Class {
    unsafe { objc_getClass(name.as_ptr()) }
}

/// Send a message: `msg!(receiver, "selector:", args… => ReturnType)`. objc_msgSend called
/// through a pointer of the exact type (required on arm64).
#[macro_export]
macro_rules! msg {
    ($obj:expr, $sel:literal $(, $arg:expr)* => $ret:ty) => {{
        #[allow(unused_unsafe)]
        let f: unsafe extern "C" fn($crate::mac::ffi::Id, $crate::mac::ffi::Sel $(, $crate::msg!(@ty $arg))*) -> $ret =
            unsafe { core::mem::transmute($crate::mac::ffi::objc_msgSend as *const core::ffi::c_void) };
        #[allow(unused_unsafe)]
        unsafe { f($obj as $crate::mac::ffi::Id, $crate::mac::ffi::sel($sel) $(, $arg)*) }
    }};
    ($obj:expr, $sel:literal $(, $arg:expr)*) => { $crate::msg!($obj, $sel $(, $arg)* => ()) };
    (@ty $arg:expr) => { _ };
}

/// An NSString from UTF-8 (autoreleased).
pub fn nsstring(s: &str) -> Id {
    let cls = class(c"NSString");
    let obj = msg!(cls, c"alloc" => Id);
    msg!(obj, c"initWithBytes:length:encoding:", s.as_ptr(), s.len(), 4usize => Id) // NSUTF8StringEncoding
}

/// A Rust string from an NSString.
pub fn string(ns: Id) -> String {
    if ns.is_null() {
        return String::new();
    }
    let p = msg!(ns, c"UTF8String" => *const c_char);
    if p.is_null() { String::new() } else { unsafe { core::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}
