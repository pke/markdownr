//! Image loading on Linux: each image is decoded by gdk-pixbuf on a worker thread (web
//! images are fetched with curl first); results come back over a channel and a byte on
//! the wake socket rouses the event loop, which collects them. Drawn as cairo surfaces.

use super::ffi::*;
use crate::util::{ImageSource, VecMap};
use std::cell::RefCell;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

type Decoded = Option<(u32, u32, Vec<u8>)>;

enum State {
    Loading,
    Failed,
    Ready { w: u32, h: u32, pixels: Vec<u8>, surface: *mut cairo_surface_t },
}

pub struct Images {
    map: RefCell<VecMap<String, State>>,
    base: RefCell<Option<PathBuf>>,
    tx: Sender<(String, Decoded)>,
    rx: Receiver<(String, Decoded)>,
    wake: UnixStream,
}

impl Images {
    /// `wake` gets a byte whenever a result is ready.
    pub fn new(wake: UnixStream) -> Self {
        let (tx, rx) = channel();
        Images { map: RefCell::default(), base: RefCell::default(), tx, rx, wake }
    }

    pub fn set_base(&self, dir: Option<PathBuf>) {
        *self.base.borrow_mut() = dir;
        for (_, s) in self.map.borrow_mut().drain() {
            if let State::Ready { surface, .. } = s {
                if !surface.is_null() {
                    unsafe { cairo_surface_destroy(surface) };
                }
            }
        }
    }

    /// Size in pixels if decoded; otherwise starts loading (once) and returns None.
    pub fn size(&self, url: &str) -> Option<(f32, f32)> {
        if let Some(state) = self.map.borrow().get(url) {
            return match state {
                State::Ready { w, h, .. } => Some((*w as f32, *h as f32)),
                _ => None,
            };
        }
        self.map.borrow_mut().insert(url.to_string(), State::Loading);
        let source = crate::util::image_source(url, self.base.borrow().as_deref());
        let (tx, owned, wake) = (self.tx.clone(), url.to_string(), self.wake.try_clone().ok());
        let spawned = std::thread::Builder::new().spawn(move || {
            let decoded = source.and_then(|s| load(&s));
            let _ = tx.send((owned, decoded));
            if let Some(mut w) = wake {
                let _ = w.write(&[1]);
            }
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
        while let Ok((url, decoded)) = self.rx.try_recv() {
            // A newer document may have cleared the map; drop stale results.
            if !map.contains_key(&url) {
                continue;
            }
            let state = match decoded {
                Some((w, h, pixels)) => {
                    any = true;
                    State::Ready { w, h, pixels, surface: core::ptr::null_mut() }
                }
                None => State::Failed,
            };
            map.insert(url, state);
        }
        any
    }

    /// The cairo surface of a loaded image and its pixel size.
    pub fn surface(&self, url: &str) -> Option<(*mut cairo_surface_t, u32, u32)> {
        let mut map = self.map.borrow_mut();
        let State::Ready { w, h, pixels, surface } = map.get_mut(url)? else { return None };
        if surface.is_null() {
            unsafe {
                let s = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, *w as i32, *h as i32);
                let stride = cairo_image_surface_get_stride(s) as usize;
                let data = cairo_image_surface_get_data(s);
                if data.is_null() {
                    cairo_surface_destroy(s);
                    return None;
                }
                cairo_surface_flush(s);
                for y in 0..*h as usize {
                    let row = &pixels[y * *w as usize * 4..(y + 1) * *w as usize * 4];
                    core::ptr::copy_nonoverlapping(row.as_ptr(), data.add(y * stride), row.len());
                }
                cairo_surface_mark_dirty(s);
                *surface = s;
            }
        }
        Some((*surface, *w, *h))
    }
}

fn load(source: &ImageSource) -> Decoded {
    match source {
        ImageSource::File(p) => decode(p),
        ImageSource::Url(u) => {
            // A temporary file per download, removed after decoding.
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let tmp = crate::util::path::join(&crate::util::env::temp_dir(), Path::new(&["markdownr-", &std::process::id().to_string(), "-", &n.to_string()].concat()));
            let ok = std::process::Command::new("curl").args(["-sfL", "--max-time", "30", "-o"]).arg(&tmp).arg(u).status().is_ok_and(|s| s.success());
            let decoded = if ok { decode(&tmp) } else { None };
            let _ = std::fs::remove_file(&tmp);
            decoded
        }
    }
}

/// Premultiplied BGRA pixels (cairo's ARGB32 on little-endian) of an image file.
fn decode(path: &Path) -> Decoded {
    let c = std::ffi::CString::new(<std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::as_bytes(path.as_os_str())).ok()?;
    unsafe {
        let mut err = core::ptr::null_mut();
        let raw = gdk_pixbuf_new_from_file(c.as_ptr(), &mut err);
        if raw.is_null() {
            if !err.is_null() {
                g_error_free(err);
            }
            return None;
        }
        let pb = gdk_pixbuf_apply_embedded_orientation(raw);
        g_object_unref(raw);
        if pb.is_null() {
            return None;
        }
        let (w, h) = (gdk_pixbuf_get_width(pb) as u32, gdk_pixbuf_get_height(pb) as u32);
        let (stride, n) = (gdk_pixbuf_get_rowstride(pb) as usize, gdk_pixbuf_get_n_channels(pb) as usize);
        let src = gdk_pixbuf_get_pixels(pb);
        let ok = w > 0 && h > 0 && (w as u64 * h as u64) <= 64 * 1024 * 1024 && (n == 3 || n == 4) && !src.is_null();
        let mut out = Vec::new();
        if ok {
            out.reserve_exact((w * h * 4) as usize);
            for y in 0..h as usize {
                let row = core::slice::from_raw_parts(src.add(y * stride), w as usize * n);
                for px in row.chunks_exact(n) {
                    let a = if n == 4 { px[3] as u32 } else { 255 };
                    let pm = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                    out.extend_from_slice(&[pm(px[2]), pm(px[1]), pm(px[0]), a as u8]);
                }
            }
        }
        g_object_unref(pb);
        ok.then_some((w, h, out))
    }
}
