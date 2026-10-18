//! Battle rendering: palette, sprite sheets, viewports, HUD and radars (ACTION.PRG drawing code).

use super::map::TileSet;
use super::sim::{Sim, NMECH};
use super::tables::*;
use crate::assets::{terrain_tilesets, Assets};
use crate::gfx::{Palette, Rect, Surface};

pub const VIEW_W: i32 = 145;
pub const VIEW_H: i32 = 139;
pub const VIEW_X: [i32; 2] = [0, 175];

pub struct BattleArt {
    pub mechset: Surface,
    pub newmars: Surface,
    pub newmars2: Surface,
    pub sheets: [Surface; 3],
    pub frame: Surface,
    pub pal: Palette,
    pub msize: [[[i32; 4]; 8]; 13],
    /// colour remap for side 1 sprites (11->9, 14->12, 31->13)
    pub side1_map: [u8; 256],
}

fn st_rgb(c: u16) -> [u8; 3] {
    let r = ((c >> 8) & 7) as u8;
    let g = ((c >> 4) & 7) as u8;
    let b = (c & 7) as u8;
    [r * 36, g * 36, b * 36]
}

impl BattleArt {
    pub fn load(assets: &mut Assets, ttype: usize, pc: [u8; 2]) -> Option<BattleArt> {
        let (frame, mut pal, _) = assets.image_cloned("batframe.gif")?;
        let mechset = assets.surface("mechset8.gif")?;
        let newmars = assets.surface("newmars.gif")?;
        let newmars2 = assets.surface("newmars2.gif")?;
        let names = terrain_tilesets(ttype as u8 + 1);
        let sheets = [assets.surface(names[0])?, assets.surface(names[1])?, assets.surface(names[2])?];
        // palette overrides
        for i in 0..16 {
            pal[i] = st_rgb(COLOUR_PALETTE[i]);
        }
        pal[0] = [0, 0, 0];
        pal[1] = [0, 0, 0];
        pal[2] = [0, 0, 0];
        for (k, &slot) in TOFF.iter().enumerate() {
            pal[slot as usize] = st_rgb(TERRAIN_COLOURS[ttype.min(4) * 5 + k]);
        }
        for (k, &slot) in P1OFF.iter().enumerate() {
            pal[slot as usize] = st_rgb(PLAYER_COLOURS[(pc[0] as usize).min(4) * 3 + k]);
        }
        for (k, &slot) in P2OFF.iter().enumerate() {
            pal[slot as usize] = st_rgb(PLAYER_COLOURS[(pc[1] as usize).min(4) * 3 + k]);
        }
        let mut side1_map = [0u8; 256];
        for i in 0..256 {
            side1_map[i] = i as u8;
        }
        side1_map[11] = 9;
        side1_map[14] = 12;
        side1_map[31] = 13;
        let mut art = BattleArt { mechset, newmars, newmars2, sheets, frame, pal, msize: [[[0; 4]; 8]; 13], side1_map };
        art.make_sizes();
        Some(art)
    }

    /// Sprite id -> (sheet, source rect, size)
    pub fn sprite_rect(&self, id: i32) -> Option<(&Surface, Rect, i32)> {
        if id <= 0 {
            return None;
        }
        match id / 1024 {
            1 => {
                let c = id - 1024;
                Some((&self.mechset, Rect::new((c % 20) * 16, (c / 20) * 16, 16, 16), 16))
            }
            0 => {
                let c = id - 1;
                Some((&self.newmars, Rect::new((c % 10) * 32, (c / 10) * 32, 32, 32), 32))
            }
            _ => {
                let c = id - 2048;
                Some((&self.newmars2, Rect::new((c % 10) * 32, (c / 10) * 32, 32, 32), 32))
            }
        }
    }

    fn make_sizes(&mut self) {
        for t in 0..13usize {
            for d in 0..8usize {
                let a = &ANIMATION[t];
                let fi = a[0];
                let n = a[1] as usize;
                if fi < 0 {
                    continue;
                }
                let frames = FRAMES[fi as usize];
                let idx = d * n;
                let id = if idx < frames.len() { frames[idx] as i32 } else { 0 };
                let (sheet, r, size) = match self.sprite_rect(id) {
                    Some(v) => v,
                    None => continue,
                };
                let half = size / 2;
                let mut left = -half - 1;
                let mut top = -half - 1;
                let mut right = half + 1;
                let mut bottom = half + 1;
                let col_has = |x: i32| (0..size).any(|y| sheet.get(r.x + x, r.y + y) != 0);
                let row_has = |y: i32| (0..size).any(|x| sheet.get(r.x + x, r.y + y) != 0);
                for x in 0..half {
                    if col_has(x) {
                        left = x - half - 1;
                        break;
                    }
                }
                for y in 0..half {
                    if row_has(y) {
                        top = y - half - 1;
                        break;
                    }
                }
                for x in (1..size).rev() {
                    if col_has(x) {
                        right = x - half + 1;
                        break;
                    }
                }
                for y in (1..size).rev() {
                    if row_has(y) {
                        bottom = y - half + 1;
                        break;
                    }
                }
                self.msize[t][d] = [left, top, right, bottom];
            }
        }
    }

    /// Draw sprite `id` centred at (x,y) into `dst`, clipped to `clip`.
    pub fn draw_sprite(&self, dst: &mut Surface, x: i32, y: i32, id: i32, clip: Rect, side1: bool) -> i32 {
        if let Some((sheet, r, size)) = self.sprite_rect(id) {
            let dx = x - size / 2;
            let dy = y - size / 2;
            if side1 {
                blit_clipped_map(dst, sheet, r, dx, dy, clip, &self.side1_map);
            } else {
                dst.blit_clipped(sheet, r, dx, dy, clip, Some(0));
            }
            size
        } else {
            0
        }
    }

    /// `_DRAW_ANIMATION`: pick the frame for (anim, dir, moving, part, counter) and draw it.
    pub fn draw_animation(&self, dst: &mut Surface, x: i32, y: i32, anim: i32, dir: i32, moving: i32, part: i32, counter: i32, clip: Rect, side1: bool) {
        if anim < 0 || anim as usize >= ANIMATION.len() {
            return;
        }
        let a = &ANIMATION[anim as usize];
        if a[0] < 0 {
            return;
        }
        let frames = FRAMES[a[0] as usize];
        let n = a[1] as i32;
        let e = a[2] as i32;
        let d = dir.rem_euclid(8);
        let i = if part == 0 {
            let k = if e != 0 { e } else { n };
            let period = (k - 1).max(1);
            d * n + (counter.rem_euclid(period)) * moving
        } else {
            let k = n - e;
            let period = (k - 1).max(1);
            d * n + (counter.rem_euclid(period)) * moving + e
        };
        let i = i.max(0) as usize;
        if i >= frames.len() {
            return;
        }
        let mut id = frames[i] as i32;
        if (id == 0 || id == -1) && i > 0 {
            id = frames[i - 1] as i32;
        }
        self.draw_sprite(dst, x, y, id, clip, side1);
    }
}

fn blit_clipped_map(dst: &mut Surface, src: &Surface, r: Rect, dx: i32, dy: i32, clip: Rect, map: &[u8; 256]) {
    for yy in 0..r.h {
        let sy = r.y + yy;
        let ty = dy + yy;
        if ty < clip.y || ty >= clip.bottom() || ty < 0 || ty >= dst.h as i32 {
            continue;
        }
        for xx in 0..r.w {
            let sx = r.x + xx;
            let tx = dx + xx;
            if tx < clip.x || tx >= clip.right() || tx < 0 || tx >= dst.w as i32 {
                continue;
            }
            let c = src.get(sx, sy);
            if c == 0 {
                continue;
            }
            dst.px[ty as usize * dst.w + tx as usize] = map[c as usize];
        }
    }
}

/// Camera (top-left world pixel) for side p's viewport.
pub fn camera(sim: &Sim, p: usize) -> (i32, i32) {
    let m = &sim.mechs[sim.cur[p]];
    if m.nmissiles > 0 || m.redraw > 0 {
        let bi = m.missile[0];
        if bi >= 0 && (bi as usize) < sim.bullets.len() && sim.bullets[bi as usize].alive {
            let b = &sim.bullets[bi as usize];
            return (b.x - 72, b.y - 69);
        }
    }
    (m.x - 72, m.y - 69)
}

pub struct Renderer {
    pub art: BattleArt,
    pub clock: i32,
}

impl Renderer {
    pub fn new(art: BattleArt) -> Renderer {
        Renderer { art, clock: 0 }
    }

    pub fn draw_frame(&self, fb: &mut Surface) {
        fb.blit(&self.art.frame, Rect::new(0, 0, 320, 200), 0, 0);
    }

    /// Draw one side's viewport: tiles, mechs, bullets.
    pub fn draw_view(&self, fb: &mut Surface, sim: &Sim, p: usize) {
        let clip = Rect::new(VIEW_X[p], 0, VIEW_W, VIEW_H);
        let (cx, cy) = camera(sim, p);
        fb.fill_rect(clip, 0);
        // tiles
        let tx0 = cx.div_euclid(32);
        let ty0 = cy.div_euclid(32);
        let tx1 = (cx + VIEW_W - 1).div_euclid(32);
        let ty1 = (cy + VIEW_H - 1).div_euclid(32);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let idx = sim.map.get(tx, ty);
                let (sheet, r) = TileSet::tile_rect(idx);
                fb.blit_clipped(&self.art.sheets[sheet], r, clip.x + tx * 32 - cx, ty * 32 - cy, clip, None);
            }
        }
        let ox = clip.x - cx;
        let oy = -cy;
        // mechs
        for m in 0..NMECH {
            let mm = &sim.mechs[m];
            if mm.status == -1 || mm.status == -4 {
                continue;
            }
            let sx = mm.x + ox;
            let sy = mm.y + oy;
            if sx < clip.x - 32 || sx > clip.right() + 32 || sy < -32 || sy > VIEW_H + 32 {
                continue;
            }
            let side1 = mm.side == 1;
            if mm.status == -2 {
                let (a, _limit) = if mm.kind < 6 { (24, 16) } else { (23, 12) };
                self.art.draw_animation(fb, sx, sy, a, 0, 1, 0, mm.anim, clip, false);
                continue;
            }
            let kind = mm.kind.clamp(0, 12) as usize;
            let feet = STATMECH[kind][0] as i32;
            if !mm.in_water && feet >= 0 {
                let moving = if mm.status == 1 { 1 } else { 0 };
                self.art.draw_animation(fb, sx, sy, feet, mm.dir, moving, 0, self.clock, clip, side1);
            }
            if mm.reload != 0 && mm.weapon == 0 {
                self.art.draw_animation(fb, sx, sy, kind as i32, mm.dir, 1, 1, mm.anim, clip, side1);
            } else {
                self.art.draw_animation(fb, sx, sy, kind as i32, mm.dir, 0, 0, mm.anim, clip, side1);
            }
        }
        // bullets
        for b in sim.bullets.iter() {
            if !b.alive {
                continue;
            }
            let sx = b.x + ox;
            let sy = b.y + oy;
            if sx < clip.x - 32 || sx > clip.right() + 32 || sy < -32 || sy > VIEW_H + 32 {
                continue;
            }
            let w = b.wtype;
            if b.age < 0 {
                let a = (25 + super::sim::wtab(w)[8] as i32).clamp(25, 29);
                let n = ANIMATION[a as usize][1] as i32;
                self.art.draw_animation(fb, sx, sy, a, 0, 1, 0, n + b.age, clip, false);
            } else if b.age < 4 {
                self.art.draw_animation(fb, sx, sy, 15 + w as i32, b.dir, 1, 1, b.age, clip, false);
            } else {
                self.art.draw_animation(fb, sx, sy, 15 + w as i32, b.dir, 1, 0, b.age, clip, false);
            }
        }
    }

    pub fn draw_hud(&self, fb: &mut Surface, sim: &Sim) {
        for p in 0..2 {
            let m = &sim.mechs[sim.cur[p]];
            let kind = m.kind.clamp(0, 12) as usize;
            let px = p as i32;
            // weapon icons
            for s in 0..4 {
                let w = STATMECH[kind][3 + s];
                let icon = if w >= 0 { super::sim::wtab(w as usize)[5] as i32 } else { 1241 };
                let x = 58 + 128 * px + 20 * s as i32;
                self.draw_icon(fb, x, 145, icon, false);
                if s == m.weapon {
                    self.draw_icon(fb, x, 145, 1242, true);
                }
                // ammo bar
                let maxa = STATMECH[kind][7 + s] as i32;
                let len = if m.ammo[s] == 0 || w < 0 {
                    0
                } else if m.ammo[s] < 0 || maxa <= 0 {
                    15
                } else {
                    (m.ammo[s] * 15 / maxa).clamp(0, 15)
                };
                bar(fb, x, 164, len, 15);
            }
            // heat / armour
            bar(fb, 79 + 129 * px, 170, (m.heat * 54 / 150).clamp(0, 54), 54);
            let maxh = STATMECH[kind][1].max(1) as i32;
            bar(fb, 79 + 129 * px, 177, (m.hits * 54 / maxh).clamp(0, 54), 54);
            // hold / retreat indicator
            let tx = 59 + 128 * px;
            fb.blit(&self.art.mechset, Rect::xyxy(304, 178, 319, 189), tx, 186);
            fb.blit(&self.art.mechset, Rect::xyxy(304, 178, 319, 189), tx + 16, 186);
            if sim.retreat[p] {
                fb.blit(&self.art.mechset, Rect::xyxy(112, 178, 143, 189), tx, 186);
            } else if sim.lho[p] == 1 {
                fb.blit(&self.art.mechset, Rect::xyxy(96, 146, 111, 157), tx, 186);
            }
        }
        self.draw_radars(fb, sim);
    }

    fn draw_icon(&self, fb: &mut Surface, x: i32, y: i32, id: i32, transparent: bool) {
        if let Some((sheet, r, _)) = self.art.sprite_rect(id) {
            if transparent {
                fb.blit_key(sheet, r, x, y, 0);
            } else {
                fb.blit(sheet, r, x, y);
            }
        }
    }

    fn draw_radars(&self, fb: &mut Surface, sim: &Sim) {
        for p in 0..2 {
            let xo = 3 + 269 * p as i32;
            let box_r = Rect::new(xo, 146, 48, 48);
            fb.fill_rect(box_r, 0);
            let (cx, cy) = {
                let m = &sim.mechs[sim.cur[p]];
                (m.x, m.y)
            };
            for (dx, dy) in [(23, 22), (23, 24), (24, 23), (22, 23)] {
                fb.put(xo + dx, 146 + dy, 4);
            }
            for (i, m) in sim.mechs.iter().enumerate() {
                if m.status < 0 {
                    continue;
                }
                let sx = (m.x - cx) / 32 + 23;
                let sy = (m.y - cy) / 32 + 23;
                if (0..=47).contains(&sx) && (0..=47).contains(&sy) {
                    let c = if i < 10 { 31 } else { 13 };
                    if fb.get(xo + sx, 146 + sy) == 0 {
                        fb.put(xo + sx, 146 + sy, c);
                    }
                }
            }
            for b in sim.bullets.iter() {
                if !b.alive {
                    continue;
                }
                let sx = (b.x - cx) / 32 + 23;
                let sy = (b.y - cy) / 32 + 23;
                if (0..=47).contains(&sx) && (0..=47).contains(&sy) && fb.get(xo + sx, 146 + sy) == 0 {
                    fb.put(xo + sx, 146 + sy, 8);
                }
            }
        }
        // long range radar
        fb.fill_rect(Rect::xyxy(141, 146, 179, 184), 0);
        for p in 0..2 {
            let m = &sim.mechs[sim.cur[p]];
            if m.status >= 0 {
                let lx = (m.x - 1280) / 70 + 18;
                let ly = (m.y - 1152) / 70 + 18;
                if (0..=38).contains(&lx) && (0..=38).contains(&ly) {
                    fb.put(141 + lx, 146 + ly, if p == 0 { 31 } else { 13 });
                }
            }
        }
        for (i, m) in sim.mechs.iter().enumerate().take(20) {
            if m.status < 0 {
                continue;
            }
            let lx = (m.x - 1280) / 70 + 18;
            let ly = (m.y - 1152) / 70 + 18;
            if (0..=38).contains(&lx) && (0..=38).contains(&ly) && fb.get(141 + lx, 146 + ly) == 0 {
                fb.put(141 + lx, 146 + ly, if i < 10 { 14 } else { 12 });
            }
        }
    }

    /// Pre-battle: flags and "HIT KEY TO START".
    pub fn draw_prestart(&self, fb: &mut Surface, pc: [u8; 2]) {
        for p in 0..2 {
            let x = [56, 229][p];
            let flag = Rect::new(160 + 32 * pc[p] as i32, 32, 32, 32);
            fb.blit(&self.art.sheets[1], flag, x, 20);
            fb.blit(&self.art.mechset, Rect::xyxy(272, 176, 303, 191), x, 90);
        }
    }
}

fn bar(fb: &mut Surface, x: i32, y: i32, len: i32, width: i32) {
    fb.fill_rect(Rect::new(x, y, len.clamp(0, width), 3), 8);
    fb.fill_rect(Rect::new(x + len.clamp(0, width), y, width - len.clamp(0, width), 3), 0);
}
