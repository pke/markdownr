//! Image loading on macOS: ImageIO decodes each image on a worker thread (web images
//! straight from their URL); the result comes back over a channel and a block on the main
//! queue tells the app to collect it. Drawn as CGImages.

use super::ffi::*;
use crate::util::{ImageSource, VecMap};
use core::ffi::c_void;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

/// A decoded image handed between threads (CGImage is immutable and thread-safe).
struct Image(CGImageRef);
unsafe impl Send for Image {}

enum State {
    Loading,
    Failed,
    Ready(CGImageRef, u32, u32),
}

pub struct Images {
    map: RefCell<VecMap<String, State>>,
    base: RefCell<Option<PathBuf>>,
    tx: Sender<(String, Option<Image>)>,
    rx: Receiver<(String, Option<Image>)>,
    /// Runs on the main queue after a result arrived.
    ready: unsafe extern "C" fn(*mut c_void),
}

impl Images {
    pub fn new(ready: unsafe extern "C" fn(*mut c_void)) -> Self {
        let (tx, rx) = channel();
        Images { map: RefCell::default(), base: RefCell::default(), tx, rx, ready }
    }

    pub fn set_base(&self, dir: Option<PathBuf>) {
        *self.base.borrow_mut() = dir;
        for (_, s) in self.map.borrow_mut().drain() {
            if let State::Ready(img, ..) = s {
                unsafe { CGImageRelease(img) };
            }
        }
    }

    pub fn size(&self, url: &str) -> Option<(f32, f32)> {
        if let Some(state) = self.map.borrow().get(url) {
            return match state {
                State::Ready(_, w, h) => Some((*w as f32, *h as f32)),
                _ => None,
            };
        }
        self.map.borrow_mut().insert(url.to_string(), State::Loading);
        let source = crate::util::image_source(url, self.base.borrow().as_deref());
        let (tx, owned, ready) = (self.tx.clone(), url.to_string(), self.ready as usize);
        let spawned = std::thread::Builder::new().spawn(move || {
            let img = source.and_then(|s| load(&s)).map(Image);
            let _ = tx.send((owned, img));
            // SAFETY: `ready` is the fn pointer given to `new`.
            let ready: unsafe extern "C" fn(*mut c_void) = unsafe { core::mem::transmute(ready) };
            unsafe { dispatch_async_f(&_dispatch_main_q as *const _ as *const c_void, core::ptr::null_mut(), ready) };
        });
        if spawned.is_err() {
            self.map.borrow_mut().insert(url.to_string(), State::Failed);
        }
        None
    }

    /// Takes finished loads; true if an image became available.
    pub fn collect(&self) -> bool {
        let mut any = false;
        let mut map = self.map.borrow_mut();
        while let Ok((url, img)) = self.rx.try_recv() {
            if !map.contains_key(&url) {
                if let Some(Image(i)) = img {
                    unsafe { CGImageRelease(i) };
                }
                continue;
            }
            let state = match img {
                Some(Image(i)) => {
                    any = true;
                    unsafe { State::Ready(i, CGImageGetWidth(i) as u32, CGImageGetHeight(i) as u32) }
                }
                None => State::Failed,
            };
            map.insert(url, state);
        }
        any
    }

    pub fn image(&self, url: &str) -> Option<CGImageRef> {
        match self.map.borrow().get(url)? {
            State::Ready(i, ..) => Some(*i),
            _ => None,
        }
    }
}

/// Decode with ImageIO, EXIF orientation applied, pixels decoded now (off the main thread).
fn load(source: &ImageSource) -> Option<CGImageRef> {
    unsafe {
        let url = match source {
            ImageSource::File(p) => {
                let b = <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::as_bytes(p.as_os_str());
                CFURLCreateFromFileSystemRepresentation(kCFAllocatorDefault, b.as_ptr(), b.len() as CFIndex, false)
            }
            ImageSource::Url(u) => CFURLCreateWithBytes(kCFAllocatorDefault, u.as_ptr(), u.len() as CFIndex, kCFStringEncodingUTF8, core::ptr::null()),
        };
        if url.is_null() {
            return None;
        }
        let src = CGImageSourceCreateWithURL(url, core::ptr::null());
        CFRelease(url);
        if src.is_null() {
            return None;
        }
        let keys = [kCGImageSourceShouldCacheImmediately];
        let values = [kCFBooleanTrue];
        let opts = CFDictionaryCreate(kCFAllocatorDefault, keys.as_ptr(), values.as_ptr(), 1, &kCFTypeDictionaryKeyCallBacks as *const _ as *const c_void, &kCFTypeDictionaryValueCallBacks as *const _ as *const c_void);
        let img = CGImageSourceCreateImageAtIndex(src, 0, opts);
        CFRelease(opts);
        CFRelease(src);
        (!img.is_null()).then_some(img)
    }
}
