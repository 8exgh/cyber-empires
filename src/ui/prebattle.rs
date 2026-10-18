//! Pre-battle screens: attacker setup (mission, commitment, control cyborg) and defender setup (placement).

use super::Sheets;
use crate::battle::map::BattleMap;
use crate::battle::setup::{insert_structures, Structures};
use crate::battle::tables::{AP, BO, PL};
use crate::data::{ARMOR, CYBORG_NAMES, NO_CYBORG};
use crate::gfx::{Rect, Surface};
use crate::input::{key, Input};
use crate::strategy::resolve::{calculate_damage, Battle};
use crate::strategy::state::Game;
use crate::text::Font;

pub const MISSION_NAMES: [&str; 4] = ["GUERRILLA ATTACK:", "STRUCTURE ATTACK:", "CAPITAL ATTACK:", "CONQUER COUNTRY:"];
pub const MISSION_TEXT: [[&str; 3]; 4] = [
    ["CYBORG UNITS", "SEEK AND FIRE UPON ANY ENEMY", "STRUCTURES OR CYBORG UNITS..."],
    ["CYBORG UNITS", "ARE TO SEEK AND FIRE UPON ANY", "ENEMY STRUCTURES ..."],
    ["CYBORG UNITS ARE", "TO SEEK AND DESTROY ENEMY CAPITAL", "BUILDING..."],
    ["CYBORG UNITS", "ARE TO SEEK AND FIRE UPON ALL", "ENEMY UNITS UNTIL DESTROYED..."],
];
pub const AGG_TEXT: [[&str; 2]; 5] = [
    ["UNCOMMITTED ATTACK", "MAY NOT FOLLOW ORDERS"],
    ["LOOSELY COMMITTED ATTACK:", "FOLLOW ORDERS LOOSELY"],
    ["MODERATELY COMMITTED ATTACK:", "FOLLOW ORDERS MODERATELY CLOSE"],
    ["STRONGLY COMMITTED ATTACK:", "FOLLOW ORDERS VERY CLOSE"],
    ["EXTREMELY COMMITTED ATTACK:", "FOLLOW ORDERS EXTREMELY CLOSE"],
];

const DONE: Rect = Rect::xyxy(0xD, 0xA, 0x4C, 0x1C);
const SIMULATE: Rect = Rect::xyxy(0x57, 0xA, 0x93, 0x1C);
const MISSION_BTN: [Rect; 4] = [Rect::xyxy(0x48, 0x3C, 0x67, 0x5A), Rect::xyxy(0x88, 0x3C, 0xA7, 0x5A), Rect::xyxy(0xC8, 0x3C, 0xE7, 0x5A), Rect::xyxy(0x108, 0x3C, 0x127, 0x5A)];
const AGG_DEC: Rect = Rect::xyxy(0xCC, 0x80, 0xDF, 0x8A);
const AGG_INC: Rect = Rect::xyxy(0xE6, 0x80, 0xF9, 0x8A);
const AUTO: Rect = Rect::xyxy(0xDA, 0x41, 0x119, 0x53);
const MINIMAP: Rect = Rect::xyxy(17, 53, 137, 173);

pub struct AttackSetup {
    pub mission: usize,
    pub agg: usize,
    pub chosen: usize,
    pub slots: Vec<usize>,
    pub done: bool,
    pub simulate: bool,
}

impl AttackSetup {
    pub fn new(b: &Battle) -> AttackSetup {
        let slots: Vec<usize> = (0..10).filter(|&i| b.types[i] != NO_CYBORG).collect();
        AttackSetup { mission: 0, agg: 2, chosen: 0, slots, done: false, simulate: false }
    }
    pub fn update(&mut self, input: &Input) {
        if !input.clicked() {
            if input.pressed(key::ENTER) {
                self.done = true;
            }
            return;
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if DONE.contains(mx, my) {
            self.done = true;
        }
        if SIMULATE.contains(mx, my) {
            self.simulate = true;
            self.done = true;
        }
        for (i, r) in MISSION_BTN.iter().enumerate() {
            if r.contains(mx, my) {
                self.mission = i;
            }
        }
        if AGG_DEC.contains(mx, my) && self.agg > 0 {
            self.agg -= 1;
        }
        if AGG_INC.contains(mx, my) && self.agg < 4 {
            self.agg += 1;
        }
        if mx < 0x84 && my > 0x6C {
            for (k, _) in self.slots.iter().enumerate() {
                let y = 0x3F + 7 * k as i32;
                if mx >= 0x39 && mx <= 0x69 && my >= y && my <= y + 5 {
                    self.chosen = k;
                }
            }
        }
    }
    pub fn control_slot(&self) -> usize {
        self.slots.get(self.chosen).cloned().unwrap_or(0)
    }
    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets, g: &Game, b: &Battle) {
        fb.clear(50);
        let f = &sheets.fonts;
        let def = g.country(b.defend_cty);
        draw_header(fb, sheets, g, b);
        f.draw(fb, Font::SmallYellow, 34, 0x22, &format!("COUNTRY: {}", def.name()));
        f.draw(fb, Font::SmallYellow, 34, 0x28, &format!("TERRAIN: {}", crate::data::TERRAIN_NAMES[def.terrain() as usize]));
        // cyborg list
        fb.fill_rect(Rect::xyxy(19, 100, 151, 185), 0);
        fb.rect_outline(Rect::xyxy(19, 100, 151, 185), 7);
        f.draw(fb, Font::SmallYellow, 0x20, 0x66, "ATTACKING CYBORGS");
        f.draw(fb, Font::SmallYellow, 0x20 + 0x4C, 0x66, "STATUS");
        for (k, &i) in self.slots.iter().enumerate() {
            let y = 0x3F + 7 * k as i32 + 0x30;
            let font = if k == self.chosen { Font::SmallRed } else { Font::SmallYellow };
            f.draw(fb, font, 0x39 - 0x10, y, CYBORG_NAMES[b.types[i] as usize]);
            f.draw(fb, font, 0x39 + 0x40, y, &format!("{}", calculate_damage(b.hits[i], b.types[i])));
        }
        // mission buttons
        for (i, r) in MISSION_BTN.iter().enumerate() {
            fb.fill_rect(*r, if i == self.mission { 8 } else { 54 });
            fb.rect_outline(*r, 7);
            f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 4, ["GUER", "STRU", "CAPI", "CONQ"][i]);
            f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 12, ["RILLA", "CTURE", "TAL", "UER"][i]);
        }
        fb.fill_rect(Rect::xyxy(0xA8, 0x91, 0x122, 0xA2), 0);
        f.draw(fb, Font::SmallYellow, 0xA8, 0x93, MISSION_NAMES[self.mission]);
        for (k, t) in MISSION_TEXT[self.mission].iter().enumerate() {
            f.draw(fb, Font::SmallYellow, 0xA8, 0x99 + 6 * k as i32, t);
        }
        f.draw(fb, Font::SmallYellow, 167, 101, "TARGET PRIORITY");
        // aggression slider
        fb.fill_rect(Rect::xyxy(175, 0x73, 279, 0x78), 54);
        let kx = 175 + 26 * self.agg as i32;
        fb.fill_rect(Rect::new(kx - 3, 0x71, 7, 9), 8);
        for (r, label) in [(AGG_DEC, "<"), (AGG_INC, ">")] {
            fb.fill_rect(r, 54);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::Orange, (r.x + r.right()) / 2, r.y + 2, label);
        }
        fb.fill_rect(Rect::xyxy(0xA8, 0xA4, 0x122, 0xB6), 0);
        f.draw(fb, Font::SmallYellow, 0xA8, 0xAA, AGG_TEXT[self.agg][0]);
        f.draw(fb, Font::SmallYellow, 0xA8, 0xB0, AGG_TEXT[self.agg][1]);
        draw_buttons(fb, sheets, true);
    }
}

fn draw_header(fb: &mut Surface, sheets: &Sheets, g: &Game, b: &Battle) {
    let f = &sheets.fonts;
    let ao = b.att_owner;
    let d_o = b.def_owner;
    if ao < 5 {
        sheets.flag_icon(fb, ao, 0xB0, 8);
        f.draw(fb, Font::SmallYellow, 168, 45, &g.players[ao as usize].name);
    }
    if d_o < 5 {
        sheets.flag_icon(fb, d_o, 0xFE, 8);
        f.draw(fb, Font::SmallRed, 246, 45, &g.players[d_o as usize].name);
    }
    f.draw(fb, Font::SmallYellow, 0xC0, 10, "VS");
}

fn draw_buttons(fb: &mut Surface, sheets: &Sheets, show_sim: bool) {
    let f = &sheets.fonts;
    fb.fill_rect(DONE, 54);
    fb.rect_outline(DONE, 7);
    f.draw_centered(fb, Font::SmallYellow, (DONE.x + DONE.right()) / 2, DONE.y + 7, "DONE");
    if show_sim {
        fb.fill_rect(SIMULATE, 54);
        fb.rect_outline(SIMULATE, 7);
        f.draw_centered(fb, Font::SmallYellow, (SIMULATE.x + SIMULATE.right()) / 2, SIMULATE.y + 7, "SIMULATE");
    }
}

pub struct DefendSetup {
    pub map: BattleMap,
    pub structures: Structures,
    pub adir: usize,
    pub slots: Vec<usize>,
    pub positions: Vec<(i32, i32)>,
    pub chosen: usize,
    pub placed_all: bool,
    pub done: bool,
    pub simulate: bool,
    pub auto: bool,
    pub show_sim: bool,
    ttype: usize,
}

impl DefendSetup {
    pub fn new(map: BattleMap, structures: Structures, adir: usize, b: &Battle, ttype: usize, show_sim: bool) -> DefendSetup {
        let slots: Vec<usize> = (10..20).filter(|&i| b.types[i] != NO_CYBORG).collect();
        DefendSetup { map, structures, adir, slots, positions: Vec::new(), chosen: 0, placed_all: false, done: false, simulate: false, auto: false, show_sim, ttype }
    }
    fn limits(&self) -> Rect {
        let d = if self.ttype == 4 { 4 } else { self.adir };
        Rect::xyxy(PL[d * 4] as i32, PL[d * 4 + 2] as i32, PL[d * 4 + 1] as i32, PL[d * 4 + 3] as i32)
    }
    pub fn update(&mut self, input: &Input) {
        if !input.clicked() {
            return;
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if self.placed_all && DONE.contains(mx, my) {
            self.done = true;
            return;
        }
        if self.show_sim && SIMULATE.contains(mx, my) {
            self.simulate = true;
            self.done = true;
            return;
        }
        if !self.placed_all && self.positions.is_empty() && AUTO.contains(mx, my) {
            self.auto = true;
            for (j, _) in self.slots.iter().enumerate() {
                self.positions.push((AP[self.adir * 20 + j * 2] as i32, AP[self.adir * 20 + j * 2 + 1] as i32));
            }
            self.placed_all = true;
            return;
        }
        if !self.placed_all && MINIMAP.contains(mx, my) {
            if self.limits().contains(mx, my) {
                self.positions.push((mx - 1, my - 1));
                if self.positions.len() >= self.slots.len() {
                    self.placed_all = true;
                }
            }
            return;
        }
        if self.placed_all && mx >= 0xA4 && mx <= 0x126 && my >= 0x64 && my <= 0xBB {
            for (k, _) in self.slots.iter().enumerate() {
                let y = 0x6E + 7 * k as i32;
                if my >= y && my <= y + 6 {
                    self.chosen = k;
                }
            }
        }
    }
    pub fn control_slot(&self) -> usize {
        self.slots.get(self.chosen).map(|&s| s - 10).unwrap_or(0)
    }
    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets, g: &Game, b: &Battle, player_pal_idx: u8) {
        fb.clear(50);
        let f = &sheets.fonts;
        let def = g.country(b.defend_cty);
        draw_header(fb, sheets, g, b);
        f.draw(fb, Font::SmallYellow, 34, 0x22, &format!("COUNTRY: {}", def.name()));
        f.draw(fb, Font::SmallYellow, 34, 0x28, &format!("TERRAIN: {}", crate::data::TERRAIN_NAMES[def.terrain() as usize]));
        // minimap frame
        fb.fill_rect(Rect::xyxy(15, 51, 136, 172), 0);
        fb.rect_outline(Rect::xyxy(15, 51, 136, 172), 7);
        let (xo, yo) = if self.ttype == 4 { (0, 0) } else { (BO[self.adir * 2] as i32, BO[self.adir * 2 + 1] as i32) };
        for ty in 9..72 {
            for tx in 10..74 {
                let t = self.map.get(tx + xo, ty + yo);
                let c = tile_colour(t, self.ttype, player_pal_idx);
                let px = 17 + (tx - 10) * 2;
                let py = 53 + (ty - 9) * 2;
                fb.fill_rect(Rect::new(px, py, 2, 2), c);
            }
        }
        // attack direction arrows (red) on the attacker's edge
        let lim = self.limits();
        let arrow_c = 8;
        match self.adir {
            0 => fb.fill_rect(Rect::new(17, 51, 120, 2), arrow_c),
            1 => fb.fill_rect(Rect::new(135, 53, 2, 120), arrow_c),
            2 => fb.fill_rect(Rect::new(17, 171, 120, 2), arrow_c),
            _ => fb.fill_rect(Rect::new(15, 53, 2, 120), arrow_c),
        }
        let _ = lim;
        // placed markers
        for (k, &(px, py)) in self.positions.iter().enumerate() {
            let c = if self.placed_all && k == self.chosen { 2 } else { 4 };
            fb.fill_rect(Rect::new(px, py, 3, 3), c);
        }
        // list
        fb.fill_rect(Rect::xyxy(163, 99, 291, 184), 0);
        fb.rect_outline(Rect::xyxy(163, 99, 291, 184), 7);
        f.draw(fb, Font::SmallYellow, 0xA6, 0x64, "DEFENDING CYBORGS");
        f.draw(fb, Font::SmallYellow, 0xA6 + 0x4C, 0x64, "STATUS");
        for (k, &i) in self.slots.iter().enumerate() {
            let y = 0x6E + 7 * k as i32;
            let active = if self.placed_all { k == self.chosen } else { k == self.positions.len() };
            let font = if active { Font::SmallRed } else { Font::SmallYellow };
            f.draw(fb, font, 0xA6, y, CYBORG_NAMES[b.types[i] as usize]);
            f.draw(fb, font, 0xA6 + 0x4C, y, &format!("{}", calculate_damage(b.hits[i], b.types[i])));
        }
        if !self.placed_all {
            f.draw(fb, Font::SmallYellow, 0xA6, 0x52, "PLACE EACH CYBORG ON");
            f.draw(fb, Font::SmallYellow, 0xA6, 0x58, "THE MAP, OR PRESS AUTO");
            if self.positions.is_empty() {
                fb.fill_rect(AUTO, 54);
                fb.rect_outline(AUTO, 7);
                f.draw_centered(fb, Font::SmallYellow, (AUTO.x + AUTO.right()) / 2, AUTO.y + 6, "AUTO");
            }
        } else {
            f.draw(fb, Font::SmallYellow, 0xA6, 0x52, "SELECT THE CYBORG");
            f.draw(fb, Font::SmallYellow, 0xA6, 0x58, "YOU WILL CONTROL");
            draw_buttons(fb, sheets, self.show_sim);
        }
        if !self.placed_all && self.show_sim {
            fb.fill_rect(SIMULATE, 54);
            fb.rect_outline(SIMULATE, 7);
            f.draw_centered(fb, Font::SmallYellow, (SIMULATE.x + SIMULATE.right()) / 2, SIMULATE.y + 7, "SIMULATE");
        }
        let _ = ARMOR;
    }
}

fn tile_colour(t: u8, ttype: usize, player_idx: u8) -> u8 {
    // colours are indices of the strategy (button) palette
    match t {
        2 => 2,
        60..=69 => 7,
        70..=79 => player_idx,
        80..=119 => 54,
        120..=179 => 45,
        _ => match ttype {
            0 => 43, // arctic: light grey
            1 => 6,  // conifer: green
            2 => 2,  // desert: yellow
            3 => 4,  // volcanic: red-ish
            _ => 49, // city grey
        },
    }
}

/// Build the defender map for the setup screen (READ_MATRIX + INSERT_STRUCTURES).
pub fn prepare_defender_map(assets: &mut crate::assets::Assets, g: &Game, b: &Battle, adir: usize) -> Option<(BattleMap, Structures)> {
    let def = g.country(b.defend_cty);
    let mut map = BattleMap::load(assets, def.terrain(), b.defend_cty)?;
    let st = insert_structures(&mut map, &def.slots, def.owner.min(4) as usize, def.fort, adir, def.terrain() == 5);
    Some((map, st))
}
