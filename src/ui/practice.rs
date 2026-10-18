//! Battle practice: Showdown and Survivor (CYBOR `_DO_ACTION`, `_DO_SHOWDOWN`, `_CHOOSE_CYBORG`).

use super::art::OK_HIT;
use super::summary::OK_HIT as SUMMARY_OK;
use super::Sheets;
use crate::assets::Assets;
use crate::battle::screen::BattleScreen;
use crate::battle::setup::compose_practice;
use crate::battle::tables::SDCTRY;
use crate::data::*;
use crate::gfx::{Rect, Surface};
use crate::input::{key, Input};
use crate::rng::Rng;
use crate::text::Font;

/// Hall of Cybernetic Legends: per cyborg class (name, score, level).
#[derive(Clone, Debug, Default)]
pub struct Scores {
    pub entries: [(String, i64, i32); 9],
}

impl Scores {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for e in &self.entries {
            out.extend_from_slice(format!("{}\t{}\t{}\n", e.0, e.1, e.2).as_bytes());
        }
        out
    }
    pub fn from_bytes(b: &[u8]) -> Scores {
        let mut s = Scores::default();
        for (i, line) in String::from_utf8_lossy(b).lines().enumerate().take(9) {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() == 3 {
                s.entries[i] = (parts[0].to_string(), parts[1].parse().unwrap_or(0), parts[2].parse().unwrap_or(0));
            }
        }
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayersChoice {
    HumanHuman,
    HumanComputer,
    ComputerHuman,
    Demo,
}

impl PlayersChoice {
    fn next(self) -> PlayersChoice {
        match self {
            PlayersChoice::HumanHuman => PlayersChoice::HumanComputer,
            PlayersChoice::HumanComputer => PlayersChoice::ComputerHuman,
            PlayersChoice::ComputerHuman => PlayersChoice::Demo,
            PlayersChoice::Demo => PlayersChoice::HumanHuman,
        }
    }
    fn prev(self) -> PlayersChoice {
        self.next().next().next()
    }
    fn label(self) -> &'static str {
        match self {
            PlayersChoice::HumanHuman => "HUMAN VS. HUMAN",
            PlayersChoice::HumanComputer => "HUMAN VS. COMPUTER",
            PlayersChoice::ComputerHuman => "COMPUTER VS. HUMAN",
            PlayersChoice::Demo => "DEMO",
        }
    }
    fn human(self) -> [bool; 2] {
        match self {
            PlayersChoice::HumanHuman => [true, true],
            PlayersChoice::HumanComputer => [true, false],
            PlayersChoice::ComputerHuman => [false, true],
            PlayersChoice::Demo => [false, false],
        }
    }
}

pub enum Stage {
    Menu,
    ShowdownPlayers { choice: PlayersChoice },
    ShowdownTerrain { choice: PlayersChoice, terrain: usize },
    ShowdownMoney { choice: PlayersChoice, terrain: usize, money: i64 },
    /// shop for player `who` (1 = defender first, then 2 = attacker)
    Shop { choice: PlayersChoice, terrain: usize, money: i64, who: usize, bought: Vec<usize>, spent: i64, def: Vec<usize>, msg: String },
    SurvivorChoose { class: usize },
    Fight { screen: Box<BattleScreen>, survivor: Option<SurvivorState>, att: Vec<(u8, i32)>, def: Vec<(u8, i32)> },
    Summary { lines: Vec<String>, next: Box<Stage> },
    EnterName { class: usize, score: i64, level: i32, name: String },
    SurvivorNext(SurvivorState),
}

#[derive(Clone, Debug)]
pub struct SurvivorState {
    pub class: usize,
    pub level: i32,
    pub upbound: i32,
    pub lowbound: i32,
    pub killtons: i64,
}

pub struct PracticeScreen {
    pub stage: Stage,
    pub scores: Scores,
    pub scores_dirty: bool,
    pub exit: bool,
    pub names: [String; 2],
}

const SHOWDOWN_BTN: Rect = Rect::xyxy(0x1D, 0x30, 0x44, 0x56);
const SURVIVOR_BTN: Rect = Rect::xyxy(0x1D, 0x5E, 0x44, 0x84);
const EXIT_BTN: Rect = Rect::xyxy(0xF2, 5, 0x12B, 0x12);
const MINUS: Rect = Rect::xyxy(0x73, 0x6E, 0x8A, 0x7C);
const PLUS: Rect = Rect::xyxy(0xB4, 0x6E, 0xCB, 0x7C);
const MINUS2: Rect = Rect::xyxy(0x73, 0x5E, 0x8A, 0x6C);
const PLUS2: Rect = Rect::xyxy(0xB4, 0x5E, 0xCB, 0x6C);
const MINUS3: Rect = Rect::xyxy(0x73, 0x4E, 0x8A, 0x5C);
const PLUS3: Rect = Rect::xyxy(0xB4, 0x4E, 0xCB, 0x5C);

impl PracticeScreen {
    pub fn new(scores: Scores) -> PracticeScreen {
        PracticeScreen { stage: Stage::Menu, scores, scores_dirty: false, exit: false, names: ["WAR HAMMER".into(), "DESTROYER".into()] }
    }

    fn computer_buy(rng: &mut Rng, money: i64) -> Vec<usize> {
        let mut vault = money;
        let mut out = Vec::new();
        let k = rng.range(0, 9);
        let try_buy = |t: usize, vault: &mut i64, out: &mut Vec<usize>| {
            for s in t..9 {
                if *vault >= UNIT_COST[s] as i64 {
                    out.push(s);
                    *vault -= UNIT_COST[s] as i64;
                    return;
                }
            }
        };
        for _ in 0..k {
            let t = rng.range(0, 8) as usize;
            try_buy(t, &mut vault, &mut out);
        }
        while out.len() < 10 {
            let before = out.len();
            try_buy(0, &mut vault, &mut out);
            if out.len() == before {
                break;
            }
        }
        out.truncate(10);
        out
    }

    fn start_showdown(&mut self, assets: &mut Assets, rng: &mut Rng, choice: PlayersChoice, terrain: usize, att: Vec<usize>, def: Vec<usize>) {
        let country = SDCTRY[terrain.min(4)] as u8;
        let attv: Vec<(u8, i32)> = att.iter().map(|&t| (t as u8, 100)).collect();
        let defv: Vec<(u8, i32)> = def.iter().map(|&t| (t as u8, 100)).collect();
        let pc = [rng.range(1, 4) as u8, 0u8];
        match compose_practice(assets, terrain as u8 + 1, country, &attv, &defv, choice.human(), pc) {
            Some(init) => match BattleScreen::new(assets, &init) {
                Some(screen) => self.stage = Stage::Fight { screen: Box::new(screen), survivor: None, att: attv, def: defv },
                None => self.stage = Stage::Menu,
            },
            None => self.stage = Stage::Menu,
        }
    }

    fn start_survivor_level(&mut self, assets: &mut Assets, rng: &mut Rng, st: SurvivorState) {
        let mut st = st;
        if st.level > 10 && st.upbound > 0 {
            st.upbound -= 1;
        } else if st.level < 10 && st.lowbound > 0 {
            st.lowbound -= 1;
        }
        let n = st.level.min(10).max(1) as usize;
        let mut att: Vec<(u8, i32)> = Vec::new();
        for _ in 0..n {
            let mut t = rng.range(st.lowbound, st.upbound) as u8;
            if t == 4 {
                t = 5;
            }
            att.push((t, 100));
        }
        let def = vec![(st.class as u8, 100)];
        let country = rng.range(1, 72) as u8;
        let terrain = crate::world_data::COUNTRIES[country as usize].terrain;
        match compose_practice(assets, terrain, country, &att, &def, [false, true], [1, 0]) {
            Some(init) => match BattleScreen::new(assets, &init) {
                Some(screen) => self.stage = Stage::Fight { screen: Box::new(screen), survivor: Some(st), att, def },
                None => self.stage = Stage::Menu,
            },
            None => self.stage = Stage::Menu,
        }
    }

    pub fn update(&mut self, input: &Input, dt: f32, assets: &mut Assets, rng: &mut Rng) {
        let stage = std::mem::replace(&mut self.stage, Stage::Menu);
        let (mx, my) = (input.mouse_x, input.mouse_y);
        let clicked = input.clicked();
        self.stage = match stage {
            Stage::Menu => {
                if clicked {
                    if SHOWDOWN_BTN.contains(mx, my) {
                        Stage::ShowdownPlayers { choice: PlayersChoice::HumanComputer }
                    } else if SURVIVOR_BTN.contains(mx, my) {
                        Stage::SurvivorChoose { class: 0 }
                    } else if EXIT_BTN.contains(mx, my) {
                        self.exit = true;
                        Stage::Menu
                    } else {
                        Stage::Menu
                    }
                } else if input.pressed(key::ESC) {
                    self.exit = true;
                    Stage::Menu
                } else {
                    Stage::Menu
                }
            }
            Stage::ShowdownPlayers { mut choice } => {
                if clicked {
                    if MINUS.contains(mx, my) {
                        choice = choice.prev();
                    } else if PLUS.contains(mx, my) {
                        choice = choice.next();
                    } else if OK_HIT.contains(mx, my) {
                        return self.stage = Stage::ShowdownTerrain { choice, terrain: 0 };
                    }
                }
                Stage::ShowdownPlayers { choice }
            }
            Stage::ShowdownTerrain { choice, mut terrain } => {
                if clicked {
                    if MINUS.contains(mx, my) {
                        terrain = (terrain + 4) % 5;
                    } else if PLUS.contains(mx, my) {
                        terrain = (terrain + 1) % 5;
                    } else if OK_HIT.contains(mx, my) {
                        if choice == PlayersChoice::Demo {
                            let money = rng.range(5, 100) as i64 * rng.range(10, 100) as i64 * rng.range(10, 100) as i64;
                            let t = rng.range(0, 4) as usize;
                            let def = Self::computer_buy(rng, money);
                            let att = Self::computer_buy(rng, money);
                            self.start_showdown(assets, rng, choice, t, att, def);
                            return;
                        }
                        return self.stage = Stage::ShowdownMoney { choice, terrain, money: 2500 };
                    }
                }
                Stage::ShowdownTerrain { choice, terrain }
            }
            Stage::ShowdownMoney { choice, terrain, mut money } => {
                if clicked {
                    let d = if MINUS.contains(mx, my) {
                        -500
                    } else if PLUS.contains(mx, my) {
                        500
                    } else if MINUS2.contains(mx, my) {
                        -10000
                    } else if PLUS2.contains(mx, my) {
                        10000
                    } else if MINUS3.contains(mx, my) {
                        -100000
                    } else if PLUS3.contains(mx, my) {
                        100000
                    } else {
                        0
                    };
                    money = (money + d).clamp(2500, 1_000_000);
                    if OK_HIT.contains(mx, my) {
                        // defender (player 1) shops first
                        let human = choice.human();
                        if !human[1] {
                            let def = Self::computer_buy(rng, money);
                            if !human[0] {
                                let att = Self::computer_buy(rng, money);
                                self.start_showdown(assets, rng, choice, terrain, att, def);
                                return;
                            }
                            return self.stage = Stage::Shop { choice, terrain, money, who: 2, bought: Vec::new(), spent: 0, def, msg: String::new() };
                        }
                        return self.stage = Stage::Shop { choice, terrain, money, who: 1, bought: Vec::new(), spent: 0, def: Vec::new(), msg: String::new() };
                    }
                }
                Stage::ShowdownMoney { choice, terrain, money }
            }
            Stage::Shop { choice, terrain, money, who, mut bought, mut spent, def, mut msg } => {
                if clicked {
                    if super::art::CYB_OK.contains(mx, my) && !bought.is_empty() {
                        if who == 1 {
                            let human = choice.human();
                            if human[0] {
                                return self.stage = Stage::Shop { choice, terrain, money, who: 2, bought: Vec::new(), spent: 0, def: bought, msg: String::new() };
                            }
                            let att = Self::computer_buy(rng, money);
                            self.start_showdown(assets, rng, choice, terrain, att, bought);
                            return;
                        }
                        self.start_showdown(assets, rng, choice, terrain, bought, def);
                        return;
                    }
                    if super::art::CYB_CLEAR.contains(mx, my) {
                        bought.clear();
                        spent = 0;
                        msg.clear();
                    }
                    for t in 0..9 {
                        let s = super::art::CYB_SLX[t];
                        if mx >= s[0] && mx <= s[1] && my >= s[2] && my <= s[3] {
                            let cost = UNIT_COST[t] as i64;
                            if money - spent - cost < 0 {
                                msg = "YOU CANNOT AFFORD THIS CYBORG.".into();
                            } else if bought.len() >= 10 {
                                msg = "ALL FACTORY BAYS UTILIZED.".into();
                            } else {
                                bought.push(t);
                                spent += cost;
                                msg.clear();
                            }
                        }
                    }
                }
                Stage::Shop { choice, terrain, money, who, bought, spent, def, msg }
            }
            Stage::SurvivorChoose { mut class } => {
                if clicked {
                    if MINUS.contains(mx, my) {
                        class = (class + 8) % 9;
                    } else if PLUS.contains(mx, my) {
                        class = (class + 1) % 9;
                    } else if OK_HIT.contains(mx, my) {
                        let st = SurvivorState { class, level: 1, upbound: 8, lowbound: 8, killtons: 0 };
                        self.start_survivor_level(assets, rng, st);
                        return;
                    }
                }
                Stage::SurvivorChoose { class }
            }
            Stage::Fight { mut screen, survivor, att, def } => {
                screen.update(input, dt);
                if screen.finished() {
                    let (res, aret, dret, health) = screen.sim.result();
                    let mut lines = Vec::new();
                    lines.push("BATTLE SUMMARY".to_string());
                    let a_lost: Vec<String> = (0..att.len()).filter(|&i| health[i] > 100).map(|i| CYBORG_NAMES[att[i].0 as usize].to_string()).collect();
                    let d_lost: Vec<String> = (0..def.len()).filter(|&i| health[10 + i] > 100).map(|i| CYBORG_NAMES[def[i].0 as usize].to_string()).collect();
                    lines.push(format!("ATTACKER CASUALTIES: {}", if a_lost.is_empty() { "NONE".to_string() } else { a_lost.join(", ") }));
                    lines.push(format!("DEFENDER CASUALTIES: {}", if d_lost.is_empty() { "NONE".to_string() } else { d_lost.join(", ") }));
                    match res {
                        2 => lines.push(format!("{} HAS DEFEATED {}", self.names[0], self.names[1])),
                        1 => lines.push(format!("{} HAS DEFEATED {}", self.names[1], self.names[0])),
                        _ => lines.push("OPPOSING FORCES WERE BOTH DESTROYED".into()),
                    }
                    if aret {
                        lines.push(format!("{} RETREATED", self.names[0]));
                    }
                    if dret {
                        lines.push(format!("{} RETREATED", self.names[1]));
                    }
                    if let Some(mut st) = survivor {
                        for i in 0..att.len() {
                            if health[i] > 100 {
                                st.killtons += ARMOR[att[i].0 as usize] as i64;
                            }
                        }
                        lines.push(format!("LEVEL {}   SCORE {}", st.level, st.killtons));
                        if res == 1 {
                            st.level += 1;
                            let next = Stage::SurvivorNext(st);
                            return self.stage = Stage::Summary { lines, next: Box::new(next) };
                        }
                        let next = if self.scores.entries[st.class].1 < st.killtons {
                            Stage::EnterName { class: st.class, score: st.killtons, level: st.level, name: String::new() }
                        } else {
                            Stage::Menu
                        };
                        return self.stage = Stage::Summary { lines, next: Box::new(next) };
                    }
                    return self.stage = Stage::Summary { lines, next: Box::new(Stage::Menu) };
                }
                Stage::Fight { screen, survivor, att, def }
            }
            Stage::Summary { lines, next } => {
                if clicked || input.pressed(key::ENTER) {
                    match *next {
                        Stage::SurvivorNext(st) => {
                            self.start_survivor_level(assets, rng, st);
                            return;
                        }
                        other => other,
                    }
                } else {
                    Stage::Summary { lines, next }
                }
            }
            Stage::SurvivorNext(st) => {
                self.start_survivor_level(assets, rng, st);
                return;
            }
            Stage::EnterName { class, score, level, mut name } => {
                for &c in &input.typed {
                    let c = c.to_ascii_uppercase();
                    if c == 8 {
                        name.pop();
                    } else if (c.is_ascii_uppercase() || c == b'-' || c == b' ') && name.len() < 11 {
                        name.push(c as char);
                    }
                }
                if input.pressed(key::ENTER) && !name.trim().is_empty() {
                    self.scores.entries[class] = (name.clone(), score, level - 1);
                    self.scores_dirty = true;
                    Stage::Menu
                } else {
                    Stage::EnterName { class, score, level, name }
                }
            }
        };
    }

    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets) -> Option<crate::gfx::Palette> {
        let f = &sheets.fonts;
        match &self.stage {
            Stage::Fight { screen, .. } => {
                screen.draw(fb);
                return Some(screen.palette());
            }
            _ => {}
        }
        fb.clear(0);
        match &self.stage {
            Stage::Menu => {
                fb.blit(&sheets.door, Rect::new(0, 0, 320, 152), 0, 0);
                fb.fill_rect(Rect::new(8, 2, 304, 196), 0);
                fb.rect_outline(Rect::new(8, 2, 304, 196), 7);
                f.draw(fb, Font::Orange, 20, 10, "SELECT A BATTLE PRACTICE SCENARIO");
                for (r, label) in [(SHOWDOWN_BTN, "SHOW\nDOWN"), (SURVIVOR_BTN, "SURV\nIVOR")] {
                    fb.fill_rect(r, 54);
                    fb.rect_outline(r, 7);
                    let mut y = r.y + 10;
                    for l in label.split('\n') {
                        f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, y, l);
                        y += 7;
                    }
                }
                f.draw(fb, Font::SmallYellow, 0x50, 0x38, "SHOWDOWN: TWO SIDES FIGHT TO THE DEATH.");
                f.draw(fb, Font::SmallYellow, 0x50, 0x3F, "1 OR 2 PLAYERS.");
                f.draw(fb, Font::SmallYellow, 0x50, 0x66, "SURVIVOR: SURVIVE AS LONG AS POSSIBLE");
                f.draw(fb, Font::SmallYellow, 0x50, 0x6D, "AGAINST A SUPERIOR ENEMY. 1 PLAYER ONLY.");
                f.draw(fb, Font::Orange, 0x8C, 0x7C, "CYBERNETIC LEGENDS");
                f.draw(fb, Font::SmallRed, 0x50, 0x8A, "CYBORG");
                f.draw(fb, Font::SmallRed, 0xCF, 0x8A, "GENERAL");
                f.draw(fb, Font::SmallRed, 0xFA, 0x8A, "TONNAGE");
                f.draw(fb, Font::SmallRed, 0x11E, 0x8A, "LEVEL");
                for i in 0..9 {
                    let y = 0x92 + 6 * i as i32;
                    let e = &self.scores.entries[i];
                    f.draw(fb, Font::SmallYellow, 0x50, y, CYBORG_NAMES[i]);
                    f.draw(fb, Font::SmallYellow, 0xCF, y, &e.0);
                    f.draw(fb, Font::SmallYellow, 0xFA, y, &format!("{}", e.1));
                    f.draw(fb, Font::SmallYellow, 0x11E, y, &format!("{}", e.2));
                }
                fb.fill_rect(EXIT_BTN, 54);
                fb.rect_outline(EXIT_BTN, 7);
                f.draw_centered(fb, Font::SmallYellow, (EXIT_BTN.x + EXIT_BTN.right()) / 2, EXIT_BTN.y + 4, "OK");
            }
            Stage::ShowdownPlayers { choice } => {
                self.dialog_with_arrows(fb, sheets, "PLAYER OPTIONS", &["SELECT HUMAN OR", "COMPUTER PLAYERS"], "PLAYERS:", choice.label(), false);
            }
            Stage::ShowdownTerrain { terrain, .. } => {
                self.dialog_with_arrows(fb, sheets, "TERRAIN SELECTION", &["SELECT TERRAIN FOR CONTEST."], "TERRAIN:", TERRAIN_NAMES[*terrain + 1], false);
            }
            Stage::ShowdownMoney { money, .. } => {
                self.dialog_with_arrows(fb, sheets, "AMOUNT ALLOCATED", &["ENTER THE AMOUNT MONEY TO", "ALLOCATE FOR EACH PLAYER."], "AMOUNT:", &format!("{}", money), true);
            }
            Stage::SurvivorChoose { class } => {
                self.dialog_with_arrows(fb, sheets, "CYBORG SELECTION", &["SELECT CYBORG FOR SURVIVAL"], "CYBORG:", CYBORG_NAMES[*class], false);
                let cp = super::art::CYB_PORTRAITS[*class];
                fb.blit_key(&sheets.button, Rect::xyxy(cp[2], cp[3], cp[4], cp[5]), 0x40, 0x8C, 0);
            }
            Stage::Shop { money, who, bought, spent, msg, .. } => {
                self.draw_shop(fb, sheets, *money, *who, bought, *spent, msg);
            }
            Stage::Summary { lines, .. } => {
                fb.blit(&sheets.door, Rect::new(0, 0, 320, 152), 0, 0);
                fb.fill_rect(Rect::new(12, 12, 296, 176), 0);
                fb.rect_outline(Rect::new(12, 12, 296, 176), 7);
                let mut y = 20;
                for (i, l) in lines.iter().enumerate() {
                    let font = if i == 0 { Font::Orange } else { Font::SmallYellow };
                    f.draw(fb, font, 20, y, l);
                    y += if i == 0 { 14 } else { 8 };
                }
                fb.fill_rect(SUMMARY_OK, 54);
                fb.rect_outline(SUMMARY_OK, 7);
                f.draw_centered(fb, Font::SmallYellow, (SUMMARY_OK.x + SUMMARY_OK.right()) / 2, SUMMARY_OK.y + 4, "OK");
            }
            Stage::EnterName { name, .. } => {
                sheets.dialog(fb);
                f.draw(fb, Font::SmallYellow, 0x5C, 0x28, "CONGRATULATIONS  YOU HAVE ENTERED");
                f.draw(fb, Font::SmallYellow, 0x5C, 0x30, "THE HALL OF CYBERNETIC LEGENDS...");
                f.draw(fb, Font::SmallYellow, 0x5C, 0x40, "ENTER YOUR NAME");
                f.draw(fb, Font::SmallRed, 0x5C, 0x4C, &format!("{}_", name));
            }
            Stage::Fight { .. } | Stage::SurvivorNext(_) => {}
        }
        None
    }

    fn dialog_with_arrows(&self, fb: &mut Surface, sheets: &Sheets, title: &str, text: &[&str], label: &str, value: &str, money: bool) {
        let f = &sheets.fonts;
        sheets.dialog(fb);
        f.draw(fb, Font::SmallYellow, 0x5C, 0x1C, title);
        let mut y = 0x2A;
        for t in text {
            f.draw(fb, Font::SmallYellow, 0x5C, y, t);
            y += 7;
        }
        f.draw(fb, Font::SmallYellow, 0x5C, 0x60, label);
        f.draw(fb, Font::SmallRed, 0x5C + 40, 0x60, value);
        let mut buttons = vec![(MINUS, "-"), (PLUS, "+")];
        if money {
            buttons.push((MINUS2, "--"));
            buttons.push((PLUS2, "++"));
            buttons.push((MINUS3, "---"));
            buttons.push((PLUS3, "+++"));
        }
        for (r, l) in buttons {
            fb.fill_rect(r, 54);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 5, l);
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_shop(&self, fb: &mut Surface, sheets: &Sheets, money: i64, who: usize, bought: &[usize], spent: i64, msg: &str) {
        let f = &sheets.fonts;
        for t in 0..9 {
            let s = super::art::CYB_SLX[t];
            let pr = super::art::CYB_PLX[t];
            fb.rect_outline(Rect::xyxy(pr[0], pr[2], pr[1], pr[3]), 7);
            let cp = super::art::CYB_PORTRAITS[t];
            fb.blit_key(&sheets.button, Rect::xyxy(cp[2], cp[3], cp[4], cp[5]), cp[0], cp[1], 0);
            fb.rect_outline(Rect::xyxy(s[0], s[2], s[1], s[3]), 7);
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 3, CYBORG_NAMES[t]);
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 10, &format!("WGT: {} TONS", ARMOR[t]));
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 17, &format!("COST: {}", UNIT_COST[t]));
        }
        f.draw(fb, Font::Orange, 90, 12, &format!("PLAYER {}: SELECT YOUR", who));
        f.draw(fb, Font::SmallYellow, 90, 24, "CYBORGS TO BATTLE YOUR OPPONENT.");
        for (r, l) in [(super::art::CYB_OK, "OK"), (super::art::CYB_CLEAR, "CLEAR")] {
            fb.fill_rect(r, 5);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 7, l);
        }
        let mut counts = [0; 9];
        for &b in bought {
            counts[b] += 1;
        }
        for t in 0..9 {
            let (x, y) = if t < 5 { (14, 152 + 6 * t as i32) } else { (110, 152 + 6 * (t as i32 - 5)) };
            f.draw(fb, Font::SmallYellow, x, y, &format!("{:<10} {}", CYBORG_NAMES[t], counts[t]));
        }
        f.draw(fb, Font::SmallYellow, 200, 152, &format!("FACTORY MAX: 10"));
        f.draw(fb, Font::SmallYellow, 200, 158, &format!("FACTORY BAY: {}", 10 - bought.len()));
        f.draw(fb, Font::SmallYellow, 200, 170, &format!("VAULT: {}", money - spent));
        f.draw(fb, Font::SmallYellow, 200, 176, &format!("TOTAL COST: {}", spent));
        if !msg.is_empty() {
            f.draw(fb, Font::SmallRed, 14, 186, msg);
        }
    }
}
