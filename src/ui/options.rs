//! Campaign settings screen (`_GET_OPTIONS`) and the name / skill entry (`_GET_PLAYER_NAMES`).

use super::Sheets;
use crate::data::{EMPIRE_NAMES, GENERAL_NAMES, PLANET_NAMES};
use crate::gfx::{Rect, Surface};
use crate::input::{key, Input};
use crate::strategy::state::{NewGameOptions, PlayerType};
use crate::text::Font;

pub struct OptionsScreen {
    pub types: [PlayerType; 5],
    pub planet: i16,
    pub fog: bool,
    pub complete: bool,
    pub load_requested: bool,
    pub done: bool,
    /// selected load slot (1..5) when the load box is open
    pub load_box: bool,
}

/// Column x ranges of the five empire radio buttons and the three row y ranges.
const COLS: [(i32, i32); 5] = [(0x4C, 0x5D), (0x79, 0x8A), (0xA6, 0xB7), (0xD3, 0xE4), (0x100, 0x111)];
const ROWS: [(i32, i32); 3] = [(0x48, 0x50), (0x54, 0x5C), (0x60, 0x68)];
const GO: Rect = Rect::xyxy(0x1B, 0x95, 0x41, 0xAF);
const LOAD: Rect = Rect::xyxy(0x114, 0x9B, 0x11B, 0xB3);
const PLANET_HIT: [Rect; 3] = [Rect::xyxy(0x4C, 0xA8, 0x7F, 0xB0), Rect::xyxy(0x8B, 0xA8, 0xC3, 0xB0), Rect::xyxy(0xC8, 0xA8, 0x100, 0xB0)];
const SHOWALL: Rect = Rect::xyxy(0x3D, 0x7D, 0x85, 0x8B);
const FOG: Rect = Rect::xyxy(0xC1, 0x7D, 0x109, 0x8B);
/// Load-box slot buttons (x1,x2,y1,y2) from `_LOADB`: 0 = cancel.
const LOADB: [[i32; 4]; 6] = [[112, 148, 62, 70], [112, 148, 82, 90], [112, 148, 102, 110], [162, 198, 62, 70], [162, 198, 82, 90], [162, 198, 102, 110]];

impl OptionsScreen {
    pub fn new(complete: bool) -> OptionsScreen {
        OptionsScreen {
            types: [PlayerType::Human, PlayerType::Computer, PlayerType::None, PlayerType::None, PlayerType::None],
            planet: 2,
            fog: false,
            complete,
            load_requested: false,
            done: false,
            load_box: false,
        }
    }

    pub fn valid(&self) -> bool {
        let n = self.types.iter().filter(|&&t| t != PlayerType::None).count();
        n >= 2 && self.types.iter().any(|&t| t == PlayerType::Human)
    }

    /// Returns Some(slot) when a load slot was chosen (0 = cancel).
    pub fn update(&mut self, input: &Input) -> Option<usize> {
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if !input.clicked() {
            return None;
        }
        if self.load_box {
            for (i, b) in LOADB.iter().enumerate() {
                if mx >= b[0] && mx <= b[1] && my >= b[2] && my <= b[3] {
                    self.load_box = false;
                    return Some(i);
                }
            }
            return None;
        }
        for (p, &(x0, x1)) in COLS.iter().enumerate() {
            if mx >= x0 && mx <= x1 {
                for (r, &(y0, y1)) in ROWS.iter().enumerate() {
                    if my >= y0 && my <= y1 {
                        self.types[p] = [PlayerType::None, PlayerType::Human, PlayerType::Computer][r];
                    }
                }
            }
        }
        for (i, r) in PLANET_HIT.iter().enumerate() {
            if r.contains(mx, my) {
                self.planet = i as i16 + 1;
            }
        }
        if SHOWALL.contains(mx, my) {
            self.fog = false;
        }
        if FOG.contains(mx, my) {
            self.fog = true;
        }
        if GO.contains(mx, my) && self.valid() {
            self.done = true;
        }
        if LOAD.contains(mx, my) {
            self.load_box = true;
        }
        None
    }

    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets) {
        // DOOR3.GIF holds the settings panel in its top 152 rows.
        fb.blit(&sheets.door, Rect::new(0, 0, 320, 152), 0, 0);
        fb.fill_rect(Rect::new(0, 152, 320, 48), 0);
        let f = &sheets.fonts;
        // column headers: empire symbols come from BUTTON10 (the five 32x32 symbols at (163..,30..))
        for p in 0..5 {
            let x = COLS[p].0;
            fb.blit_key(&sheets.button, super::art::flag(p as u8), x + 1, 0x38, 0);
            for r in 0..3 {
                let (y0, y1) = ROWS[r];
                let sel = self.types[p] == [PlayerType::None, PlayerType::Human, PlayerType::Computer][r];
                let rect = Rect::xyxy(x, y0, COLS[p].1, y1);
                fb.fill_rect(rect, if sel { 2 } else { 0 });
                fb.rect_outline(rect, 7);
            }
        }
        f.draw(fb, Font::SmallYellow, 0x1C, 0x4A, "EMPIRE");
        f.draw(fb, Font::SmallRed, 0x1C, 0x4A + 9, "NON-ACTIVE");
        f.draw(fb, Font::SmallYellow, 0x1C, 0x56, "HUMAN");
        f.draw(fb, Font::SmallYellow, 0x1C, 0x62, "COMPUTER");
        // map display
        f.draw(fb, Font::SmallYellow, 0x1C, 0x72, "MAP DISPLAY");
        f.draw(fb, if !self.fog { Font::SmallRed } else { Font::SmallYellow }, SHOWALL.x + 4, SHOWALL.y + 4, "SHOW ALL");
        f.draw(fb, if self.fog { Font::SmallRed } else { Font::SmallYellow }, FOG.x + 4, FOG.y + 4, "FOG OF WAR");
        f.draw(fb, Font::SmallYellow, 0x1C, 0x9C, "PLANET TYPE");
        for i in 0..3 {
            let r = PLANET_HIT[i];
            let font = if self.planet == i as i16 + 1 { Font::SmallRed } else { Font::SmallYellow };
            f.draw(fb, font, r.x + 4, r.y + 2, PLANET_NAMES[i]);
        }
        // GO and LOAD buttons
        fb.rect_outline(GO, 7);
        f.draw(fb, if self.valid() { Font::SmallYellow } else { Font::SmallRed }, GO.x + 13, GO.y + 10, "GO");
        fb.rect_outline(Rect::xyxy(0x10C, 0x9B, 0x123, 0xB3), 7);
        f.draw(fb, Font::SmallYellow, 0x10E, 0xA4, "LOAD");
        f.draw(fb, Font::Big, 0x44, 0x1C, if self.complete { "COMPLETE CAMPAIGN" } else { "STRATEGY CAMPAIGN" });
        if self.load_box {
            draw_slot_box(fb, sheets, "SELECT LOAD GAME");
        }
    }

    pub fn to_new_game(&self, names: [String; 5], skills: [i16; 5]) -> NewGameOptions {
        NewGameOptions { types: self.types, names, skills, planet: self.planet, fog: self.fog, complete_campaign: self.complete }
    }
}

pub fn draw_slot_box(fb: &mut Surface, sheets: &Sheets, title: &str) {
    sheets.dialog(fb);
    sheets.fonts.draw_centered(fb, Font::SmallYellow, 160, 40, title);
    for (i, b) in LOADB.iter().enumerate() {
        let r = Rect::xyxy(b[0], b[2], b[1], b[3]);
        fb.fill_rect(r, 5);
        fb.rect_outline(r, 7);
        let label = if i == 0 { "CANCEL".to_string() } else { format!("GAME {}", i) };
        sheets.fonts.draw_centered(fb, Font::SmallYellow, (b[0] + b[1]) / 2, b[2] + 2, &label);
    }
}

/// Name entry / computer skill selection for each active player in turn.
pub struct NamesScreen {
    pub idx: usize,
    pub names: [String; 5],
    pub skills: [i16; 5],
    pub types: [PlayerType; 5],
    pub done: bool,
}

impl NamesScreen {
    pub fn new(types: [PlayerType; 5]) -> NamesScreen {
        let mut s = NamesScreen { idx: 0, names: Default::default(), skills: [2; 5], types, done: false };
        for p in 0..5 {
            if types[p] == PlayerType::Computer {
                s.names[p] = EMPIRE_NAMES[p].to_string();
            }
        }
        s.skip();
        s
    }
    fn skip(&mut self) {
        while self.idx < 5 && self.types[self.idx] == PlayerType::None {
            self.idx += 1;
        }
        if self.idx >= 5 {
            self.done = true;
        }
    }
    pub fn update(&mut self, input: &Input) {
        if self.done {
            return;
        }
        let p = self.idx;
        match self.types[p] {
            PlayerType::Human => {
                for &c in &input.typed {
                    let c = c.to_ascii_uppercase();
                    if c == 8 {
                        self.names[p].pop();
                    } else if (c.is_ascii_uppercase() || c == b'-' || c == b' ') && self.names[p].len() < 11 {
                        self.names[p].push(c as char);
                    }
                }
                if input.pressed(key::BACKSPACE) && !input.typed.contains(&8) {
                    self.names[p].pop();
                }
                if (input.pressed(key::ENTER) || input.pressed(key::KP_ENTER)) && !self.names[p].trim().is_empty() {
                    self.idx += 1;
                    self.skip();
                }
            }
            PlayerType::Computer => {
                if input.clicked() {
                    for i in 0..6 {
                        let bx = 0x70 + 16 * i;
                        if input.mouse_x >= bx && input.mouse_x <= bx + 9 && input.mouse_y > 0x68 && input.mouse_y < 0x71 {
                            self.skills[p] = i as i16;
                            self.idx += 1;
                            self.skip();
                            break;
                        }
                    }
                }
            }
            PlayerType::None => {
                self.idx += 1;
                self.skip();
            }
        }
    }
    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets) {
        fb.clear(0);
        sheets.dialog(fb);
        if self.done {
            return;
        }
        let p = self.idx;
        let f = &sheets.fonts;
        sheets.flag_icon(fb, p as u8, 0x5A, 0x1C);
        f.draw(fb, Font::SmallYellow, 0x70, 0x1E, &format!("{} GENERAL:", GENERAL_NAMES[p]));
        match self.types[p] {
            PlayerType::Human => {
                f.draw(fb, Font::SmallYellow, 0x5A, 0x30, "ENTER YOUR NAME");
                f.draw(fb, Font::SmallRed, 0x5A, 0x40, &format!("{}_", self.names[p]));
                f.draw(fb, Font::SmallYellow, 0x5A, 0x60, "THEN PRESS ENTER");
            }
            _ => {
                f.draw(fb, Font::SmallYellow, 0x5A, 0x30, "CHOOSE COMPUTER");
                f.draw(fb, Font::SmallYellow, 0x5A, 0x38, "SKILL LEVEL...");
                f.draw(fb, Font::SmallRed, 0x5A, 0x48, &self.names[p]);
                f.draw(fb, Font::SmallYellow, 0x5A, 0x6A, "NASTY");
                f.draw(fb, Font::SmallYellow, 0xD4, 0x6A, "WIMPY");
                for i in 0..6 {
                    let bx = 0x70 + 16 * i;
                    let r = Rect::xyxy(bx, 0x68, bx + 9, 0x71);
                    fb.fill_rect(r, if self.skills[p] == i as i16 { 2 } else { 5 });
                    fb.rect_outline(r, 7);
                    f.draw(fb, Font::SmallYellow, bx + 3, 0x6A, &format!("{}", i + 1));
                }
            }
        }
    }
}
