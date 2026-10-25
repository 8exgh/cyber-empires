//! "Ported by" screen shown for five seconds before the original publisher logos.
//! The background and portrait come from `credit.gif` (built by scripts/make-credit.py);
//! the caption is drawn with the game's own fonts.

use crate::gfx::{Palette, Rect, Surface};
use crate::text::{Font, Fonts};

/// How long the screen stays up.
pub const DURATION: f32 = 5.0;
/// Clicks and keys only skip it after this long, so it is always seen.
pub const MIN_SHOWN: f32 = 2.0;

const IMAGE: &[u8] = include_bytes!("credit.gif");
/// credit.gif uses palette slots below this; the font colours go above it.
const FIRST_FREE_SLOT: usize = 240;

pub struct Credit {
    pub image: Surface,
    pub pal: Palette,
}

/// Compose the screen once: decode the embedded image and draw the caption with the
/// game fonts, whose pixels are indices into `font_pal` (the strategy palette).
pub fn compose(fonts: &Fonts, font_pal: &Palette) -> Credit {
    let gif = crate::gif::decode(IMAGE).expect("embedded credit.gif");
    let mut image = Surface::from_gif(&gif);
    let mut pal = gif.palette;

    let cx = image.w as i32 / 2;
    let mut next = FIRST_FREE_SLOT;
    let mut line = Surface::new(image.w, image.h);
    fonts.draw_centered(&mut line, Font::Orange, cx, 144, "PORTED BY");
    overlay(&mut image, &mut pal, &mut next, &line, font_pal, Some([185, 160, 255]));
    let mut line = Surface::new(image.w, image.h);
    fonts.draw_centered(&mut line, Font::Big, cx, 156, "SEAN BENNETT");
    overlay(&mut image, &mut pal, &mut next, &line, font_pal, None);
    Credit { image, pal }
}

/// Draw the non-zero pixels of `text` over `image`, giving each font colour its own palette
/// slot from `next` on. With `tint`, the colour keeps the font's brightness in that hue.
fn overlay(image: &mut Surface, pal: &mut Palette, next: &mut usize, text: &Surface, font_pal: &Palette, tint: Option<[u8; 3]>) {
    let mut map = [0u8; 256];
    for &c in &text.px {
        if c == 0 || map[c as usize] != 0 || *next >= 256 {
            continue;
        }
        let src = font_pal[c as usize];
        pal[*next] = match tint {
            None => src,
            Some(t) => {
                let lum = (src[0] as u32 * 30 + src[1] as u32 * 59 + src[2] as u32 * 11) / 100;
                let scale = (lum * 2).min(255);
                [(t[0] as u32 * scale / 255) as u8, (t[1] as u32 * scale / 255) as u8, (t[2] as u32 * scale / 255) as u8]
            }
        };
        map[c as usize] = *next as u8;
        *next += 1;
    }
    image.blit_key_map(text, Rect::new(0, 0, text.w as i32, text.h as i32), 0, 0, 0, &map);
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_image_leaves_font_slots_free() {
        let gif = crate::gif::decode(super::IMAGE).unwrap();
        assert_eq!((gif.width, gif.height), (320, 200));
        assert!(gif.pixels.iter().all(|&p| (p as usize) < super::FIRST_FREE_SLOT));
    }
}
