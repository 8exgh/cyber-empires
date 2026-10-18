//! Battlefield tile map: 80x78 tiles of 32x32 pixels drawn from three 60-tile sheets.

use crate::assets::{terrain_prefix, terrain_tilesets, Assets};
use crate::gfx::{remap_table, Palette, Rect, Surface};

pub const MAP_W: usize = 80;
pub const MAP_H: usize = 78;
pub const TILE: i32 = 32;
pub const TILES_PER_SHEET: u8 = 60;
/// Border tile value (yellow/black striped boundary) — 3 tiles thick on every side.
pub const BORDER_TILE: u8 = 2;

#[derive(Clone, Debug)]
pub struct BattleMap {
    pub terrain: u8,
    pub country: u8,
    pub tiles: Vec<u8>,
}

impl BattleMap {
    pub fn empty(terrain: u8) -> BattleMap {
        BattleMap { terrain, country: 0, tiles: vec![0; MAP_W * MAP_H] }
    }
    /// Load `<prefix><country>.map` (a GIF whose top-left pixels are the tile indices).
    pub fn load(assets: &mut Assets, terrain: u8, country: u8) -> Option<BattleMap> {
        let name = format!("{}{}.map", terrain_prefix(terrain), country);
        let s = assets.surface(&name)?;
        let mut tiles = vec![0u8; MAP_W * MAP_H];
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                tiles[y * MAP_W + x] = s.get(x as i32, y as i32);
            }
        }
        Some(BattleMap { terrain, country, tiles })
    }
    #[inline]
    pub fn get(&self, tx: i32, ty: i32) -> u8 {
        if tx < 0 || ty < 0 || tx >= MAP_W as i32 || ty >= MAP_H as i32 {
            BORDER_TILE
        } else {
            self.tiles[ty as usize * MAP_W + tx as usize]
        }
    }
    #[inline]
    pub fn set(&mut self, tx: i32, ty: i32, v: u8) {
        if tx >= 0 && ty >= 0 && tx < MAP_W as i32 && ty < MAP_H as i32 {
            self.tiles[ty as usize * MAP_W + tx as usize] = v;
        }
    }
}

/// The three tile sheets of a terrain, remapped into one shared palette.
pub struct TileSet {
    pub sheets: [Surface; 3],
    pub pal: Palette,
    pub ncolors: usize,
}

impl TileSet {
    /// Build a tileset for `terrain`. `base_pal`/`base_n` give colours that must stay fixed
    /// (e.g. HUD/sprite colours); tile colours are appended after them.
    pub fn load(assets: &mut Assets, terrain: u8, base_pal: &Palette, base_n: usize) -> Option<TileSet> {
        let names = terrain_tilesets(terrain);
        let mut pal = *base_pal;
        let mut n = base_n;
        let mut sheets: Vec<Surface> = Vec::new();
        for name in names.iter() {
            let (mut s, spal, bits) = assets.image_cloned(name)?;
            // add this sheet's used colours to the palette (dedup exact matches)
            let mut used = [false; 256];
            for &p in &s.px {
                used[p as usize] = true;
            }
            let mut map = [0u8; 256];
            for i in 0..(1usize << bits) {
                if !used[i] {
                    continue;
                }
                let c = spal[i];
                if let Some(j) = (0..n).find(|&j| pal[j] == c) {
                    map[i] = j as u8;
                } else if n < 256 {
                    pal[n] = c;
                    map[i] = n as u8;
                    n += 1;
                } else {
                    let t = remap_table(&spal, 256, &pal, n);
                    map[i] = t[i];
                }
            }
            s.remap(&map);
            sheets.push(s);
        }
        let sheets: [Surface; 3] = [sheets.remove(0), sheets.remove(0), sheets.remove(0)];
        Some(TileSet { sheets, pal, ncolors: n })
    }

    pub fn tile_rect(idx: u8) -> (usize, Rect) {
        let sheet = (idx / TILES_PER_SHEET).min(2) as usize;
        let i = (idx % TILES_PER_SHEET) as i32;
        (sheet, Rect::new((i % 10) * TILE, (i / 10) * TILE, TILE, TILE))
    }

    pub fn draw_tile(&self, dst: &mut Surface, idx: u8, x: i32, y: i32, clip: Rect) {
        let (sheet, r) = Self::tile_rect(idx);
        dst.blit_clipped(&self.sheets[sheet], r, x, y, clip, None);
    }

    /// Draw the portion of `map` visible in `view` (screen rect) with the camera at world pixel (cx, cy)
    /// in the top-left of the view.
    pub fn draw_map(&self, dst: &mut Surface, map: &BattleMap, view: Rect, cx: i32, cy: i32) {
        let tx0 = cx.div_euclid(TILE);
        let ty0 = cy.div_euclid(TILE);
        let tx1 = (cx + view.w - 1).div_euclid(TILE);
        let ty1 = (cy + view.h - 1).div_euclid(TILE);
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let idx = map.get(tx, ty);
                self.draw_tile(dst, idx, view.x + tx * TILE - cx, view.y + ty * TILE - cy, view);
            }
        }
    }
}
