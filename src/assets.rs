//! Asset registry: raw original game files supplied by the host, decoded on demand.

use crate::gfx::{Palette, Surface};
use crate::gif;
use std::collections::HashMap;

pub struct Image {
    pub surf: Surface,
    pub pal: Palette,
    pub pal_bits: u8,
}

#[derive(Default)]
pub struct Assets {
    raw: HashMap<String, Vec<u8>>,
    images: HashMap<String, Image>,
}

pub fn norm(name: &str) -> String {
    let base = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(name);
    base.to_ascii_lowercase()
}

impl Assets {
    pub fn new() -> Assets {
        Assets::default()
    }
    pub fn add(&mut self, name: &str, data: Vec<u8>) {
        self.raw.insert(norm(name), data);
    }
    pub fn has(&self, name: &str) -> bool {
        self.raw.contains_key(&norm(name))
    }
    pub fn raw(&self, name: &str) -> Option<&[u8]> {
        self.raw.get(&norm(name)).map(|v| v.as_slice())
    }
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.raw.keys().cloned().collect();
        v.sort();
        v
    }
    /// Decode (and cache) a GIF asset.
    pub fn image(&mut self, name: &str) -> Option<&Image> {
        let key = norm(name);
        if !self.images.contains_key(&key) {
            let data = self.raw.get(&key)?;
            let g = gif::decode(data).ok()?;
            let img = Image { surf: Surface::from_gif(&g), pal: g.palette, pal_bits: g.palette_bits };
            self.images.insert(key.clone(), img);
        }
        self.images.get(&key)
    }
    pub fn image_cloned(&mut self, name: &str) -> Option<(Surface, Palette, u8)> {
        let i = self.image(name)?;
        Some((i.surf.clone(), i.pal, i.pal_bits))
    }
    pub fn surface(&mut self, name: &str) -> Option<Surface> {
        self.image(name).map(|i| i.surf.clone())
    }
    pub fn palette(&mut self, name: &str) -> Option<Palette> {
        self.image(name).map(|i| i.pal)
    }
    /// Which required files are missing.
    pub fn missing(&self) -> Vec<&'static str> {
        REQUIRED.iter().filter(|n| !self.has(n)).cloned().collect()
    }
}

/// Files the game needs (everything else is optional).
pub const REQUIRED: &[&str] = &[
    "mainscr3.gif", "topw1b.gif", "topw2b.gif", "topw3b.gif", "topw4b.gif", "button10.gif", "framebt3.gif",
    "door3.gif", "mechset8.gif", "batframe.gif", "arctic4.gif", "conifer.gif", "desert3.gif", "volcanic.gif",
    "city1f.gif", "city2f.gif", "struct4.gif", "wall4.gif", "newmars.gif", "newmars2.gif", "openfii.gif",
    "openfii2.gif", "bay2.gif", "let.gif", "titan.gif", "behemoth.gif", "hercules.gif", "achilles.gif",
    "crosbolt.gif", "mars.gif", "dragon.gif", "cyclops.gif", "mercury.gif",
];

/// The terrain tileset file names in terrain order (1..=5), as the original loads them.
pub fn terrain_tilesets(terrain: u8) -> [&'static str; 3] {
    match terrain {
        1 => ["arctic4.gif", "struct4.gif", "wall4.gif"],
        2 => ["conifer.gif", "struct4.gif", "wall4.gif"],
        3 => ["desert3.gif", "struct4.gif", "wall4.gif"],
        4 => ["volcanic.gif", "struct4.gif", "wall4.gif"],
        _ => ["city1f.gif", "struct4.gif", "city2f.gif"],
    }
}

pub fn terrain_prefix(terrain: u8) -> &'static str {
    match terrain {
        1 => "arct_",
        2 => "coni_",
        3 => "dest_",
        4 => "volc_",
        _ => "city_",
    }
}
