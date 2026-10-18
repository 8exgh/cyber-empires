//! Battle summary screen (`_BATTLE_SUMMARY`) and small full-screen notices.

use super::Sheets;
use crate::data::{CYBORG_NAMES, NO_CYBORG};
use crate::gfx::{Rect, Surface};
use crate::strategy::resolve::Battle;
use crate::strategy::state::Game;
use crate::text::Font;

pub const OK_HIT: Rect = Rect::xyxy(0xF2, 0x2A, 0x12B, 0x37);

pub fn draw_battle_summary(fb: &mut Surface, sheets: &Sheets, g: &Game, b: &Battle) {
    fb.clear(0);
    // frame from DOOR3 bottom panel art and BUTTON10 dialog
    fb.blit(&sheets.door, Rect::new(0, 0, 320, 152), 0, 0);
    fb.fill_rect(Rect::new(12, 12, 296, 180), 0);
    fb.rect_outline(Rect::new(12, 12, 296, 180), 7);
    let f = &sheets.fonts;
    f.draw_centered(fb, Font::Orange, 160, 16, "BATTLE SUMMARY");
    let ao = b.att_owner;
    let d_o = b.def_owner;
    let aname = if ao < 5 { g.players[ao as usize].name.clone() } else { "NEUTRAL".into() };
    let dname = if d_o < 5 { g.players[d_o as usize].name.clone() } else { "NEUTRAL".into() };
    if ao < 5 {
        sheets.flag_icon(fb, ao, 20, 26);
    }
    if d_o < 5 {
        sheets.flag_icon(fb, d_o, 240, 26);
    }
    f.draw(fb, Font::SmallYellow, 40, 28, "ATTACKER");
    f.draw(fb, Font::SmallRed, 40, 35, &aname);
    f.draw(fb, Font::SmallYellow, 160, 28, "DEFENDER");
    f.draw(fb, Font::SmallRed, 160, 35, &dname);
    f.draw(fb, Font::SmallYellow, 0x1D, 48, "CASUALTIES");
    f.draw(fb, Font::SmallYellow, 0xFC, 48, "CASUALTIES");
    let mut ya = 56;
    let mut yd = 56;
    for i in 0..20 {
        if b.hits[i] < 0 && b.temp[i] != NO_CYBORG {
            let name = CYBORG_NAMES[b.temp[i] as usize];
            if i < 10 {
                f.draw(fb, Font::SmallRed, 0x1D, ya, name);
                ya += 6;
            } else {
                f.draw(fb, Font::SmallRed, 0xFC, yd, name);
                yd += 6;
            }
        }
    }
    let line = match b.victor {
        2 => {
            if b.defender_retreated {
                format!("{} HAS SPINELESSLY RETREATED", dname)
            } else {
                format!("{}{}{}", aname, b.summary_verb(), dname)
            }
        }
        1 => {
            if b.attacker_retreated {
                format!("{} HAS SPINELESSLY RETREATED", aname)
            } else {
                format!("{}{}{}", dname, b.summary_verb(), aname)
            }
        }
        _ => "OPPOSING FORCES WERE BOTH DESTROYED".to_string(),
    };
    f.draw_centered(fb, Font::SmallYellow, 160, 128, &line);
    let cty = g.country(b.defend_cty);
    let mut y = 138;
    if b.victor == 1 {
        let msg = if b.bdlevel == 30 {
            vec!["CAPITAL HAS BEEN DESTROYED.............".to_string(), "ALL STRUCTURES CEASE TO EXIST".to_string()]
        } else if b.bdlevel > 1 {
            vec![format!("{} STRUCTURES HAVE BEEN DESTROYED", b.bdlevel)]
        } else if b.bdlevel == 1 {
            vec!["1 STRUCTURE HAS BEEN DESTROYED".to_string()]
        } else if !cty.is_city() {
            vec!["NO STRUCTURES HAVE BEEN DESTROYED".to_string()]
        } else {
            vec![]
        };
        for m in msg {
            f.draw_centered(fb, Font::SmallYellow, 160, y, &m);
            y += 7;
        }
    }
    if cty.is_city() {
        let delta = (cty.damage - b.old_city_damage).max(0);
        f.draw_centered(fb, Font::SmallYellow, 160, y, &format!("CONFLICT HAS DAMAGED CITY {} PERCENT", delta));
        f.draw_centered(fb, Font::SmallYellow, 160, y + 7, &format!("CITY DAMAGE CURRENTLY STANDS AT {}", cty.damage));
    }
    fb.fill_rect(OK_HIT, 5);
    fb.rect_outline(OK_HIT, 7);
    f.draw_centered(fb, Font::SmallYellow, (OK_HIT.x + OK_HIT.right()) / 2, OK_HIT.y + 4, "OK");
}

/// "X HAS BEEN ANNIHILATED" intelligence report.
pub fn draw_annihilated(fb: &mut Surface, sheets: &Sheets, g: &Game, p: u8) {
    sheets.dialog(fb);
    let f = &sheets.fonts;
    f.draw(fb, Font::SmallYellow, 100, 30, "INTELLIGENCE");
    f.draw(fb, Font::SmallYellow, 100, 37, "      REPORT");
    sheets.flag_icon(fb, p, 100, 60);
    f.draw(fb, Font::SmallRed, 120, 62, &g.players[p as usize].name);
    f.draw(fb, Font::SmallYellow, 100, 80, "HAS BEEN ANNIHILATED");
    draw_ok(fb, sheets);
}

pub fn draw_ok(fb: &mut Surface, sheets: &Sheets) {
    let r = super::art::OK_HIT;
    fb.fill_rect(r, 5);
    fb.rect_outline(r, 7);
    sheets.fonts.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 7, "OK");
}
