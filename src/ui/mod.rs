//! Screens and widgets built on the original art sheets.
pub mod battleflow;
pub mod credit;
pub mod options;
pub mod practice;
pub mod prebattle;
pub mod strategy;
pub mod summary;

use crate::gfx::{Rect, Surface};
use crate::text::{Font, Fonts};

/// Art sheets remapped into the strategy palette.
pub struct Sheets {
    pub button: Surface,
    pub door: Surface,
    pub frame: Surface,
    pub world: Surface,
    pub fonts: Fonts,
    pub victory: Surface,
    pub victory_pal: crate::gfx::Palette,
}

/// Source rectangles in BUTTON10.GIF (inclusive corners) used by the strategy UI.
pub mod art {
    use crate::gfx::Rect;
    pub const DIALOG: Rect = Rect::xyxy(163, 0, 319, 125);
    pub const DIALOG_DEST: (i32, i32) = (80, 16);
    /// Interior of the dialog on screen and the panel grey colour index.
    pub const DIALOG_INNER: Rect = Rect::xyxy(92, 23, 225, 135);
    pub const PANEL_GREY: u8 = 49;
    pub const MSGBOX_CLEAR: Rect = Rect::xyxy(0, 0, 76, 29);
    pub const MSGBOX_DEST: (i32, i32) = (233, 163);
    pub const OK_HIT: Rect = Rect::xyxy(191, 26, 208, 45);
    pub const OK_PRESSED: Rect = Rect::xyxy(145, 0, 161, 7);
    pub const CLEAR_HIT: Rect = Rect::xyxy(98, 25, 159, 44);
    pub const CLEAR_PRESSED: Rect = Rect::xyxy(178, 145, 230, 154);
    /// flag icon for player p: (178+16p,184)-(193+16p,199)
    pub fn flag(p: u8) -> Rect {
        Rect::xyxy(178 + 16 * p as i32, 184, 193 + 16 * p as i32, 199)
    }
    pub fn factory(p: u8) -> Rect {
        Rect::xyxy(147, 9 + 12 * p as i32, 162, 20 + 12 * p as i32)
    }
    pub fn fort_icon(level: u8) -> Rect {
        let l = level.max(1) as i32 - 1;
        Rect::xyxy(151, 80 + 11 * l, 159, 90 + 11 * l)
    }
    pub fn cyb(p: u8) -> Rect {
        Rect::xyxy(226 + 16 * p as i32, 104, 241 + 16 * p as i32, 113)
    }
    pub fn explode(p: u8) -> Rect {
        if p < 4 {
            Rect::xyxy(246 + 16 * p as i32, 91, 260 + 16 * p as i32, 102)
        } else {
            Rect::xyxy(204, 172, 218, 183)
        }
    }
    /// Button bar: (hit rect, pressed-art rect) per operation.
    pub const BTN_MOVE: (Rect, Rect) = (Rect::xyxy(80, 159, 107, 176), Rect::xyxy(0, 133, 21, 144));
    pub const BTN_REPAIR: (Rect, Rect) = (Rect::xyxy(109, 159, 136, 176), Rect::xyxy(23, 133, 44, 144));
    pub const BTN_SPY: (Rect, Rect) = (Rect::xyxy(138, 159, 165, 176), Rect::xyxy(46, 133, 67, 144));
    pub const BTN_CYBORGS: (Rect, Rect) = (Rect::xyxy(167, 159, 194, 176), Rect::xyxy(69, 133, 90, 144));
    pub const BTN_DOOR: Rect = Rect::xyxy(196, 159, 223, 176);
    pub const BTN_CAPITAL: (Rect, Rect) = (Rect::xyxy(80, 180, 107, 198), Rect::xyxy(0, 146, 21, 157));
    pub const BTN_FACTORY: (Rect, Rect) = (Rect::xyxy(109, 180, 136, 198), Rect::xyxy(23, 146, 44, 157));
    pub const BTN_ENHANCE: (Rect, Rect) = (Rect::xyxy(138, 180, 165, 198), Rect::xyxy(46, 146, 67, 157));
    pub const BTN_FORT: (Rect, Rect) = (Rect::xyxy(167, 180, 194, 198), Rect::xyxy(69, 146, 90, 157));
    pub const BTN_OPTIONS: Rect = Rect::xyxy(196, 180, 223, 198);
    pub const ARROW_LEFT: Rect = Rect::xyxy(8, 171, 31, 185);
    pub const ARROW_RIGHT: Rect = Rect::xyxy(53, 171, 76, 185);
    pub const ARROW_UP: Rect = Rect::xyxy(34, 158, 50, 176);
    pub const ARROW_DOWN: Rect = Rect::xyxy(34, 179, 50, 197);
    pub const MAP_WIN: Rect = Rect::new(16, 16, 288, 126);
    pub const MSG_X: i32 = 234;
    pub const MSG_Y: [i32; 4] = [164, 172, 179, 186];
    /// Sub-menu icon buttons (fort / factory menus): hit rects and art.
    pub const SUB_HIT: [Rect; 3] = [Rect::xyxy(94, 46, 116, 63), Rect::xyxy(94, 68, 116, 85), Rect::xyxy(94, 90, 116, 112)];
    pub const FACT_ART: [Rect; 3] = [Rect::xyxy(155, 126, 177, 143), Rect::xyxy(155, 144, 177, 161), Rect::xyxy(155, 162, 177, 179)];
    pub const FORT_ART: [Rect; 3] = [Rect::xyxy(155, 126, 177, 143), Rect::xyxy(155, 180, 177, 197), Rect::xyxy(233, 145, 255, 162)];
    /// Cyborg menu: stats box hit rects (x1,x2,y1,y2) index t = col*3+row
    pub const CYB_SLX: [[i32; 4]; 9] = [
        [54, 108, 35, 67], [54, 108, 72, 103], [54, 108, 109, 140], [149, 203, 35, 67], [149, 203, 72, 103], [149, 203, 109, 140],
        [245, 300, 35, 67], [245, 300, 72, 103], [245, 300, 109, 140],
    ];
    pub const CYB_PLX: [[i32; 4]; 9] = [
        [18, 52, 35, 67], [18, 52, 72, 103], [18, 52, 109, 140], [114, 147, 35, 67], [114, 147, 72, 103], [114, 147, 109, 140],
        [210, 243, 35, 67], [210, 243, 72, 103], [210, 243, 109, 140],
    ];
    pub const CYB_OK: Rect = Rect::xyxy(241, 11, 304, 29);
    pub const CYB_CLEAR: Rect = Rect::xyxy(12, 11, 75, 29);
    /// Portraits of the 9 cyborgs in BUTTON10 — CYBOR `_CY`: (dest x, dest y, x1, y1, x2, y2).
    pub const CYB_PORTRAITS: [[i32; 6]; 9] = [
        [19, 44, 0, 180, 29, 197], [24, 78, 31, 178, 51, 198], [22, 118, 127, 184, 152, 199], [119, 43, 80, 180, 103, 198],
        [120, 78, 105, 180, 125, 199], [118, 119, 54, 187, 78, 199], [221, 48, 220, 176, 231, 183], [220, 83, 55, 174, 67, 185],
        [219, 121, 122, 158, 136, 167],
    ];
}

pub fn pt_in(r: Rect, x: i32, y: i32) -> bool {
    r.contains(x, y)
}

impl Sheets {
    pub fn blit_button(&self, fb: &mut Surface, src: Rect, dx: i32, dy: i32, transparent: bool) {
        if transparent {
            fb.blit_key(&self.button, src, dx, dy, 0);
        } else {
            fb.blit(&self.button, src, dx, dy);
        }
    }
    /// Blank dialog: the panel frame from the button sheet with its interior cleared to panel grey.
    pub fn dialog(&self, fb: &mut Surface) {
        fb.blit(&self.button, art::DIALOG, art::DIALOG_DEST.0, art::DIALOG_DEST.1);
        fb.fill_rect(art::DIALOG_INNER, art::PANEL_GREY);
    }
    /// Clear the message box and print up to 4 lines (small yellow font).
    pub fn message(&self, fb: &mut Surface, lines: &[&str]) {
        fb.blit(&self.button, art::MSGBOX_CLEAR, art::MSGBOX_DEST.0, art::MSGBOX_DEST.1);
        for (i, l) in lines.iter().take(4).enumerate() {
            self.fonts.draw(fb, Font::SmallYellow, art::MSG_X, art::MSG_Y[i], l);
        }
    }
    pub fn text(&self, fb: &mut Surface, font: Font, x: i32, y: i32, s: &str) -> i32 {
        self.fonts.draw(fb, font, x, y, s)
    }
    pub fn text_c(&self, fb: &mut Surface, font: Font, cx: i32, y: i32, s: &str) {
        self.fonts.draw_centered(fb, font, cx, y, s)
    }
    pub fn flag_icon(&self, fb: &mut Surface, p: u8, x: i32, y: i32) {
        fb.blit_key(&self.button, art::flag(p), x, y, 0);
    }
}

/// Format a number with thousands separated by spaces like the original ("1 200 000").
pub fn fmt_money(v: i64) -> String {
    let neg = v < 0;
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    if neg {
        format!("-{}", out)
    } else {
        out
    }
}

/// Simple word wrap for the message box (max `cols` characters per line).
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if cur.is_empty() {
            cur = w.to_string();
        } else if cur.len() + 1 + w.len() <= cols {
            cur.push(' ');
            cur.push_str(w);
        } else {
            lines.push(cur);
            cur = w.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
