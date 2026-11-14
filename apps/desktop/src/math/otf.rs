//! Minimal OpenType reader: just what math layout needs from a math font
//! (head, hhea/hmtx, cmap 4/12, glyf/loca bounding boxes and the MATH table).
//! Replaces ttf-parser (~36 KB of code) with a few hundred bytes of lookups.

#[derive(Clone, Copy)]
pub struct Font {
    d: &'static [u8],
    pub upem: f32,
    cmap: Option<usize>,
    hmtx: usize,
    n_hmetrics: u16,
    loca: usize,
    loca_long: bool,
    glyf: usize,
    consts: usize,
    glyph_info: usize,
    variants: usize,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Offsets of the faces in a font file (one for a plain font, n for a .ttc).
pub fn faces(d: &[u8]) -> Vec<usize> {
    if d.get(0..4) == Some(b"ttcf") {
        let n = u32_at(d, 8).unwrap_or(0) as usize;
        (0..n).filter_map(|i| u32_at(d, 12 + 4 * i).map(|o| o as usize)).collect()
    } else {
        vec![0]
    }
}

fn table(d: &[u8], face: usize, tag: &[u8; 4]) -> Option<usize> {
    let n = u16_at(d, face + 4)? as usize;
    (0..n).map(|i| face + 12 + 16 * i).find(|&r| d.get(r..r + 4) == Some(tag)).and_then(|r| u32_at(d, r + 8)).map(|o| o as usize)
}

/// Glyph-indexed table of MathValueRecords behind a Coverage table.
fn covered_value(d: &[u8], sub: usize, gid: u16) -> Option<i16> {
    let idx = coverage(d, sub + u16_at(d, sub)? as usize, gid)?;
    (idx < u16_at(d, sub + 2)? as usize).then(|| u16_at(d, sub + 4 + 4 * idx).map(|v| v as i16))?
}

/// First index in `0..n` for which `below(i)` is false (the records are sorted, as
/// the OpenType spec requires for cmap groups/segments and coverage tables).
fn lower_bound(n: usize, below: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if below(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn coverage(d: &[u8], c: usize, gid: u16) -> Option<usize> {
    let n = u16_at(d, c + 2)? as usize;
    match u16_at(d, c)? {
        1 => {
            let i = lower_bound(n, |i| u16_at(d, c + 4 + 2 * i).is_some_and(|g| g < gid));
            (i < n && u16_at(d, c + 4 + 2 * i) == Some(gid)).then_some(i)
        }
        2 => {
            // First range whose end is >= gid.
            let i = lower_bound(n, |i| u16_at(d, c + 4 + 6 * i + 2).is_some_and(|e| e < gid));
            let r = c + 4 + 6 * i;
            let (s, e, start) = (u16_at(d, r)?, u16_at(d, r + 2)?, u16_at(d, r + 4)?);
            (i < n && (s..=e).contains(&gid)).then(|| (start + gid - s) as usize)
        }
        _ => None,
    }
}

pub struct Part {
    pub glyph: u16,
    pub full_advance: u16,
    pub extender: bool,
}

pub struct Construction {
    /// (glyph, advance measurement), smallest first.
    pub variants: Vec<(u16, u16)>,
    /// (italics correction, parts bottom→top / left→right).
    pub assembly: Option<(i16, Vec<Part>)>,
}

impl Font {
    /// Parses the face at `face` (offset from [`faces`]); None if it has no MATH table.
    pub fn new(data: Vec<u8>, face: usize) -> Option<Font> {
        let d: &'static [u8] = Box::leak(data.into_boxed_slice());
        let head = table(d, face, b"head")?;
        let math = table(d, face, b"MATH")?;
        let cmap = table(d, face, b"cmap")?;
        let mut best = None;
        for i in 0..u16_at(d, cmap + 2)? as usize {
            let r = cmap + 4 + 8 * i;
            let (pid, eid) = (u16_at(d, r)?, u16_at(d, r + 2)?);
            let sub = cmap + u32_at(d, r + 4)? as usize;
            let fmt = u16_at(d, sub)?;
            let unicode = pid == 0 || (pid == 3 && (eid == 1 || eid == 10));
            if unicode && (fmt == 12 || (fmt == 4 && best.is_none())) {
                best = Some(sub);
            }
        }
        let f = Font {
            d,
            upem: u16_at(d, head + 18)? as f32,
            cmap: best,
            hmtx: table(d, face, b"hmtx")?,
            n_hmetrics: u16_at(d, table(d, face, b"hhea")? + 34)?,
            loca: table(d, face, b"loca").unwrap_or(0),
            loca_long: u16_at(d, head + 50)? == 1,
            glyf: table(d, face, b"glyf").unwrap_or(0),
            consts: math + u16_at(d, math + 4)? as usize,
            glyph_info: math + u16_at(d, math + 6)? as usize,
            variants: math + u16_at(d, math + 8)? as usize,
        };
        Some(f)
    }

    fn u16(&self, o: usize) -> Option<u16> {
        u16_at(self.d, o)
    }

    pub fn glyph_index(&self, c: char) -> Option<u16> {
        let s = self.cmap?;
        let c = c as u32;
        let g = match self.u16(s)? {
            12 => {
                // Sequential map groups (start, end, glyph), sorted by start.
                let n = u32_at(self.d, s + 12)? as usize;
                let i = lower_bound(n, |i| u32_at(self.d, s + 16 + 12 * i + 4).is_some_and(|end| end < c));
                let r = s + 16 + 12 * i;
                let (a, b, g) = (u32_at(self.d, r)?, u32_at(self.d, r + 4)?, u32_at(self.d, r + 8)?);
                if i >= n || !(a..=b).contains(&c) {
                    return None;
                }
                (g + c - a) as u16
            }
            4 if c <= 0xFFFF => {
                let segx2 = self.u16(s + 6)? as usize;
                let c = c as u16;
                let segs = segx2 / 2;
                let i = lower_bound(segs, |i| self.u16(s + 14 + 2 * i).is_some_and(|e| e < c));
                if i >= segs {
                    return None;
                }
                let start = self.u16(s + 16 + segx2 + 2 * i)?;
                if start > c {
                    return None;
                }
                let delta = self.u16(s + 16 + 2 * segx2 + 2 * i)?;
                let ro_pos = s + 16 + 3 * segx2 + 2 * i;
                let ro = self.u16(ro_pos)?;
                if ro == 0 {
                    c.wrapping_add(delta)
                } else {
                    let g = self.u16(ro_pos + ro as usize + 2 * (c - start) as usize)?;
                    if g == 0 { 0 } else { g.wrapping_add(delta) }
                }
            }
            _ => return None,
        };
        (g != 0).then_some(g)
    }

    pub fn advance(&self, g: u16) -> Option<u16> {
        let i = g.min(self.n_hmetrics.checked_sub(1)?) as usize;
        self.u16(self.hmtx + 4 * i)
    }

    /// (x_min, y_min, x_max, y_max) from the glyf header; None for empty glyphs.
    pub fn bbox(&self, g: u16) -> Option<[i16; 4]> {
        if self.glyf == 0 {
            return None;
        }
        let g = g as usize;
        let (a, b) = if self.loca_long {
            (u32_at(self.d, self.loca + 4 * g)? as usize, u32_at(self.d, self.loca + 4 * g + 4)? as usize)
        } else {
            (self.u16(self.loca + 2 * g)? as usize * 2, self.u16(self.loca + 2 * g + 2)? as usize * 2)
        };
        if a == b {
            return None;
        }
        let o = self.glyf + a;
        Some([self.u16(o + 2)? as i16, self.u16(o + 4)? as i16, self.u16(o + 6)? as i16, self.u16(o + 8)? as i16])
    }

    /// MathConstants header fields (byte offset 0..8 and 212).
    pub fn const_i16(&self, offset: usize) -> i16 {
        self.u16(self.consts + offset).unwrap_or(0) as i16
    }

    /// MathValueRecord number `i` of MathConstants (axisHeight = 1, …).
    pub fn value(&self, i: usize) -> i16 {
        self.const_i16(8 + 4 * i)
    }

    pub fn italic_correction(&self, g: u16) -> Option<i16> {
        let off = self.u16(self.glyph_info)?;
        (off != 0).then(|| covered_value(self.d, self.glyph_info + off as usize, g))?
    }

    pub fn top_accent(&self, g: u16) -> Option<i16> {
        let off = self.u16(self.glyph_info + 2)?;
        (off != 0).then(|| covered_value(self.d, self.glyph_info + off as usize, g))?
    }

    pub fn min_connector_overlap(&self) -> u16 {
        self.u16(self.variants).unwrap_or(0)
    }

    pub fn construction(&self, g: u16, vertical: bool) -> Option<Construction> {
        let v = self.variants;
        let cov = v + self.u16(v + if vertical { 2 } else { 4 })? as usize;
        let idx = coverage(self.d, cov, g)?;
        let (nv, nh) = (self.u16(v + 6)? as usize, self.u16(v + 8)? as usize);
        if idx >= if vertical { nv } else { nh } {
            return None;
        }
        let slot = v + 10 + 2 * (if vertical { idx } else { nv + idx });
        let c = v + self.u16(slot)? as usize;
        let n = self.u16(c + 2)? as usize;
        let variants = (0..n).filter_map(|i| Some((self.u16(c + 4 + 4 * i)?, self.u16(c + 6 + 4 * i)?))).collect();
        let ao = self.u16(c)? as usize;
        let assembly = (ao != 0).then(|| {
            let a = c + ao;
            let parts = (0..self.u16(a + 4)? as usize)
                .filter_map(|i| {
                    let p = a + 6 + 10 * i;
                    Some(Part { glyph: self.u16(p)?, full_advance: self.u16(p + 6)?, extender: self.u16(p + 8)? & 1 != 0 })
                })
                .collect();
            Some((self.u16(a)? as i16, parts))
        });
        Some(Construction { variants, assembly: assembly.flatten() })
    }
}

/// Whether the face at `face` carries an OpenType MATH table.
pub fn has_math(d: &[u8], face: usize) -> bool {
    table(d, face, b"MATH").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The former linear scans, as the reference for the binary searches.
    fn ref_coverage(d: &[u8], c: usize, gid: u16) -> Option<usize> {
        let n = u16_at(d, c + 2)? as usize;
        match u16_at(d, c)? {
            1 => (0..n).find(|&i| u16_at(d, c + 4 + 2 * i) == Some(gid)),
            2 => (0..n).find_map(|i| {
                let r = c + 4 + 6 * i;
                let (s, e, start) = (u16_at(d, r)?, u16_at(d, r + 2)?, u16_at(d, r + 4)?);
                (s..=e).contains(&gid).then(|| (start + gid - s) as usize)
            }),
            _ => None,
        }
    }

    fn ref_lookup(f: &Font, s: usize, c: u32) -> Option<u16> {
        let d = f.d;
        match u16_at(d, s)? {
            12 => (0..u32_at(d, s + 12)? as usize).find_map(|i| {
                let r = s + 16 + 12 * i;
                let (a, b, g) = (u32_at(d, r)?, u32_at(d, r + 4)?, u32_at(d, r + 8)?);
                (a..=b).contains(&c).then(|| (g + c - a) as u16)
            }),
            4 if c <= 0xFFFF => {
                let segx2 = u16_at(d, s + 6)? as usize;
                let c = c as u16;
                let i = (0..segx2 / 2).find(|&i| u16_at(d, s + 14 + 2 * i).is_some_and(|e| e >= c))?;
                let start = u16_at(d, s + 16 + segx2 + 2 * i)?;
                if start > c {
                    return None;
                }
                let delta = u16_at(d, s + 16 + 2 * segx2 + 2 * i)?;
                let ro_pos = s + 16 + 3 * segx2 + 2 * i;
                let ro = u16_at(d, ro_pos)?;
                Some(if ro == 0 {
                    c.wrapping_add(delta)
                } else {
                    let g = u16_at(d, ro_pos + ro as usize + 2 * (c - start) as usize)?;
                    if g == 0 { 0 } else { g.wrapping_add(delta) }
                })
            }
            _ => None,
        }
        .filter(|&g| g != 0)
    }

    #[test]
    fn binary_searches_match_linear_scans() {
        let Ok(data) = std::fs::read(r"C:\Windows\Fonts\cambria.ttc") else { return };
        let face = faces(&data)[1];
        let f = Font::new(data, face).expect("Cambria Math");
        let d = f.d;
        // Every unicode cmap subtable of the face (format 12 and format 4).
        let cmap = table(d, face, b"cmap").unwrap();
        let mut checked = 0;
        for i in 0..u16_at(d, cmap + 2).unwrap() as usize {
            let sub = cmap + u32_at(d, cmap + 4 + 8 * i + 4).unwrap() as usize;
            if !matches!(u16_at(d, sub), Some(4 | 12)) {
                continue;
            }
            let g = Font { cmap: Some(sub), ..f };
            for c in 0..0x2_0000u32 {
                let Some(ch) = char::from_u32(c) else { continue };
                assert_eq!(g.glyph_index(ch), ref_lookup(&f, sub, c), "cmap {sub} U+{c:X}");
            }
            checked += 1;
        }
        assert!(checked >= 2, "expected format 4 and 12 subtables");
        // Every coverage table the layout uses, for every glyph id.
        let gi = f.glyph_info;
        let v = f.variants;
        let covs = [
            gi + u16_at(d, gi).unwrap() as usize,
            gi + u16_at(d, gi + 2).unwrap() as usize,
            v + u16_at(d, v + 2).unwrap() as usize,
            v + u16_at(d, v + 4).unwrap() as usize,
        ];
        for (k, base) in covs.into_iter().enumerate() {
            // MathItalicsCorrectionInfo / MathTopAccentAttachment start with their coverage offset.
            let cov = if k < 2 { base + u16_at(d, base).unwrap() as usize } else { base };
            for gid in 0..=u16::MAX {
                assert_eq!(coverage(d, cov, gid), ref_coverage(d, cov, gid), "coverage {k} glyph {gid}");
            }
        }
    }
}
