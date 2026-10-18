//! Bitmap text rendering using the glyph strips in the original button sheets.
//! Decoded from CYBOR.PRG _PRINTN: small 4x5 fonts (yellow row y=69, red row y=74,
//! digits at y=168/173), the 8x9 white capital font in DOOR3.GIF (y=191), and the
//! 5x6 orange font (digits y=56, letters y=63) in BUTTON10.GIF.

use crate::gfx::{Rect, Surface};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    /// 4x5 yellow (BUTTON10)
    SmallYellow,
    /// 4x5 red (BUTTON10)
    SmallRed,
    /// 8x9 white capitals (DOOR3 bottom strip)
    Big,
    /// 5x6 orange (BUTTON10)
    Orange,
}

pub struct Fonts {
    pub button: Surface,
    pub door: Surface,
}

impl Fonts {
    /// Width in pixels of `text` rendered in `font`.
    pub fn width(&self, font: Font, text: &str) -> i32 {
        let mut w = 0;
        for c in text.bytes() {
            w += advance(font, c);
        }
        w
    }

    /// Draw `text` at (x,y). Returns the x after the last glyph.
    pub fn draw(&self, dst: &mut Surface, font: Font, x: i32, y: i32, text: &str) -> i32 {
        let mut cx = x;
        for c in text.bytes() {
            let c = c.to_ascii_uppercase();
            if let Some((src, r)) = self.glyph(font, c) {
                dst.blit_key(src, r, cx, y, 0);
            }
            cx += advance(font, c);
        }
        cx
    }

    pub fn draw_centered(&self, dst: &mut Surface, font: Font, cx: i32, y: i32, text: &str) {
        let w = self.width(font, text);
        self.draw(dst, font, cx - w / 2, y, text);
    }

    pub fn draw_right(&self, dst: &mut Surface, font: Font, rx: i32, y: i32, text: &str) {
        let w = self.width(font, text);
        self.draw(dst, font, rx - w, y, text);
    }

    fn glyph(&self, font: Font, c: u8) -> Option<(&Surface, Rect)> {
        match font {
            Font::SmallYellow | Font::SmallRed => {
                let dy = if font == Font::SmallRed { 5 } else { 0 };
                let r = match c {
                    b':' => Rect::xyxy(0x98, 0x45 + dy, 0x9B, 0x49 + dy),
                    b'-' => Rect::xyxy(0x9C, 0x45 + dy, 0xA0, 0x49 + dy),
                    b'.' => Rect::xyxy(0xA0, 0x45 + dy, 0xA2, 0x49 + dy),
                    b'0' => Rect::xyxy(0x73, 0xA8 + dy, 0x75, 0xAC + dy),
                    b'A'..=b'Z' => {
                        let sx = (c - b'A') as i32 * 4 + 0x30;
                        Rect::xyxy(sx, 0x45 + dy, sx + 3, 0x49 + dy)
                    }
                    b'1'..=b'9' => {
                        let sx = (c - b'1') as i32 * 4 + 0x4F;
                        Rect::xyxy(sx, 0xA8 + dy, sx + 3, 0xAC + dy)
                    }
                    _ => return None,
                };
                Some((&self.button, r))
            }
            Font::Big => match c {
                b'A'..=b'Z' => {
                    let sx = (c - b'A') as i32 * 8;
                    Some((&self.door, Rect::xyxy(sx, 0xBF, sx + 7, 0xC7)))
                }
                _ => None,
            },
            Font::Orange => {
                let r = match c {
                    b':' => Rect::xyxy(0x57, 0x38, 0x5B, 0x3D),
                    b'-' => Rect::xyxy(0x5C, 0x38, 0x61, 0x3D),
                    b'0'..=b'9' => {
                        let sx = (c - b'0') as i32 * 5 + 0x25;
                        Rect::xyxy(sx, 0x38, sx + 4, 0x3D)
                    }
                    b'A'..=b'Z' => {
                        let sx = (c - b'A') as i32 * 5;
                        Rect::xyxy(sx, 0x3F, sx + 4, 0x44)
                    }
                    _ => return None,
                };
                Some((&self.button, r))
            }
        }
    }
}

pub fn advance(font: Font, c: u8) -> i32 {
    match font {
        Font::SmallYellow | Font::SmallRed => match c {
            b'.' | b' ' => 2,
            _ => 4,
        },
        Font::Big => 8,
        Font::Orange => match c {
            b' ' => 4,
            _ => 7,
        },
    }
}

pub fn line_height(font: Font) -> i32 {
    match font {
        Font::SmallYellow | Font::SmallRed => 7,
        Font::Big => 10,
        Font::Orange => 8,
    }
}
