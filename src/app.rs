//! Top-level application: owns assets, framebuffer, input and the screen state machine.

use crate::assets::Assets;
use crate::gfx::{remap_table, Palette, Rect, Surface};
use crate::input::{key, Input};
use crate::rng::Rng;
use crate::strategy::resolve::simulate_battle;
use crate::strategy::state::Game;
use crate::text::{Font, Fonts};
use crate::ui::battleflow::BattleFlow;
use crate::ui::options::{NamesScreen, OptionsScreen};
use crate::ui::practice::{PracticeScreen, Scores};
use crate::ui::strategy::{Request, StrategyScreen};
use crate::ui::Sheets;

pub const SCREEN_W: usize = 320;
pub const SCREEN_H: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainChoice {
    CompleteCampaign,
    StrategyCampaign,
    BattlePractice,
}

pub enum Screen {
    Boot,
    Credit { t: f32, screen: Box<crate::ui::credit::Credit> },
    Logo { index: usize, t: f32 },
    MainMenu,
    Options(Box<OptionsScreen>),
    Names { options: Box<OptionsScreen>, names: Box<NamesScreen> },
    Strategy(Box<StrategyScreen>),
    Battle { strategy: Box<StrategyScreen>, flow: Box<BattleFlow> },
    Practice(Box<PracticeScreen>),
    Message { text: String },
}

/// Host storage interface for save games (slot 1..=5).
pub trait Storage {
    fn store(&mut self, slot: usize, data: &[u8]);
    fn fetch(&mut self, slot: usize) -> Option<Vec<u8>>;
}

pub struct MemStorage {
    pub slots: Vec<Option<Vec<u8>>>,
}
impl Storage for MemStorage {
    fn store(&mut self, slot: usize, data: &[u8]) {
        if slot < self.slots.len() {
            self.slots[slot] = Some(data.to_vec());
        }
    }
    fn fetch(&mut self, slot: usize) -> Option<Vec<u8>> {
        self.slots.get(slot).cloned().flatten()
    }
}

pub struct App {
    pub assets: Assets,
    pub fb: Surface,
    pub pal: Palette,
    pub rgba: Vec<u8>,
    pub input: Input,
    pub rng: Rng,
    pub screen: Screen,
    pub fonts: Option<Fonts>,
    pub sheets: Option<Sheets>,
    pub strategy_pal: Palette,
    pub time: f64,
    pub log: Vec<String>,
    pub cursor_visible: bool,
    pub storage: Box<dyn Storage>,
}

impl App {
    pub fn new() -> App {
        App {
            assets: Assets::new(),
            fb: Surface::new(SCREEN_W, SCREEN_H),
            pal: [[0; 3]; 256],
            rgba: vec![0; SCREEN_W * SCREEN_H * 4],
            input: Input::new(),
            rng: Rng::new(0x1234_5678),
            screen: Screen::Boot,
            fonts: None,
            sheets: None,
            strategy_pal: [[0; 3]; 256],
            time: 0.0,
            log: Vec::new(),
            cursor_visible: true,
            storage: Box::new(MemStorage { slots: vec![None; 6] }),
        }
    }

    pub fn add_asset(&mut self, name: &str, data: Vec<u8>) {
        self.assets.add(name, data);
    }

    /// Call once all assets have been registered.
    pub fn start(&mut self, seed: u32) -> Result<(), String> {
        let missing = self.assets.missing();
        if !missing.is_empty() {
            return Err(format!("missing game files: {}", missing.join(", ")));
        }
        self.rng.seed(seed);
        // strategy palette = BUTTON10's; remap the other sheets into it
        let (button, bpal, _) = self.assets.image_cloned("button10.gif").ok_or("button10.gif")?;
        self.strategy_pal = bpal;
        let door = self.remapped("door3.gif", &bpal)?;
        let frame = self.remapped("framebt3.gif", &bpal)?;
        let mut world = Surface::new(640, 400);
        for (name, dx, dy) in [("topw1b.gif", 0, 0), ("topw2b.gif", 320, 0), ("topw4b.gif", 0, 200), ("topw3b.gif", 320, 200)] {
            let s = self.remapped(name, &bpal)?;
            world.blit(&s, Rect::new(0, 0, 320, 200), dx, dy);
        }
        let fonts = Fonts { button: button.clone(), door: door.clone() };
        self.fonts = Some(Fonts { button: button.clone(), door: door.clone() });
        let (victory, victory_pal, _) = self.assets.image_cloned("openfii2.gif").ok_or("openfii2.gif")?;
        self.sheets = Some(Sheets { button, door, frame, world, fonts, victory, victory_pal });
        let credit = crate::ui::credit::compose(self.fonts.as_ref().ok_or("fonts")?, &bpal);
        self.screen = Screen::Credit { t: 0.0, screen: Box::new(credit) };
        Ok(())
    }

    fn remapped(&mut self, name: &str, pal: &Palette) -> Result<Surface, String> {
        let (mut s, spal, bits) = self.assets.image_cloned(name).ok_or_else(|| name.to_string())?;
        let t = remap_table(&spal, 1 << bits, pal, 256);
        s.remap(&t);
        Ok(s)
    }

    pub fn show_image(&mut self, name: &str) {
        if let Some((s, pal, _)) = self.assets.image_cloned(name) {
            self.fb.blit(&s, Rect::new(0, 0, s.w as i32, s.h as i32), 0, 0);
            self.pal = pal;
        }
    }

    pub fn frame(&mut self, dt: f32) {
        self.time += dt as f64;
        let screen = std::mem::replace(&mut self.screen, Screen::Boot);
        self.screen = self.update(screen, dt);
        self.present();
        self.input.end_frame();
    }

    fn update(&mut self, screen: Screen, dt: f32) -> Screen {
        match screen {
            Screen::Boot => {
                self.fb.clear(0);
                Screen::Boot
            }
            Screen::Credit { t, screen } => {
                use crate::ui::credit::{DURATION, MIN_SHOWN};
                let t2 = t + dt;
                self.fb.blit(&screen.image, Rect::new(0, 0, screen.image.w as i32, screen.image.h as i32), 0, 0);
                self.pal = screen.pal;
                let skip = t2 >= MIN_SHOWN
                    && (self.input.clicked() || self.input.pressed(key::ENTER) || self.input.pressed(key::SPACE) || self.input.pressed(key::ESC));
                if t2 >= DURATION || skip {
                    Screen::Logo { index: 0, t: 0.0 }
                } else {
                    Screen::Credit { t: t2, screen }
                }
            }
            Screen::Logo { index, t } => {
                const LOGOS: [&str; 3] = ["silknght.gif", "ssi.gif", "openfii.gif"];
                let t2 = t + dt;
                self.show_image(LOGOS[index]);
                let skip = self.input.clicked() || self.input.pressed(key::ENTER) || self.input.pressed(key::SPACE) || self.input.pressed(key::ESC);
                if t2 > 3.0 || skip {
                    if index + 1 < LOGOS.len() {
                        Screen::Logo { index: index + 1, t: 0.0 }
                    } else {
                        Screen::MainMenu
                    }
                } else {
                    Screen::Logo { index, t: t2 }
                }
            }
            Screen::MainMenu => self.main_menu(),
            Screen::Options(mut o) => {
                let slot = o.update(&self.input);
                self.pal = self.strategy_pal;
                if let Some(slot) = slot {
                    if slot > 0 {
                        if let Some(data) = self.storage.fetch(slot) {
                            if let Some(g) = Game::from_dat(&data, self.rng.rand() as u32) {
                                let complete = o.complete;
                                return Screen::Strategy(Box::new(StrategyScreen::new(g, complete)));
                            }
                        }
                    }
                }
                o.draw(&mut self.fb, self.sheets.as_ref().unwrap());
                if o.done {
                    let names = NamesScreen::new(o.types);
                    return Screen::Names { options: o, names: Box::new(names) };
                }
                Screen::Options(o)
            }
            Screen::Names { options, mut names } => {
                names.update(&self.input);
                self.pal = self.strategy_pal;
                names.draw(&mut self.fb, self.sheets.as_ref().unwrap());
                if names.done {
                    let opts = options.to_new_game(names.names.clone(), names.skills);
                    let seed = self.rng.rand() as u32 ^ ((self.time * 1000.0) as u32);
                    let g = Game::new_game(&opts, seed);
                    return Screen::Strategy(Box::new(StrategyScreen::new(g, options.complete)));
                }
                Screen::Names { options, names }
            }
            Screen::Strategy(mut s) => {
                s.update(&self.input, dt);
                self.pal = self.strategy_pal;
                let req = std::mem::replace(&mut s.request, Request::None);
                match req {
                    Request::None => {}
                    Request::Exit => return Screen::MainMenu,
                    Request::Save(slot) => {
                        let data = s.game.to_dat();
                        self.storage.store(slot, &data);
                    }
                    Request::Load(slot) => {
                        if let Some(data) = self.storage.fetch(slot) {
                            if let Some(g) = Game::from_dat(&data, self.rng.rand() as u32) {
                                let complete = s.complete;
                                return Screen::Strategy(Box::new(StrategyScreen::new(g, complete)));
                            }
                        }
                    }
                    Request::Battle(b) => {
                        let mut flow = Box::new(BattleFlow::new(&s.game, b, &mut self.rng));
                        if flow.advance(&mut s.game, &mut self.assets, &mut self.rng) {
                            if let Some(b) = flow.result.take() {
                                s.finish_battle(b);
                            }
                        } else {
                            return Screen::Battle { strategy: s, flow };
                        }
                    }
                }
                if let Some(p) = s.draw(&mut self.fb, self.sheets.as_ref().unwrap()) {
                    self.pal = p;
                }
                Screen::Strategy(s)
            }
            Screen::Battle { mut strategy, mut flow } => {
                flow.update(&self.input, dt);
                if flow.advance(&mut strategy.game, &mut self.assets, &mut self.rng) {
                    if let Some(b) = flow.result.take() {
                        strategy.finish_battle(b);
                    } else {
                        let b = simulate_battle(&mut strategy.game, flow.battle.clone());
                        strategy.finish_battle(b);
                    }
                    self.pal = self.strategy_pal;
                    if let Some(p) = strategy.draw(&mut self.fb, self.sheets.as_ref().unwrap()) {
                        self.pal = p;
                    }
                    return Screen::Strategy(strategy);
                }
                self.pal = flow.draw(&mut self.fb, self.sheets.as_ref().unwrap(), &strategy.game, &self.strategy_pal);
                Screen::Battle { strategy, flow }
            }
            Screen::Practice(mut p) => {
                p.update(&self.input, dt, &mut self.assets, &mut self.rng);
                if p.scores_dirty {
                    let b = p.scores.to_bytes();
                    self.storage.store(0, &b);
                    p.scores_dirty = false;
                }
                if p.exit {
                    return Screen::MainMenu;
                }
                let pal = p.draw(&mut self.fb, self.sheets.as_ref().unwrap());
                self.pal = pal.unwrap_or(self.strategy_pal);
                Screen::Practice(p)
            }
            Screen::Message { text } => {
                self.fb.clear(0);
                let f = self.fonts.as_ref().unwrap();
                f.draw(&mut self.fb, Font::Orange, 20, 90, &text);
                if self.input.clicked() || self.input.any_pressed() {
                    Screen::MainMenu
                } else {
                    Screen::Message { text }
                }
            }
        }
    }

    pub const MAIN_BUTTONS: [Rect; 3] = [Rect::xyxy(109, 20, 208, 73), Rect::xyxy(109, 77, 208, 130), Rect::xyxy(109, 134, 208, 187)];

    fn main_menu(&mut self) -> Screen {
        self.show_image("mainscr3.gif");
        let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
        let hover = Self::MAIN_BUTTONS.iter().position(|r| r.contains(mx, my));
        if let Some(h) = hover {
            let r = Self::MAIN_BUTTONS[h];
            let c = self.nearest_color([255, 60, 40]);
            self.fb.rect_outline(Rect::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2), c);
        }
        if self.input.clicked() {
            if let Some(h) = hover {
                return match h {
                    0 => Screen::Options(Box::new(OptionsScreen::new(true))),
                    1 => Screen::Options(Box::new(OptionsScreen::new(false))),
                    _ => {
                        let scores = self.storage.fetch(0).map(|b| Scores::from_bytes(&b)).unwrap_or_default();
                        Screen::Practice(Box::new(PracticeScreen::new(scores)))
                    }
                };
            }
        }
        Screen::MainMenu
    }

    /// One-line description of the current screen (for the headless harness).
    pub fn debug_state(&self) -> String {
        match &self.screen {
            Screen::Boot => "boot".into(),
            Screen::Credit { t, .. } => format!("credit {:.1}", t),
            Screen::Logo { index, .. } => format!("logo {}", index),
            Screen::MainMenu => "mainmenu".into(),
            Screen::Options(o) => format!("options types={:?} planet={} fog={} done={}", o.types, o.planet, o.fog, o.done),
            Screen::Names { names, .. } => format!("names idx={} names={:?} skills={:?} done={}", names.idx, names.names, names.skills, names.done),
            Screen::Strategy(s) => format!("strategy mode={:?} cur={} doors={} year={} season={} vault={}", s.mode, s.game.cur_player, s.doors_open, s.game.year, s.game.season, s.game.vault(s.game.cur_player.max(0) as u8)),
            Screen::Battle { flow, .. } => format!("battle stage={}", match &flow.stage { crate::ui::battleflow::Stage::Attack(_) => "attack-setup", crate::ui::battleflow::Stage::Defend(_) => "defend-setup", crate::ui::battleflow::Stage::Fight { .. } => "fight", crate::ui::battleflow::Stage::Done => "done" }),
            Screen::Practice(p) => format!("practice {}", match &p.stage {
                crate::ui::practice::Stage::Menu => "menu".to_string(),
                crate::ui::practice::Stage::ShowdownPlayers { choice } => format!("players {:?}", choice),
                crate::ui::practice::Stage::ShowdownTerrain { terrain, .. } => format!("terrain {}", terrain),
                crate::ui::practice::Stage::ShowdownMoney { money, .. } => format!("money {}", money),
                crate::ui::practice::Stage::Shop { who, bought, .. } => format!("shop {} {:?}", who, bought),
                crate::ui::practice::Stage::SurvivorChoose { class } => format!("survivor {}", class),
                crate::ui::practice::Stage::Fight { screen, survivor, .. } => format!("fight phase={:?} ticks={} survivor={:?}", screen.phase, screen.sim.itime, survivor.as_ref().map(|s| s.level)),
                crate::ui::practice::Stage::Summary { lines, .. } => format!("summary {:?}", lines),
                crate::ui::practice::Stage::EnterName { .. } => "entername".to_string(),
                crate::ui::practice::Stage::SurvivorNext(_) => "next".to_string(),
            }),
            Screen::Message { text } => format!("message {}", text),
        }
    }

    pub fn nearest_color(&self, rgb: [u8; 3]) -> u8 {
        let mut best = 0;
        let mut bd = i64::MAX;
        for i in 0..256 {
            let c = self.pal[i];
            let d = (c[0] as i64 - rgb[0] as i64).pow(2) + (c[1] as i64 - rgb[1] as i64).pow(2) + (c[2] as i64 - rgb[2] as i64).pow(2);
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best as u8
    }

    pub fn present(&mut self) {
        for (i, &p) in self.fb.px.iter().enumerate() {
            let c = self.pal[p as usize];
            self.rgba[i * 4] = c[0];
            self.rgba[i * 4 + 1] = c[1];
            self.rgba[i * 4 + 2] = c[2];
            self.rgba[i * 4 + 3] = 255;
        }
    }
}

impl Default for App {
    fn default() -> Self {
        App::new()
    }
}
