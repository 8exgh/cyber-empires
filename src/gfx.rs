//! 8-bit indexed software rendering: surfaces, blits, palettes.

#[derive(Clone, Debug)]
pub struct Surface {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

pub type Palette = [[u8; 3]; 256];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    /// Inclusive corner form (x1,y1)-(x2,y2) as used by the original DO_BLIT calls.
    pub const fn xyxy(x1: i32, y1: i32, x2: i32, y2: i32) -> Rect {
        Rect { x: x1, y: y1, w: x2 - x1 + 1, h: y2 - y1 + 1 }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

impl Surface {
    pub fn new(w: usize, h: usize) -> Surface {
        Surface { w, h, px: vec![0; w * h] }
    }
    pub fn from_gif(g: &crate::gif::Gif) -> Surface {
        Surface { w: g.width, h: g.height, px: g.pixels.clone() }
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            0
        } else {
            self.px[y as usize * self.w + x as usize]
        }
    }
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    pub fn clear(&mut self, c: u8) {
        for p in self.px.iter_mut() {
            *p = c;
        }
    }
    pub fn fill_rect(&mut self, r: Rect, c: u8) {
        let x0 = r.x.max(0);
        let y0 = r.y.max(0);
        let x1 = r.right().min(self.w as i32);
        let y1 = r.bottom().min(self.h as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let row = &mut self.px[y as usize * self.w + x0 as usize..y as usize * self.w + x1 as usize];
            for p in row.iter_mut() {
                *p = c;
            }
        }
    }
    pub fn rect_outline(&mut self, r: Rect, c: u8) {
        self.hline(r.x, r.right() - 1, r.y, c);
        self.hline(r.x, r.right() - 1, r.bottom() - 1, c);
        self.vline(r.x, r.y, r.bottom() - 1, c);
        self.vline(r.right() - 1, r.y, r.bottom() - 1, c);
    }
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: u8) {
        for x in x0.min(x1)..=x0.max(x1) {
            self.put(x, y, c);
        }
    }
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: u8) {
        for y in y0.min(y1)..=y0.max(y1) {
            self.put(x, y, c);
        }
    }
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    /// Copy a rectangle `src_r` of `src` to (dx,dy). Opaque.
    pub fn blit(&mut self, src: &Surface, src_r: Rect, dx: i32, dy: i32) {
        self.blit_impl(src, src_r, dx, dy, None, None);
    }
    /// Copy with colour `key` treated as transparent.
    pub fn blit_key(&mut self, src: &Surface, src_r: Rect, dx: i32, dy: i32, key: u8) {
        self.blit_impl(src, src_r, dx, dy, Some(key), None);
    }
    /// Copy with transparency and a colour remap table (e.g. player colours).
    pub fn blit_key_map(&mut self, src: &Surface, src_r: Rect, dx: i32, dy: i32, key: u8, map: &[u8; 256]) {
        self.blit_impl(src, src_r, dx, dy, Some(key), Some(map));
    }
    fn blit_impl(&mut self, src: &Surface, src_r: Rect, dx: i32, dy: i32, key: Option<u8>, map: Option<&[u8; 256]>) {
        for yy in 0..src_r.h {
            let sy = src_r.y + yy;
            let ty = dy + yy;
            if sy < 0 || sy >= src.h as i32 || ty < 0 || ty >= self.h as i32 {
                continue;
            }
            for xx in 0..src_r.w {
                let sx = src_r.x + xx;
                let tx = dx + xx;
                if sx < 0 || sx >= src.w as i32 || tx < 0 || tx >= self.w as i32 {
                    continue;
                }
                let mut c = src.px[sy as usize * src.w + sx as usize];
                if let Some(k) = key {
                    if c == k {
                        continue;
                    }
                }
                if let Some(m) = map {
                    c = m[c as usize];
                }
                self.px[ty as usize * self.w + tx as usize] = c;
            }
        }
    }
    /// Blit clipped to a destination rectangle (used for the scrolling map viewport).
    pub fn blit_clipped(&mut self, src: &Surface, src_r: Rect, dx: i32, dy: i32, clip: Rect, key: Option<u8>) {
        let cx0 = clip.x.max(0);
        let cy0 = clip.y.max(0);
        let cx1 = clip.right().min(self.w as i32);
        let cy1 = clip.bottom().min(self.h as i32);
        for yy in 0..src_r.h {
            let sy = src_r.y + yy;
            let ty = dy + yy;
            if sy < 0 || sy >= src.h as i32 || ty < cy0 || ty >= cy1 {
                continue;
            }
            for xx in 0..src_r.w {
                let sx = src_r.x + xx;
                let tx = dx + xx;
                if sx < 0 || sx >= src.w as i32 || tx < cx0 || tx >= cx1 {
                    continue;
                }
                let c = src.px[sy as usize * src.w + sx as usize];
                if Some(c) == key {
                    continue;
                }
                self.px[ty as usize * self.w + tx as usize] = c;
            }
        }
    }
    /// Remap every pixel through `table`.
    pub fn remap(&mut self, table: &[u8; 256]) {
        for p in self.px.iter_mut() {
            *p = table[*p as usize];
        }
    }
    pub fn sub(&self, r: Rect) -> Surface {
        let mut s = Surface::new(r.w.max(0) as usize, r.h.max(0) as usize);
        s.blit(self, r, 0, 0);
        s
    }
}

/// Build a remap table from palette `from` into the nearest colours of palette `to`,
/// considering only the first `n_to` entries of `to`.
pub fn remap_table(from: &Palette, n_from: usize, to: &Palette, n_to: usize) -> [u8; 256] {
    let mut t = [0u8; 256];
    for i in 0..n_from.min(256) {
        let c = from[i];
        let mut best = 0usize;
        let mut bd = i64::MAX;
        for j in 0..n_to.min(256) {
            let d = to[j];
            let dr = c[0] as i64 - d[0] as i64;
            let dg = c[1] as i64 - d[1] as i64;
            let db = c[2] as i64 - d[2] as i64;
            let dist = dr * dr * 3 + dg * dg * 4 + db * db * 2;
            if dist < bd {
                bd = dist;
                best = j;
            }
        }
        t[i] = best as u8;
    }
    t
}

pub fn palette_from_gif(g: &crate::gif::Gif) -> Palette {
    g.palette
}

/// Scale a 6-bit VGA style palette entry etc. (not needed: GIF palettes are 8-bit).
pub fn lerp_palette(a: &Palette, b: &Palette, t: u8) -> Palette {
    let mut out = [[0u8; 3]; 256];
    for i in 0..256 {
        for k in 0..3 {
            let va = a[i][k] as i32;
            let vb = b[i][k] as i32;
            out[i][k] = (va + (vb - va) * t as i32 / 255) as u8;
        }
    }
    out
}
